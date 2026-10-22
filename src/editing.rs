// editing.rs — the text operations: typing, breaking and joining lines,
// deleting, cutting/copying/pasting/zapping, undoing and redoing, verbatim
// input, comments, indenting, matching brackets, justifying, and the
// word counter.
//
// Every mutating operation gathers its data from the untouched buffer,
// builds an ordered list of `Prim`s with pure helpers, applies them with
// `apply_prims()`, and then records one undo operation (or extends the top
// one when actions are contiguous).  Undoing replays the inverse prims in
// reverse order, so prims must always be appended in chronological order.

use crate::buffer::Buffer;
use crate::defs::*;
use crate::display::{advance, column_of};
use crate::editor::{Aim, Editor};

use unicode_width::UnicodeWidthChar;

// ```````````````````````````````````````````````` character classification --

/// Is the character some kind of punctuation?  (Approximates iswpunct().)
pub fn is_punct(ch: char) -> bool {
    !ch.is_alphanumeric() && !ch.is_whitespace() && !ch.is_control()
}

/// Is the character word-forming: alphanumeric, or punctuation when
/// punctuation is allowed to form words (--wordbounds)?  Singular '\0'
/// (our stand-in for end-of-string) is never word-forming.
pub fn is_word_char(ch: char, allow_punct: bool) -> bool {
    ch != '\0' && (ch.is_alphanumeric() || (allow_punct && is_punct(ch)))
}

fn word_char(ch: char, punct: bool, extra: Option<&str>) -> bool {
    if let Some(extra) = extra {
        ch.is_alphanumeric() || extra.contains(ch)
    } else {
        is_word_char(ch, punct)
    }
}

/// Is it a blank: a space, a tab, or some other kind of whitespace?
pub fn is_blank_char(ch: char) -> bool {
    ch == ' ' || ch == '\t' || ((ch as u32) > 0x7F && ch.is_whitespace())
}

/// Test whether a row contains only indentation characters or carriage returns.
pub fn white_string(chars: &[char]) -> bool {
    chars.iter().all(|ch| is_blank_char(*ch) || *ch == '\r')
}

/// Only from U+0300 can code points have zero display width.
pub fn is_zerowidth(ch: char) -> bool {
    (ch as u32) >= 0x300 && UnicodeWidthChar::width(ch) == Some(0)
}

/// Fetch a char safely; the end-of-string stands in as '\0'.
fn fetch(chars: &[char], x: usize) -> char {
    chars.get(x).copied().unwrap_or('\0')
}

/// Count the leading characters removable by one unindent operation. A tab
/// ends the removable prefix; a run of spaces is limited by the tab width.
pub fn length_of_white(chars: &[char], tabsize: usize) -> usize {
    let mut count = 0;
    for &ch in chars {
        if ch == '\t' {
            return count + 1;
        }
        if ch != ' ' {
            return count;
        }
        count += 1;
        if count == tabsize {
            return tabsize;
        }
    }
    chars.len().min(tabsize)
}

/// Count indentation characters before the first nonblank character.
pub fn indent_length(chars: &[char]) -> usize {
    let mut i = 0;
    while i < chars.len() && is_blank_char(chars[i]) {
        i += 1;
    }
    i
}

// ````````````````````````````````````````````````` word stepping helpers --

// Step back until we pass over the start of a word (like do_prev_word()).
// Works on pure character positions; zero-widths are skipped over.
pub fn prev_word_from(
    buf: &Buffer,
    mut y: usize,
    mut x: usize,
    punct_words: bool,
    wordchars: Option<&str>,
) -> (usize, usize) {
    let mut seen_word = false;
    let mut step_forward = false;

    loop {
        // If at the head of a line, move to the end of the preceding one.
        if x == 0 {
            if y == 0 {
                break;
            }
            y -= 1;
            x = buf.line_len(y);
        }

        // Step back one character.
        x = x.saturating_sub(1);

        let row = buf.line(y);
        let ch = fetch(row, x);

        if word_char(ch, punct_words, wordchars) {
            seen_word = true;
            // If at the head of a line now, this surely is a word start.
            if x == 0 {
                break;
            }
        } else if is_zerowidth(ch) {
            // Skip over it.
        } else if seen_word {
            // This is a separator now: we've overshot the word start.
            step_forward = true;
            break;
        }
    }

    if step_forward {
        // Move one character forward again, to sit on the start of a word.
        x = (x + 1).min(buf.line_len(y));
    }

    (y, x)
}

// Step forward until we reach the start (or, when after_ends is TRUE, the
// end) of a word (like do_next_word()).  Returns whether we started on a word.
pub fn next_word_from(
    buf: &Buffer,
    mut y: usize,
    mut x: usize,
    after_ends: bool,
    punct_words: bool,
    wordchars: Option<&str>,
) -> (bool, usize, usize) {
    let row = buf.line(y);
    let started_on_word = word_char(fetch(row, x), punct_words, wordchars);
    let mut seen_space = !started_on_word;
    let mut seen_word = started_on_word;

    loop {
        // If at the end of a line, move to the beginning of the next one.
        if x >= buf.line_len(y) {
            if y + 1 >= buf.lines.len() {
                break;
            }
            y += 1;
            x = 0;
            seen_space = true;
        } else {
            // Step forward one character.
            x += 1;
        }

        let row = buf.line(y);
        let ch = fetch(row, x);

        if after_ends {
            if word_char(ch, punct_words, wordchars) {
                seen_word = true;
            } else if is_zerowidth(ch) {
                // Skip over it.
            } else if seen_word {
                break;
            }
        } else {
            if is_zerowidth(ch) {
                // Skip over it.
            } else if !word_char(ch, punct_words, wordchars) {
                seen_space = true;
            } else if seen_space {
                break;
            }
        }
    }

    (started_on_word, y, x)
}

// ````````````````````````````````````````````````````` wrap and rewrite --

/// The target wrapping width: display columns, adjusted for --fill.
pub fn wrap_at(cfg: &Config, cols: usize) -> usize {
    let fill = cfg.fill;
    if (cols as isize) + fill < 0 {
        0
    } else if fill <= 0 {
        (cols as isize + fill) as usize
    } else {
        fill as usize
    }
}

/// Select a whitespace boundary for a hard line break near `goal`.
/// Keep initial indentation intact and include the entire whitespace run at
/// the chosen boundary. Return the line length when it already fits, or -1
/// when the text provides no usable boundary.
pub fn break_line(chars: &[char], goal: isize, tabsize: usize) -> isize {
    let mut lastblank: Option<usize> = None;
    let mut pos = 0usize;
    let mut column: isize = 0;

    // Indentation belongs to the prefix, not to the candidate breakpoints.
    while pos < chars.len() && is_blank_char(chars[pos]) {
        column += advance(chars[pos], column as usize, tabsize) as isize;
        pos += 1;
    }

    // Measure the prefix and retain an eligible whitespace position.
    while pos < chars.len() && column <= goal {
        if is_blank_char(chars[pos]) {
            lastblank = Some(pos);
        }
        column += advance(chars[pos], column as usize, tabsize) as isize;
        pos += 1;
    }

    // No wrapping is needed when the measured text fits.
    if column <= goal {
        return pos as isize;
    }

    // An overlong first word requires a boundary beyond the preferred width.
    if lastblank.is_none() {
        while pos < chars.len() {
            if is_blank_char(chars[pos]) {
                lastblank = Some(pos);
                break;
            }
            pos += 1;
        }
        if lastblank.is_none() {
            return -1;
        }
    }

    // Keep the selected whitespace run together.
    let lb = lastblank.unwrap();
    let mut pointer = lb + 1;
    while pointer < chars.len() && is_blank_char(chars[pointer]) {
        lastblank = Some(pointer);
        pointer += 1;
    }

    lastblank.unwrap() as isize
}

// ````````````````````````````````````````````````````` paragraph bits --

impl Editor {
    /// Measure the quote prefix identified by the configured expression.
    fn quotelen(&self, chars: &[char]) -> usize {
        let regex = match &self.quoteregex {
            Some(r) => r,
            None => return 0,
        };
        let text: String = chars.iter().collect();
        regex
            .find(&text)
            .map(|m| text[..m.end()].chars().count())
            .unwrap_or(0)
    }

    /// Whether the given row is the beginning of a paragraph (BOP).
    fn begpar(&self, rows: &[Vec<char>], i: usize, depth: isize) -> bool {
        // The very first row counts as a BOP, even when it has no text.
        if i == 0 {
            return true;
        }
        // If recursion is going too deep, just say it's not a BOP.
        if depth > 222 {
            return false;
        }

        let row = &rows[i];
        let quot_len = self.quotelen(row);
        let indent_len = indent_length(&row[quot_len.min(row.len())..]);

        // If this row contains no text, it is not a BOP.
        if fetch(row, quot_len + indent_len) == '\0' {
            return false;
        }

        // When requested, treat a row that starts with whitespace as a BOP.
        if self.cfg.bookstyle
            && !self.cfg.autoindent
            && fetch(row, 0) != '\0'
            && is_blank_char(row[0])
        {
            return true;
        }

        // If the quote part of the preceding row differs, this is a BOP.
        let prev = &rows[i - 1];
        let prev_quot = self.quotelen(prev);
        if quot_len != prev_quot
            || (quot_len > 0 && row[..quot_len] != prev[..quot_len.min(prev.len())])
        {
            return true;
        }

        let prev_dent = indent_length(&prev[prev_quot.min(prev.len())..]);

        // If the preceding row contains no text, this is a BOP.
        if fetch(prev, prev_quot + prev_dent) == '\0' {
            return true;
        }

        // If indentation of this and the preceding row are equal: not a BOP.
        let tab = self.cfg.tabsize;
        if column_of(prev, prev_quot + prev_dent, tab) == column_of(row, quot_len + indent_len, tab)
        {
            return false;
        }

        // Otherwise, this is a BOP when the preceding row is not.
        !self.begpar(rows, i - 1, depth + 1)
    }

    /// Whether the given row is part of a paragraph: when it contains
    /// something more than quoting and leading whitespace.
    fn inpar(&self, rows: &[Vec<char>], i: usize) -> bool {
        let row = &rows[i];
        let quot = self.quotelen(row);
        let dent = indent_length(&row[quot.min(row.len())..]);
        fetch(row, quot + dent) != '\0'
    }

    /// Find the first occurring paragraph in the forward direction, starting
    /// at index `from`.  Returns the index of its first row and the number
    /// of rows, or None when there isn't any paragraph left.
    fn find_paragraph_in(&self, rows: &[Vec<char>], from: usize) -> Option<(usize, usize)> {
        let mut i = from;
        // When not currently in a paragraph, move forward to a row that is.
        while i < rows.len() && !self.inpar(rows, i) {
            i += 1;
        }
        if i >= rows.len() {
            return None;
        }
        let first = i;

        // Move down to the last row of the paragraph (if any).
        while i + 1 < rows.len() && !self.inpar(rows, i) {
            i += 1;
        }
        while i + 1 < rows.len() && self.inpar(rows, i + 1) && !self.begpar(rows, i + 1, 0) {
            i += 1;
        }
        // When not in a paragraph now, there aren't any paragraphs left.
        if !self.inpar(rows, i) {
            return None;
        }

        Some((first, i - first + 1))
    }

    // Moves to the first beginning of a paragraph before the given row.
    fn para_begin_of(&self, rows: &[Vec<char>], mut i: usize) -> usize {
        i = i.saturating_sub(1);
        while !self.begpar(rows, i, 0) {
            i -= 1;
        }
        i
    }

    // Moves down to the last row of the first found paragraph at/after i.
    fn para_end_of(&self, rows: &[Vec<char>], mut i: usize) -> usize {
        let last = rows.len() - 1;
        while i < last && !self.inpar(rows, i) {
            i += 1;
        }
        while i < last && self.inpar(rows, i + 1) && !self.begpar(rows, i + 1, 0) {
            i += 1;
        }
        i
    }

    /// Change all blank characters to spaces, remove excess spaces, and
    /// trim trailing spaces of the row (from the given index onward).
    /// After punctuation, at most two following blanks become two spaces.
    fn squeeze_at(&self, rows: &mut [Vec<char>], i: usize, skip: usize) {
        let src = rows[i].clone();
        let mut out: Vec<char> = src[..skip.min(src.len())].to_vec();
        let mut from = skip.min(src.len());
        let len = src.len();

        while from < len {
            let ch = src[from];
            if is_blank_char(ch) {
                from += 1;
                out.push(' ');
                while from < len && is_blank_char(src[from]) {
                    from += 1;
                }
            } else if self.cfg.punct.contains(ch) {
                out.push(ch);
                from += 1;
                if from < len && self.cfg.brackets.contains(src[from]) {
                    out.push(src[from]);
                    from += 1;
                }
                if from < len && is_blank_char(src[from]) {
                    out.push(' ');
                    from += 1;
                }
                if from < len && is_blank_char(src[from]) {
                    out.push(' ');
                    from += 1;
                }
                while from < len && is_blank_char(src[from]) {
                    from += 1;
                }
            } else {
                out.push(ch);
                from += 1;
            }
        }

        // If there are spaces at the end of the line, remove them.
        while out.len() > skip && out[out.len() - 1] == ' ' {
            out.pop();
        }

        rows[i] = out;
    }

    /// Concatenate into the row at `first` all the rows up to (first+count-1),
    /// skipping the quoting and indentation on all rows after the first, and
    /// making sure each joined stretch ends in a space when needed.
    fn concat_paragraph(&self, rows: &mut Vec<Vec<char>>, first: usize, count: usize) {
        for i in (first + 1)..(first + count) {
            if i >= rows.len() {
                break;
            }
            let next = rows[i].clone();
            let next_quot = self.quotelen(&next);
            let next_lead = next_quot + indent_length(&next[next_quot.min(next.len())..]);

            // If the current row isn't empty, make sure it ends in a space.
            if !rows[first].is_empty() && rows[first][rows[first].len() - 1] != ' ' {
                rows[first].push(' ');
            }
            rows[first].extend_from_slice(&next[next_lead.min(next.len())..]);
        }
        let r = first + count - 1;
        for j in ((first + 1)..=r).rev() {
            if j < rows.len() {
                rows.remove(j);
            }
        }
    }

    /// Rewrap the rows at `i` (starting with the given lead string) into
    /// rows that fit within the target width.  Returns the index one row
    /// past the last rewrapped row (or `i` itself when it is the last).
    fn rewrap_at(
        &self,
        rows: &mut Vec<Vec<char>>,
        mut idx: usize,
        lead: &[char],
        lead_len: usize,
    ) -> usize {
        let wrap = wrap_at(&self.cfg, self.term.cols);
        let tab = self.cfg.tabsize;

        loop {
            if column_of(&rows[idx], rows[idx].len(), tab) <= wrap {
                break;
            }
            let line_len = rows[idx].len();
            let goal = wrap as isize - column_of(&rows[idx], lead_len, tab) as isize;
            let found = break_line(&rows[idx][lead_len.min(line_len)..], goal, tab);

            // If we can't break the line, or don't need to, we're done.
            if found < 0 || lead_len + found as usize >= line_len {
                break;
            }
            let mut break_pos = lead_len + found as usize + 1;

            let rest: Vec<char> = rows[idx][break_pos..].to_vec();

            // When requested, snip the one or two trailing spaces.
            if self.cfg.trimblanks {
                while break_pos > 0 && rows[idx][break_pos - 1] == ' ' {
                    break_pos -= 1;
                }
            }

            let mut new_row: Vec<char> = lead.to_vec();
            new_row.extend(rest);
            rows.insert(idx + 1, new_row);
            rows[idx].truncate(break_pos);
            idx += 1;
        }

        // When possible, point one row past the rewrapped paragraph.
        if idx + 1 < rows.len() {
            idx + 1
        } else {
            idx
        }
    }
}

// `````````````````````````````````````````````````` small state helpers --

impl Editor {
    /// Return selection endpoints ordered from the beginning of the document.
    pub fn region(&self) -> (Pos, Pos) {
        let buf = self.borig();
        let cur = buf.cursor;
        match buf.mark {
            None => (cur, cur),
            Some(m) => {
                if m.y < cur.y || (m.y == cur.y && m.x <= cur.x) {
                    (m, cur)
                } else {
                    (cur, m)
                }
            }
        }
    }

    /// Determine the inclusive row range for a line operation. A selection
    /// ending at column zero normally excludes that final row.
    fn range(&mut self) -> (usize, usize) {
        let buf = self.borig();
        let cur = buf.cursor;
        let mark = buf.mark;
        match mark {
            None => (cur.y, cur.y),
            Some(m) => {
                let (top, bot) = if m.y < cur.y || (m.y == cur.y && m.x <= cur.x) {
                    (m, cur)
                } else {
                    (cur, m)
                };
                if bot.x == 0 && bot.y != top.y && !self.also_the_last {
                    (top.y, bot.y - 1)
                } else {
                    self.also_the_last = true;
                    (top.y, bot.y)
                }
            }
        }
    }

    /// Drop the redo items (they become invalid), then record one complete
    /// operation.  The prims must already have been applied.
    fn record(&mut self, kind: UndoType, _label: &str, prims: Vec<Prim>, was: Pos, is: Pos) {
        self.b().push_undo_full(kind, was, None, is, None, prims);
        self.last_action = kind;
    }

    /// Grow the topmost op with more prims (contiguous typing/deleting).
    /// The prims must already have been applied.  Also refreshes the
    /// last-action state without touching the op's cursor_was.
    fn extend_top(
        &mut self,
        kind: UndoType,
        prims: Vec<Prim>,
        cursor_is: Pos,
        mark_is: Option<Pos>,
    ) {
        self.b().extend_add(&prims, cursor_is, mark_is);
        self.last_action = kind;
    }

    /// Apply an edit batch and include any required final newline in the same
    /// transaction so that undo restores the complete previous state.
    fn apply_with_magic(&mut self, prims: &mut Vec<Prim>) {
        self.b().apply_prims(prims);
        if !self.cfg.nonewlines
            && self
                .borig()
                .lines
                .last()
                .is_some_and(|l| !l.chars.is_empty())
        {
            let at = self.borig().lines.len();
            let prim = Prim::LineIns {
                at,
                text: Vec::new(),
            };
            self.b().apply_prims(std::slice::from_ref(&prim));
            prims.push(prim);
        }
    }

    // Whether the marked region is empty (both endpoints on one spot).
}

// `````````````````````````` extract / graft planning (pure functions) --

/// Extract a range into clipboard fragments while preserving its line breaks.
pub(crate) fn nodes_for(buf: &Buffer, top: Pos, bot: Pos) -> Vec<Vec<char>> {
    let y0 = top.y;
    let y1 = bot.y;
    let rows: Vec<Vec<char>> = buf.lines[y0..=y1].iter().map(|l| l.chars.clone()).collect();

    if y0 == y1 {
        return vec![rows[0][top.x..bot.x].to_vec()];
    }
    if top.x == 0 && bot.x == 0 {
        // Rows y0 .. y1-1 go away together with the first row's newline;
        // the end row's content moves up, and a newline is remembered.
        let mut nodes: Vec<Vec<char>> = rows[..rows.len() - 1].to_vec();
        nodes.push(Vec::new());
        return nodes;
    }
    let mut nodes: Vec<Vec<char>> = vec![rows[0][top.x..].to_vec()];
    for row in &rows[1..rows.len() - 1] {
        nodes.push(row.clone());
    }
    nodes.push(rows[rows.len() - 1][..bot.x].to_vec());
    nodes
}

/// The prims that excise the text between the two points (same geometry
/// as nodes_for()), in chronological application order.
pub(crate) fn prims_for(buf: &Buffer, top: Pos, bot: Pos) -> Vec<Prim> {
    let y0 = top.y;
    let y1 = bot.y;
    let rows: Vec<Vec<char>> = buf.lines[y0..=y1].iter().map(|l| l.chars.clone()).collect();

    let mut prims: Vec<Prim> = Vec::new();

    if y0 == y1 {
        prims.push(Prim::Text {
            y: y0,
            from: top.x,
            old: rows[0][top.x..bot.x].to_vec(),
            new: Vec::new(),
        });
    } else if top.x == 0 && bot.x == 0 {
        for yy in (y0 + 1..=y1).rev() {
            prims.push(Prim::LineDel {
                at: yy,
                text: rows[yy - y0].clone(),
            });
        }
        prims.push(Prim::Text {
            y: y0,
            from: 0,
            old: rows[0].clone(),
            new: rows[rows.len() - 1].clone(),
        });
    } else {
        prims.push(Prim::Text {
            y: y0,
            from: top.x,
            old: rows[0][top.x..].to_vec(),
            new: rows[rows.len() - 1][bot.x..].to_vec(),
        });
        for yy in (y0 + 1..=y1).rev() {
            prims.push(Prim::LineDel {
                at: yy,
                text: rows[yy - y0].clone(),
            });
        }
    }

    prims
}

/// Plan insertion of clipboard fragments. Split the destination row at the
/// insertion point and attach its remaining text after the final fragment.
pub(crate) fn graft_prims(buf: &Buffer, ty: usize, tx: usize, pieces: &[Vec<char>]) -> Vec<Prim> {
    let mut prims: Vec<Prim> = Vec::new();
    let tail: Vec<char> = buf.line(ty)[tx..].to_vec();

    if pieces.len() == 1 {
        prims.push(Prim::Text {
            y: ty,
            from: tx,
            old: Vec::new(),
            new: pieces[0].clone(),
        });
    } else {
        prims.push(Prim::Text {
            y: ty,
            from: tx,
            old: tail.clone(),
            new: pieces[0].clone(),
        });
        for (i, piece) in pieces.iter().enumerate().skip(1) {
            prims.push(Prim::LineIns {
                at: ty + i,
                text: piece.clone(),
            });
        }
        let last_row = ty + pieces.len() - 1;
        prims.push(Prim::Text {
            y: last_row,
            from: pieces[pieces.len() - 1].len(),
            old: Vec::new(),
            new: tail,
        });
    }

    prims
}

/// Calculate the cursor position immediately after inserted clipboard text.
fn graft_end(ty: usize, tx: usize, pieces: &[Vec<char>]) -> Pos {
    if pieces.len() == 1 {
        Pos::new(ty, tx + pieces[0].len())
    } else {
        Pos::new(ty + pieces.len() - 1, pieces[pieces.len() - 1].len())
    }
}

// `````````````````````````````````````` cutting, copying, and pasting --

impl Editor {
    /// Validate a cut target. Reject empty selections and unavailable text at
    /// EOF; optionally protect the required final newline.
    pub fn is_cuttable(&mut self, test_cliff: bool) -> bool {
        let buf = self.borig();
        let cur = buf.cursor;
        let last = buf.lines.len() - 1;
        let from = if test_cliff { cur.x } else { 0 };

        if (cur.y == last && fetch(buf.line(cur.y), from) == '\0' && buf.mark.is_none())
            || buf.mark == Some(cur)
            || (test_cliff
                && from > 0
                && !self.cfg.nonewlines
                && fetch(buf.line(cur.y), from) == '\0'
                && cur.y + 1 == last)
        {
            self.status(Importance::Hush, "Nothing was cut");
            self.b().mark = None;
            return false;
        }
        true
    }

    /// The region plus pieces plus prims for a snip into the cutbuffer,
    /// all gathered from the intact buffer.
    fn snip_plan(&self, until_eof: bool) -> (Pos, Pos, Vec<Vec<char>>, Vec<Prim>) {
        let buf = self.borig();
        let last = buf.lines.len() - 1;

        let (top, bot) = if until_eof {
            (buf.cursor, Pos::new(last, buf.line_len(last)))
        } else if buf.mark.is_some() {
            self.region()
        } else if self.cfg.cut_from_cursor {
            let y = buf.cursor.y;
            if buf.cursor.x < buf.line_len(y) {
                (buf.cursor, Pos::new(y, buf.line_len(y)))
            } else {
                (buf.cursor, Pos::new(y + 1, 0))
            }
        } else {
            let y = buf.cursor.y;
            if y != last {
                (Pos::new(y, 0), Pos::new(y + 1, 0))
            } else {
                (Pos::new(y, 0), Pos::new(y, buf.line_len(y)))
            }
        };

        let nodes = nodes_for(buf, top, bot);
        let prims = prims_for(buf, top, bot);
        (top, bot, nodes, prims)
    }

    /// Execute a cut plan, update the clipboard accumulation state, and return
    /// the applied edits together with the beginning of the affected range.
    fn do_snip(&mut self, marked: bool, until_eof: bool, append: bool) -> (Vec<Prim>, Pos) {
        // A cut after a copy never glues onto what was copied.
        self.keep_cutbuffer &= self.last_action != UndoType::Copy;

        let (top, _bot, nodes, mut prims) = self.snip_plan(until_eof);

        // If cuts were not continuous, or when cutting a region, clear.
        if (marked || until_eof || !self.keep_cutbuffer) && !append {
            self.cutbuffer.clear();
        }
        cutbuffer_receive(&mut self.cutbuffer, nodes);

        self.apply_with_magic(&mut prims);

        // After a line operation, future ones should add to the cutbuffer.
        self.keep_cutbuffer = !marked && !until_eof;

        (prims, top)
    }

    /// Park the buffer cursor and set the wanted column the way a cut's
    /// aftermath does: keep it for an in-row CFF cut, the cursor's column
    /// when a region or a line break was cut, zero for a whole-line cut.
    fn settle_after_cut(&mut self, marked: bool, here: Pos) {
        {
            let b = self.b();
            b.cursor = b.clamp_pos(here);
        }
        if self.cfg.cut_from_cursor && !marked && here.x >= self.borig().line_len(here.y) {
            self.turn_goal_into_cursor();
        } else if self.cfg.cut_from_cursor && !marked {
            // An in-row cut: the wanted column stays as it was.
        } else if marked {
            self.turn_goal_into_cursor();
        } else {
            self.b().goal = 0;
        }
        self.ensure_viewport(Aim::Minimal);
    }

    /// Set the wanted column to the column of the current cursor position.
    fn turn_goal_into_cursor(&mut self) {
        let buf = self.borig();
        let (y, x) = (buf.cursor.y, buf.cursor.x);
        let col = column_of(buf.line(y), x, self.cfg.tabsize);
        self.b().goal = col;
    }

    /// Cut the current line or the marked region into the cutbuffer (^K).
    pub fn cut_text(&mut self) {
        let marked = self.borig().mark.is_some();

        if !self.is_cuttable(self.cfg.cut_from_cursor && !marked) {
            return;
        }

        // Only add a new undo item when the current one is not a cut, or
        // when the cutting is not contiguous.
        let contiguous = self.last_action == UndoType::Cut && self.keep_cutbuffer;

        let was = self.borig().cursor;
        let (prims, top) = self.do_snip(marked, false, false);
        let here = top;

        if contiguous {
            self.extend_top(UndoType::Cut, prims, here, None);
        } else {
            self.record(UndoType::Cut, "", prims, was, here);
        }

        self.settle_after_cut(marked, here);
        self.wipe_status();
    }

    /// Cut from the current cursor position to the end of the file (M-T).
    pub fn cut_till_eof(&mut self) {
        let (_y, _x, _last, nothing): (usize, usize, usize, bool) = {
            let buf = self.borig();
            let y = buf.cursor.y;
            let x = buf.cursor.x;
            let last = buf.lines.len() - 1;
            (
                y,
                x,
                last,
                fetch(buf.line(y), x) == '\0'
                    && (y == last || (!self.cfg.nonewlines && x > 0 && y + 1 == last)),
            )
        };

        if nothing {
            self.status(Importance::Hush, "Nothing was cut");
            return;
        }

        let was = self.borig().cursor;
        let (prims, _top) = self.do_snip(false, true, false);
        let here = self.borig().cursor;

        self.record(UndoType::CutTillEof, "", prims, was, here);
        self.ensure_viewport(Aim::Minimal);
        self.wipe_status();
    }

    /// Erase the current row or the marked region, sending it into oblivion
    /// (running from ^H/^M? no: from DEL and Backspace when --zap is set).
    pub fn zap_text(&mut self) {
        let marked = self.borig().mark.is_some();

        if !self.is_cuttable(self.cfg.cut_from_cursor && !marked) {
            return;
        }

        // Add a new undo item only when the current one is not a zap, or
        // when the zapping is not contiguous.
        let contiguous = self.last_action == UndoType::Zap && self.keep_cutbuffer;

        let was = self.borig().cursor;
        let (prims, top) = self.do_snip(marked, false, true);
        let here = top;

        if contiguous {
            self.extend_top(UndoType::Zap, prims, here, None);
        } else {
            self.record(UndoType::Zap, "", prims, was, here);
        }

        self.settle_after_cut(marked, here);
        self.wipe_status();
    }

    /// Make a copy of the marked region, putting it in the cutbuffer
    /// (the marked branch of ^M).
    fn copy_marked_region(&mut self) {
        let (top, bot) = self.region();

        self.last_action = UndoType::Other;
        self.keep_cutbuffer = false;
        self.b().mark = None;

        if top.y == bot.y && top.x == bot.x {
            self.status(Importance::Hush, "Copied nothing");
            return;
        }

        // The freshly gathered pieces directly become the cutbuffer.
        self.cutbuffer = nodes_for(self.borig(), top, bot);
    }

    /// Copy text into the cutbuffer (^M): the marked region, the whole
    /// row, the text from cursor to end-of-row, or just the row break.
    pub fn copy_text(&mut self) {
        let (y, x, len, total, addition): (usize, usize, usize, usize, Vec<char>) = {
            let buf = self.borig();
            let (y, x) = (buf.cursor.y, buf.cursor.x);
            let len = buf.line_len(y);
            let cff = self.cfg.cut_from_cursor;
            let from_x = if cff { x } else { 0 };
            (y, x, len, buf.lines.len(), buf.line(y)[from_x..].to_vec())
        };

        let at_eol = x >= len;
        let base_sans = self.cfg.nonewlines && y + 1 == total;
        let cff = self.cfg.cut_from_cursor;

        if self.borig().mark.is_some() || self.last_action != UndoType::Copy {
            self.keep_cutbuffer = false;
        }

        if !self.keep_cutbuffer {
            self.cutbuffer.clear();
        }
        self.wipe_status();

        if self.borig().mark.is_some() {
            self.copy_marked_region();
            return;
        }

        // When at the very end of the buffer, there is nothing to do.
        if y + 1 == total && at_eol && (cff || x == 0 || !self.cutbuffer.is_empty()) {
            self.status(Importance::Hush, "Copied nothing");
            return;
        }

        let sans_newline = if cff { !at_eol } else { base_sans };

        if self.cutbuffer.is_empty() && sans_newline {
            self.cutbuffer = vec![addition];
        } else if self.cutbuffer.is_empty() {
            self.cutbuffer = vec![addition, Vec::new()];
        } else if sans_newline {
            // Append the row's tail to the last piece directly.
            let n = self.cutbuffer.len();
            self.cutbuffer[n - 1] = addition;
        } else if cff {
            self.cutbuffer.push(addition);
        } else {
            // Add it just before the trailing empty piece.
            let n = self.cutbuffer.len();
            self.cutbuffer.insert(n - 1, addition);
        }

        // When needed and possible, move the cursor to the next row.
        if (!cff || at_eol) && y + 1 < total {
            self.goto_pos(y + 1, 0);
        } else {
            self.goto_pos(y, self.borig().line_len(y));
        }
        self.last_action = UndoType::Copy;
        self.keep_cutbuffer = true;
    }

    /// Copy text from the cutbuffer into the current buffer (^U).
    pub fn paste_text(&mut self) {
        if self.cutbuffer.is_empty() {
            self.status(Importance::Ahem, "Cutbuffer is empty");
            return;
        }

        let was = self.borig().cursor;
        let pieces: Vec<Vec<char>> = self.cutbuffer.clone();

        let mut prims = graft_prims(self.borig(), was.y, was.x, &pieces);
        let end = graft_end(was.y, was.x, &pieces);
        self.apply_with_magic(&mut prims);

        let old_mark = self.borig().mark;
        self.record(UndoType::Paste, "", prims, was, end);
        if let Some(mark) = old_mark {
            let new_mark = if mark.y > was.y {
                Pos::new(mark.y + pieces.len() - 1, mark.x)
            } else if mark.y == was.y && mark.x > was.x {
                if pieces.len() == 1 {
                    Pos::new(mark.y, mark.x + pieces[0].len())
                } else {
                    Pos::new(end.y, end.x + mark.x - was.x)
                }
            } else {
                mark
            };
            self.b().mark = Some(new_mark);
            if let Some(op) = self.b().undos.last_mut() {
                op.mark_was = old_mark;
                op.mark_is = Some(new_mark);
            }
        }

        {
            let b = self.b();
            b.cursor = b.clamp_pos(end);
        }
        self.turn_goal_into_cursor();
        self.ensure_viewport(Aim::Minimal);
        self.wipe_status();
    }

    /// Delete the word on either side of the cursor (M-BSpace / M-DEL).
    fn chop_word(&mut self, forward: bool) {
        // Where the nearby word boundary sits, from the intact buffer.
        let mark_at: Pos = {
            let buf = self.borig();
            let is_cur = buf.cursor;
            if !forward {
                let (ny, nx) = prev_word_from(
                    buf,
                    is_cur.y,
                    is_cur.x,
                    self.cfg.wordbounds,
                    self.cfg.wordchars.as_deref(),
                );
                if ny != is_cur.y {
                    if is_cur.x > 0 {
                        Pos::new(is_cur.y, 0)
                    } else {
                        Pos::new(ny, buf.line_len(ny))
                    }
                } else {
                    Pos::new(ny, nx)
                }
            } else {
                let (_ok, ny, nx) = next_word_from(
                    buf,
                    is_cur.y,
                    is_cur.x,
                    self.cfg.afterends,
                    self.cfg.wordbounds,
                    self.cfg.wordchars.as_deref(),
                );
                if ny != is_cur.y && is_cur.x < buf.line_len(is_cur.y) {
                    Pos::new(is_cur.y, buf.line_len(is_cur.y))
                } else {
                    Pos::new(ny, nx)
                }
            }
        };

        // The word pieces go to a throwaway cutbuffer; the old one returns.
        let is_cutbuffer = std::mem::take(&mut self.cutbuffer);

        let was = self.borig().cursor;
        self.b().mark = Some(mark_at);

        let (prims, top) = self.do_snip(true, false, false);
        self.record(UndoType::Cut, "", prims, was, top);

        {
            let b = self.b();
            b.cursor = b.clamp_pos(top);
        }
        self.turn_goal_into_cursor();
        self.ensure_viewport(Aim::Minimal);

        // Discard the cut word; restore the original cutbuffer.
        self.cutbuffer = is_cutbuffer;
    }

    /// Delete the word before the cursor (M-BSpace).
    pub fn chop_previous_word(&mut self) {
        let cur = self.borig().cursor;
        if cur.y == 0 && cur.x == 0 {
            self.status(Importance::Hush, "Nothing was cut");
            return;
        }
        self.chop_word(false);
    }

    /// Delete the word after the cursor (M-DEL).
    pub fn chop_next_word(&mut self) {
        self.b().mark = None;
        if self.is_cuttable(true) {
            self.chop_word(true);
        }
    }
}

// ``````````````````````````````````````````````*( typing and deleting --

impl Editor {
    /// A copy of all rows, for the pure paragraph helpers.
    fn row_snapshot(&self) -> Vec<Vec<char>> {
        self.borig().lines.iter().map(|l| l.chars.clone()).collect()
    }

    /// Insert a character batch and combine adjacent typing into one undo step.
    pub fn inject_text(&mut self, burst: &[char]) {
        if burst.is_empty() {
            return;
        }

        let buf = self.borig();
        let (y, x) = (buf.cursor.y, buf.cursor.x);
        let was = buf.cursor;

        // Encode an embedded NUL byte as 0x0A ('\n' means NUL here).
        let burst: Vec<char> = burst.to_vec();
        let count = burst.len();

        // Only add a new undo item when the current item is not an ADD
        // or when the current typing is not contiguous with the previous.
        let contiguous = self.last_action == UndoType::Add
            && !self.borig().saved_matches()
            && self.borig().redos.is_empty()
            && self
                .borig()
                .undos
                .last()
                .is_some_and(|op| op.kind == UndoType::Add && op.cursor_is == Pos::new(y, x));

        // The mark wants to move along: it is to the right of the cursor.
        let mark_shift: Option<Pos> = match buf.mark {
            Some(m) if m.y == y && x < m.x => Some(Pos::new(y, m.x + count)),
            _ => None,
        };

        let mut prims = vec![Prim::Text {
            y,
            from: x,
            old: Vec::new(),
            new: burst,
        }];

        // If text was added to the magic line, create a new magic line.
        self.apply_with_magic(&mut prims);

        let end = Pos::new(y, x + count);

        if contiguous {
            self.extend_top(UndoType::Add, prims, end, None);
        } else {
            self.record(UndoType::Add, "", prims, was, end);
        }

        {
            let b = self.b();
            b.cursor = end;
            if let Some(m) = mark_shift {
                b.mark = Some(m);
            }
        }
        self.turn_goal_into_cursor();
        self.ensure_viewport(Aim::Minimal);
    }

    /// Insert a tab -- or a tab's worth of spaces, or indent the marked
    /// region when a region is marked on more than one row (^I).
    pub fn insert_tab(&mut self) {
        let buf = self.borig();
        let cross = matches!(buf.mark, Some(m) if m.y != buf.cursor.y);

        if cross {
            self.do_indent();
        } else if self.cfg.tabs_to_spaces {
            let col = column_of(buf.line(buf.cursor.y), buf.cursor.x, self.cfg.tabsize);
            let size = self.cfg.tabsize;
            let length = size - (col % size);
            self.inject_text(&vec![' '; length]);
        } else {
            self.inject_text(&['\t']);
        }
    }

    /// Break the current row at the cursor position (<Enter>).
    pub fn enter_key(&mut self) {
        let buf = self.borig();
        let (y, x) = (buf.cursor.y, buf.cursor.x);
        let was = buf.cursor;

        // The sample row: with automatic long-line wrapping active and the
        // next row inside this same paragraph, use its indentation.
        let rows = self.row_snapshot();
        let mut sample = y;
        if self.cfg.autoindent
            && self.cfg.break_long_lines
            && y + 1 < rows.len()
            && self.inpar(&rows, y + 1)
            && !self.begpar(&rows, y + 1, 0)
        {
            sample = y + 1;
        }

        let mut extra = 0usize;
        let mut allblanks = false;
        if self.cfg.autoindent {
            extra = indent_length(&rows[sample]);
            if extra > x {
                extra = x;
            } else if extra == x {
                allblanks = indent_length(&rows[y]) == extra;
            }
        }

        // When there were only blanks before the cursor, the whole blank
        // run moves below: the old row gets cut at zero in that case.
        let at = if allblanks { 0 } else { x };
        let tail: Vec<char> = rows[y][x.min(rows[y].len())..].to_vec();
        let mut new_row: Vec<char> = rows[sample][..extra.min(rows[sample].len())].to_vec();
        new_row.extend(&tail);

        // The mark lives on this row and lies after the cursor: it moves.
        let mark_shift: Option<Pos> = match buf.mark {
            Some(m) if m.y == y && x < m.x => Some(Pos::new(y + 1, m.x - x + extra)),
            _ => None,
        };

        let prims = vec![
            Prim::Text {
                y,
                from: at,
                old: rows[y][at.min(rows[y].len())..].to_vec(),
                new: Vec::new(),
            },
            Prim::LineIns {
                at: y + 1,
                text: new_row,
            },
        ];
        self.b().apply_prims(&prims);

        self.record(UndoType::Enter, "", prims, was, Pos::new(y + 1, extra));

        {
            let b = self.b();
            b.cursor = Pos::new(y + 1, extra);
            if let Some(m) = mark_shift {
                b.mark = Some(m);
            }
        }
        self.turn_goal_into_cursor();
        self.ensure_viewport(Aim::Minimal);
    }

    /// Remove text at the cursor, joining rows at a line boundary. Record the
    /// edit separately or append it to an adjacent deletion transaction.
    fn expunge(&mut self, action: UndoType) -> bool {
        let buf = self.borig();
        let (y, x) = (buf.cursor.y, buf.cursor.x);
        let len = buf.line_len(y);
        let last = buf.lines.len() - 1;

        if x < len {
            // Remove the single character under the cursor.
            let ch = buf.line(y)[x];

            // Marks to the right of the cursor on this row slide left.
            let mark_shift: Option<Pos> = match buf.mark {
                Some(m) if m.y == y && x < m.x => Some(Pos::new(y, m.x - 1)),
                _ => None,
            };

            let mergeable = self.deletable_top(action, y, x);
            let prim = Prim::Text {
                y,
                from: x,
                old: vec![ch],
                new: Vec::new(),
            };
            self.b().apply_prims(std::slice::from_ref(&prim));

            if mergeable {
                self.extend_top(action, vec![prim], Pos::new(y, x), None);
            } else {
                self.record(
                    action,
                    "",
                    vec![prim],
                    Pos::new(y, x + usize::from(action == UndoType::Back)),
                    Pos::new(y, x),
                );
            }

            {
                let b = self.b();
                b.cursor = Pos::new(y, x);
                if let Some(m) = mark_shift {
                    b.mark = Some(m);
                }
            }
            self.turn_goal_into_cursor();
            self.ensure_viewport(Aim::Minimal);
            return true;
        }

        if y == last {
            return false;
        }

        // At end of row: when there is a magic line after a text-heavy
        // row and we're not at row start: don't eat the final newline.
        if y + 1 == last && x != 0 && !self.cfg.nonewlines {
            if action == UndoType::Back {
                // Record a no-op deletion, and move the cursor along.
                self.record(
                    UndoType::Join,
                    "",
                    Vec::new(),
                    Pos::new(y + 1, 0),
                    Pos::new(y, len),
                );
                self.turn_goal_into_cursor();
            }
            return false;
        }

        // Join this row with the next one.
        let eaten: Vec<char> = self.borig().line(y + 1).to_vec();

        // A mark on the eaten row folds into this row.
        let mark_shift: Option<Pos> = match buf.mark {
            Some(m) if m.y == y + 1 => Some(Pos::new(y, m.x + len)),
            _ => None,
        };

        let prims = vec![
            Prim::Text {
                y,
                from: len,
                old: Vec::new(),
                new: eaten.clone(),
            },
            Prim::LineDel {
                at: y + 1,
                text: eaten,
            },
        ];
        self.b().apply_prims(&prims);

        // Undo lands back on the eaten row; redo stays at the end here.
        let was = if action == UndoType::Back {
            Pos::new(y + 1, 0)
        } else {
            Pos::new(y, len)
        };
        self.record(UndoType::Join, "", prims, was, Pos::new(y, len));

        {
            let b = self.b();
            b.cursor = Pos::new(y, len);
            if let Some(m) = mark_shift {
                b.mark = Some(m);
            }
        }
        self.turn_goal_into_cursor();
        self.ensure_viewport(Aim::Minimal);
        true
    }

    /// Check whether this deletion continues the latest recorded deletion.
    fn deletable_top(&self, action: UndoType, y: usize, x: usize) -> bool {
        self.last_action == action
            && self.borig().undos.last().is_some_and(|op| {
                op.kind == action
                    && op.cursor_is.y == y
                    && match action {
                        UndoType::Del => x == op.cursor_is.x,
                        _ => x + 1 == op.cursor_is.x,
                    }
            })
    }

    /// Delete the character under the cursor plus any succeeding
    /// zero-widths, or the marked region with --zap on (<Delete>).
    pub fn delete_forward(&mut self) {
        if self.borig().mark.is_some() && self.cfg.zap {
            self.zap_text();
            return;
        }

        if !self.expunge(UndoType::Del) {
            return;
        }

        loop {
            let buf = self.borig();
            let (y, x) = (buf.cursor.y, buf.cursor.x);
            let len = buf.line_len(y);
            if x < len && is_zerowidth(buf.line(y)[x]) {
                self.expunge(UndoType::Del);
            } else {
                break;
            }
        }
    }

    /// Backspace over one character, or delete the marked region with
    /// --zap on, or join two rows (<Backspace>).
    pub fn backspace(&mut self) {
        if self.borig().mark.is_some() && self.cfg.zap {
            self.zap_text();
            return;
        }

        let buf = self.borig();
        let (y, x) = (buf.cursor.y, buf.cursor.x);

        if x > 0 {
            // Step left over the preceding character, then delete it.
            let target = crate::editor::step_left(buf.line(y), x);
            self.b().cursor = Pos::new(y, target);
            for _ in target..x {
                self.expunge(UndoType::Back);
            }
            if let Some(op) = self.b().undos.last_mut() {
                op.cursor_was = Pos::new(y, x);
            }
        } else if y != 0 {
            self.move_left();
            self.expunge(UndoType::Back);
        }
    }

    // ------------------------------------------------------------- undo --

    /// Undo the recorded changes of the last operation (M-U).
    pub fn do_undo(&mut self) {
        let kind = match self.b().undo() {
            Some(kind) => kind,
            None => {
                self.status(Importance::Ahem, "Nothing to undo");
                return;
            }
        };
        self.status(
            Importance::Hush,
            &format!("Undid {}", crate::buffer::undo_word(kind)),
        );
        self.last_action = UndoType::Other;
        self.turn_goal_into_cursor();
        self.ensure_viewport(Aim::Minimal);
    }

    /// Redo the last thing that was undone (M-R).
    pub fn do_redo(&mut self) {
        let kind = match self.b().redo() {
            Some(kind) => kind,
            None => {
                self.status(Importance::Ahem, "Nothing to redo");
                return;
            }
        };
        self.status(
            Importance::Hush,
            &format!("Redid {}", crate::buffer::undo_word(kind)),
        );
        self.last_action = UndoType::Other;
        self.turn_goal_into_cursor();
        self.ensure_viewport(Aim::Minimal);
    }
}

impl Editor {
    // --------------------------------------------------------- verbatim --

    /// Resolve the ASCII control value for a verbatim Ctrl combination.
    fn ctrl_code(ch: char) -> u32 {
        let c = ch as u32;
        if c >= '@' as u32 && c <= '_' as u32 {
            c - '@' as u32
        } else if c >= '`' as u32 && c <= '~' as u32 {
            c - '`' as u32
        } else if c >= '3' as u32 && c <= '7' as u32 {
            c - '3' as u32 + 27
        } else if ch == '?' || ch == '8' {
            0x7F
        } else if ch == ' ' || ch == '2' {
            0
        } else if ch == '/' {
            31
        } else {
            c
        }
    }

    /// Ask the keyboard for the bytes of the next key codes, and stress
    /// them into the buffer, verbatim (M-V).
    pub fn verbatim_input(&mut self) {
        use crossterm::event::{Event, KeyCode, KeyModifiers};

        self.status(Importance::Info, "Verbatim Input");
        self.draw();

        // Read one keystroke, verbatim: no shortcut should fire.  The
        // outcome: the code points to inject, or None when invalid.
        let byte_code: Option<Vec<char>> = match self.term.next_event() {
            Event::Key(key) => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                let alt = key.modifiers.contains(KeyModifiers::ALT);
                match key.code {
                    // Ctrl-@ (Ctrl-Space, Ctrl-2) gives the NUL byte.
                    KeyCode::Char(ch) if ctrl => {
                        let code = Self::ctrl_code(ch);
                        if code == '\n' as u32 {
                            None
                        } else {
                            Some(vec![char::from_u32(code).unwrap_or('\u{1}')])
                        }
                    }
                    // Enter is invalid in this mode (it would break lines).
                    KeyCode::Enter | KeyCode::Char('\n') | KeyCode::Char('\r') => None,
                    // An escape plus a code: inject the raw escape sequence.
                    KeyCode::Esc => Some(vec!['\u{1B}', '\u{1B}']),
                    KeyCode::Char(ch) if alt => Some(vec!['\u{1B}', ch]),
                    KeyCode::Backspace => Some(vec!['\u{7F}']),
                    // A hexadecimal digit starts six-digit Unicode input.
                    KeyCode::Char(ch) if ch.is_ascii_hexdigit() => self.unicode_entry(ch),
                    // Any other character is entered directly.
                    KeyCode::Char(ch) => Some(vec![ch]),
                    _ => None,
                }
            }
            _ => None,
        };

        match byte_code {
            Some(burst) => {
                self.inject_text(&burst);
                self.wipe_status();
            }
            None => self.status(Importance::Ahem, "Invalid code"),
        }
    }

    /// Gather hexadecimal digits (finishing with Space or Enter) and turn
    /// them into a Unicode character; shows the partial code so far.
    fn unicode_entry(&mut self, first: char) -> Option<Vec<char>> {
        use crossterm::event::{Event, KeyCode};

        let mut digits = 1;
        let mut value: u32 = first.to_digit(16).unwrap_or(0);

        loop {
            if digits < 6 {
                // TRANSLATORS: This is shown while a six-digit hexadecimal
                // Unicode character code (%s) is being typed in.
                let shown = format!("{:0>width$X}", value, width = digits);
                let padded = format!("{:>6}", shown);
                self.status(Importance::Info, &format!("Unicode Input: {}", padded));
                self.draw();
            }

            match self.term.next_event() {
                Event::Key(key) => match key.code {
                    // Finish the code with Enter or Space.
                    KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('\r') => break,
                    KeyCode::Char(that) if ctrl_digit(that) => {
                        value = (value << 4) | that.to_digit(16).unwrap_or(0);
                        digits += 1;
                        if digits == 6 {
                            break;
                        }
                    }
                    _ => return None,
                },
                _ => return None,
            }
        }

        // For a value beyond the Unicode range, or zero, plain fail.
        if value < 0x110000 {
            char::from_u32(value).map(|ch| vec![ch])
        } else {
            None
        }
    }
}

/// Whether the character is a valid hexadecimal digit for Unicode entry.
fn ctrl_digit(ch: char) -> bool {
    ch.is_ascii_hexdigit()
}

// //```````````````````````````````````````` comment, indent and counting --
impl Editor {
    /// Comment or uncomment the current line (or the marked ones) (M-3).
    pub fn do_comment(&mut self) {
        let (top, bot) = self.range();
        let last = self.borig().lines_count() - 1;

        // If only the magic line is selected, don't do anything.
        if top == bot && bot == last && !self.cfg.nonewlines {
            self.status(Importance::Ahem, "Cannot comment past end of file");
            return;
        }

        let press = self.borig().cursor;
        let mark = self.borig().mark;

        // The comment sequence: a simple '#' (no syntax support yet).
        let snap: Vec<Vec<char>> = (top..=bot).map(|y| self.borig().line(y).to_vec()).collect();
        // The magic line cannot be touched (unless it is a written row).
        let magic = |y: usize| !self.cfg.nonewlines && y == last;

        // Figure out whether to comment or uncomment the selected rows.
        let mut all_empty = true;
        let mut action = UndoType::Uncomment;
        for (i, row) in snap.iter().enumerate() {
            let empty = white_string(row);
            let uncommentable = !magic(top + i) && !row.is_empty() && row[0] == '#';
            if !empty && !uncommentable {
                action = UndoType::Comment;
                break;
            }
            all_empty = all_empty && empty;
        }
        // If all selected lines are blank, we comment them.
        if all_empty {
            action = UndoType::Comment;
        }

        // Comment/uncomment each of the selected rows that is possible.
        let mut prims: Vec<Prim> = Vec::new();
        for (i, row) in snap.iter().enumerate() {
            let y = top + i;
            if magic(y) {
                continue;
            }
            match action {
                UndoType::Comment => prims.push(Prim::Text {
                    y,
                    from: 0,
                    old: Vec::new(),
                    new: vec!['#'],
                }),
                _ => {
                    if !row.is_empty() && row[0] == '#' {
                        prims.push(Prim::Text {
                            y,
                            from: 0,
                            old: vec!['#'],
                            new: Vec::new(),
                        });
                    }
                }
            }
        }

        // Shift the mark and the cursor along with the inserted prefix.
        let shifted = |p: Pos| -> Pos {
            if p.y < top || p.y > bot {
                return p;
            }
            let row = &snap[p.y - top];
            match action {
                UndoType::Comment => {
                    if !magic(p.y) && p.x > 0 {
                        Pos::new(p.y, p.x + 1)
                    } else {
                        p
                    }
                }
                _ => {
                    if !magic(p.y) && !row.is_empty() && row[0] == '#' {
                        Pos::new(p.y, p.x.saturating_sub(1))
                    } else {
                        p
                    }
                }
            }
        };

        let cursor_is = shifted(press);
        let mark_fix = mark.map(shifted);

        self.b().apply_prims(&prims);
        {
            let b = self.b();
            b.cursor = cursor_is;
            b.mark = mark_fix;
        }
        if cursor_is != press {
            self.turn_goal_into_cursor();
        }
        self.record(action, "", prims, press, cursor_is);
        self.shift_held = true;
    }

    /// Add an indent (a tab, or a tab's worth of spaces) to the current
    /// line or to all the marked ones (M-}).
    pub fn do_indent(&mut self) {
        let (top, bot) = self.range();

        // Skip any leading empty lines; when they are all empty, give up.
        let mut first = top;
        while first <= bot && self.borig().line_len(first) == 0 {
            first += 1;
        }
        if first > bot {
            return;
        }

        // The indentation: either a bunch of spaces or a single tab.
        let indent: Vec<char> = if self.cfg.tabs_to_spaces {
            vec![' '; self.cfg.tabsize]
        } else {
            vec!['\t']
        };
        let n = indent.len();

        let press = self.borig().cursor;
        let mark = self.borig().mark;

        // Every non-empty line gets the indentation.
        let mut prims: Vec<Prim> = Vec::new();
        for y in first..=bot {
            if self.borig().line_len(y) > 0 {
                prims.push(Prim::Text {
                    y,
                    from: 0,
                    old: Vec::new(),
                    new: indent.clone(),
                });
            }
        }

        let inside = |y: usize| first <= y && y <= bot && !self.borig().line(y).is_empty();
        let cursor_is = if press.x > 0 && inside(press.y) {
            Pos::new(press.y, press.x + n)
        } else {
            press
        };
        let mark_fix = match mark {
            Some(m) if m.x > 0 && inside(m.y) => Some(Pos::new(m.y, m.x + n)),
            other => other,
        };

        self.b().apply_prims(&prims);
        {
            let b = self.b();
            b.cursor = cursor_is;
            b.mark = mark_fix;
        }
        if cursor_is != press {
            self.turn_goal_into_cursor();
        }
        self.record(UndoType::Indent, "", prims, press, cursor_is);
        self.shift_held = true;
    }

    /// Remove an indent (some whitespace, at most a tab's worth) from the
    /// current line or from all the marked ones (M-{ and Shift-Tab).
    pub fn do_unindent(&mut self) {
        let (top, bot) = self.range();
        let tab = self.cfg.tabsize;

        // Skip any leading rows that cannot be unindented.
        let mut first = top;
        while first <= bot && length_of_white(self.borig().line(first), tab) == 0 {
            first += 1;
        }
        if first > bot {
            return;
        }

        let press = self.borig().cursor;
        let mark = self.borig().mark;

        // Remove the leading whitespace, but at most a tab's worth.
        let mut prims: Vec<Prim> = Vec::new();
        for y in top..=bot {
            let len = length_of_white(self.borig().line(y), tab);
            if len > 0 {
                prims.push(Prim::Text {
                    y,
                    from: 0,
                    old: self.borig().line(y)[..len].to_vec(),
                    new: Vec::new(),
                });
            }
        }

        let unshifted = |p: Pos| -> Pos {
            if p.y < top || p.y > bot {
                return p;
            }
            let len = length_of_white(self.borig().line(p.y), tab);
            if len == 0 {
                return p;
            }
            Pos::new(p.y, p.x.saturating_sub(len))
        };

        let cursor_is = unshifted(press);
        let mark_fix = mark.map(unshifted);

        self.b().apply_prims(&prims);
        {
            let b = self.b();
            b.cursor = cursor_is;
            b.mark = mark_fix;
        }
        if cursor_is != press {
            self.turn_goal_into_cursor();
        }
        self.record(UndoType::Unindent, "", prims, press, cursor_is);
        self.shift_held = true;
    }

    /// Report the number of lines, words, and characters, either of the
    /// whole file or of the marked region (M-D).
    pub fn word_count(&mut self) {
        let buf = self.borig();
        let marked = buf.mark.is_some();

        let (top, bot) = if marked {
            self.region()
        } else {
            let last = buf.lines.len() - 1;
            (Pos::new(0, 0), Pos::new(last, buf.line_len(last)))
        };
        let top = Pos::new(top.y, top.x.min(buf.line_len(top.y)));
        let bot = Pos::new(bot.y, bot.x.min(buf.line_len(bot.y)));

        // The number of characters in the relevant area.
        let mut chars: usize = 0;
        if !marked {
            for line in &buf.lines {
                chars += line.chars.len() + 1;
            }
            chars = chars.saturating_sub(1);
        } else {
            if top.y != bot.y {
                for y in top.y + 1..=bot.y {
                    chars += buf.line_len(y) + 1;
                }
                chars += 1;
            }
            chars += buf.line_len(top.y) - top.x;
            chars -= buf.line_len(bot.y) - bot.x;
        }

        // The number of lines.
        let mut lines = bot.y - top.y;
        if bot.x != 0 && !(top.y == bot.y && top.x == bot.x) {
            lines += 1;
        }

        // The number of words: stepping to each next word start, counting
        // punctuation as part of the words, like "wc -w" (in effect).
        let mut words: usize = 0;
        let (mut wy, mut wx) = (top.y, top.x);
        while wy < bot.y || (wy == bot.y && wx < bot.x) {
            let (started, ny, nx) = next_word_from(
                buf,
                wy,
                wx,
                false,
                self.cfg.wordbounds,
                self.cfg.wordchars.as_deref(),
            );
            if started {
                words += 1;
            }
            if (ny, nx) == (wy, wx) {
                break;
            }
            wy = ny;
            wx = nx;
        }

        let prefix = if marked { "In Selection:  " } else { "" };
        self.status(
            Importance::Info,
            &format!(
                "{}{} {},  {} {},  {} {}",
                prefix,
                lines,
                if lines == 1 { "line" } else { "lines" },
                words,
                if words == 1 { "word" } else { "words" },
                chars,
                if chars == 1 {
                    "character"
                } else {
                    "characters"
                },
            ),
        );
    }

    /// Search for a match to the bracket at the cursor position (M-]).
    pub fn match_bracket(&mut self) {
        let (start, pair, reverse) = {
            let buf = self.borig();
            let (y, x) = (buf.cursor.y, buf.cursor.x);
            let ch = fetch(buf.line(y), x);
            let mb: Vec<char> = self.cfg.matchbrackets.chars().collect();
            match mb.iter().position(|b| *b == ch) {
                None => {
                    self.status(Importance::Ahem, "Not a bracket");
                    return;
                }
                Some(idx) => {
                    let half = mb.len() / 2;
                    let wanted = if idx >= half {
                        mb[idx - half]
                    } else {
                        mb[idx + half]
                    };
                    let reverse = idx >= half;
                    (Pos::new(y, x), vec![ch, wanted], reverse)
                }
            }
        };

        // Walk to each next bracket, until the initial one is matched.
        let mut pos = start;
        let mut balance: isize = 1;
        loop {
            let found = find_bracket_from(self.borig(), pos.y, pos.x, reverse, &pair);
            match found {
                None => {
                    // Nothing matched; the cursor never left its spot.
                    self.status(Importance::Ahem, "No matching bracket");
                    return;
                }
                Some(there) => {
                    let same = fetch(self.borig().line(there.y), there.x) == pair[0];
                    balance += if same { 1 } else { -1 };
                    if balance == 0 {
                        self.goto_pos(there.y, there.x);
                        return;
                    }
                    pos = there;
                }
            }
        }
    }
}

/// Scan for either bracket in `pair`, respecting the requested direction.
/// Return its document position, or None at the end of the search.
fn find_bracket_from(
    buf: &Buffer,
    y0: usize,
    x0: usize,
    reverse: bool,
    pair: &[char],
) -> Option<Pos> {
    if reverse {
        let mut y = y0;
        // First step away from the current bracket.
        let mut limit = if x0 == 0 {
            if y == 0 {
                return None;
            }
            y -= 1;
            usize::MAX
        } else {
            x0 - 1
        };
        loop {
            let row = buf.line(y);
            let upto = if limit == usize::MAX || limit + 1 >= row.len() {
                row.len()
            } else {
                limit + 1
            };
            if let Some(found) = row[..upto].iter().rposition(|c| pair.contains(c)) {
                return Some(Pos::new(y, found));
            }
            if y == 0 {
                return None;
            }
            y -= 1;
            limit = usize::MAX;
        }
    } else {
        let mut y = y0;
        let mut from = x0 + 1;
        loop {
            let row = buf.line(y);
            if from < row.len() {
                if let Some(found) = row[from..].iter().position(|c| pair.contains(c)) {
                    return Some(Pos::new(y, from + found));
                }
            }
            if y + 1 >= buf.lines.len() {
                return None;
            }
            y += 1;
            from = 0;
        }
    }
}

// ``````````````````````````````````````````````````````` moving motions --

impl Editor {
    /// Move to the previous word in the file (M-Space).
    pub fn move_prev_word(&mut self) {
        let buf = self.borig();
        let (y, x) = prev_word_from(
            buf,
            buf.cursor.y,
            buf.cursor.x,
            self.cfg.wordbounds,
            self.cfg.wordchars.as_deref(),
        );
        self.goto_pos(y, x);
    }

    /// Move to the next word in the file -- to the end of the current word
    /// when --afterends is set (^Space).
    pub fn move_next_word(&mut self) {
        let buf = self.borig();
        let (_, y, x) = next_word_from(
            buf,
            buf.cursor.y,
            buf.cursor.x,
            self.cfg.afterends,
            self.cfg.wordbounds,
            self.cfg.wordchars.as_deref(),
        );
        self.goto_pos(y, x);
    }

    /// Move up to the first start of a paragraph before the cursor (M-9).
    pub fn move_para_begin(&mut self) {
        let rows = self.row_snapshot();
        let y = self.para_begin_of(&rows, self.borig().cursor.y);
        self.goto_pos(y, 0);
        self.ensure_viewport(Aim::Centering);
    }

    /// Move down to just after the end of the first found paragraph (M-0).
    pub fn move_para_end(&mut self) {
        let rows = self.row_snapshot();
        let y = self.para_end_of(&rows, self.borig().cursor.y);
        if y + 1 < rows.len() {
            self.goto_pos(y + 1, 0);
        } else {
            self.goto_pos(y, self.borig().line_len(y));
        }
        self.ensure_viewport(Aim::Centering);
    }

    /// Move to the preceding block of text (M-7).
    pub fn move_prev_block(&mut self) {
        let buf = self.borig();
        let mut y = buf.cursor.y;
        let mut is_text = false;
        let mut seen_text = false;

        // Skip backward until first blank row after some nonblank ones.
        while y > 0 && (!seen_text || is_text) {
            y -= 1;
            is_text = !white_string(buf.line(y));
            seen_text = seen_text || is_text;
        }

        // Step forward again if we passed text but this row is blank.
        if seen_text && y + 1 < buf.lines.len() && white_string(buf.line(y)) {
            y += 1;
        }

        self.goto_pos(y, 0);
        self.ensure_viewport(Aim::Centering);
    }

    /// Move to the next block of text (M-8).
    pub fn move_next_block(&mut self) {
        let buf = self.borig();
        let last = buf.lines.len() - 1;
        let mut y = buf.cursor.y;
        let mut is_white = white_string(buf.line(y));
        let mut seen_white = is_white;

        // Skip forward until first nonblank row after some blank ones.
        while y < last && (!seen_white || is_white) {
            y += 1;
            is_white = white_string(buf.line(y));
            seen_white = seen_white || is_white;
        }

        self.goto_pos(y, 0);
        self.ensure_viewport(Aim::Centering);
    }
}

// ~~CONT~~

impl Editor {
    /// Reflow paragraphs as a single undoable operation, retaining quote prefixes.
    pub fn justify(&mut self, all: bool) {
        let mut rows = self.row_snapshot();
        let original = rows.clone();
        let (mut at, end) = if all {
            (0, rows.len())
        } else {
            let p = self.borig().cursor.y;
            let mut first = p;
            while first > 0 && !self.begpar(&rows, first, 0) {
                first -= 1;
            }
            let Some((first, count)) = self.find_paragraph_in(&rows, first) else {
                return;
            };
            (first, first + count)
        };
        let mut remaining = end;
        while at < rows.len() && at < remaining {
            if !self.inpar(&rows, at) {
                at += 1;
                continue;
            }
            let last = self.para_end_of(&rows, at).min(remaining.saturating_sub(1));
            let count = last - at + 1;
            let quote = self.quotelen(&rows[at]);
            let leading = quote + indent_length(&rows[at][quote..]);
            let lead = rows[at][..leading].to_vec();
            self.concat_paragraph(&mut rows, at, count);
            self.squeeze_at(&mut rows, at, leading);
            let oldlen = rows.len();
            let next = self.rewrap_at(&mut rows, at, &lead, leading);
            remaining = remaining.saturating_sub(count - 1) + (rows.len() - oldlen);
            at = next.max(at + 1);
        }
        if rows != original {
            let text = rows
                .iter()
                .map(|l| l.iter().collect::<String>())
                .collect::<Vec<_>>()
                .join("\n");
            let (a, b) = self.whole_range();
            self.replace_range(a, b, &text, UndoType::Justify);
        }
    }
    pub fn hard_wrap(&mut self) {
        if !self.cfg.break_long_lines {
            return;
        }
        let y = self.borig().cursor.y;
        let x = self.borig().cursor.x;
        let line = self.borig().line(y);
        let width = wrap_at(&self.cfg, self.term.cols);
        if column_of(line, line.len(), self.cfg.tabsize) <= width {
            return;
        }
        let point = break_line(line, width as isize, self.cfg.tabsize);
        if point < 0 {
            return;
        }
        let point = point as usize + 1;
        if point >= line.len() || point > x {
            return;
        }
        self.goto_pos(y, point);
        self.enter_key();
        let newy = self.borig().cursor.y;
        let indent = self.borig().cursor.x;
        self.goto_pos(newy, indent + x - point);
    }
}
