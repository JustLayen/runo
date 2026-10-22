//! Full screen painting, using terminal display columns throughout.
use crate::{defs::*, display::*, editor::Editor, terminal::Painter};

pub fn clean(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            if is_cntrl(c) {
                cntrl_rep(c).chars().collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect()
}
pub fn fit(text: &str, width: usize) -> String {
    let mut out = String::new();
    let mut col = 0;
    for ch in clean(text).chars() {
        let w = advance(ch, col, 8);
        if col + w > width {
            break;
        }
        out.push(ch);
        col += w;
    }
    out.extend(std::iter::repeat_n(' ', width - col));
    out
}

impl Editor {
    pub fn edit_top(&self) -> usize {
        if self.cfg.zero || self.cfg.minibar {
            0
        } else if self.cfg.emptyline && self.term.rows > 5 {
            2
        } else {
            1
        }
    }
    pub fn visible_rows(&self) -> Vec<(usize, usize, usize)> {
        let b = self.borig();
        let mut out = Vec::new();
        for y in b.scroll.top_line..b.lines.len() {
            if self.cfg.softwrap {
                for (i, c) in compute_chunks(
                    b.line(y),
                    self.editwincols(),
                    self.cfg.tabsize,
                    self.cfg.atblanks,
                )
                .iter()
                .enumerate()
                {
                    if y == b.scroll.top_line && i < b.scroll.top_col {
                        continue;
                    }
                    out.push((y, c.left, c.right));
                    if out.len() >= self.edit_rows() {
                        return out;
                    }
                }
            } else {
                out.push((y, b.scroll.page_col, b.scroll.page_col + self.editwincols()));
            }
            if out.len() >= self.edit_rows() {
                break;
            }
        }
        out
    }
    pub fn position_message(&self) -> String {
        let b = self.borig();
        let lines = b.lines.len();
        let y = b.cursor.y + 1;
        let col = self.cursor_col(b) + 1;
        let chars = b.lines.iter().map(|l| l.chars.len()).sum::<usize>() + lines - 1;
        let before = b
            .lines
            .iter()
            .take(b.cursor.y)
            .map(|l| l.chars.len() + 1)
            .sum::<usize>()
            + b.cursor.x;
        format!(
            "line {y}/{lines} ({}%), col {col}/{} ({}%), char {before}/{chars} ({}%)",
            y * 100 / lines,
            wideness(b.line(b.cursor.y), self.cfg.tabsize) + 1,
            col * 100 / (wideness(b.line(b.cursor.y), self.cfg.tabsize) + 1),
            before * 100 / chars.max(1)
        )
    }
    pub fn active_syntax(&self) -> Option<&crate::syntax::Syntax> {
        if self.cfg.no_syntax || self.help_view || self.cfg.syntax_name.as_deref() == Some("none") {
            return None;
        }
        let b = self.borig();
        let name = b.name.as_deref().unwrap_or("");
        let header = b.line(0).iter().collect::<String>();
        if let Some(n) = &self.cfg.syntax_name {
            self.cfg.syntaxes.iter().rev().find(|s| &s.name == n)
        } else {
            self.cfg
                .syntaxes
                .iter()
                .rev()
                .find(|s| {
                    s.matches(name, &header)
                        || b.mime
                            .as_ref()
                            .is_some_and(|mime| s.magic.iter().any(|r| r.is_match(mime)))
                })
                .or_else(|| self.cfg.syntaxes.iter().find(|s| s.name == "default"))
        }
    }
    pub fn interface_style(&self, name: &str, default: &str) -> String {
        self.cfg.colors.get(name).cloned().unwrap_or_else(|| {
            if self.cfg.bold && default == "7" {
                "1".into()
            } else {
                default.into()
            }
        })
    }
    pub fn draw(&mut self) {
        let rows = self.term.rows.max(1);
        let cols = self.term.cols.max(1);
        let mut p = Painter::new(rows, cols);
        p.buf.push_str("\x1b[?25l\x1b[?7l\x1b[0m");
        for row in 0..rows {
            p.goto(row, 0);
            p.clr_eol();
        }
        let b = self.borig();
        if !self.cfg.zero && !self.cfg.minibar {
            p.goto(0, 0);
            p.sgr(&self.interface_style("titlecolor", "7"));
            let name = if self.help_view {
                "Help"
            } else {
                b.name.as_deref().unwrap_or("New Buffer")
            };
            let right = if self.cfg.view_mode {
                "View"
            } else if b.is_modified() {
                "Modified"
            } else {
                ""
            };
            let mut title = vec![' '; cols];
            let left = format!("  {}", BRANDING);
            overlay(&mut title, 0, &left);
            let room = cols.saturating_sub(left.chars().count() + right.len() + 5);
            let label = clean(name);
            let label = if label.chars().count() > room {
                format!(
                    "…{}",
                    label
                        .chars()
                        .rev()
                        .take(room.saturating_sub(1))
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect::<String>()
                )
            } else {
                label
            };
            overlay(
                &mut title,
                cols.saturating_sub(label.chars().count()) / 2,
                &label,
            );
            overlay(&mut title, cols.saturating_sub(right.len() + 2), right);
            if self.cfg.stateflags {
                let flags = format!(
                    "{}{}{}{}{}",
                    if self.cfg.autoindent { 'I' } else { '-' },
                    if b.mark.is_some() { 'M' } else { '-' },
                    if self.cfg.break_long_lines { 'L' } else { '-' },
                    if self.recording { 'R' } else { '-' },
                    if self.cfg.softwrap { 'S' } else { '-' }
                );
                overlay(&mut title, cols.saturating_sub(6), &flags);
            }
            p.buf
                .push_str(&fit(&title.into_iter().collect::<String>(), cols));
            p.reset();
        }
        let visible = self.visible_rows();
        let colors = self
            .active_syntax()
            .map(|s| s.highlight(&b.lines, visible.last().map_or(0, |v| v.0 + 1)));
        let region = b.mark.map(|_| b.get_region());
        let margin = self.number_margin().min(cols.saturating_sub(1));
        let textwidth = cols
            .saturating_sub(margin + usize::from(self.cfg.indicator))
            .max(1);
        let mut cursor = (self.edit_top(), margin);
        for (row, &(y, left, right)) in visible.iter().enumerate() {
            let screenrow = self.edit_top() + row;
            if screenrow >= rows {
                break;
            }
            p.goto(screenrow, 0);
            if margin > 0 {
                p.sgr(&self.interface_style("numbercolor", "2"));
                p.buf.push_str(&fit(
                    &format!(
                        "{:>w$}{}",
                        y + 1,
                        if b.lines[y].has_anchor { '◆' } else { ' ' },
                        w = margin.saturating_sub(1)
                    ),
                    margin,
                ));
                p.reset();
            }
            let line = b.line(y);
            let limit = if self.cfg.softwrap {
                right.min(left + textwidth)
            } else {
                left + textwidth
            };
            let mut col = 0;
            let mut printed = 0;
            for (x, &ch) in line.iter().enumerate() {
                let w = advance(ch, col, self.cfg.tabsize);
                if col + w <= left && w > 0 {
                    col += w;
                    continue;
                }
                if col >= limit && w > 0 {
                    break;
                }
                let selected =
                    region.is_some_and(|(a, z)| (y, x) >= (a.y, a.x) && (y, x) < (z.y, z.x));
                let spot = self
                    .spotlight
                    .is_some_and(|(s, n)| y == s.y && x >= s.x && x < s.x + n);
                let style = if selected {
                    self.interface_style("selectedcolor", "7")
                } else if spot {
                    self.interface_style("spotlightcolor", "30;103")
                } else {
                    colors
                        .as_ref()
                        .and_then(|c| c.get(y))
                        .and_then(|c| c.get(x))
                        .and_then(|s| *s)
                        .unwrap_or("")
                        .to_string()
                };
                p.reset();
                if !style.is_empty() {
                    p.sgr(&style);
                }
                let a = col.max(left);
                let z = (col + w).min(limit);
                let available = z.saturating_sub(a);
                if ch == '\t' {
                    for i in 0..available {
                        p.buf.push(if self.cfg.whitespace && i == 0 && col >= left {
                            self.cfg.whitespace_chars.chars().next().unwrap_or('»')
                        } else {
                            ' '
                        });
                    }
                } else if is_cntrl(ch) {
                    let rep = cntrl_rep(ch);
                    p.buf
                        .extend(rep.chars().skip(left.saturating_sub(col)).take(available));
                } else if col < left || col + w > limit {
                    p.buf.extend(std::iter::repeat_n(' ', available));
                } else if self.cfg.whitespace && ch == ' ' {
                    p.buf
                        .push(self.cfg.whitespace_chars.chars().nth(1).unwrap_or('·'));
                } else {
                    p.buf.push(ch);
                }
                printed += available;
                col += w;
            }
            p.reset();
            if let Some(stripe) = self.cfg.guidestripe {
                if stripe > 0 {
                    let stripe = stripe as usize - 1;
                    if stripe >= left + printed && stripe >= left && stripe < left + textwidth {
                        p.goto(screenrow, margin + stripe - left);
                        p.sgr(&self.interface_style("stripecolor", "48;5;238"));
                        p.buf.push(' ');
                        p.reset();
                    }
                }
            }
            if !self.cfg.softwrap {
                if left > 0 {
                    p.goto(screenrow, margin);
                    p.buf.push('$');
                }
                if wideness(line, self.cfg.tabsize) > left + textwidth {
                    p.goto(screenrow, cols.saturating_sub(1));
                    p.buf.push('$');
                }
            }
            if self.cfg.indicator {
                p.goto(screenrow, cols - 1);
                p.sgr(&self.interface_style("scrollercolor", "7"));
                p.buf.push(if y == b.cursor.y { '#' } else { '│' });
                p.reset();
            }
            let ccol = self.cursor_col(b);
            let lastchunk =
                !self.cfg.softwrap || row + 1 == visible.len() || visible[row + 1].0 != y;
            if y == b.cursor.y && ccol >= left && (ccol < limit || lastchunk) {
                cursor = (
                    screenrow,
                    margin + ccol.saturating_sub(left).min(textwidth - 1),
                );
            }
        }
        if !self.cfg.zero || self.prompt.is_some() {
            let statusrow = rows.saturating_sub(if self.cfg.nohelp || rows < 5 { 1 } else { 3 });
            p.goto(statusrow, 0);
            if let Some(prompt) = &self.prompt {
                p.sgr(&self.interface_style("promptcolor", "7"));
                let q = clean(&prompt.question);
                let qwidth =
                    wideness(&q.chars().collect::<Vec<_>>(), 8).min(cols.saturating_sub(2));
                p.buf.push_str(&fit(&q, qwidth));
                let cx = column_of(
                    &prompt.answer.chars().collect::<Vec<_>>(),
                    prompt.typing_x,
                    8,
                );
                let avail = cols.saturating_sub(qwidth).max(1);
                let page = cx.saturating_sub(avail.saturating_sub(1));
                let text = slice_columns(&prompt.answer, page, avail, 8);
                p.buf.push_str(&text);
                cursor = (statusrow, (qwidth + cx - page).min(cols - 1));
                p.reset();
            } else if let Some(m) = &self.message {
                let style = if matches!(m.kind, Importance::Alert | Importance::Ahem) {
                    self.interface_style("errorcolor", "1;37;41")
                } else {
                    self.interface_style("statuscolor", "7")
                };
                let text = format!("[ {} ]", clean(&m.text));
                let width = wideness(&text.chars().collect::<Vec<_>>(), 8).min(cols);
                p.goto(statusrow, (cols - width) / 2);
                p.sgr(&style);
                p.buf.push_str(&fit(&text, width));
                p.reset();
            } else if self.cfg.constantshow || self.cfg.minibar {
                let text = if self.cfg.minibar {
                    format!(
                        "{}{}  {}",
                        b.name.as_deref().unwrap_or("New Buffer"),
                        if b.is_modified() { " *" } else { "" },
                        self.position_message()
                    )
                } else {
                    self.position_message()
                };
                p.sgr(&self.interface_style("minicolor", "0"));
                p.buf.push_str(&fit(&text, cols));
                p.reset();
            }
            if !self.cfg.nohelp && rows >= 5 {
                let hints = if self.active_menu() == MYESNO {
                    vec![
                        ("Y".into(), "Yes"),
                        ("N".into(), "No"),
                        ("^C".into(), "Cancel"),
                    ]
                } else {
                    self.menu_hints(self.active_menu())
                };
                let columns = hints.len().div_ceil(2).max(1);
                let width = cols / columns;
                for (i, (key, tag)) in hints.iter().enumerate() {
                    let (row, column) = (i % 2, i / 2);
                    p.goto(rows - 2 + row, column * width);
                    p.sgr(&self.interface_style("keycolor", "7"));
                    p.buf.push_str(key);
                    p.reset();
                    p.sgr(&self.interface_style("functioncolor", "0"));
                    let label = format!(" {}", tag);
                    p.buf
                        .push_str(&fit(&label, width.saturating_sub(key.len())));
                    p.reset();
                }
            }
        }
        p.goto(cursor.0.min(rows - 1), cursor.1.min(cols - 1));
        p.buf.push_str(
            if self.prompt.as_ref().is_some_and(|p| !p.allow_editing)
                || (self.help_view && !self.cfg.showcursor)
            {
                "\x1b[?7h\x1b[?25l"
            } else {
                "\x1b[?7h\x1b[?25h"
            },
        );
        p.flush(&mut self.term.out);
        self.force_redraw = false;
    }
}
fn overlay(row: &mut [char], start: usize, text: &str) {
    for (slot, c) in row.iter_mut().skip(start).zip(text.chars()) {
        *slot = c;
    }
}

/// Clip a prompt by display columns without splitting Unicode or terminal codes.
pub fn slice_columns(text: &str, left: usize, width: usize, tab: usize) -> String {
    let mut out = String::new();
    let mut col = 0;
    let mut used = 0;
    for ch in text.chars() {
        let w = advance(ch, col, tab);
        let end = col + w;
        if end <= left && w > 0 {
            col = end;
            continue;
        }
        if col >= left + width && w > 0 {
            break;
        }
        let n = end.min(left + width).saturating_sub(col.max(left));
        if ch == '\t' {
            out.extend(std::iter::repeat_n(' ', n));
        } else if is_cntrl(ch) {
            out.extend(cntrl_rep(ch).chars().skip(left.saturating_sub(col)).take(n));
        } else if col < left || end > left + width {
            out.extend(std::iter::repeat_n(' ', n));
        } else {
            out.push(ch);
        }
        used += n;
        col = end;
    }
    out.extend(std::iter::repeat_n(' ', width - used));
    out
}
