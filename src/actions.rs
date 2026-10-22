use crate::{
    defs::*,
    editor::{Aim, Editor},
    keys::F,
    runtime::Answer,
};
use std::os::unix::{fs::OpenOptionsExt, process::CommandExt};
use std::{
    fs,
    io::{self, Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temporary(PathBuf);
impl Temporary {
    fn new(text: &str) -> io::Result<Self> {
        for _ in 0..100 {
            let p = std::env::temp_dir().join(format!(
                "runo-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&p)
            {
                Ok(mut f) => {
                    f.write_all(text.as_bytes())?;
                    return Ok(Self(p));
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::other("Cannot create temporary file"))
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
impl Editor {
    pub fn execute_prompt(&mut self) {
        if self.cfg.restricted {
            self.status(
                Importance::Ahem,
                "Command execution is forbidden in restricted mode",
            );
            return;
        }
        let mut command = String::new();
        let mut pipe = false;
        let mut newbuffer = self.cfg.multibuffer;
        loop {
            let q = format!(
                "Command to execute{}{}: ",
                if pipe { " [Pipe Text]" } else { "" },
                if newbuffer { " [New Buffer]" } else { "" }
            );
            match self.prompt_input(MEXECUTE, &q, &command) {
                Answer::Cancel => {
                    self.statusbar("Cancelled");
                    return;
                }
                Answer::Text(s) => {
                    if s.trim().is_empty() {
                        return;
                    }
                    crate::search::remember(&mut self.history.execute, &s);
                    let (s, pipe) = if let Some(cmd) = s.strip_prefix('|') {
                        (cmd, true)
                    } else {
                        (s.as_str(), pipe)
                    };
                    let input = if pipe {
                        Some(self.selected_text())
                    } else {
                        None
                    };
                    match self.capture_command(s, input) {
                        Ok((text, success)) => {
                            if newbuffer {
                                self.bufs.push(crate::buffer::Buffer::new_empty());
                                self.cur = self.bufs.len() - 1;
                                self.insert_string(&text);
                            } else if pipe {
                                let (a, b) = if self.borig().mark.is_some() {
                                    self.borig().get_region()
                                } else {
                                    self.whole_range()
                                };
                                self.replace_range(a, b, &text, UndoType::Other);
                            } else {
                                self.insert_string(&text);
                            }
                            self.statusbar(if success {
                                "Command finished"
                            } else {
                                "Command exited with a nonzero status"
                            });
                        }
                        Err(e) => self.status(Importance::Alert, &e.to_string()),
                    }
                    return;
                }
                Answer::Action(f) => {
                    command = self.browser_answer.take().unwrap_or(command);
                    match f {
                        F::PipeToggle => pipe ^= true,
                        F::NewBufferToggle => newbuffer ^= true,
                        F::FlipExecute => {
                            self.read_prompt();
                            return;
                        }
                        F::RecordMacro => {
                            if self.recording {
                                self.macro_keys
                                    .truncate(self.macro_keys.len().saturating_sub(2));
                            }
                            self.toggle_recording();
                            return;
                        }
                        F::Formatter => {
                            self.external_tool(false);
                            return;
                        }
                        F::Linter => {
                            self.external_tool(true);
                            return;
                        }
                        F::Suspend => {
                            self.suspend();
                            return;
                        }
                        F::FullJustify => {
                            self.justify(true);
                            return;
                        }
                        F::CutTillEof => {
                            self.cut_till_eof();
                            return;
                        }
                        F::SpellCheck => {
                            self.spell_check();
                            return;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    fn capture_command(
        &mut self,
        command: &str,
        input: Option<String>,
    ) -> io::Result<(String, bool)> {
        let mut cmd = Command::new("/bin/sh");
        cmd.args(["-c", command])
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        unsafe {
            cmd.pre_exec(|| {
                if libc::setpgid(0, 0) == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = cmd.spawn()?;
        let pid = child.id();
        let writer = if let Some(input) = input {
            let mut stdin = child.stdin.take().unwrap();
            Some(std::thread::spawn(move || {
                let _ = stdin.write_all(input.as_bytes());
            }))
        } else {
            None
        };
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        let out = std::thread::spawn(move || {
            let mut s = Vec::new();
            stdout.read_to_end(&mut s).map(|_| s)
        });
        let err = std::thread::spawn(move || {
            let mut s = Vec::new();
            stderr.read_to_end(&mut s).map(|_| s)
        });
        self.statusbar("Executing...  ^C to cancel");
        self.draw();
        let mut cancelled = false;
        let mut finished = None;
        let status = loop {
            if finished.is_none() {
                finished = child.try_wait()?;
            }
            if let Some(status) = finished {
                if out.is_finished() && err.is_finished() {
                    break status;
                }
            }
            if crate::signals::terminating() {
                cancelled = true;
            }
            if let Some(e) = self.term.poll_event(Duration::from_millis(50)) {
                match e {
                    crossterm::event::Event::Key(k)
                        if k.code == crossterm::event::KeyCode::Char('c')
                            && k.modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        cancelled = true
                    }
                    crossterm::event::Event::Resize(_, _) => {
                        self.term.measure();
                        self.draw();
                    }
                    _ => {}
                }
            }
            if cancelled {
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGKILL);
                }
                let _ = child.kill();
                break child.wait()?;
            }
        };
        if let Some(w) = writer {
            let _ = w.join();
        }
        let mut bytes = out
            .join()
            .map_err(|_| io::Error::other("Command output reader failed"))??;
        let errors = err
            .join()
            .map_err(|_| io::Error::other("Command error reader failed"))??;
        if cancelled {
            return Err(io::Error::other("Cancelled"));
        }
        bytes.extend(errors);
        let text = String::from_utf8(bytes)
            .map_err(|_| io::Error::other("Command output is not valid UTF-8"))?;
        Ok((text, status.success()))
    }
    pub fn spell_check(&mut self) {
        if self.cfg.restricted {
            self.status(
                Importance::Ahem,
                "Spell checking is forbidden in restricted mode",
            );
            return;
        }
        let command = self
            .cfg
            .speller
            .as_ref()
            .map(|v| v.join(" "))
            .or_else(|| std::env::var("SPELL").ok())
            .unwrap_or_else(|| "spell".into());
        let text = self.selected_text();
        if self.cfg.speller.is_some() || std::env::var_os("SPELL").is_some() {
            let tmp = match Temporary::new(&text) {
                Ok(t) => t,
                Err(e) => {
                    self.status(Importance::Alert, &e.to_string());
                    return;
                }
            };
            self.term.leave_for_suspend();
            let result = Command::new("/bin/sh")
                .args([
                    "-c",
                    &format!("{} {}", command, quote(&tmp.0.to_string_lossy())),
                ])
                .status();
            self.term.resume_after_suspend();
            match result {
                Ok(s) if s.success() => match fs::read_to_string(&tmp.0) {
                    Ok(new) => {
                        if new != text {
                            let (a, b) = if self.borig().mark.is_some() {
                                self.borig().get_region()
                            } else {
                                self.whole_range()
                            };
                            self.replace_range(a, b, &new, UndoType::Replace);
                        }
                        self.statusbar("Finished checking spelling");
                    }
                    Err(e) => self.status(Importance::Alert, &e.to_string()),
                },
                Ok(_) => self.status(Importance::Alert, "Spell checker failed"),
                Err(e) => self.status(Importance::Alert, &e.to_string()),
            }
            return;
        }
        let words = match self.capture_command(&command, Some(text)) {
            Ok((s, true)) => s,
            Ok((s, false)) => {
                self.status(
                    Importance::Alert,
                    &format!("Spell checker failed: {}", s.trim()),
                );
                return;
            }
            Err(e) => {
                self.status(Importance::Alert, &e.to_string());
                return;
            }
        };
        let (a, b) = if self.borig().mark.is_some() {
            self.borig().get_region()
        } else {
            self.whole_range()
        };
        for word in words.lines() {
            if word.is_empty() {
                continue;
            }
            let replacement = match self.prompt_input(
                MSPELL,
                &format!("Edit a replacement for \"{word}\": "),
                word,
            ) {
                Answer::Text(s) => s,
                _ => break,
            };
            if replacement == word {
                continue;
            }
            let re = crate::search::expression(
                &format!("\\b{}\\b", crate::posix::escape(word)),
                true,
                true,
            )
            .unwrap();
            let found = crate::search::matches(&self.borig().lines, &re, None);
            for m in found.into_iter().rev() {
                if (m.pos.y, m.pos.x) >= (a.y, a.x) && (m.pos.y, m.pos.x + m.len) <= (b.y, b.x) {
                    self.replace_range(
                        m.pos,
                        Pos::new(m.pos.y, m.pos.x + m.len),
                        &replacement,
                        UndoType::Replace,
                    );
                }
            }
        }
        self.statusbar("Finished checking spelling");
    }
    pub fn suspend(&mut self) {
        self.term.leave_for_suspend();
        unsafe {
            libc::raise(libc::SIGTSTP);
        }
        self.term.resume_after_suspend();
        self.ensure_viewport(Aim::Minimal);
    }
    pub fn complete_word(&mut self) {
        if !self.editable() {
            return;
        }
        if let Some(mut state) = self.completion.take() {
            if self.borig().cursor == state.end {
                let word = state.candidates[state.next].clone();
                state.next = (state.next + 1) % state.candidates.len();
                self.replace_range(state.start, state.end, &word, UndoType::Add);
                state.end = self.borig().cursor;
                self.completion = Some(state);
                return;
            }
        }
        let b = self.borig();
        let p = b.cursor;
        let row = b.line(p.y);
        let mut start = p.x;
        while start > 0 && (row[start - 1].is_alphanumeric() || row[start - 1] == '_') {
            start -= 1;
        }
        if start == p.x {
            self.beep();
            return;
        }
        let prefix = row[start..p.x].iter().collect::<String>();
        let mut words = b
            .lines
            .iter()
            .enumerate()
            .flat_map(|(y, l)| {
                let s = l.chars.iter().collect::<String>();
                let mut x = 0;
                let mut out = Vec::new();
                for word in s.split(|c: char| !c.is_alphanumeric() && c != '_') {
                    if !word.is_empty() {
                        let offset = s[x..].find(word).unwrap_or(0) + x;
                        if !(y == p.y && s[..offset].chars().count() == start)
                            && word.starts_with(&prefix)
                            && word.len() > prefix.len()
                        {
                            out.push(word.to_string());
                        }
                        x = offset + word.len();
                    }
                }
                out
            })
            .collect::<Vec<_>>();
        let mut seen = std::collections::HashSet::new();
        words.retain(|w| seen.insert(w.clone()));
        if let Some(word) = words.first() {
            let a = Pos::new(p.y, start);
            let word = word.clone();
            self.replace_range(a, p, &word, UndoType::Add);
            self.completion = Some(crate::editor::Completion {
                start: a,
                end: self.borig().cursor,
                next: usize::from(words.len() > 1),
                candidates: words,
            });
        } else {
            self.statusbar("No matches");
        }
    }
    pub fn comment_lines(&mut self) {
        let comment = self
            .active_syntax()
            .and_then(|s| s.comment.clone())
            .unwrap_or_else(|| "#".into());
        if comment == "#" {
            self.do_comment();
            return;
        }
        let (a, z) = if self.borig().mark.is_some() {
            self.borig().get_region()
        } else {
            let p = self.borig().cursor;
            (p, p)
        };
        let end = if z.x == 0 && z.y > a.y { z.y - 1 } else { z.y };
        let all = (a.y..=end).all(|y| {
            self.borig()
                .line(y)
                .iter()
                .collect::<String>()
                .trim_start()
                .starts_with(&comment)
        });
        let mut lines = Vec::new();
        for y in a.y..=end {
            let s = self.borig().line(y).iter().collect::<String>();
            let indent = s.len() - s.trim_start().len();
            let line = if all {
                format!("{}{}", &s[..indent], &s[indent + comment.len()..])
            } else {
                format!("{}{}{}", &s[..indent], comment, &s[indent..])
            };
            lines.push(line);
        }
        self.replace_range(
            Pos::new(a.y, 0),
            Pos::new(end, self.borig().line_len(end)),
            &lines.join("\n"),
            UndoType::Comment,
        );
    }
}

impl Editor {
    pub fn external_tool(&mut self, linter: bool) {
        if self.cfg.restricted {
            self.status(
                Importance::Ahem,
                "External tools are forbidden in restricted mode",
            );
            return;
        }
        let command = self.active_syntax().and_then(|s| {
            if linter {
                s.linter.clone()
            } else {
                s.formatter.clone()
            }
        });
        let Some(command) = command else {
            self.status(
                Importance::Ahem,
                if linter {
                    "No linter is defined for this syntax"
                } else {
                    "No formatter is defined for this syntax"
                },
            );
            return;
        };
        let text = self.selected_text();
        let tmp = match Temporary::new(&text) {
            Ok(t) => t,
            Err(e) => {
                self.status(Importance::Alert, &e.to_string());
                return;
            }
        };
        match self.capture_command(
            &format!("{} {}", command, quote(&tmp.0.to_string_lossy())),
            None,
        ) {
            Ok((output, ok)) => {
                if linter {
                    let mut b = crate::buffer::Buffer::from_string(&output, true);
                    b.name = Some("Linter output".into());
                    self.bufs.push(b);
                    self.cur = self.bufs.len() - 1;
                } else if ok {
                    match fs::read_to_string(&tmp.0) {
                        Ok(new) => {
                            let (a, b) = if self.borig().mark.is_some() {
                                self.borig().get_region()
                            } else {
                                self.whole_range()
                            };
                            self.replace_range(a, b, &new, UndoType::Other);
                        }
                        Err(e) => self.status(Importance::Alert, &e.to_string()),
                    }
                } else {
                    self.status(
                        Importance::Alert,
                        &format!("Formatter failed: {}", output.trim()),
                    );
                }
            }
            Err(e) => self.status(Importance::Alert, &e.to_string()),
        }
    }
}
