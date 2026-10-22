use crate::posix::Regex;
use crate::{
    defs::*,
    editor::{Aim, Editor},
    keys::F,
    runtime::Answer,
};

pub fn expression(needle: &str, regexp: bool, case: bool) -> Result<Regex, String> {
    Regex::new(
        &if regexp {
            needle.to_string()
        } else {
            crate::posix::escape(needle)
        },
        case,
    )
}
pub fn remember(history: &mut Vec<String>, s: &str) {
    if s.is_empty() {
        return;
    }
    history.retain(|v| v != s);
    history.push(s.into());
    if history.len() > 100 {
        history.remove(0);
    }
}
#[derive(Clone, Debug)]
pub struct Found {
    pub pos: Pos,
    pub len: usize,
    pub replacement: String,
}
pub fn matches(lines: &[BLine], re: &Regex, replacement: Option<&str>) -> Vec<Found> {
    let mut out = Vec::new();
    for (y, line) in lines.iter().enumerate() {
        let s = line.chars.iter().collect::<String>();
        for cap in re.captures(&s) {
            let (start, end) = cap[0].unwrap();
            let mut rep = String::new();
            if let Some(replacement) = replacement {
                let mut chars = replacement.chars();
                while let Some(c) = chars.next() {
                    if c == '\\' {
                        if let Some(next) = chars.next() {
                            if let Some(n) = next.to_digit(10) {
                                if let Some(Some((a, b))) = cap.get(n as usize) {
                                    rep.push_str(&s[*a..*b]);
                                }
                            } else {
                                rep.push(next);
                            }
                        } else {
                            rep.push('\\');
                        }
                    } else {
                        rep.push(c);
                    }
                }
            }
            out.push(Found {
                pos: Pos::new(y, s[..start].chars().count()),
                len: s[start..end].chars().count(),
                replacement: rep,
            });
        }
    }
    out
}
impl Editor {
    pub fn search_prompt(&mut self, back: bool, mut replace: bool) {
        self.search_back = back;
        let mut answer = String::new();
        loop {
            let q = format!(
                "Search{}{}{}{}: ",
                if self.search_case {
                    " [Case Sensitive]"
                } else {
                    ""
                },
                if self.search_regexp { " [Regexp]" } else { "" },
                if self.search_back { " [Backwards]" } else { "" },
                if self.last_search.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", self.last_search)
                }
            );
            match self.prompt_input(if replace { MREPLACE } else { MWHEREIS }, &q, &answer) {
                Answer::Cancel => {
                    self.statusbar("Cancelled");
                    return;
                }
                Answer::Text(s) => {
                    let s = if s.is_empty() {
                        self.last_search.clone()
                    } else {
                        s
                    };
                    if s.is_empty() {
                        self.statusbar("Cancelled");
                        return;
                    }
                    if let Err(e) = expression(&s, self.search_regexp, self.search_case) {
                        self.status(Importance::Ahem, &format!("Invalid regex: {e}"));
                        return;
                    }
                    self.last_search = s.clone();
                    remember(&mut self.history.search, &s);
                    if replace {
                        self.replace_prompt();
                    } else {
                        self.repeat_search(self.search_back);
                    }
                    return;
                }
                Answer::Action(f) => {
                    answer = self.browser_answer.take().unwrap_or(answer);
                    match f {
                        F::CaseSens => self.search_case ^= true,
                        F::Regexp => self.search_regexp ^= true,
                        F::Backwards => self.search_back ^= true,
                        F::FlipReplace => replace ^= true,
                        F::FlipGoto => {
                            self.goto_prompt();
                            return;
                        }
                        F::FirstLine => {
                            self.first_line();
                            return;
                        }
                        F::LastLine => {
                            self.last_line();
                            return;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    pub fn repeat_search(&mut self, back: bool) {
        if self.last_search.is_empty() {
            self.search_prompt(back, false);
            return;
        }
        let re = match expression(&self.last_search, self.search_regexp, self.search_case) {
            Ok(r) => r,
            Err(e) => {
                self.status(Importance::Ahem, &e.to_string());
                return;
            }
        };
        let found = matches(&self.borig().lines, &re, None);
        let at = self.borig().cursor;
        let next = if back {
            found
                .iter()
                .rev()
                .find(|m| (m.pos.y, m.pos.x) < (at.y, at.x))
                .or_else(|| found.last())
        } else {
            found
                .iter()
                .find(|m| (m.pos.y, m.pos.x) > (at.y, at.x))
                .or_else(|| found.first())
        };
        if let Some(m) = next {
            let wrapped = if back {
                (m.pos.y, m.pos.x) >= (at.y, at.x)
            } else {
                (m.pos.y, m.pos.x) <= (at.y, at.x)
            };
            self.goto_pos(m.pos.y, m.pos.x);
            self.spotlight = Some((m.pos, m.len));
            self.ensure_viewport(Aim::Centering);
            if found.len() == 1 && m.pos == at {
                self.statusbar("This is the only occurrence");
            } else if wrapped {
                self.statusbar("Search Wrapped");
            }
        } else {
            self.status(
                Importance::Ahem,
                &format!("\"{}\" not found", self.last_search),
            );
        }
    }
    fn replace_prompt(&mut self) {
        if !self.editable() {
            return;
        }
        let replacement =
            match self.prompt_input(MREPLACEWITH, "Replace with: ", &self.last_replace.clone()) {
                Answer::Text(s) => s,
                _ => {
                    self.statusbar("Cancelled");
                    return;
                }
            };
        remember(&mut self.history.replace, &replacement);
        self.last_replace = replacement.clone();
        let re = match expression(&self.last_search, self.search_regexp, self.search_case) {
            Ok(r) => r,
            Err(e) => {
                self.status(Importance::Ahem, &e.to_string());
                return;
            }
        };
        let mut found = matches(&self.borig().lines, &re, Some(&replacement));
        if !self.search_regexp {
            for m in &mut found {
                m.replacement = replacement.clone();
            }
        }
        if self.borig().mark.is_some() {
            let (a, b) = self.borig().get_region();
            found.retain(|m| {
                (m.pos.y, m.pos.x) >= (a.y, a.x) && (m.pos.y, m.pos.x + m.len) <= (b.y, b.x)
            });
        }
        if found.is_empty() {
            self.status(
                Importance::Ahem,
                &format!("\"{}\" not found", self.last_search),
            );
            return;
        }
        let original = self.borig().cursor;
        let start = found
            .iter()
            .position(|m| (m.pos.y, m.pos.x) >= (original.y, original.x))
            .unwrap_or(0);
        found.rotate_left(start);
        if self.search_back {
            found.reverse();
        }
        let mut offsets = vec![Vec::<(usize, isize)>::new(); self.borig().lines.len()];
        let mut all = false;
        let mut count = 0;
        for m in found {
            let delta = offsets[m.pos.y]
                .iter()
                .filter(|(x, _)| *x < m.pos.x)
                .map(|(_, d)| d)
                .sum::<isize>();
            let p = Pos::new(m.pos.y, (m.pos.x as isize + delta).max(0) as usize);
            self.goto_pos(p.y, p.x);
            self.spotlight = Some((p, m.len));
            self.ensure_viewport(Aim::Centering);
            let choice = if all {
                Some('y')
            } else {
                self.confirm("Replace this instance? (Y)es/(N)o/(A)ll: ", true)
            };
            if choice.is_none() {
                break;
            }
            if choice == Some('n') {
                continue;
            }
            if choice == Some('a') {
                all = true;
            }
            self.replace_range(
                p,
                Pos::new(p.y, p.x + m.len),
                &m.replacement,
                UndoType::Replace,
            );
            offsets[p.y].push((
                m.pos.x,
                m.replacement.chars().count() as isize - m.len as isize,
            ));
            count += 1;
        }
        self.spotlight = None;
        let p = self.borig().clamp_pos(original);
        self.goto_pos(p.y, p.x);
        self.statusbar(&format!(
            "Replaced {count} {}",
            if count == 1 {
                "occurrence"
            } else {
                "occurrences"
            }
        ));
    }
}
