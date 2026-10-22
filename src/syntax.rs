//! Parse nanorc highlighting rules and map their colors to terminal attributes.
use std::rc::Rc;
#[derive(Clone, Debug)]
pub struct Regex(Rc<crate::posix::Regex>);
#[derive(Clone, Copy)]
pub struct Match {
    start: usize,
    end: usize,
}
impl Match {
    pub fn start(self) -> usize {
        self.start
    }
    pub fn end(self) -> usize {
        self.end
    }
}
impl Regex {
    pub fn find(&self, text: &str) -> Option<Match> {
        self.find_at(text, 0)
    }
    pub fn find_at(&self, text: &str, at: usize) -> Option<Match> {
        self.0
            .first_match(text, at)
            .map(|(start, end)| Match { start, end })
    }
    pub fn find_iter(&self, text: &str) -> std::vec::IntoIter<Match> {
        self.0
            .captures(text)
            .into_iter()
            .filter_map(|c| c[0].map(|(start, end)| Match { start, end }))
            .collect::<Vec<_>>()
            .into_iter()
    }
    pub fn is_match(&self, text: &str) -> bool {
        self.find(text).is_some()
    }
}

#[derive(Clone, Debug)]
pub struct Rule {
    pub color: String,
    pub start: Regex,
    pub end: Option<Regex>,
}
#[derive(Clone, Debug, Default)]
pub struct Syntax {
    pub name: String,
    pub filenames: Vec<Regex>,
    pub headers: Vec<Regex>,
    pub magic: Vec<Regex>,
    pub rules: Vec<Rule>,
    pub comment: Option<String>,
    pub tab: Option<String>,
    pub formatter: Option<String>,
    pub linter: Option<String>,
}

pub fn compile(pattern: &str, insensitive: bool) -> Result<Regex, String> {
    crate::posix::Regex::new(pattern, !insensitive).map(|r| Regex(Rc::new(r)))
}

pub fn color(spec: &str) -> String {
    let mut codes = Vec::new();
    let mut parts = spec.split(',').peekable();
    while matches!(parts.peek(), Some(&"bold" | &"italic" | &"normal")) {
        match parts.next().unwrap() {
            "bold" => codes.push("1".into()),
            "italic" => codes.push("3".into()),
            _ => {}
        }
    }
    for (i, c) in parts.enumerate() {
        if c.is_empty() {
            continue;
        }
        let bg = i > 0;
        if let Some(hex) = c.strip_prefix('#') {
            let hex = if hex.len() == 3 {
                hex.chars().flat_map(|c| [c, c]).collect::<String>()
            } else {
                hex.to_string()
            };
            if hex.len() == 6 {
                if let Ok(n) = u32::from_str_radix(&hex, 16) {
                    codes.push(format!(
                        "{};2;{};{};{}",
                        if bg { 48 } else { 38 },
                        (n >> 16) & 255,
                        (n >> 8) & 255,
                        n & 255
                    ));
                }
            }
            continue;
        }
        let (bright, name) = if let Some(n) = c.strip_prefix("light") {
            (true, n)
        } else if let Some(n) = c.strip_prefix("bright") {
            (true, n)
        } else {
            (false, c)
        };
        let n = match name {
            "black" => 0,
            "red" => 1,
            "green" => 2,
            "yellow" => 3,
            "blue" => 4,
            "magenta" | "purple" => 5,
            "cyan" => 6,
            "white" => 7,
            "grey" | "gray" => 8,
            "normal" => 9,
            _ => continue,
        };
        codes.push(
            (if n == 8 {
                if bg {
                    100
                } else {
                    90
                }
            } else {
                (if bg { 40 } else { 30 }) + n + if bright && n != 9 { 60 } else { 0 }
            })
            .to_string(),
        );
    }
    codes.join(";")
}

impl Syntax {
    pub fn matches(&self, name: &str, header: &str) -> bool {
        self.filenames.iter().any(|r| r.is_match(name))
            || self.headers.iter().any(|r| r.is_match(header))
    }

    /// Compute colors for visible rows while retaining multiline-rule state.
    pub fn highlight(&self, lines: &[crate::defs::BLine], last: usize) -> Vec<Vec<Option<&str>>> {
        let mut result: Vec<Vec<Option<&str>>> = lines
            .iter()
            .take(last)
            .map(|l| vec![None; l.chars.len()])
            .collect();
        for rule in &self.rules {
            let mut active = false;
            for (y, line) in lines.iter().take(last).enumerate() {
                let text: String = line.chars.iter().collect();
                let mut spans = Vec::new();
                let mut at = 0;
                if let Some(end) = &rule.end {
                    while at <= text.len() {
                        let begin = if active {
                            at
                        } else if let Some(m) = rule.start.find_at(&text, at) {
                            m.start()
                        } else {
                            break;
                        };
                        let seek = if active {
                            at
                        } else {
                            rule.start.find_at(&text, at).unwrap().end()
                        };
                        if let Some(m) = end.find_at(&text, seek) {
                            spans.push((begin, m.end()));
                            active = false;
                            at = m.end();
                            if at == begin {
                                at += text[at..].chars().next().map_or(1, char::len_utf8);
                            }
                        } else {
                            spans.push((begin, text.len()));
                            active = true;
                            break;
                        }
                    }
                } else {
                    spans.extend(rule.start.find_iter(&text).map(|m| (m.start(), m.end())));
                }
                for (a, b) in spans {
                    let a = text[..a].chars().count();
                    let b = text[..b].chars().count();
                    for slot in &mut result[y][a..b] {
                        *slot = Some(rule.color.as_str());
                    }
                }
            }
        }
        result
    }
}
