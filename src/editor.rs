// editor.rs — the Editor: state, layout/viewport machinery and also the
// cursor-motion functions.

use crate::buffer::{self, Buffer};
use crate::defs::*;
use crate::display;
use crate::display::*;
use crate::terminal::Terminal;

use crate::syntax::Regex;

#[derive(Clone, Debug)]
pub struct Message {
    pub text: String,
    pub kind: Importance,
}

#[derive(Clone, Debug)]
pub struct PromptState {
    pub menu: u32,
    pub question: String,
    pub answer: String,
    pub typing_x: usize,
    pub allow_editing: bool,
}

#[derive(Default, Debug)]
pub struct History {
    pub search: Vec<String>,
    pub replace: Vec<String>,
    pub execute: Vec<String>,
    pub file: Vec<String>,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Aim {
    /// Bring the cursor on screen with as little scrolling as possible.
    Minimal,
    /// Put the cursor at the vertical middle, unconditionally.
    Center,
    /// Recenter only when the cursor is outside the viewport.
    Centering,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Dir {
    Forward,
    Back,
}

pub struct Completion {
    pub start: Pos,
    pub end: Pos,
    pub candidates: Vec<String>,
    pub next: usize,
}

pub struct Editor {
    pub completion: Option<Completion>,
    pub replay: std::collections::VecDeque<crossterm::event::Event>,
    pub recording: bool,
    pub macro_keys: Vec<crossterm::event::Event>,
    pub playing_macro: bool,
    pub cfg: Config,
    pub term: Terminal,
    pub bufs: Vec<Buffer>,
    pub cur: usize,
    /// the buffer currently on screen: index or the help file
    pub help_view: bool,
    pub cutbuffer: CutBuffer,
    pub keep_cutbuffer: bool,
    pub last_action: UndoType,
    pub message: Option<Message>,
    pub strokes_left: u32,
    pub search_case: bool,
    pub search_regexp: bool,
    pub search_back: bool,
    pub last_search: String,
    pub last_replace: String,
    pub history: History,
    pub spotlight: Option<(Pos, usize)>,
    pub force_redraw: bool,
    /// A mark set while a shift key is held (vanishes on the next movement).
    pub softmark: Option<Pos>,
    /// Whether the range functions still count the mark's end row (get_range()).
    pub also_the_last: bool,
    /// Whether a shift key (or function wanting a mark) is currently held.
    pub shift_held: bool,
    /// The compiled quoting regex, for paragraph detection.
    pub quoteregex: Option<Regex>,
    pub prompt: Option<PromptState>,
    pub browser_answer: Option<String>,
    pub quit: bool,
    pub exit_status: i32,
    pub cycle_stage: u8,
}

impl Editor {
    pub fn new(cfg: Config, term: Terminal, bufs: Vec<Buffer>) -> Editor {
        let mouse = cfg.mouse;
        let search_case = cfg.search_case;
        let search_regexp = cfg.search_regexp;
        let quoteregex = crate::syntax::compile(&cfg.quotestr, false).ok();
        let mut ed = Editor {
            completion: None,
            replay: std::collections::VecDeque::new(),
            recording: false,
            macro_keys: Vec::new(),
            playing_macro: false,
            cfg,
            term,
            bufs,
            cur: 0,
            help_view: false,
            cutbuffer: CutBuffer::default(),
            keep_cutbuffer: false,
            last_action: UndoType::Other,
            message: None,
            strokes_left: 0,
            search_case,
            search_regexp,
            search_back: false,
            last_search: String::new(),
            last_replace: String::new(),
            history: History::default(),
            spotlight: None,
            force_redraw: false,
            softmark: None,
            also_the_last: false,
            shift_held: false,
            quoteregex,
            prompt: None,
            browser_answer: None,
            quit: false,
            exit_status: 0,
            cycle_stage: 0,
        };
        if mouse {
            ed.term.set_mouse(true);
        }
        ed
    }

    pub fn b(&mut self) -> &mut Buffer {
        &mut self.bufs[self.cur]
    }
    pub fn borig(&self) -> &Buffer {
        &self.bufs[self.cur]
    }

    pub fn active_menu(&self) -> u32 {
        if let Some(p) = &self.prompt {
            p.menu
        } else if self.help_view {
            MHELP
        } else {
            MMAIN
        }
    }

    pub fn status(&mut self, kind: Importance, text: &str) {
        let strokes = match kind {
            Importance::Hush => {
                if self.cfg.quickblank {
                    1
                } else {
                    20
                }
            }
            Importance::Info => 20,
            _ => 1,
        };
        self.strokes_left = strokes;
        self.message = Some(Message {
            text: text.to_string(),
            kind,
        });
    }

    pub fn statusbar(&mut self, text: &str) {
        self.status(Importance::Hush, text);
    }

    pub fn wipe_status(&mut self) {
        self.message = None;
        self.strokes_left = 0;
    }

    pub fn beep(&mut self) {
        // The terminal bell: write BEL.
        self.term.write_raw("\x07");
    }

    // ------------------------------------------------------------ layout --

    pub fn edit_rows(&self) -> usize {
        let rows = self.term.rows;
        if self.cfg.zero {
            return rows.max(1);
        }
        if rows < 3 {
            return 1;
        }
        let minimum = if self.cfg.minibar { 4 } else { 5 };
        let toprows = if self.cfg.emptyline && rows > minimum {
            2
        } else {
            1
        };
        let bottomrows = if self.cfg.nohelp || rows < minimum {
            1
        } else {
            3
        };
        if rows < toprows + bottomrows {
            return 1;
        }
        if self.cfg.minibar {
            return rows - bottomrows;
        }
        rows - toprows - bottomrows
    }

    pub fn number_margin(&self) -> usize {
        if self.cfg.linenumbers {
            buffer::digits(self.borig().lines_count()) + 1
        } else {
            0
        }
    }

    pub fn editwincols(&self) -> usize {
        let cols = self.term.cols;
        let margin = self.number_margin();
        cols.saturating_sub(margin + usize::from(self.cfg.indicator))
            .max(1)
    }

    // ------------------------------------------------------------ viewport --

    pub fn cursor_col(&self, buf: &Buffer) -> usize {
        let (y, x) = (buf.cursor.y, buf.cursor.x);
        let x = x.min(buf.line_len(y));
        column_of(buf.line(y), x, self.cfg.tabsize)
    }

    pub fn cursor_chunk(&self, buf: &Buffer) -> usize {
        if !self.cfg.softwrap {
            return 0;
        }
        let col = self.cursor_col(buf);
        let chunks = compute_chunks(
            buf.line(buf.cursor.y),
            self.editwincols(),
            self.cfg.tabsize,
            self.cfg.atblanks,
        );
        chunks.iter().rposition(|c| col >= c.left).unwrap_or(0)
    }

    /// Rows that the stretch from (top,topchunk) to (y,chunk) occupies.
    pub fn span_rows(
        &self,
        buf: &Buffer,
        top: usize,
        topchunk: usize,
        y: usize,
        chunk: usize,
    ) -> isize {
        if y < top {
            return -1_000_000;
        }
        if !self.cfg.softwrap {
            return (y - top) as isize;
        }
        let editcols = self.editwincols();
        let tab = self.cfg.tabsize;
        let atblanks = self.cfg.atblanks;
        let mut total: i64 = 0;
        for row in top..=y {
            let n = rows_in(buf.line(row), editcols, tab, atblanks) as i64;
            let begin = if row == top {
                (topchunk as i64).min(n - 1)
            } else {
                0
            };
            let end = if row == y { (chunk as i64).min(n) } else { n };
            total += end - begin;
        }
        total as isize
    }

    /// Adjust the scroll so the cursor is on screen (or centre the view).
    pub fn ensure_viewport(&mut self, aim: Aim) {
        let rows = self.edit_rows().max(1);
        let editcols = self.editwincols();
        let tab = self.cfg.tabsize;
        let soft = self.cfg.softwrap;
        let atblanks = self.cfg.atblanks;
        let cy = self.borig().cursor.y;
        let cchunk = self.cursor_chunk(self.borig());
        let top = self.borig().scroll.top_line;
        let topchunk = if soft { self.borig().scroll.top_col } else { 0 };
        let span = self.span_rows(self.borig(), top, topchunk, cy, cchunk);

        let centered = aim == Aim::Center
            || (self.cfg.jumpyscrolling && (span < 0 || span >= rows as isize))
            || (aim == Aim::Centering && (span < 0 || (rows as isize - span) <= 0));

        let mut newtop = top;
        let mut newchunk = topchunk;

        if centered {
            if soft {
                (newtop, newchunk) =
                    back_chunks(self.borig(), cy, cchunk, rows / 2, editcols, tab, atblanks);
            } else {
                newtop = cy.saturating_sub(rows / 2);
            }
        } else if (rows as isize - span) <= 0 {
            if soft {
                (newtop, newchunk) =
                    back_chunks(self.borig(), cy, cchunk, rows - 1, editcols, tab, atblanks);
            } else if cy + 1 > newtop + rows {
                newtop = cy + 1 - rows;
            }
        } else if span < 0 {
            if soft {
                newtop = cy;
                newchunk = cchunk;
            } else if cy < newtop {
                newtop = cy;
            }
        }

        if !soft {
            let col = self.cursor_col(self.borig());
            let page = page_start(col, editcols, false);
            if page != self.borig().scroll.page_col {
                self.b().scroll.page_col = page;
            }
        }
        if newtop != top || (soft && newchunk != topchunk) {
            let b = self.b();
            b.scroll.top_line = newtop;
            if soft {
                b.scroll.top_col = newchunk;
            }
        }
    }

    /// Make sure the viewport exposes the modified spotlight (no-op repair).
    pub fn spotlight_fix(&mut self) {
        if let Some((p, _)) = self.spotlight {
            if p.y >= self.borig().lines.len() {
                self.spotlight = None;
            } else {
                let len = self.borig().line_len(p.y);
                if p.x > len {
                    self.spotlight = Some((Pos::new(p.y, len), 0));
                }
            }
        }
    }

    // ------------------------------------------------------------- moves --

    /// Compute the column for goal on row y without overshooting text.
    pub fn x_for_goal(&self, y: usize, goal: usize) -> usize {
        actual_x(self.borig().line(y), goal, self.cfg.tabsize)
    }

    pub fn goto_pos(&mut self, y: usize, x: usize) {
        let pos = {
            let b = self.b();
            let y = y.min(b.lines.len() - 1);
            let x = x.min(b.lines[y].chars.len());
            b.cursor = Pos::new(y, x);
            b.cursor
        };
        let col = self.cursor_col(self.borig());
        self.b().goal = col;
        self.ensure_viewport(Aim::Minimal);
        self.spotlight_fix();
        let _ = pos;
    }

    /// Move to (y, x), skipping wide chars that straddle chunk edges.
    pub fn goto_proper(&mut self, y: usize, x: usize, _dir: Dir) {
        self.goto_pos(y, x);
    }

    pub fn move_left(&mut self) {
        let mut jumped = false;
        let (y, x0) = {
            let b = self.borig();
            let mut pos = b.cursor;
            if pos.x > 0 {
                pos.x = step_left(b.line(pos.y), pos.x);
            } else if pos.y > 0 {
                pos.y -= 1;
                pos.x = b.line_len(pos.y);
                jumped = true;
            }
            (pos.y, pos.x)
        };
        if jumped {
            self.goto_pos(y, x0);
        } else {
            self.goto_proper(y, x0, Dir::Back);
        }
    }

    pub fn move_right(&mut self) {
        let mut jumped = false;
        let (y, x0) = {
            let b = self.borig();
            let mut pos = b.cursor;
            if pos.x < b.line_len(pos.y) {
                pos.x = step_right(b.line(pos.y), pos.x);
            } else if pos.y + 1 < b.lines.len() {
                pos.y += 1;
                pos.x = 0;
                jumped = true;
            }
            (pos.y, pos.x)
        };
        if jumped {
            self.goto_pos(y, x0);
        } else {
            self.goto_proper(y, x0, Dir::Forward);
        }
    }

    pub fn move_up(&mut self) {
        let goal = self.borig().goal;
        let b = self.borig();
        let (y, x);
        if b.cursor.y == 0 {
            return;
        } else if self.cfg.softwrap {
            let chunk = self.cursor_chunk(b);
            if chunk > 0 {
                let chunks = compute_chunks(
                    b.line(b.cursor.y),
                    self.editwincols(),
                    self.cfg.tabsize,
                    self.cfg.atblanks,
                );
                let left = chunks[chunk - 1].left;
                let right = chunks[chunk - 1].right;
                x = actual_x(
                    b.line(b.cursor.y),
                    (left + goal).min(right.max(left + 1).saturating_sub(1)),
                    self.cfg.tabsize,
                );
                self.goto_pos(b.cursor.y, x);
                return;
            } else {
                y = b.cursor.y - 1;
                x = self.x_for_goal(y, goal);
            }
        } else {
            y = b.cursor.y - 1;
            x = self.x_for_goal(y, goal);
        }
        self.goto_proper(y, x, Dir::Forward);
    }

    pub fn move_down(&mut self) {
        let goal = self.borig().goal;
        let b = self.borig();
        if self.cfg.softwrap {
            let chunk = self.cursor_chunk(b);
            let chunks = compute_chunks(
                b.line(b.cursor.y),
                self.editwincols(),
                self.cfg.tabsize,
                self.cfg.atblanks,
            );
            if chunk + 1 < chunks.len() {
                let left = chunks[chunk + 1].left;
                let right = chunks[chunk + 1].right;
                let x = actual_x(
                    b.line(b.cursor.y),
                    (left + goal).min(right.max(left + 1).saturating_sub(1)),
                    self.cfg.tabsize,
                );
                self.goto_pos(b.cursor.y, x);
                return;
            }
            let y = b.cursor.y + 1;
            if y >= b.lines.len() {
                return;
            }
            let x = self.x_for_goal(y, goal);
            self.goto_pos(y, x);
        } else {
            let y = b.cursor.y + 1;
            if y >= b.lines.len() {
                return;
            }
            let x = self.x_for_goal(y, goal);
            self.goto_pos(y, x);
        }
    }

    pub fn move_home(&mut self) {
        let b = self.borig();
        let y = b.cursor.y;
        let mut x = 0;
        if self.cfg.smarthome {
            let line = b.line(y);
            let indent = line
                .iter()
                .take_while(|c| **c == ' ' || **c == '\t')
                .count();
            if line.len() > indent {
                let curcol = self.cursor_col(b);
                let indentcol = column_of(line, indent, self.cfg.tabsize);
                if b.cursor.x > indent && curcol >= indentcol {
                    x = indent;
                }
            }
        }
        self.goto_pos(y, x);
    }

    pub fn move_end(&mut self) {
        let y = self.borig().cursor.y;
        self.goto_pos(y, self.borig().line_len(y));
    }

    pub fn first_line(&mut self) {
        self.goto_pos(0, 0);
    }

    pub fn last_line(&mut self) {
        let last = self.borig().lines.len() - 1;
        self.goto_pos(last, self.borig().line_len(last));
    }

    pub fn page_up(&mut self) {
        self.page_shift(false);
    }
    pub fn page_down(&mut self) {
        self.page_shift(true);
    }
    fn page_shift(&mut self, down: bool) {
        let rows = self.edit_rows().saturating_sub(2).max(1);
        let b = self.borig();
        let goal = b.goal;
        if self.cfg.softwrap {
            let chunk = self.cursor_chunk(b);
            let chunks = display::compute_chunks(
                b.line(b.cursor.y),
                self.editwincols(),
                self.cfg.tabsize,
                self.cfg.atblanks,
            );
            let relative = goal.saturating_sub(chunks[chunk].left);
            let (y, c) = if down {
                forward_chunks(
                    b,
                    b.cursor.y,
                    chunk,
                    rows,
                    self.editwincols(),
                    self.cfg.tabsize,
                    self.cfg.atblanks,
                )
            } else {
                back_chunks(
                    b,
                    b.cursor.y,
                    chunk,
                    rows,
                    self.editwincols(),
                    self.cfg.tabsize,
                    self.cfg.atblanks,
                )
            };
            let chunks = display::compute_chunks(
                b.line(y),
                self.editwincols(),
                self.cfg.tabsize,
                self.cfg.atblanks,
            );
            let target = &chunks[c];
            let col = (target.left + relative).min(
                target
                    .right
                    .saturating_sub(usize::from(c + 1 < chunks.len())),
            );
            let x = display::actual_x(b.line(y), col, self.cfg.tabsize);
            let goal = target.left + relative;
            self.goto_pos(y, x);
            self.b().goal = goal;
        } else {
            let y = if down {
                (b.cursor.y + rows).min(b.lines.len() - 1)
            } else {
                b.cursor.y.saturating_sub(rows)
            };
            let x = self.x_for_goal(y, goal);
            self.goto_pos(y, x);
            self.b().goal = goal;
        }
    }

    pub fn top_row(&mut self) {
        let b = self.borig();
        let goal = b.goal;
        let y = b.scroll.top_line;
        self.goto_pos(y, self.x_for_goal(y, goal));
    }

    pub fn bottom_row(&mut self) {
        let b = self.borig();
        let rows = self.edit_rows().max(1);
        let (y, _) = forward_chunks(
            b,
            b.scroll.top_line,
            if self.cfg.softwrap {
                b.scroll.top_col
            } else {
                0
            },
            rows - 1,
            self.editwincols(),
            self.cfg.tabsize,
            self.cfg.atblanks,
        );
        let goal = self.borig().goal;
        self.goto_pos(y, self.x_for_goal(y, goal));
    }

    pub fn scroll_up(&mut self) {
        let b = self.borig();
        if self.cfg.softwrap {
            let (top, tc) = (b.scroll.top_line, b.scroll.top_col);
            if top == 0 && tc == 0 {
                return;
            }
            let (y, c) = back_chunks(
                b,
                top,
                tc,
                1,
                self.editwincols(),
                self.cfg.tabsize,
                self.cfg.atblanks,
            );
            self.b().scroll.top_line = y;
            self.b().scroll.top_col = c;
            self.keep_cursor_on_screen();
        } else if b.scroll.top_line > 0 {
            self.b().scroll.top_line -= 1;
            self.keep_cursor_on_screen();
        }
    }

    pub fn scroll_down(&mut self) {
        let b = self.borig();
        if self.cfg.softwrap {
            let (top, tc) = (b.scroll.top_line, b.scroll.top_col);
            let last = b.lines.len() - 1;
            let n = rows_in(
                b.line(last),
                self.editwincols(),
                self.cfg.tabsize,
                self.cfg.atblanks,
            );
            if top >= last && tc + 1 >= n {
                return;
            }
            let (y, c) = forward_chunks(
                b,
                top,
                tc,
                1,
                self.editwincols(),
                self.cfg.tabsize,
                self.cfg.atblanks,
            );
            self.b().scroll.top_line = y;
            self.b().scroll.top_col = c;
            self.keep_cursor_on_screen();
        } else if b.scroll.top_line + self.edit_rows() < b.lines.len() {
            self.b().scroll.top_line += 1;
            self.keep_cursor_on_screen();
        }
        self.ensure_viewport(Aim::Minimal);
    }

    pub fn keep_cursor_on_screen(&mut self) {
        let rows = self.edit_rows().max(1);
        let b = self.borig();
        let (top, tc) = (
            b.scroll.top_line,
            if self.cfg.softwrap {
                b.scroll.top_col
            } else {
                0
            },
        );
        let cy = b.cursor.y;
        let cc = self.cursor_chunk(b);
        let span = self.span_rows(b, top, tc, cy, cc);
        if span < 0 {
            let goal = b.goal;
            self.goto_pos(top, self.x_for_goal(top, goal));
        } else if span > (rows as isize - 1) {
            let (yy, _) = back_chunks(
                self.borig(),
                cy,
                cc,
                rows - 1,
                self.editwincols(),
                self.cfg.tabsize,
                self.cfg.atblanks,
            );
            let goal = self.borig().goal;
            self.goto_pos(yy, self.x_for_goal(yy, goal));
        }
    }

    /// Rotate the cursor line between the middle, top, and bottom of the view.
    pub fn cycle(&mut self) {
        let rows = self.edit_rows().max(1);
        let b = self.borig();
        let cy = b.cursor.y;
        let cc = self.cursor_chunk(b);
        let aim = match self.cycle_stage {
            0 => rows / 2,
            1 => 0,
            _ => rows - 1,
        };
        let (ty, tc) = back_chunks(
            b,
            cy,
            cc,
            aim,
            self.editwincols(),
            self.cfg.tabsize,
            self.cfg.atblanks,
        );
        self.b().scroll.top_line = ty;
        if self.cfg.softwrap {
            self.b().scroll.top_col = tc;
        }
        self.cycle_stage = (self.cycle_stage + 1) % 3;
        self.force_redraw = true;
    }
}

// ------------------------------------------------------------ backend bits --

pub fn rows_in(chars: &[char], editcols: usize, tab: usize, atblanks: bool) -> usize {
    display::rows_in(chars, editcols, tab, atblanks).max(1)
}

/// Walk back the given number of display rows.
pub fn back_chunks(
    buf: &Buffer,
    cy: usize,
    cchunk: usize,
    mut rows: usize,
    editcols: usize,
    tab: usize,
    atblanks: bool,
) -> (usize, usize) {
    let mut y = cy;
    let mut chunk = cchunk;
    while rows > 0 {
        if chunk > 0 {
            chunk -= 1;
        } else if y > 0 {
            y -= 1;
            chunk = display::rows_in(buf.line(y), editcols, tab, atblanks).max(1) - 1;
        } else {
            break;
        }
        rows -= 1;
    }
    (y, chunk)
}

pub fn forward_chunks(
    buf: &Buffer,
    top: usize,
    topchunk: usize,
    mut rows: usize,
    editcols: usize,
    tab: usize,
    atblanks: bool,
) -> (usize, usize) {
    let mut y = top;
    let mut chunk = topchunk;
    let last = buf.lines.len() - 1;
    while rows > 0 && y <= last {
        let n = display::rows_in(buf.line(y), editcols, tab, atblanks).max(1);
        if chunk + 1 < n {
            chunk += 1;
        } else if y < last {
            y += 1;
            chunk = 0;
        } else {
            break;
        }
        rows -= 1;
    }
    (y, chunk)
}

/// Move across a displayed character together with its combining marks.
pub fn step_left(chars: &[char], x: usize) -> usize {
    let mut x = x.saturating_sub(1);
    while x > 0 && display::advance(chars[x], 0, 1) == 0 {
        x -= 1;
    }
    x
}

pub fn step_right(chars: &[char], x: usize) -> usize {
    let mut x = x + 1;
    while x < chars.len() && display::advance(chars[x], 0, 1) == 0 {
        x += 1;
    }
    x
}

// (Word-step helpers live in editing.rs, next to the word motions.)

// ---------- re-exports used by other modules -------------------------------

pub use crate::defs::{Config, Importance, Pos};
