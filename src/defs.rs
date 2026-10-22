// definitions.rs — shared types and constants for Runo.

use std::collections::HashMap;

pub const NAME: &str = "MIT/Runo";
pub const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION_MAJOR"),
    ".",
    env!("CARGO_PKG_VERSION_MINOR")
);
pub const BRANDING: &str = concat!(
    "MIT/Runo ",
    env!("CARGO_PKG_VERSION_MAJOR"),
    ".",
    env!("CARGO_PKG_VERSION_MINOR")
);

// }} ---------------------------------------------------------- messages --
/// Status message priority and display lifetime.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Importance {
    /// shown with default formatting, wipes upon the next 20 keypresses
    Hush,
    /// mid-blue on default
    Info,
    /// shown with warning colors, leaps away after one keypress
    Ahem,
    /// shown with alert colors
    Alert,
}

// }} ---------------------------------------------------------------- menus --
pub const MMAIN: u32 = 1 << 0;
pub const MWHEREIS: u32 = 1 << 1;
pub const MREPLACE: u32 = 1 << 2;
pub const MREPLACEWITH: u32 = 1 << 3;
pub const MGOTOLINE: u32 = 1 << 4;
pub const MWRITEFILE: u32 = 1 << 5;
pub const MINSERTFILE: u32 = 1 << 6;
pub const MEXECUTE: u32 = 1 << 7;
pub const MHELP: u32 = 1 << 8;
pub const MSPELL: u32 = 1 << 9;
pub const MBROWSER: u32 = 1 << 10;
pub const MWHEREISFILE: u32 = 1 << 11;
pub const MGOTODIR: u32 = 1 << 12;
pub const MYESNO: u32 = 1 << 13;
pub const MFINDINHELP: u32 = 1 << 15;

pub const MMOST: u32 = MMAIN
    | MWHEREIS
    | MREPLACE
    | MREPLACEWITH
    | MGOTOLINE
    | MWRITEFILE
    | MINSERTFILE
    | MEXECUTE
    | MWHEREISFILE
    | MGOTODIR
    | MFINDINHELP
    | MSPELL;
pub const MSOME: u32 = MMOST | MBROWSER;

// }} --------------------------------------------------------- undo types --
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UndoType {
    Add,
    Enter,
    Back,
    Del,
    Join,
    Replace,
    Indent,
    Unindent,
    Comment,
    Uncomment,
    Zap,
    Cut,
    CutTillEof,
    Paste,
    Insert,
    Justify,
    Other,
    /// The "just copied" pseudo-state of last_action (never undoable).
    Copy,
}

// }} ------------------------------------------------------------- formats --
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Nix,
    Dos,
    Mac,
}

// ------------------------------------------------------------------ types --
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Pos {
    pub y: usize,
    pub x: usize,
}

impl Pos {
    pub fn new(y: usize, x: usize) -> Self {
        Pos { y, x }
    }
}

/// Settings, mutable through command-line options and toggles.
pub struct Config {
    pub showcursor: bool,
    pub colonparsing: bool,
    pub magic: bool,
    pub wordchars: Option<String>,
    pub punct: String,
    pub brackets: String,
    pub whitespace_chars: String,
    pub noread: bool,
    pub rebinddelete: bool,
    pub preserve: bool,
    pub whitespace: bool,
    pub no_syntax: bool,
    pub syntax_name: Option<String>,
    pub syntaxes: Vec<crate::syntax::Syntax>,
    pub expansions: Vec<String>,
    pub bindings: Vec<(u32, crate::keys::Key, Option<crate::keys::F>)>,
    pub colors: HashMap<String, String>,
    pub historylog: bool,
    pub positionlog: bool,
    pub locking: bool,
    pub unix: bool,
    pub search_case: bool,
    pub search_regexp: bool,
    pub smarthome: bool,
    pub tabs_to_spaces: bool,
    pub cut_from_cursor: bool,
    pub zap: bool,
    pub jumpyscrolling: bool,
    pub linenumbers: bool,
    pub softwrap: bool,
    pub atblanks: bool,
    pub break_long_lines: bool,
    pub autoindent: bool,
    pub bookstyle: bool,
    pub trimblanks: bool,
    pub constantshow: bool,
    pub quickblank: bool,
    pub mouse: bool,
    pub view_mode: bool,
    pub restricted: bool,
    pub nohelp: bool,
    pub emptyline: bool,
    pub zero: bool,
    pub minibar: bool,
    pub stateflags: bool,
    pub indicator: bool,
    pub save_on_exit: bool,
    pub backup: bool,
    pub backup_dir: Option<String>,
    pub nonewlines: bool,
    pub noconvert: bool,
    pub multibuffer: bool,
    pub bold: bool,
    pub wordbounds: bool,
    pub afterends: bool,
    pub tabsize: usize,
    pub fill: isize,
    pub guidestripe: Option<isize>,
    pub quotestr: String,
    pub matchbrackets: String,
    pub operdir: Option<String>,
    pub speller: Option<Vec<String>>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            showcursor: false,
            colonparsing: false,
            magic: false,
            wordchars: None,
            punct: "!.?".into(),
            brackets: "\"')>]}".into(),
            whitespace_chars: "»·".into(),
            noread: false,
            rebinddelete: false,
            preserve: false,
            whitespace: false,
            no_syntax: false,
            syntax_name: None,
            syntaxes: Vec::new(),
            expansions: Vec::new(),
            bindings: Vec::new(),
            colors: HashMap::new(),
            historylog: false,
            positionlog: false,
            locking: false,
            unix: false,
            search_case: false,
            search_regexp: false,
            smarthome: false,
            tabs_to_spaces: false,
            cut_from_cursor: false,
            zap: false,
            jumpyscrolling: false,
            linenumbers: false,
            softwrap: false,
            atblanks: false,
            break_long_lines: false,
            autoindent: false,
            bookstyle: false,
            trimblanks: false,
            constantshow: false,
            quickblank: false,
            mouse: false,
            view_mode: false,
            restricted: false,
            nohelp: false,
            emptyline: false,
            zero: false,
            minibar: false,
            stateflags: false,
            indicator: false,
            save_on_exit: false,
            backup: false,
            backup_dir: None,
            nonewlines: false,
            noconvert: false,
            multibuffer: false,
            bold: false,
            wordbounds: false,
            afterends: false,
            tabsize: 8,
            fill: -8,
            guidestripe: None,
            quotestr: String::from(r"^([ \t]*([!#%:;>|}]|/{2}))+"),
            matchbrackets: String::from("(<[{)>]}"),
            operdir: None,
            speller: None,
        }
    }
}

/// A single line of a buffer.
#[derive(Clone, Debug)]
pub struct BLine {
    pub chars: Vec<char>,
    pub has_anchor: bool,
}

impl BLine {
    pub fn new(chars: Vec<char>) -> Self {
        BLine {
            chars,
            has_anchor: false,
        }
    }

    pub fn empty() -> Self {
        BLine::new(Vec::new())
    }
}

/// Undo primitives; applying them in order reproduces the edit.
/// Undoing means applying the inverses in reverse order.
#[derive(Clone, Debug)]
pub enum Prim {
    /// Replace the stretch (from, from+old.len()) on the given row.
    Text {
        y: usize,
        from: usize,
        old: Vec<char>,
        new: Vec<char>,
    },
    /// Insert a fresh line *before* row `at`, with the given content.
    LineIns { at: usize, text: Vec<char> },
    /// Delete row `at` (remembering its content for the inverse).
    LineDel { at: usize, text: Vec<char> },
}

#[derive(Clone, Debug)]
pub struct UndoOp {
    pub kind: UndoType,
    pub prims: Vec<Prim>,
    pub cursor_was: Pos,
    pub cursor_is: Pos,
    pub mark_was: Option<Pos>,
    /// whether the mark existed after the op
    pub mark_is: Option<Pos>,
    /// history position before this op was applied
    pub serial_was: usize,
    /// history position after this op was applied
    pub serial_is: usize,
}

/// Clipboard text split at line breaks. A final empty fragment preserves a
/// trailing newline; the first fragment is inserted at the current column.
pub type CutBuffer = Vec<Vec<char>>;

/// Accumulate a consecutive cut, joining the touching fragments and retaining
/// every intervening line break.
pub fn cutbuffer_receive(cut: &mut Vec<Vec<char>>, mut nodes: Vec<Vec<char>>) {
    if nodes.is_empty() {
        return;
    }
    if !cut.is_empty() {
        let first = nodes.remove(0);
        if let Some(last) = cut.last_mut() {
            last.extend(first);
        }
    }
    cut.extend(nodes);
}

/// Simple wrap-part cache for one row of the buffer (in columns).
#[derive(Clone, Debug, Default)]
pub struct Chunk {
    /// display column at which this chunk starts (0 for the first)
    pub left: usize,
    /// display column just beyond this chunk
    pub right: usize,
}

/// The current viewport state of one file.
#[derive(Default)]
pub struct Scroll {
    /// first visible line
    pub top_line: usize,
    /// for softwrap: column at which the first displayed row starts
    pub top_col: usize,
    /// non-softwrap: horizontal page start (global "firstcolumn")
    pub page_col: usize,
}
