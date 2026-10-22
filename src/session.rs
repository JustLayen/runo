use crate::{defs::*, editor::Editor};
use std::{fs, path::PathBuf};
fn directory() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::config::expand_home("~/.local/state"))
        .join("runo")
}
impl Editor {
    pub fn load_session(&mut self) {
        if self.cfg.restricted {
            return;
        }
        if self.cfg.historylog {
            if let Ok(s) = fs::read_to_string(directory().join("history")) {
                for line in s.lines() {
                    if let Some((kind, value)) = line.split_once('\t') {
                        let value = decode(value);
                        match kind {
                            "s" => self.history.search.push(value),
                            "r" => self.history.replace.push(value),
                            "e" => self.history.execute.push(value),
                            _ => {}
                        }
                    }
                }
            }
        }
        self.restore_position();
    }
    pub fn restore_position(&mut self) {
        if !self.cfg.positionlog || self.cfg.restricted {
            return;
        }
        let Some(name) = &self.borig().name else {
            return;
        };
        let name = fs::canonicalize(name)
            .unwrap_or_else(|_| PathBuf::from(name))
            .to_string_lossy()
            .into_owned();
        if let Ok(s) = fs::read_to_string(directory().join("positions")) {
            for line in s.lines() {
                let w = line.split('\t').collect::<Vec<_>>();
                if w.len() >= 3 && decode(w[0]) == name {
                    if let (Ok(y), Ok(x)) = (w[1].parse::<usize>(), w[2].parse::<usize>()) {
                        let p = self.borig().clamp_pos(Pos::new(y, x));
                        self.goto_pos(p.y, p.x);
                        if let Some(anchors) = w.get(3) {
                            for a in anchors.split(',') {
                                if let Ok(y) = a.parse::<usize>() {
                                    if let Some(line) = self.b().lines.get_mut(y) {
                                        line.has_anchor = true;
                                    }
                                }
                            }
                        }
                    }
                    break;
                }
            }
        }
    }
    pub fn remember_position(&self) {
        if !self.cfg.positionlog || self.cfg.restricted || self.help_view {
            return;
        }
        let b = self.borig();
        let Some(name) = &b.name else {
            return;
        };
        let name = fs::canonicalize(name)
            .unwrap_or_else(|_| PathBuf::from(name))
            .to_string_lossy()
            .into_owned();
        let dir = directory();
        if fs::create_dir_all(&dir).is_err() {
            return;
        }
        let path = dir.join("positions");
        let old = fs::read_to_string(&path).unwrap_or_default();
        let mut out = String::new();
        for line in old
            .lines()
            .rev()
            .take(999)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            if line
                .split_once('\t')
                .is_some_and(|(n, _)| decode(n) != name)
            {
                out.push_str(line);
                out.push('\n');
            }
        }
        let anchors = b
            .lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.has_anchor)
            .map(|(y, _)| y.to_string())
            .collect::<Vec<_>>()
            .join(",");
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            encode(&name),
            b.cursor.y,
            b.cursor.x,
            anchors
        ));
        let _ = private_write(&path, &out);
    }
    pub fn save_session(&self) {
        if self.cfg.restricted {
            return;
        }
        self.remember_position();
        if self.cfg.historylog {
            let dir = directory();
            if fs::create_dir_all(&dir).is_err() {
                return;
            }
            let mut out = String::new();
            for (kind, hist) in [
                ("s", &self.history.search),
                ("r", &self.history.replace),
                ("e", &self.history.execute),
            ] {
                for s in hist
                    .iter()
                    .rev()
                    .take(100)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                {
                    out.push_str(&format!("{kind}\t{}\n", encode(s)));
                }
            }
            let _ = private_write(&dir.join("history"), &out);
        }
    }
}
fn encode(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}
fn decode(s: &str) -> String {
    let mut out = String::new();
    let mut c = s.chars();
    while let Some(ch) = c.next() {
        if ch == '\\' {
            match c.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(c) => out.push(c),
                _ => out.push('\\'),
            }
        } else {
            out.push(ch)
        }
    }
    out
}
fn private_write(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    let result = (|| {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        fs::rename(&tmp, path)
    })();
    let _ = fs::remove_file(tmp);
    result
}
