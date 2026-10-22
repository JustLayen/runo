// keys.rs — key encoding, the function table and the shortcut list.

use crate::defs::*;

// }} ----------------------------------------------------------------- Keys --

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Key {
    ShiftChar(char, bool, bool),
    /// char, ctrl, alt
    Char(char, bool, bool),
    /// Backspace (alt ⇨ Alt+Backspace)
    Backspace(bool),
    /// Delete: ctrl, alt
    Del(bool, bool),
    /// Home: ctrl, alt (each used for a different command)
    Home(bool, bool),
    /// End: ctrl, alt
    End(bool, bool),
    /// cursors: ctrl, alt
    Up(bool, bool),
    Down(bool, bool),
    Left(bool, bool),
    Right(bool, bool),
    Ins,
    PgUp,
    PgDn,
    Enter,
    Tab,
    ShTab,
    Space,
    F(u8),
}

// }} --------------------------------------------------------------- F list ---

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum F {
    Expansion(usize),
    RecordMacro,
    RunMacro,
    Complete,
    Anchor,
    PrevAnchor,
    NextAnchor,
    Formatter,
    Linter,
    Help,
    Cancel,
    Exit,
    WriteOut,
    ReadFile,
    SearchForward,
    SearchBackward,
    FindNext,
    FindPrev,
    Replace,
    FlipReplace,
    FlipGoto,
    Cut,
    Paste,
    Copy,
    Execute,
    Justify,
    FullJustify,
    Location,
    GoToLine,
    Undo,
    Redo,
    Mark,
    CaseSens,
    Regexp,
    Backwards,
    OlderItem,
    NewerItem,
    GotoDir,
    BrowserSearchForward,
    BrowserSearchBackward,
    BrowserFindPrev,
    BrowserFindNext,
    BrowserFirst,
    BrowserLast,
    Left,
    Right,
    PrevWord,
    NextWord,
    Home,
    End,
    Up,
    Down,
    ScrollUp,
    ScrollDown,
    PrevBlock,
    NextBlock,
    ParaBegin,
    ParaEnd,
    TopRow,
    BottomRow,
    PageUp,
    PageDown,
    FirstLine,
    LastLine,
    PrevFile,
    NextFile,
    Tab,
    Enter,
    Backspace,
    Delete,
    ChopWordLeft,
    ChopWordRight,
    CutTillEof,
    WordCount,
    Verbatim,
    Indent,
    Unindent,
    Comment,
    FindBracket,
    Zap,
    SpellCheck,
    Suspend,
    SaveFile,
    NewBufferToggle,
    PipeToggle,
    DosFormat,
    MacFormat,
    Append,
    Prepend,
    Backup,
    DiscardBuffer,
    ToFiles,
    NoConvert,
    FlipExecute,
    Refresh,
    Cycle,
    ToggleZero,
    ToggleNoHelp,
    ToggleConstant,
    ToggleSoftwrap,
    ToggleLinenumbers,
    ToggleWhitespace,
    ToggleSmartHome,
    ToggleAutoindent,
    ToggleCutFromCursor,
    ToggleBreakLong,
    ToggleTabsToSpaces,
    ToggleMouse,
    ToggleNoSyntax,
    /// ^Z in the main menu only suggests the two-step suspension.
    SuggestSuspend,
}
use F as FX;

// }} ------------------------------------------------------- function tags ---

// Menu labels in display priority order. Repeated functions may have different
// labels in separate menus; the renderer filters duplicates within each menu.
pub(crate) static FUNCS: &[(FX, &str, u32)] = &[
    (FX::RecordMacro, "Record Macro", MEXECUTE),
    (FX::Formatter, "Formatter", MEXECUTE),
    (FX::Linter, "Linter", MEXECUTE),
    (FX::Help, "Help", MSOME),
    (FX::Cancel, "Cancel", (MMOST & !MMAIN) | MYESNO),
    (FX::Exit, "Exit", MMAIN),
    (FX::Exit, "Close", MHELP),
    (FX::WriteOut, "Write Out", MMAIN),
    (FX::ReadFile, "Read File", MMAIN),
    (FX::SearchForward, "Where Is", MMAIN | MHELP),
    (FX::Replace, "Replace", MMAIN),
    (FX::Cut, "Cut", MMAIN),
    (FX::Paste, "Paste", MMAIN),
    (FX::Execute, "Execute", MMAIN),
    (FX::Justify, "Justify", MMAIN),
    (FX::Location, "Location", MMAIN),
    (FX::GoToLine, "Go To Line", MMAIN),
    (FX::Undo, "Undo", MMAIN),
    (FX::Redo, "Redo", MMAIN),
    (FX::Mark, "Set Mark", MMAIN),
    (FX::Copy, "Copy", MMAIN),
    (FX::CaseSens, "Case Sens", MWHEREIS | MREPLACE),
    (FX::Regexp, "Reg.exp.", MWHEREIS | MREPLACE),
    (FX::Backwards, "Backwards", MWHEREIS | MREPLACE),
    (FX::FlipReplace, "Replace", MWHEREIS),
    (FX::FlipReplace, "No Replace", MREPLACE),
    (
        FX::OlderItem,
        "Older",
        MWHEREIS | MREPLACE | MREPLACEWITH | MWHEREISFILE,
    ),
    (
        FX::NewerItem,
        "Newer",
        MWHEREIS | MREPLACE | MREPLACEWITH | MWHEREISFILE,
    ),
    (FX::OlderItem, "Older", MEXECUTE),
    (FX::NewerItem, "Newer", MEXECUTE),
    (FX::GotoDir, "Go To Dir", MBROWSER),
    (FX::Refresh, "Refresh", MBROWSER),
    (FX::BrowserSearchForward, "Where Is", MBROWSER),
    (FX::BrowserSearchBackward, "Where Was", MBROWSER),
    (FX::BrowserFindPrev, "Previous", MBROWSER),
    (FX::BrowserFindNext, "Next", MBROWSER),
    (FX::FindBracket, "To Bracket", MMAIN),
    (FX::SearchBackward, "Where Was", MMAIN | MHELP),
    (FX::FindPrev, "Previous", MMAIN | MHELP),
    (FX::FindNext, "Next", MMAIN | MHELP),
    (FX::Left, "Back", MMAIN),
    (FX::Right, "Forward", MMAIN),
    (FX::Left, "Back", MBROWSER),
    (FX::Right, "Forward", MBROWSER),
    (FX::PrevWord, "Prev Word", MMAIN),
    (FX::NextWord, "Next Word", MMAIN),
    (FX::Home, "Home", MMAIN),
    (FX::End, "End", MMAIN),
    (FX::Up, "Prev Line", MMAIN | MBROWSER | MHELP),
    (FX::Down, "Next Line", MMAIN | MBROWSER | MHELP),
    (FX::ScrollUp, "Scroll Up", MMAIN),
    (FX::ScrollDown, "Scroll Down", MMAIN),
    (FX::PrevBlock, "Prev Block", MMAIN),
    (FX::NextBlock, "Next Block", MMAIN),
    (FX::ParaBegin, "Begin of Paragr.", MMAIN | MGOTOLINE),
    (FX::ParaEnd, "End of Paragraph", MMAIN | MGOTOLINE),
    (FX::TopRow, "Top Row", MMAIN),
    (FX::BottomRow, "Bottom Row", MMAIN),
    (FX::PageUp, "Prev Page", MMAIN | MHELP),
    (FX::PageDown, "Next Page", MMAIN | MHELP),
    (FX::FirstLine, "First Line", MMAIN | MHELP | MGOTOLINE),
    (FX::LastLine, "Last Line", MMAIN | MHELP | MGOTOLINE),
    (FX::PrevFile, "Prev File", MMAIN),
    (FX::NextFile, "Next File", MMAIN),
    (FX::Tab, "Tab", MMAIN),
    (FX::Enter, "Enter", MMAIN),
    (FX::Backspace, "Backspace", MMAIN),
    (FX::Delete, "Delete", MMAIN),
    (FX::ChopWordLeft, "Chop Left", MMAIN),
    (FX::ChopWordRight, "Chop Right", MMAIN),
    (FX::CutTillEof, "Cut Till End", MMAIN),
    (FX::WordCount, "Word Count", MMAIN),
    (FX::Verbatim, "Verbatim", MMAIN),
    (FX::Indent, "Indent", MMAIN),
    (FX::Unindent, "Unindent", MMAIN),
    (FX::Comment, "Comment Lines", MMAIN),
    (FX::Zap, "Zap", MMAIN),
    (FX::SpellCheck, "Spell Check", MMAIN),
    (FX::Suspend, "Suspend", MMAIN),
    (FX::Refresh, "Refresh", MMAIN),
    (FX::Cycle, "Cycle", MMAIN),
    (FX::SaveFile, "Save", MMAIN),
    (FX::NewBufferToggle, "New Buffer", MINSERTFILE | MEXECUTE),
    (FX::PipeToggle, "Pipe Text", MEXECUTE),
    (FX::SpellCheck, "Spell Check", MEXECUTE),
    (FX::FullJustify, "Full Justify", MEXECUTE),
    (FX::FlipGoto, "Go To Line", MWHEREIS),
    (FX::FlipGoto, "Go To Text", MGOTOLINE),
    (FX::DosFormat, "DOS Format", MWRITEFILE),
    (FX::MacFormat, "Mac Format", MWRITEFILE),
    (FX::Append, "Append", MWRITEFILE),
    (FX::Prepend, "Prepend", MWRITEFILE),
    (FX::Backup, "Backup File", MWRITEFILE),
    (FX::NoConvert, "No Conversion", MINSERTFILE),
    (FX::FlipExecute, "Execute Command", MINSERTFILE),
    (FX::CutTillEof, "Cut Till End", MEXECUTE),
    (FX::Suspend, "Suspend", MEXECUTE),
    (FX::ToFiles, "Browse", MWRITEFILE | MINSERTFILE),
    (FX::PageUp, "Prev Page", MBROWSER),
    (FX::PageDown, "Next Page", MBROWSER),
    (FX::BrowserFirst, "First File", MBROWSER | MWHEREISFILE),
    (FX::BrowserLast, "Last File", MBROWSER | MWHEREISFILE),
    (FX::PrevWord, "Left Column", MBROWSER),
    (FX::NextWord, "Right Column", MBROWSER),
    (FX::PrevBlock, "Top Row", MBROWSER),
    (FX::NextBlock, "Bottom Row", MBROWSER),
    (FX::DiscardBuffer, "Discard buffer", MWRITEFILE),
];

// }} -------------------------------------------------------- shortcut keys ---

// Default shortcuts and their active menu masks. The first applicable entry
// determines how an otherwise unconfigured key is handled.
static SCS: &[(&str, u32, FX)] = &[
    ("M-:", MEXECUTE, FX::RecordMacro),
    ("M-:", MMAIN, FX::RunMacro),
    ("^]", MMAIN, FX::Complete),
    ("^F", MEXECUTE, FX::Formatter),
    ("^L", MEXECUTE, FX::Linter),
    ("^T", MEXECUTE, FX::SpellCheck),
    ("^M", MSOME, FX::Enter),
    ("Enter", MSOME, FX::Enter),
    ("^I", MSOME, FX::Tab),
    ("Tab", MSOME, FX::Tab),
    ("Bsp", MBROWSER | MHELP, FX::PageUp),
    ("Space", MBROWSER | MHELP, FX::PageDown),
    ("^B", MMAIN | MBROWSER | MHELP, FX::SearchBackward),
    ("^F", MMAIN | MBROWSER | MHELP, FX::SearchForward),
    ("^G", MSOME, FX::Help),
    ("^X", MMAIN | MBROWSER | MHELP, FX::Exit),
    ("^S", MMAIN, FX::SaveFile),
    ("^O", MMAIN, FX::WriteOut),
    ("^R", MMAIN, FX::ReadFile),
    ("^Q", MMAIN | MBROWSER | MHELP, FX::SearchBackward),
    ("^W", MMAIN | MBROWSER | MHELP, FX::SearchForward),
    ("^A", MSOME, FX::Home),
    ("^E", MSOME, FX::End),
    ("^P", MMAIN | MBROWSER | MHELP, FX::Up),
    ("^N", MMAIN | MBROWSER | MHELP, FX::Down),
    ("^Y", MMAIN | MHELP, FX::PageUp),
    ("^V", MMAIN | MHELP, FX::PageDown),
    ("^C", MMAIN, FX::Location),
    ("^H", MSOME, FX::Backspace),
    ("^D", MSOME, FX::Delete),
    ("Bsp", MSOME, FX::Backspace),
    ("Del", MSOME, FX::Delete),
    ("Ins", MMAIN, FX::ReadFile),
    ("^\\", MMAIN, FX::Replace),
    ("M-R", MMAIN, FX::Replace),
    ("^K", MSOME, FX::Cut),
    ("M-6", MSOME, FX::Copy),
    ("M-^", MSOME, FX::Copy),
    ("^U", MSOME, FX::Paste),
    ("^T", MMAIN, FX::Execute),
    ("^J", MMAIN, FX::Justify),
    ("^/", MMAIN, FX::GoToLine),
    ("M-G", MMAIN, FX::GoToLine),
    ("^_", MMAIN, FX::GoToLine),
    ("PgUp", MMAIN | MHELP | MBROWSER, FX::PageUp),
    ("PgDn", MMAIN | MHELP | MBROWSER, FX::PageDown),
    ("M-\\", MMAIN | MHELP, FX::FirstLine),
    ("^Home", MMAIN | MHELP, FX::FirstLine),
    ("M-/", MMAIN | MHELP, FX::LastLine),
    ("^End", MMAIN | MHELP, FX::LastLine),
    ("M-B", MMAIN | MBROWSER | MHELP, FX::FindPrev),
    ("M-F", MMAIN | MBROWSER | MHELP, FX::FindNext),
    ("M-W", MMAIN | MBROWSER | MHELP, FX::FindNext),
    ("M-Q", MMAIN | MBROWSER | MHELP, FX::FindPrev),
    ("M-]", MMAIN, FX::FindBracket),
    ("M-A", MMAIN, FX::Mark),
    ("^6", MMAIN, FX::Mark),
    ("^^", MMAIN, FX::Mark),
    ("M-}", MMAIN, FX::Indent),
    ("M-{", MMAIN, FX::Unindent),
    ("Sh-Tab", MMAIN, FX::Unindent),
    ("M-U", MMAIN, FX::Undo),
    ("M-E", MMAIN, FX::Redo),
    ("M-Bsp", MMAIN, FX::ChopWordLeft),
    ("^Del", MMAIN, FX::ChopWordRight),
    ("M-Del", MMAIN, FX::Zap),
    ("M-3", MMAIN, FX::Comment),
    ("M-Home", MMAIN, FX::TopRow),
    ("M-End", MMAIN, FX::BottomRow),
    ("M-7", MMAIN, FX::PrevBlock),
    ("M-8", MMAIN, FX::NextBlock),
    ("M-(", MMAIN, FX::ParaBegin),
    ("M-9", MMAIN, FX::ParaBegin),
    ("M-)", MMAIN, FX::ParaEnd),
    ("M-0", MMAIN, FX::ParaEnd),
    ("M-Up", MMAIN | MHELP, FX::ScrollUp),
    ("M-Down", MMAIN | MHELP, FX::ScrollDown),
    ("M--", MMAIN | MHELP, FX::ScrollUp),
    ("M-_", MMAIN | MHELP, FX::ScrollUp),
    ("M-+", MMAIN | MHELP, FX::ScrollDown),
    ("M-=", MMAIN | MHELP, FX::ScrollDown),
    ("M-,", MMAIN, FX::PrevFile),
    ("M-<", MMAIN, FX::PrevFile),
    ("M-.", MMAIN, FX::NextFile),
    ("M->", MMAIN, FX::NextFile),
    ("M-V", MSOME, FX::Verbatim),
    ("M-T", MMAIN, FX::CutTillEof),
    ("^V", MEXECUTE, FX::CutTillEof),
    ("^Z", MEXECUTE, FX::Suspend),
    ("^Z", MMAIN, FX::SuggestSuspend),
    ("M-D", MMAIN, FX::WordCount),
    ("M-J", MMAIN, FX::FullJustify),
    ("^J", MEXECUTE, FX::FullJustify),
    ("^L", MMAIN, FX::Cycle),
    ("^L", MMOST | MBROWSER | MHELP | MYESNO, FX::Refresh),
    ("M-Z", MMAIN, FX::ToggleZero),
    (
        "M-X",
        (MMOST | MBROWSER | MYESNO) & !MFINDINHELP,
        FX::ToggleNoHelp,
    ),
    ("M-C", MMAIN, FX::ToggleConstant),
    ("M-S", MMAIN, FX::ToggleSoftwrap),
    ("M-$", MMAIN, FX::ToggleSoftwrap),
    ("M-N", MMAIN, FX::ToggleLinenumbers),
    ("M-#", MMAIN, FX::ToggleLinenumbers),
    ("M-P", MMAIN, FX::ToggleWhitespace),
    ("M-Y", MMAIN, FX::ToggleNoSyntax),
    ("M-H", MMAIN, FX::ToggleSmartHome),
    ("M-I", MMAIN, FX::ToggleAutoindent),
    ("M-K", MMAIN, FX::ToggleCutFromCursor),
    ("M-L", MMAIN, FX::ToggleBreakLong),
    ("M-O", MMAIN, FX::ToggleTabsToSpaces),
    ("M-M", MMAIN, FX::ToggleMouse),
    ("^C", (MMOST | MYESNO) & !MMAIN, FX::Cancel),
    ("M-C", MWHEREIS | MREPLACE, FX::CaseSens),
    ("M-R", MWHEREIS | MREPLACE, FX::Regexp),
    ("M-B", MWHEREIS | MREPLACE, FX::Backwards),
    ("^R", MWHEREIS | MREPLACE, FX::FlipReplace),
    ("^T", MWHEREIS | MGOTOLINE, FX::FlipGoto),
    (
        "^P",
        MWHEREIS | MREPLACE | MREPLACEWITH | MWHEREISFILE | MFINDINHELP | MEXECUTE,
        FX::OlderItem,
    ),
    (
        "^N",
        MWHEREIS | MREPLACE | MREPLACEWITH | MWHEREISFILE | MFINDINHELP | MEXECUTE,
        FX::NewerItem,
    ),
    (
        "Up",
        MWHEREIS | MREPLACE | MREPLACEWITH | MWHEREISFILE | MFINDINHELP | MEXECUTE,
        FX::OlderItem,
    ),
    (
        "Down",
        MWHEREIS | MREPLACE | MREPLACEWITH | MWHEREISFILE | MFINDINHELP | MEXECUTE,
        FX::NewerItem,
    ),
    ("^W", MGOTOLINE, FX::ParaBegin),
    ("^O", MGOTOLINE, FX::ParaEnd),
    ("^Y", MGOTOLINE | MWHEREIS | MFINDINHELP, FX::FirstLine),
    ("^V", MGOTOLINE | MWHEREIS | MFINDINHELP, FX::LastLine),
    ("^Y", MWHEREISFILE, FX::BrowserFirst),
    ("^V", MWHEREISFILE, FX::BrowserLast),
    ("M-\\", MBROWSER | MWHEREISFILE, FX::BrowserFirst),
    ("M-/", MBROWSER | MWHEREISFILE, FX::BrowserLast),
    ("Home", MBROWSER, FX::BrowserFirst),
    ("End", MBROWSER, FX::BrowserLast),
    ("^Home", MBROWSER, FX::BrowserFirst),
    ("^End", MBROWSER, FX::BrowserLast),
    ("^/", MBROWSER, FX::GotoDir),
    ("M-G", MBROWSER, FX::GotoDir),
    ("^_", MBROWSER, FX::GotoDir),
    ("^Q", MWRITEFILE, FX::DiscardBuffer),
    ("M-D", MWRITEFILE, FX::DosFormat),
    ("M-M", MWRITEFILE, FX::MacFormat),
    ("M-A", MWRITEFILE, FX::Append),
    ("M-P", MWRITEFILE, FX::Prepend),
    ("M-B", MWRITEFILE, FX::Backup),
    ("^X", MINSERTFILE | MEXECUTE, FX::FlipExecute),
    ("M-N", MINSERTFILE, FX::NoConvert),
    ("M-F", MINSERTFILE | MEXECUTE, FX::NewBufferToggle),
    ("M-\\", MEXECUTE, FX::PipeToggle),
    ("^C", MBROWSER | MHELP, FX::Exit),
    ("^T", MWRITEFILE | MINSERTFILE, FX::ToFiles),
    ("^T", MBROWSER, FX::Exit),
    ("^G", MHELP, FX::Exit),
    ("F1", MHELP, FX::Exit),
    ("F1", MSOME, FX::Help),
    ("F2", MMAIN | MBROWSER | MHELP, FX::Exit),
    ("F3", MMAIN, FX::WriteOut),
    ("F4", MMAIN, FX::Justify),
    ("F5", MMAIN, FX::ReadFile),
    ("F6", MMAIN | MHELP | MBROWSER, FX::SearchForward),
    ("F7", MMAIN | MHELP | MBROWSER, FX::PageUp),
    ("F8", MMAIN | MHELP | MBROWSER, FX::PageDown),
    ("F9", MSOME, FX::Cut),
    ("F10", MSOME, FX::Paste),
    ("F11", MMAIN, FX::Location),
    ("Home", MSOME, FX::Home),
    ("End", MSOME, FX::End),
    ("Left", MSOME, FX::Left),
    ("Right", MSOME, FX::Right),
    ("Up", MMAIN | MBROWSER | MHELP, FX::Up),
    ("Down", MMAIN | MBROWSER | MHELP, FX::Down),
    ("M-Space", MSOME, FX::PrevWord),
    ("^Space", MSOME, FX::NextWord),
    ("^Left", MSOME, FX::PrevWord),
    ("^Right", MSOME, FX::NextWord),
    ("^Up", MMAIN | MBROWSER, FX::PrevBlock),
    ("^Down", MMAIN | MBROWSER, FX::NextBlock),
];

/// Parse the standard key notation into a Key.
pub fn key_from_string(s: &str) -> Option<Key> {
    if let Some(rest) = s.strip_prefix("Sh-") {
        if let Some(Key::Char(c, ctrl, alt)) = key_from_string(rest) {
            return Some(Key::ShiftChar(c.to_ascii_uppercase(), ctrl, alt));
        }
    }

    if let Some(stripped) = s.strip_prefix('^') {
        if stripped.len() == 1 {
            let c = stripped.chars().next().unwrap();
            if c == '/' {
                return Some(Key::Char('/', true, false));
            }
            if c == '\\' {
                return Some(Key::Char('\\', true, false));
            }
            if c == '_' {
                return Some(Key::Char('_', true, false));
            }
            if c == '6' {
                return Some(Key::Char('6', true, false));
            }
            if c == '^' {
                return Some(Key::Char('^', true, false));
            }
            if c == 'M' {
                return Some(Key::Enter);
            }
            if c == 'I' {
                return Some(Key::Tab);
            }
            if c == 'H' {
                return Some(Key::Backspace(false));
            }
            return Some(Key::Char(c.to_ascii_lowercase(), true, false));
        }
        match stripped {
            "Space" => return Some(Key::Char(' ', true, false)),
            "Left" => return Some(Key::Left(true, false)),
            "Right" => return Some(Key::Right(true, false)),
            "Up" => return Some(Key::Up(true, false)),
            "Down" => return Some(Key::Down(true, false)),
            _ => {}
        }
        if stripped == "Home" {
            return Some(Key::Home(true, false));
        }
        if stripped == "End" {
            return Some(Key::End(true, false));
        }
        if stripped == "Del" {
            return Some(Key::Del(true, false));
        }
        return None;
    }
    if let Some(rest) = s.strip_prefix("M-") {
        if rest == "Space" {
            return Some(Key::Char(' ', false, true));
        }
        if rest == "Bsp" {
            return Some(Key::Backspace(true));
        }
        if rest == "Home" {
            return Some(Key::Home(false, true));
        }
        if rest == "End" {
            return Some(Key::End(false, true));
        }
        if rest == "Del" {
            return Some(Key::Del(false, true));
        }
        if rest == "Up" {
            return Some(Key::Up(false, true));
        }
        if rest == "Down" {
            return Some(Key::Down(false, true));
        }
        let mut chars = rest.chars();
        let c = chars.next()?;
        if chars.next().is_some() {
            return None;
        }
        return Some(Key::Char(c, false, true));
    }
    match s {
        "Up" => Some(Key::Up(false, false)),
        "Down" => Some(Key::Down(false, false)),
        "Left" => Some(Key::Left(false, false)),
        "Right" => Some(Key::Right(false, false)),
        "Home" => Some(Key::Home(false, false)),
        "End" => Some(Key::End(false, false)),
        "Ins" => Some(Key::Ins),
        "Del" => Some(Key::Del(false, false)),
        "PgUp" => Some(Key::PgUp),
        "PgDn" => Some(Key::PgDn),
        "Bsp" => Some(Key::Backspace(false)),
        "Enter" => Some(Key::Enter),
        "Tab" => Some(Key::Tab),
        "Sh-Tab" => Some(Key::ShTab),
        "Space" => Some(Key::Space),
        _ => {
            if let Some(num) = s.strip_prefix('F') {
                return num.parse::<u8>().ok().map(Key::F);
            }
            None
        }
    }
}

/// Look up which function the given key triggers in the given menu.
pub fn func_from_key(menu: u32, key: Key) -> Option<F> {
    for (keystr, menus, f) in SCS {
        if (menus & menu) == 0 {
            continue;
        }
        if matches!(key_from_string(keystr), Some(k) if k == key) {
            return Some(*f);
        }
    }
    None
}

/// Find the preferred default key for a function in the requested menus.
pub fn first_key_for(menu: u32, f: FX) -> Option<Key> {
    for (keystr, menus, func) in SCS {
        if (menus & menu) != 0 && *func == f {
            if let Some(k) = key_from_string(keystr) {
                return Some(k);
            }
        }
    }
    None
}

/// Format a shortcut for the compact help rows.
pub fn describe(key: Key) -> String {
    match key {
        Key::ShiftChar(c, ctrl, alt) => format!(
            "Sh-{}{}{}",
            if alt { "M-" } else { "" },
            if ctrl { "^" } else { "" },
            c
        ),
        Key::Char('\r', true, _) => String::from("^M"),
        Key::Char('\t', true, _) => String::from("^I"),
        Key::Char('/', true, _) => String::from("^/"),
        Key::Char('\\', true, _) => String::from("^\\"),
        Key::Char('_', true, _) => String::from("^_"),
        Key::Char(c, true, false) => format!("^{}", c.to_ascii_uppercase()),
        Key::Char(c, false, true) => format!("M-{}", c),
        Key::Char(c, true, true) => format!("M-^{}", c),
        Key::Backspace(false) => String::from("Bsp"),
        Key::Backspace(true) => String::from("M-Bsp"),
        Key::Del(false, false) => String::from("Del"),
        Key::Del(true, false) => String::from("^Del"),
        Key::Del(false, true) => String::from("M-Del"),
        Key::Home(false, false) => String::from("Home"),
        Key::Home(true, false) => String::from("^Home"),
        Key::Home(false, true) => String::from("M-Home"),
        Key::End(false, false) => String::from("End"),
        Key::End(true, false) => String::from("^End"),
        Key::End(false, true) => String::from("M-End"),
        Key::Up(false, false) => String::from("Up"),
        Key::Down(false, false) => String::from("Down"),
        Key::Left(false, false) => String::from("Left"),
        Key::Right(false, false) => String::from("Right"),
        Key::Up(_, true) | Key::Up(true, _) => String::from("M-Up"),
        Key::Down(_, true) | Key::Down(true, _) => String::from("M-Down"),
        Key::Left(_, true) | Key::Left(true, _) => String::from("M-Left"),
        Key::Right(_, true) | Key::Right(true, _) => String::from("M-Right"),
        Key::Ins => String::from("Ins"),
        Key::PgUp => String::from("PgUp"),
        Key::PgDn => String::from("PgDn"),
        Key::Enter | Key::Char('\r', _, _) => String::from("Enter"),
        Key::Tab => String::from("Tab"),
        Key::ShTab => String::from("Sh-Tab"),
        Key::Space => String::from("Space"),
        Key::F(n) => format!("F{}", n),
        _ => String::new(),
    }
}

/// A pairing of a display key with its function tag.
pub type Hint = (String, &'static str);

pub fn shortcut_keys(menu: u32, f: F) -> Vec<Key> {
    SCS.iter()
        .filter(|(_, m, func)| m & menu != 0 && *func == f)
        .filter_map(|(k, _, _)| key_from_string(k))
        .collect()
}
