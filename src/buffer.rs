// buffer.rs — the file buffer: lines, cursor, mark, undo/redo machinery.

use crate::defs::*;

/// One open file.
pub struct Buffer {
    pub lines: Vec<BLine>,
    pub cursor: Pos,
    /// the wanted display column ("placewewant")
    pub goal: usize,
    pub mark: Option<Pos>,
    pub name: Option<String>,
    pub fmt: Format,
    pub scroll: Scroll,
    /// undo list (in order of application) and its undo-able remainder
    pub undos: Vec<UndoOp>,
    pub redos: Vec<UndoOp>,
    /// monotonically increasing counter of applied history steps
    pub serial: usize,
    pub saved_serial: usize,
    pub next_serial: usize,
    pub mime: Option<String>,
    pub disk_snapshot: Option<Vec<u8>>,
    pub lock: Option<crate::locking::Lock>,
    /// this buffer is the help viewer
    pub help: bool,
}

impl Default for Buffer {
    fn default() -> Self {
        Buffer::new_empty()
    }
}

impl Buffer {
    pub fn new_empty() -> Buffer {
        Buffer {
            lines: vec![BLine::empty()],
            cursor: Pos::new(0, 0),
            goal: 0,
            mark: None,
            name: None,
            fmt: Format::Nix,
            scroll: Scroll::default(),
            undos: Vec::new(),
            redos: Vec::new(),
            serial: 0,
            saved_serial: 0,
            next_serial: 1,
            mime: None,
            disk_snapshot: None,
            lock: None,
            help: false,
        }
    }

    /// Whether the buffer ends in a newline: a trailing empty row (the
    /// "magic line") trails the last written row.
    pub fn magic(&self) -> bool {
        self.lines.last().is_some_and(|l| l.chars.is_empty())
    }

    /// Represent a required final newline by appending an empty row when needed.
    pub fn add_magic(&mut self) {
        if let Some(last) = self.lines.last() {
            if !last.chars.is_empty() {
                self.lines.push(BLine::empty());
            }
        }
    }

    /// Build a buffer from file text; when `convert` is false
    /// (--noconvert), carriage returns are kept as literal characters.
    pub fn from_string(text: &str, convert: bool) -> Buffer {
        let mut b = Buffer::new_empty();
        let mut lines: Vec<Vec<char>> = Vec::new();
        let mut cur: Vec<char> = Vec::new();

        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let ch = chars[i];
            if convert && ch == '\r' {
                lines.push(std::mem::take(&mut cur));
                if chars.get(i + 1) == Some(&'\n') {
                    i += 1;
                }
            } else if ch == '\n' {
                lines.push(std::mem::take(&mut cur));
            } else {
                cur.push(ch);
            }
            i += 1;
        }
        lines.push(cur);

        // A file that ends in a newline leaves one empty row behind; that
        // row *is* the trailing newline (the phantom "magic line").
        b.fmt = if convert && text.contains("\r\n") {
            Format::Dos
        } else if convert && text.contains('\r') {
            Format::Mac
        } else {
            Format::Nix
        };

        b.lines = lines.into_iter().map(BLine::new).collect();
        b
    }

    /// Serialize the buffer back into file text, honouring the format.
    /// Rows are joined with the format's line separator; joining a buffer
    /// whose last row is empty automatically provides the final newline.
    pub fn serialize(&self) -> String {
        let joiner = match self.fmt {
            Format::Nix => "\n",
            Format::Dos => "\r\n",
            Format::Mac => "\r",
        };
        let mut out = String::new();
        for (i, ln) in self.lines.iter().enumerate() {
            out.push_str(&ln.chars.iter().collect::<String>());
            if i + 1 < self.lines.len() {
                out.push_str(joiner);
            }
        }
        out
    }

    pub fn line(&self, y: usize) -> &[char] {
        &self.lines[y].chars
    }

    pub fn line_len(&self, y: usize) -> usize {
        self.lines[y].chars.len()
    }

    pub fn lines_count(&self) -> usize {
        self.lines.len()
    }

    pub fn clamp_pos(&self, p: Pos) -> Pos {
        let y = p.y.min(self.lines.len() - 1);
        Pos::new(y, p.x.min(self.lines[y].chars.len()))
    }

    /// The start and end of the marked region, in reading order.
    pub fn get_region(&self) -> (Pos, Pos) {
        let mark = self.mark.unwrap_or(self.cursor);
        if (mark.y, mark.x) > (self.cursor.y, self.cursor.x) {
            (self.cursor, mark)
        } else {
            (mark, self.cursor)
        }
    }

    // ````````````````````````````````````````````````````````````` prims --

    fn apply_prim(&mut self, p: &Prim) {
        match p {
            Prim::Text { y, from, old, new } => {
                if *y < self.lines.len() {
                    let line = &mut self.lines[*y];
                    if *from + old.len() <= line.chars.len() {
                        line.chars
                            .splice(*from..(*from + old.len()), new.iter().cloned());
                    }
                }
            }
            Prim::LineIns { at, text } => {
                let at = (*at).min(self.lines.len());
                self.lines.insert(at, BLine::new(text.clone()));
            }
            Prim::LineDel { at, .. } => {
                if *at < self.lines.len() {
                    self.lines.remove(*at);
                }
            }
        }
    }

    fn apply_inverse(&mut self, p: &Prim) {
        match p {
            Prim::Text { y, from, old, new } => {
                self.apply_prim(&Prim::Text {
                    y: *y,
                    from: *from,
                    old: new.clone(),
                    new: old.clone(),
                });
            }
            Prim::LineIns { at, text } => {
                self.apply_prim(&Prim::LineDel {
                    at: *at,
                    text: text.clone(),
                });
            }
            Prim::LineDel { at, text } => {
                self.apply_prim(&Prim::LineIns {
                    at: *at,
                    text: text.clone(),
                });
            }
        }
    }

    /// Apply prims in the recorded order (redoing, or replaying an action).
    pub fn apply_prims(&mut self, prims: &[Prim]) {
        for p in prims {
            self.apply_prim(p);
        }
    }

    // ````````````````````````````````````````````````````````````` undo/redo --

    /// Push a complete operation onto the undo stack (displacing redos).
    pub fn push_undo_full(
        &mut self,
        kind: UndoType,
        cursor_was: Pos,
        mark_was: Option<Pos>,
        cursor_is: Pos,
        mark_is: Option<Pos>,
        prims: Vec<Prim>,
    ) {
        self.redos.clear();
        let op = UndoOp {
            kind,
            prims,
            cursor_was,
            cursor_is,
            mark_was,
            mark_is,
            serial_was: self.serial,
            serial_is: self.next_serial,
        };
        self.undos.push(op);
        self.serial = self.next_serial;
        self.next_serial += 1;
    }

    /// Grow the topmost ADD op (contiguous typing).  Record-only: the
    /// prims must already have been applied; cursor_was stays untouched.
    pub fn extend_add(&mut self, prims: &[Prim], cursor_is: Pos, mark_is: Option<Pos>) {
        if let Some(top) = self.undos.last_mut() {
            top.prims.extend_from_slice(prims);
            top.cursor_is = cursor_is;
            top.mark_is = mark_is;
        }
    }

    pub fn undo(&mut self) -> Option<UndoType> {
        if self.undos.is_empty() {
            return None;
        }
        let op = self.undos.pop().unwrap();
        let kind = op.kind;
        self.mark = op.mark_was;
        for p in op.prims.iter().rev() {
            self.apply_inverse(p);
        }
        self.cursor = self.clamp_pos(op.cursor_was);
        self.serial = op.serial_was;
        self.redos.push(op);
        Some(kind)
    }

    pub fn redo(&mut self) -> Option<UndoType> {
        if self.redos.is_empty() {
            return None;
        }
        let op = self.redos.pop().unwrap();
        let kind = op.kind;
        self.mark = op.mark_is;
        self.apply_prims(&op.prims);
        self.cursor = self.clamp_pos(op.cursor_is);
        self.serial = op.serial_is;
        self.undos.push(op);
        Some(kind)
    }

    pub fn is_modified(&self) -> bool {
        self.saved_serial != self.serial
    }

    pub fn set_saved(&mut self) {
        self.saved_serial = self.serial;
    }

    /// Whether saving produced a state equal to the current one.
    pub fn saved_matches(&self) -> bool {
        self.saved_serial == self.serial
    }
}

/// How many columns wide is the number `n`?
pub fn digits(n: usize) -> usize {
    let mut d = 1;
    let mut v = n;
    while v >= 10 {
        v /= 10;
        d += 1;
    }
    d
}

/// The noun used in "Undid %s" / "Redid %s".
pub fn undo_word(kind: UndoType) -> &'static str {
    match kind {
        UndoType::Add => "addition",
        UndoType::Enter => "line break",
        UndoType::Back | UndoType::Del => "deletion",
        UndoType::Join => "line join",
        UndoType::Replace => "replacement",
        UndoType::Zap => "erasure",
        UndoType::Cut | UndoType::CutTillEof => "cut",
        UndoType::Paste => "paste",
        UndoType::Insert => "insertion",
        UndoType::Indent => "indent",
        UndoType::Unindent => "unindent",
        UndoType::Comment => "comment",
        UndoType::Uncomment => "uncomment",
        UndoType::Justify => "justification",
        UndoType::Copy => "copy",
        _ => "keystroke",
    }
}
