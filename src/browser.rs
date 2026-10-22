use crate::{
    config::expand_home,
    defs::*,
    editor::{Editor, PromptState},
    files::allowed,
    keys::{Key, F},
    render::fit,
    runtime::Answer,
    terminal::{self, Painter},
};
use crossterm::event::{Event, KeyCode};
use std::{
    fs,
    path::{Path, PathBuf},
};
impl Editor {
    pub fn browse(&mut self, initial: &str) -> Option<String> {
        let initial = expand_home(initial);
        let mut dir = if initial.is_dir() {
            initial
        } else {
            initial
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."))
                .to_owned()
        };
        let mut selected = 0usize;
        let mut search = String::new();
        loop {
            if let Err(e) = allowed(&dir, self.cfg.operdir.as_deref()) {
                self.status(Importance::Ahem, &e.to_string());
                return None;
            }
            dir = match fs::canonicalize(&dir) {
                Ok(d) => d,
                Err(e) => {
                    self.status(Importance::Ahem, &e.to_string());
                    return None;
                }
            };
            let mut entries: Vec<PathBuf> = match fs::read_dir(&dir) {
                Ok(es) => es.filter_map(Result::ok).map(|e| e.path()).collect(),
                Err(e) => {
                    self.status(Importance::Ahem, &e.to_string());
                    return None;
                }
            };
            entries.sort_by_key(|p| {
                (
                    !p.is_dir(),
                    p.file_name().unwrap_or_default().to_os_string(),
                )
            });
            if let Some(parent) = dir.parent() {
                if allowed(parent, self.cfg.operdir.as_deref()).is_ok() {
                    entries.insert(0, dir.join(".."));
                }
            }
            selected = selected.min(entries.len().saturating_sub(1));
            let width = entries
                .iter()
                .map(|p| {
                    p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .chars()
                        .count()
                        + 14
                })
                .max()
                .unwrap_or(20)
                .min(self.term.cols)
                .max(1);
            let columns = (self.term.cols / width).max(1);
            let cell = self.term.cols / columns;
            let rows = self.term.rows.saturating_sub(4).max(1);
            let page = rows * columns;
            let top = selected / page * page;
            let mut p = Painter::new(self.term.rows, self.term.cols);
            p.buf.push_str("\x1b[?25l");
            p.wipescreen();
            p.goto(0, 0);
            p.sgr("7");
            p.buf.push_str(&fit(
                &format!("  {}     Directory: {}", BRANDING, dir.display()),
                self.term.cols,
            ));
            p.reset();
            for (i, path) in entries.iter().enumerate().skip(top).take(page) {
                let slot = i - top;
                p.goto(1 + slot / columns, slot % columns * cell);
                if i == selected {
                    p.sgr("7");
                }
                let size = if path.is_dir() {
                    "(dir)".into()
                } else {
                    fs::metadata(path)
                        .map(|m| m.len().to_string())
                        .unwrap_or_else(|_| "?".into())
                };
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                let label = format!(" {}  {}", name, size);
                p.buf.push_str(&fit(&label, cell));
                p.reset();
            }
            if !self.cfg.nohelp {
                for (i, (key, label)) in self.menu_hints(MBROWSER).iter().enumerate() {
                    let n = self.menu_hints(MBROWSER).len().div_ceil(2).max(1);
                    let w = self.term.cols / n;
                    p.goto(self.term.rows.saturating_sub(2) + i % 2, (i / 2) * w);
                    p.sgr("7");
                    p.buf.push_str(key);
                    p.reset();
                    p.buf
                        .push_str(&fit(&format!(" {label}"), w.saturating_sub(key.len())));
                }
            }
            p.flush(&mut self.term.out);
            let event = self.input();
            if self.quit {
                return None;
            }
            let k = match event {
                Event::Resize(_, _) => {
                    self.term.measure();
                    continue;
                }
                Event::Key(k) => k,
                Event::Mouse(m) => {
                    use crossterm::event::MouseEventKind::*;
                    match m.kind {
                        ScrollUp => selected = selected.saturating_sub(columns),
                        ScrollDown => {
                            selected = (selected + columns).min(entries.len().saturating_sub(1))
                        }
                        Down(_) => {
                            let slot = (m.row as usize).saturating_sub(1) * columns
                                + m.column as usize / cell.max(1);
                            if top + slot < entries.len() {
                                selected = top + slot;
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                _ => continue,
            };
            let Some(key) = terminal::convert_key(&k) else {
                continue;
            };
            let f = self.binding(MBROWSER, key);
            match (f, k.code) {
                (Some(F::Exit | F::Cancel), _) => return None,
                (Some(F::Enter), _) | (_, KeyCode::Char('s')) => {
                    if let Some(path) = entries.get(selected) {
                        if path.is_dir() {
                            dir = path.clone();
                            selected = 0;
                        } else {
                            return Some(path.to_string_lossy().into());
                        }
                    }
                }
                (Some(F::Left), _) => selected = selected.saturating_sub(1),
                (Some(F::Right), _) => {
                    selected = (selected + 1).min(entries.len().saturating_sub(1))
                }
                (Some(F::Up), _) => selected = selected.saturating_sub(columns),
                (Some(F::Down), _) => {
                    selected = (selected + columns).min(entries.len().saturating_sub(1))
                }
                (Some(F::PageUp), _) => selected = selected.saturating_sub(page),
                (Some(F::PageDown), _) => {
                    selected = (selected + page).min(entries.len().saturating_sub(1))
                }
                (Some(F::BrowserFirst), _) => selected = 0,
                (Some(F::BrowserLast), _) => selected = entries.len().saturating_sub(1),
                (Some(F::PrevWord), _) => selected = selected.saturating_sub(1),
                (Some(F::NextWord), _) => {
                    selected = (selected + 1).min(entries.len().saturating_sub(1))
                }
                (Some(F::PrevBlock), _) => selected = selected / columns * columns,
                (Some(F::NextBlock), _) => {
                    selected = (selected / columns * columns + columns - 1)
                        .min(entries.len().saturating_sub(1))
                }
                (Some(F::GotoDir), _) | (_, KeyCode::Char('g')) => {
                    if let Answer::Text(s) = self.prompt_input(MGOTODIR, "Go To Directory: ", "") {
                        let path = expand_home(&s);
                        let path = if path.is_absolute() {
                            path
                        } else {
                            dir.join(path)
                        };
                        if path.is_dir() && allowed(&path, self.cfg.operdir.as_deref()).is_ok() {
                            dir = path;
                            selected = 0;
                        } else {
                            self.status(Importance::Ahem, "Cannot open directory");
                        }
                    }
                }
                (
                    Some(
                        F::SearchForward
                        | F::SearchBackward
                        | F::BrowserSearchForward
                        | F::BrowserSearchBackward,
                    ),
                    _,
                )
                | (_, KeyCode::Char('w')) => {
                    self.prompt = Some(PromptState {
                        menu: MBROWSER,
                        question: String::new(),
                        answer: String::new(),
                        typing_x: 0,
                        allow_editing: false,
                    });
                    if let Answer::Text(s) = self.prompt_input(MWHEREISFILE, "Search: ", "") {
                        if !s.is_empty() {
                            search = s;
                        }
                        if let Some(i) = find_entry(&entries, selected, &search, false) {
                            selected = i;
                        }
                    }
                }
                (Some(F::FindNext | F::FindPrev | F::BrowserFindNext | F::BrowserFindPrev), _) => {
                    if let Some(i) = find_entry(
                        &entries,
                        selected,
                        &search,
                        matches!(f, Some(F::FindPrev | F::BrowserFindPrev)),
                    ) {
                        selected = i;
                    }
                }
                (Some(F::Help), _) => self.show_help(),
                (Some(F::ToggleNoHelp), _) => self.cfg.nohelp ^= true,
                (_, KeyCode::Backspace) => {
                    if let Some(parent) = dir.parent() {
                        dir = parent.to_owned();
                        selected = 0;
                    }
                }
                (_, KeyCode::Char(' ')) | (_, KeyCode::Tab) => {
                    selected = (selected + 1).min(entries.len().saturating_sub(1))
                }
                _ => {}
            }
            let _ = Key::Space;
        }
    }
}
fn find_entry(entries: &[PathBuf], at: usize, search: &str, back: bool) -> Option<usize> {
    if entries.is_empty() || search.is_empty() {
        return None;
    }
    let n = entries.len();
    for step in 1..=n {
        let i = if back {
            (at + n - step % n) % n
        } else {
            (at + step) % n
        };
        if entries[i]
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase()
            .contains(&search.to_lowercase())
        {
            return Some(i);
        }
    }
    None
}
