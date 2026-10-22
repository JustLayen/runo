use crate::{
    buffer::Buffer,
    config::expand_home,
    defs::*,
    editor::{Aim, Editor},
    keys::F,
    runtime::Answer,
};
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

pub fn allowed(path: &Path, root: Option<&str>) -> io::Result<()> {
    let Some(root) = root else {
        return Ok(());
    };
    let root = fs::canonicalize(root)?;
    // Resolve every existing ancestor to catch symlinks, including a new file
    // beneath a directory symlink. Canonicalize the final path when it exists.
    let target = if path.exists() {
        fs::canonicalize(path)?
    } else {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::canonicalize(parent)?.join(
            path.file_name()
                .ok_or_else(|| io::Error::other("Invalid filename"))?,
        )
    };
    if target.starts_with(root) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Outside operating directory",
        ))
    }
}
pub fn read_buffer(path: &str, cfg: &Config) -> io::Result<Buffer> {
    let path = expand_home(path);
    allowed(&path, cfg.operdir.as_deref())?;
    let text = if cfg.noread {
        String::new()
    } else {
        match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e),
        }
    };
    let mut b = Buffer::from_string(&text, !cfg.noconvert);
    if cfg.magic && path.is_file() {
        if let Ok(output) = std::process::Command::new("file")
            .args(["-b", "--mime-type", "--"])
            .arg(&path)
            .output()
        {
            if output.status.success() {
                b.mime = Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
            }
        }
    }
    b.disk_snapshot = if path.exists() {
        Some(fs::read(&path)?)
    } else {
        None
    };
    b.name = Some(path.to_string_lossy().into());
    if cfg.unix {
        b.fmt = Format::Nix;
    }
    if !cfg.nonewlines {
        b.add_magic();
    }
    Ok(b)
}
pub fn complete_path(answer: &str) -> Vec<String> {
    let path = expand_home(answer);
    let (dir, prefix) = if answer.ends_with('/') {
        (path.clone(), String::new())
    } else {
        (
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."))
                .to_owned(),
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
        )
    };
    let mut result = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
        .map(|e| {
            let mut s = if answer.contains('/') {
                let at = answer.rfind('/').unwrap() + 1;
                format!("{}{}", &answer[..at], e.file_name().to_string_lossy())
            } else {
                e.file_name().to_string_lossy().into_owned()
            };
            if e.path().is_dir() {
                s.push('/');
            }
            s
        })
        .collect::<Vec<_>>();
    result.sort();
    result
}
pub fn write_text(
    path: &Path,
    text: &str,
    backup: bool,
    backupdir: Option<&str>,
) -> io::Result<()> {
    if backup && path.exists() {
        let target = if let Some(dir) = backupdir {
            let dir = expand_home(dir);
            if !dir.is_dir() {
                return Err(io::Error::other("Backup directory does not exist"));
            }
            let abs = fs::canonicalize(path)?.to_string_lossy().replace('/', "!");
            let mut n = 1;
            loop {
                let p = dir.join(format!("{abs}~{n}~"));
                if !p.exists() {
                    break p;
                }
                n += 1;
            }
        } else {
            PathBuf::from(format!("{}~", path.to_string_lossy()))
        };
        // Never follow an attacker-created backup symlink or overwrite the
        // original through a hard-linked backup.
        if let Ok(meta) = fs::symlink_metadata(&target) {
            if meta.file_type().is_symlink() {
                return Err(io::Error::other("Backup path is a symbolic link"));
            }
            fs::remove_file(&target)?;
        }
        let mut source = fs::File::open(path)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)?;
        output.set_permissions(source.metadata()?.permissions())?;
        io::copy(&mut source, &mut output)?;
        output.sync_all()?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()
}

impl Editor {
    pub fn replace_range(&mut self, start: Pos, end: Pos, text: &str, kind: UndoType) {
        let start = self.borig().clamp_pos(start);
        let end = self.borig().clamp_pos(end);
        let was = self.borig().cursor;
        let mark = self.borig().mark;
        let mut prims = crate::editing::prims_for(self.borig(), start, end);
        self.b().apply_prims(&prims);
        let pieces = text
            .split('\n')
            .map(|s| s.chars().collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let graft = crate::editing::graft_prims(self.borig(), start.y, start.x, &pieces);
        self.b().apply_prims(&graft);
        prims.extend(graft);
        if !self.cfg.nonewlines && !self.borig().magic() {
            let p = Prim::LineIns {
                at: self.borig().lines.len(),
                text: Vec::new(),
            };
            self.b().apply_prims(std::slice::from_ref(&p));
            prims.push(p);
        }
        let pos = if pieces.len() == 1 {
            Pos::new(start.y, start.x + pieces[0].len())
        } else {
            Pos::new(start.y + pieces.len() - 1, pieces.last().unwrap().len())
        };
        let b = self.b();
        b.cursor = pos;
        b.mark = None;
        b.push_undo_full(kind, was, mark, pos, None, prims);
        self.last_action = UndoType::Other;
        self.goto_pos(pos.y, pos.x);
    }
    pub fn insert_string(&mut self, text: &str) {
        let p = self.borig().cursor;
        self.replace_range(p, p, text, UndoType::Insert);
    }
    pub fn whole_range(&self) -> (Pos, Pos) {
        let b = self.borig();
        let y = b.lines.len() - 1;
        (Pos::new(0, 0), Pos::new(y, b.line_len(y)))
    }
    pub fn selected_text(&self) -> String {
        let (a, b) = if self.borig().mark.is_some() {
            self.borig().get_region()
        } else {
            self.whole_range()
        };
        crate::editing::nodes_for(self.borig(), a, b)
            .iter()
            .map(|l| l.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn save_current(&mut self) -> bool {
        if self.cfg.view_mode {
            self.status(Importance::Ahem, "Key is invalid in view mode");
            return false;
        }
        let Some(name) = self.borig().name.clone() else {
            return self.write_prompt(false);
        };
        self.save_to(&name, 0, false)
    }
    fn save_to(&mut self, name: &str, mode: u8, selection: bool) -> bool {
        let path = expand_home(name);
        if mode == 0 && !selection && self.borig().name.as_deref() == Some(name) {
            if let (Some(before), Ok(now)) = (&self.borig().disk_snapshot, fs::read(&path)) {
                if before != &now
                    && self.confirm("File changed on disk; OVERWRITE? ", false) != Some('y')
                {
                    self.statusbar("Cancelled");
                    return false;
                }
            }
        }
        let result = (|| {
            allowed(&path, self.cfg.operdir.as_deref())?;
            if self.cfg.restricted && self.borig().name.as_deref() != Some(name) {
                return Err(io::Error::other(
                    "Cannot write another file in restricted mode",
                ));
            }
            let mut text = if selection {
                let mut t = self.selected_text();
                if !t.ends_with('\n') && !self.cfg.nonewlines {
                    t.push('\n');
                }
                match self.borig().fmt {
                    Format::Dos => t.replace('\n', "\r\n"),
                    Format::Mac => t.replace('\n', "\r"),
                    Format::Nix => t,
                }
            } else {
                self.borig().serialize()
            };
            if mode != 0 && path.exists() {
                let previous = fs::read_to_string(&path)?;
                text = if mode == 1 {
                    previous + &text
                } else {
                    text + &previous
                };
            }
            write_text(
                &path,
                &text,
                self.cfg.backup && !self.cfg.restricted,
                self.cfg.backup_dir.as_deref(),
            )
        })();
        match result {
            Ok(()) => {
                if !selection && mode == 0 {
                    self.b().name = Some(path.to_string_lossy().into());
                    self.b().disk_snapshot = fs::read(&path).ok();
                    self.b().set_saved();
                }
                self.last_action = UndoType::Other;
                let lines =
                    self.borig().lines.len().saturating_sub(1) + usize::from(!self.borig().magic());
                self.statusbar(&format!(
                    "Wrote {lines} {}",
                    if lines == 1 { "line" } else { "lines" }
                ));
                true
            }
            Err(e) => {
                self.status(
                    Importance::Alert,
                    &format!("Error writing {}: {e}", path.display()),
                );
                false
            }
        }
    }
    pub fn write_prompt(&mut self, exiting: bool) -> bool {
        if self.cfg.view_mode {
            self.status(Importance::Ahem, "Key is invalid in view mode");
            return false;
        }
        let mut name = self.borig().name.clone().unwrap_or_default();
        let mut mode = 0;
        let selection = self.borig().mark.is_some() && !exiting;
        loop {
            let q = format!(
                "{}{}{}: ",
                if mode == 1 {
                    "File Name to Append"
                } else if mode == 2 {
                    "File Name to Prepend"
                } else if selection {
                    "File Name to Write Selection"
                } else {
                    "File Name to Write"
                },
                match self.borig().fmt {
                    Format::Dos => " [DOS Format]",
                    Format::Mac => " [Mac Format]",
                    _ => "",
                },
                if self.cfg.backup { " [Backup]" } else { "" }
            );
            match self.prompt_input(MWRITEFILE, &q, &name) {
                Answer::Cancel => {
                    self.statusbar("Cancelled");
                    return false;
                }
                Answer::Text(s) => {
                    if s.is_empty() {
                        self.statusbar("Cancelled");
                        return false;
                    }
                    let same = self
                        .borig()
                        .name
                        .as_ref()
                        .is_some_and(|old| expand_home(old) == expand_home(&s));
                    if !same
                        && expand_home(&s).exists()
                        && mode == 0
                        && self.confirm("File exists; OVERWRITE? ", false) != Some('y')
                    {
                        name = s;
                        continue;
                    }
                    return self.save_to(&s, mode, selection);
                }
                Answer::Action(f) => {
                    name = self.browser_answer.take().unwrap_or(name);
                    match f {
                        F::DosFormat => {
                            let b = self.b();
                            b.fmt = if b.fmt == Format::Dos {
                                Format::Nix
                            } else {
                                Format::Dos
                            };
                        }
                        F::MacFormat => {
                            let b = self.b();
                            b.fmt = if b.fmt == Format::Mac {
                                Format::Nix
                            } else {
                                Format::Mac
                            };
                        }
                        F::Append => {
                            if !self.cfg.restricted {
                                mode = if mode == 1 { 0 } else { 1 };
                            }
                        }
                        F::Prepend => {
                            if !self.cfg.restricted {
                                mode = if mode == 2 { 0 } else { 2 };
                            }
                        }
                        F::Backup => self.cfg.backup ^= true,
                        F::ToFiles => {
                            if !self.cfg.restricted {
                                if let Some(s) = self.browse(&name) {
                                    name = s;
                                }
                            }
                        }
                        F::DiscardBuffer if self.cfg.save_on_exit => {
                            self.b().set_saved();
                            if self.bufs.len() == 1 {
                                self.exit_status = 2;
                            }
                            if exiting {
                                return true;
                            }
                            self.close_buffer();
                            return false;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    pub fn close_buffer(&mut self) {
        if self.help_view {
            self.help_view = false;
            return;
        }
        if self.borig().is_modified() && !self.cfg.view_mode {
            if self.cfg.save_on_exit {
                if !self.save_current() {
                    return;
                }
            } else {
                match self.confirm("Save modified buffer? ", false) {
                    Some('y') => {
                        if !self.write_prompt(true) {
                            return;
                        }
                    }
                    Some('n') => {}
                    _ => {
                        self.statusbar("Cancelled");
                        return;
                    }
                }
            }
        }
        self.remember_position();
        if self.bufs.len() == 1 {
            self.quit = true;
        } else {
            self.bufs.remove(self.cur);
            self.cur = self.cur.min(self.bufs.len() - 1);
            self.last_action = UndoType::Other;
        }
    }
    pub fn read_prompt(&mut self) {
        if self.cfg.restricted {
            self.status(
                Importance::Ahem,
                "Reading other files is forbidden in restricted mode",
            );
            return;
        }
        let mut name = String::new();
        let mut convert = !self.cfg.noconvert;
        loop {
            let q = format!(
                "File to insert{}{}: ",
                if self.cfg.multibuffer {
                    " into new buffer"
                } else {
                    ""
                },
                if !convert { " [No Conversion]" } else { "" }
            );
            match self.prompt_input(MINSERTFILE, &q, &name) {
                Answer::Cancel => {
                    self.statusbar("Cancelled");
                    return;
                }
                Answer::Text(s) => {
                    if s.is_empty() {
                        return;
                    }
                    let path = expand_home(&s);
                    if let Err(e) = allowed(&path, self.cfg.operdir.as_deref()) {
                        self.status(Importance::Alert, &e.to_string());
                        return;
                    }
                    match fs::read_to_string(&path) {
                        Ok(text) => {
                            let mut b = Buffer::from_string(&text, convert);
                            let count = b.lines.len().saturating_sub(1) + usize::from(!b.magic());
                            if self.cfg.multibuffer {
                                b.name = Some(path.to_string_lossy().into());
                                if !self.cfg.nonewlines {
                                    b.add_magic();
                                }
                                self.bufs.push(b);
                                self.cur = self.bufs.len() - 1;
                                self.restore_position();
                                self.lock_current();
                            } else {
                                b.fmt = Format::Nix;
                                self.insert_string(&b.serialize());
                            }
                            self.statusbar(&format!(
                                "Read {count} {}",
                                if count == 1 { "line" } else { "lines" }
                            ));
                        }
                        Err(e) => self.status(
                            Importance::Alert,
                            &format!("Error reading {}: {e}", path.display()),
                        ),
                    }
                    return;
                }
                Answer::Action(f) => {
                    name = self.browser_answer.take().unwrap_or(name);
                    match f {
                        F::FlipExecute => {
                            self.execute_prompt();
                            return;
                        }
                        F::NoConvert => convert ^= true,
                        F::NewBufferToggle => self.cfg.multibuffer ^= true,
                        F::ToFiles => {
                            if let Some(s) = self.browse(&name) {
                                name = s;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    pub fn emergency_save(&mut self) {
        for b in &self.bufs {
            if b.is_modified() && !b.help {
                let base = b.name.as_deref().unwrap_or("runo");
                for i in 0..1000 {
                    let path = if i == 0 {
                        format!("{base}.save")
                    } else {
                        format!("{base}.save.{i}")
                    };
                    if let Ok(mut f) = OpenOptions::new().write(true).create_new(true).open(path) {
                        let _ = f.write_all(b.serialize().as_bytes());
                        let _ = f.sync_all();
                        break;
                    }
                }
            }
        }
    }
    pub fn goto_spec(&mut self, spec: &str) {
        let (l, c) = spec.split_once(',').unwrap_or((spec, "1"));
        let n = self.borig().lines.len();
        let parse = |s: &str, len: usize| -> Option<usize> {
            let n = s.parse::<isize>().ok()?;
            Some(
                if n < 0 {
                    len.saturating_sub(n.unsigned_abs())
                } else {
                    n.saturating_sub(1) as usize
                }
                .min(len.saturating_sub(1)),
            )
        };
        let y = if let Some(s) = l.strip_prefix("++") {
            s.parse::<usize>()
                .ok()
                .map(|v| (self.borig().cursor.y + v).min(n - 1))
        } else if let Some(s) = l.strip_prefix("--") {
            s.parse::<usize>()
                .ok()
                .map(|v| self.borig().cursor.y.saturating_sub(v))
        } else {
            parse(l, n)
        };
        if let Some(y) = y {
            let max = crate::display::wideness(self.borig().line(y), self.cfg.tabsize) + 1;
            if let Some(col) = parse(c, max) {
                let x = crate::display::actual_x(self.borig().line(y), col, self.cfg.tabsize);
                self.goto_pos(y, x);
                self.ensure_viewport(Aim::Centering);
                return;
            }
        }
        self.status(Importance::Ahem, "Invalid line or column number");
    }
    pub fn goto_prompt(&mut self) {
        match self.prompt_input(MGOTOLINE, "Enter line number, column number: ", "") {
            Answer::Text(s) => self.goto_spec(&s),
            Answer::Action(F::FirstLine) => self.first_line(),
            Answer::Action(F::LastLine) => self.last_line(),
            Answer::Action(F::ParaBegin) => self.move_para_begin(),
            Answer::Action(F::ParaEnd) => self.move_para_end(),
            Answer::Action(F::FlipGoto) => self.search_prompt(false, false),
            _ => self.statusbar("Cancelled"),
        }
    }
}

impl Editor {
    pub fn lock_buffers(&mut self) {
        if !self.cfg.locking || self.cfg.view_mode {
            return;
        }
        let old = self.cur;
        for i in 0..self.bufs.len() {
            self.cur = i;
            self.lock_current();
        }
        self.cur = old;
    }
    fn lock_current(&mut self) {
        if !self.cfg.locking || self.cfg.view_mode || self.borig().lock.is_some() {
            return;
        }
        if let Some(name) = self.borig().name.clone() {
            match crate::locking::Lock::acquire(&name) {
                Ok(lock) => self.b().lock = Some(lock),
                Err(e) => {
                    self.status(Importance::Ahem, &format!("Cannot lock {name}: {e}"));
                }
            }
        }
    }
}
