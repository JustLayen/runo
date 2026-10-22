// display.rs — the "wideness" machinery: display columns, control-character
// representation, soft-wrap chunk computations and horizontal paging.

use unicode_width::UnicodeWidthChar;

use crate::defs::*;

/// Measure a character at its current screen column; tabs use the next tab stop.
pub fn advance(ch: char, col: usize, tabsize: usize) -> usize {
    if ch == '\t' {
        tabsize - (col % tabsize)
    } else if is_cntrl(ch) {
        2
    } else {
        let w = UnicodeWidthChar::width(ch).unwrap_or(1);
        if w == 0 {
            0
        } else {
            w
        }
    }
}

/// The number of display columns used by the given stretch of chars.
pub fn wideness(chars: &[char], tabsize: usize) -> usize {
    let mut col = 0;
    for &ch in chars {
        col += advance(ch, col, tabsize);
    }
    col
}

/// Measure the display width of the first `upto` characters.
pub fn column_of(chars: &[char], upto: usize, tabsize: usize) -> usize {
    let mut col = 0;
    for &ch in chars.iter().take(upto) {
        let step = advance(ch, col, tabsize);
        col += step;
    }
    col
}

/// Locate the character occupying a display column, clamping to the line end.
pub fn actual_x(chars: &[char], column: usize, tabsize: usize) -> usize {
    let mut col = 0;
    for (i, &ch) in chars.iter().enumerate() {
        let step = advance(ch, col, tabsize);
        if step == 0 {
            continue;
        }
        if col + step > column {
            return i;
        }
        col += step;
    }
    chars.len()
}

/// Convert a control character (except tab) into its two-column caret form.
pub fn cntrl_rep(ch: char) -> String {
    match ch as u32 {
        0x7F => String::from("^?"),
        c if c < 0x20 => {
            let letter = char::from_u32(c + '@' as u32).unwrap_or('?');
            format!("^{}", letter)
        }
        _ => String::new(),
    }
}

/// Is a character a control character that gets shown as a caret symbol?
pub fn is_cntrl(ch: char) -> bool {
    (ch as u32) < 0x20 || ch as u32 == 0x7F
}

/// The dimensions of one soft-wrapped chunk: the column where it starts,
/// where it ends (exclusive), and the char indexes spanning it.
pub fn compute_chunks(
    chars: &[char],
    editwincols: usize,
    tabsize: usize,
    atblanks: bool,
) -> Vec<Chunk> {
    let width = editwincols.max(1);
    let mut columns = vec![0usize];
    for &ch in chars {
        let col = *columns.last().unwrap();
        columns.push(col + advance(ch, col, tabsize));
    }
    let total = *columns.last().unwrap();
    let mut chunks = Vec::new();
    let mut left = 0;
    loop {
        let edge = left + width;
        let mut right = edge.min(total);
        if total > edge {
            // Keep a wide glyph together when it fits on a fresh row.
            if let Some((i, &col)) = columns
                .iter()
                .enumerate()
                .take(chars.len())
                .find(|&(i, &col)| col < edge && columns[i + 1] > edge)
            {
                if chars[i] != '\t' && col > left {
                    right = col;
                }
            }
            if atblanks {
                if let Some(i) = (0..chars.len()).rev().find(|&i| {
                    columns[i] >= left && columns[i + 1] <= right && matches!(chars[i], ' ' | '\t')
                }) {
                    right = columns[i + 1];
                }
            }
        }
        chunks.push(Chunk { left, right });
        if right >= total {
            if total > 0 && total == edge {
                chunks.push(Chunk {
                    left: total,
                    right: total,
                });
            }
            break;
        }
        left = right.max(left + 1);
    }
    chunks
}

/// The number of wrapped rows of a line.
pub fn rows_in(chars: &[char], editwincols: usize, tabsize: usize, atblanks: bool) -> usize {
    compute_chunks(chars, editwincols, tabsize, atblanks)
        .len()
        .max(1)
}

/// Choose the horizontal page containing the cursor, with context at each edge.
pub fn page_start(column: usize, editwincols: usize, softwrap: bool) -> usize {
    if column == 0 || column + 2 < editwincols || softwrap {
        0
    } else if editwincols > 8 {
        let base = column - 6;
        base - (base % (editwincols - 8))
    } else {
        column - (editwincols - 2)
    }
}
