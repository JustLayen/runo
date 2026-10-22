use crate::{
    defs::*,
    editor::{Aim, Editor, PromptState},
    keys::{self, Key, F},
    terminal,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers as M};
use std::time::Duration;

pub enum Answer {
    Text(String),
    Action(F),
    Cancel,
}
impl Editor {
    pub fn binding(&self, menu: u32, key: Key) -> Option<F> {
        // Normalize letter case for unshifted Meta shortcuts.
        let key = match key {
            Key::Char(c, ctrl, true) => Key::Char(c.to_ascii_uppercase(), ctrl, true),
            k => k,
        };
        for &(menus, k, f) in self.cfg.bindings.iter().rev() {
            let k = match k {
                Key::Char(c, ctrl, true) => Key::Char(c.to_ascii_uppercase(), ctrl, true),
                k => k,
            };
            if menus & menu != 0 && k == key {
                return f;
            }
        }
        if let Key::ShiftChar(c, ctrl, alt) = key {
            return self.binding(menu, Key::Char(c, ctrl, alt));
        }
        keys::func_from_key(menu, key)
    }

    pub fn input(&mut self) -> Event {
        if let Some(e) = self.replay.pop_front() {
            return e;
        }
        if self.playing_macro {
            return Event::Key(KeyEvent::new(KeyCode::Char('c'), M::CONTROL));
        }
        let event = self.live_input();
        if self.recording {
            self.macro_keys.push(event.clone());
        }
        event
    }
    fn live_input(&mut self) -> Event {
        loop {
            if crate::signals::terminating() {
                self.emergency_save();
                self.quit = true;
                return Event::Key(KeyEvent::new(KeyCode::Char('c'), M::CONTROL));
            }
            let Some(event) = self.term.poll_event(Duration::from_millis(100)) else {
                continue;
            };
            if let Event::Key(k) = event {
                if k.kind == crossterm::event::KeyEventKind::Release {
                    continue;
                }
                if k.code == KeyCode::Esc {
                    if let Some(next) = self.term.poll_event(Duration::from_millis(200)) {
                        if let Event::Key(mut next) = next {
                            if next.code == KeyCode::Esc {
                                if let Event::Key(mut third) = self.term.next_event() {
                                    if let KeyCode::Char(c) = third.code {
                                        if c.is_ascii_digit() {
                                            let mut digits = String::from(c);
                                            for _ in 0..2 {
                                                if let Event::Key(KeyEvent {
                                                    code: KeyCode::Char(d),
                                                    ..
                                                }) = self.term.next_event()
                                                {
                                                    digits.push(d);
                                                }
                                            }
                                            if let Ok(n) = digits.parse::<u8>() {
                                                return Event::Paste(char::from(n).to_string());
                                            } else {
                                                self.beep();
                                                continue;
                                            }
                                        }
                                        third.code = KeyCode::Char(c.to_ascii_lowercase());
                                        third.modifiers = M::CONTROL;
                                        return Event::Key(third);
                                    }
                                }
                                continue;
                            }
                            next.modifiers |= M::ALT;
                            return Event::Key(next);
                        } else {
                            return next;
                        }
                    }
                    continue;
                }
            }
            return event;
        }
    }
    pub fn run(&mut self) {
        while !self.quit {
            self.ensure_viewport(Aim::Minimal);
            self.draw();
            let event = self.input();
            if self.quit {
                break;
            }
            self.handle_event(event);
        }
        self.save_session();
    }
    pub fn handle_event(&mut self, event: Event) {
        match event {
            Event::Resize(_, _) => {
                self.term.measure();
                self.ensure_viewport(Aim::Minimal);
            }
            Event::Paste(s) => {
                self.completion = None;
                if self.editable() {
                    self.insert_string(&s);
                }
            }
            Event::Mouse(m) => self.mouse_event(m),
            Event::Key(k) => {
                if self.strokes_left > 0 {
                    self.strokes_left -= 1;
                    if self.strokes_left == 0 {
                        self.wipe_status();
                    }
                }
                self.spotlight = None;
                if self.cfg.preserve
                    && k.modifiers.contains(M::CONTROL)
                    && matches!(k.code, KeyCode::Char('q' | 's'))
                {
                    return;
                }
                if k.modifiers.contains(M::ALT) {
                    match k.code {
                        KeyCode::Insert => {
                            let y = self.borig().cursor.y;
                            self.b().lines[y].has_anchor ^= true;
                            return;
                        }
                        KeyCode::PageUp => {
                            self.anchor(false);
                            return;
                        }
                        KeyCode::PageDown => {
                            self.anchor(true);
                            return;
                        }
                        KeyCode::Char(':') => {
                            self.play_macro();
                            return;
                        }
                        _ => {}
                    }
                }
                if k.code == KeyCode::Char(']') && k.modifiers.contains(M::CONTROL) {
                    self.complete_word();
                    return;
                }
                let Some(mut key) = terminal::convert_key(&k) else {
                    return;
                };
                if self.cfg.rebinddelete {
                    key = match key {
                        Key::Backspace(false) => Key::Del(false, false),
                        Key::Del(false, false) => Key::Backspace(false),
                        x => x,
                    };
                }
                let f = self.binding(self.active_menu(), key);
                if f != Some(F::Complete) {
                    self.completion = None;
                }
                let motion = matches!(
                    f,
                    Some(
                        F::Left
                            | F::Right
                            | F::Up
                            | F::Down
                            | F::Home
                            | F::End
                            | F::PrevWord
                            | F::NextWord
                            | F::PageUp
                            | F::PageDown
                            | F::FirstLine
                            | F::LastLine
                    )
                );
                if motion {
                    if k.modifiers.contains(M::SHIFT) {
                        if self.borig().mark.is_none() {
                            let pos = self.borig().cursor;
                            self.b().mark = Some(pos);
                            self.softmark = Some(pos);
                        }
                    } else if self.softmark.take().is_some() {
                        self.b().mark = None;
                    }
                }
                if let Some(f) = f {
                    self.dispatch(f);
                } else {
                    match key {
                        Key::Char(c, false, false) if self.editable() => {
                            self.inject_text(&[c]);
                            self.hard_wrap();
                        }
                        Key::Space if self.editable() => {
                            self.inject_text(&[' ']);
                            self.hard_wrap();
                        }
                        _ => self.beep(),
                    }
                }
            }
            _ => {}
        }
    }
    pub fn editable(&mut self) -> bool {
        if self.cfg.view_mode || self.help_view {
            self.status(Importance::Ahem, "Key is invalid in view mode");
            false
        } else {
            true
        }
    }
    pub fn dispatch(&mut self, f: F) {
        use F::*;
        let changes = matches!(
            f,
            Undo | Redo
                | Cut
                | Paste
                | Enter
                | Tab
                | Backspace
                | Delete
                | ChopWordLeft
                | ChopWordRight
                | CutTillEof
                | Verbatim
                | Indent
                | Unindent
                | Comment
                | Zap
                | Justify
                | FullJustify
                | Replace
                | ReadFile
                | Execute
                | SpellCheck
        );
        if changes && !self.editable() {
            return;
        }
        if !matches!(f, Cut | Copy | CutTillEof) {
            self.keep_cutbuffer = false;
        }
        if !matches!(
            f,
            Tab | Enter | Backspace | Delete | Cut | Copy | CutTillEof | Paste | Undo | Redo
        ) {
            self.last_action = UndoType::Other;
        }
        match f {
            Expansion(i) => self.expand_binding(i),
            RecordMacro => self.toggle_recording(),
            RunMacro => self.play_macro(),
            Complete => self.complete_word(),
            Anchor => {
                let y = self.borig().cursor.y;
                self.b().lines[y].has_anchor ^= true;
            }
            PrevAnchor => self.anchor(false),
            NextAnchor => self.anchor(true),
            Formatter => self.external_tool(false),
            Linter => self.external_tool(true),
            Left => self.move_left(),
            Right => self.move_right(),
            Up => self.vertical(false),
            Down => self.vertical(true),
            Home => self.move_home(),
            End => self.move_end(),
            FirstLine => self.first_line(),
            LastLine => self.last_line(),
            PrevWord => self.move_prev_word(),
            NextWord => self.move_next_word(),
            PrevBlock => self.move_prev_block(),
            NextBlock => self.move_next_block(),
            ParaBegin => self.move_para_begin(),
            ParaEnd => self.move_para_end(),
            TopRow => self.top_row(),
            BottomRow => self.bottom_row(),
            PageUp => self.page_up(),
            PageDown => self.page_down(),
            ScrollUp => self.scroll_up(),
            ScrollDown => self.scroll_down(),
            Refresh => self.force_redraw = true,
            Cycle => self.cycle(),
            Tab => {
                if let Some(tab) = self.active_syntax().and_then(|s| s.tab.clone()) {
                    self.inject_text(&tab.chars().collect::<Vec<_>>());
                } else {
                    self.insert_tab();
                }
            }
            Enter => self.enter_key(),
            Backspace => self.backspace(),
            Delete => self.delete_forward(),
            Undo => self.do_undo(),
            Redo => self.do_redo(),
            Cut => self.cut_text(),
            Copy => self.copy_text(),
            Paste => self.paste_text(),
            ChopWordLeft => self.chop_previous_word(),
            ChopWordRight => self.chop_next_word(),
            CutTillEof => self.cut_till_eof(),
            Indent => self.do_indent(),
            Unindent => self.do_unindent(),
            Comment => self.comment_lines(),
            Zap => self.zap_text(),
            Verbatim => self.verbatim_input(),
            FindBracket => self.match_bracket(),
            WordCount => self.word_count(),
            Mark => {
                if self.borig().mark.is_some() {
                    self.b().mark = None;
                    self.statusbar("Mark Unset");
                } else {
                    let p = self.borig().cursor;
                    self.b().mark = Some(p);
                    self.statusbar("Mark Set");
                }
                self.softmark = None;
            }
            Location => {
                let msg = self.position_message();
                self.statusbar(&msg);
            }
            SearchForward => self.search_prompt(false, false),
            SearchBackward => self.search_prompt(true, false),
            Replace => self.search_prompt(self.search_back, true),
            FindNext => self.repeat_search(false),
            FindPrev => self.repeat_search(true),
            GoToLine => self.goto_prompt(),
            SaveFile => {
                if self.borig().name.is_some() {
                    self.save_current();
                } else {
                    self.write_prompt(false);
                }
            }
            WriteOut => {
                self.write_prompt(false);
            }
            ReadFile => self.read_prompt(),
            Execute => self.execute_prompt(),
            Exit => self.close_buffer(),
            Help => self.show_help(),
            PrevFile => self.switch_buffer(false),
            NextFile => self.switch_buffer(true),
            Justify => self.justify(false),
            FullJustify => self.justify(true),
            SpellCheck => self.spell_check(),
            Suspend => self.suspend(),
            SuggestSuspend => self.statusbar("To suspend, type ^T^Z"),
            ToggleMouse => {
                self.cfg.mouse ^= true;
                self.term.set_mouse(self.cfg.mouse);
                self.toggle_message("Mouse support", self.cfg.mouse);
            }
            ToggleZero => {
                self.cfg.zero ^= true;
            }
            ToggleNoHelp => {
                self.cfg.nohelp ^= true;
            }
            ToggleConstant => {
                self.cfg.constantshow ^= true;
                self.toggle_message("Constant cursor position display", self.cfg.constantshow);
            }
            ToggleSoftwrap => {
                self.cfg.softwrap ^= true;
                self.b().scroll.top_col = 0;
                self.toggle_message("Soft wrapping", self.cfg.softwrap);
            }
            ToggleLinenumbers => {
                self.cfg.linenumbers ^= true;
                self.toggle_message("Line numbering", self.cfg.linenumbers);
            }
            ToggleWhitespace => {
                self.cfg.whitespace ^= true;
                self.toggle_message("Whitespace display", self.cfg.whitespace);
            }
            ToggleNoSyntax => {
                self.cfg.no_syntax ^= true;
                self.toggle_message("Syntax highlighting", !self.cfg.no_syntax);
            }
            ToggleSmartHome => {
                self.cfg.smarthome ^= true;
                self.toggle_message("Smart home key", self.cfg.smarthome);
            }
            ToggleAutoindent => {
                self.cfg.autoindent ^= true;
                self.toggle_message("Auto indent", self.cfg.autoindent);
            }
            ToggleCutFromCursor => {
                self.cfg.cut_from_cursor ^= true;
                self.toggle_message("Cut from cursor to end of line", self.cfg.cut_from_cursor);
            }
            ToggleBreakLong => {
                self.cfg.break_long_lines ^= true;
                self.toggle_message("Hard wrapping of overlong lines", self.cfg.break_long_lines);
            }
            ToggleTabsToSpaces => {
                self.cfg.tabs_to_spaces ^= true;
                self.toggle_message(
                    "Conversion of typed tabs to spaces",
                    self.cfg.tabs_to_spaces,
                );
            }
            // These functions are consumed by the corresponding dialog menus.
            Cancel
            | FlipReplace
            | FlipGoto
            | CaseSens
            | Regexp
            | Backwards
            | OlderItem
            | NewerItem
            | GotoDir
            | BrowserSearchForward
            | BrowserSearchBackward
            | BrowserFindPrev
            | BrowserFindNext
            | BrowserFirst
            | BrowserLast
            | NewBufferToggle
            | PipeToggle
            | DosFormat
            | MacFormat
            | Append
            | Prepend
            | Backup
            | DiscardBuffer
            | ToFiles
            | NoConvert
            | FlipExecute => self.beep(),
        }
    }
    fn toggle_message(&mut self, label: &str, on: bool) {
        self.statusbar(&format!(
            "{label} {}",
            if on { "enabled" } else { "disabled" }
        ));
    }
    fn vertical(&mut self, down: bool) {
        let goal = self.borig().goal;
        if self.cfg.softwrap {
            let chunks = crate::display::compute_chunks(
                self.borig().line(self.borig().cursor.y),
                self.editwincols(),
                self.cfg.tabsize,
                self.cfg.atblanks,
            );
            let ci = self.cursor_chunk(self.borig());
            let relative = goal.saturating_sub(chunks[ci].left);
            let (y, i) = if down {
                crate::editor::forward_chunks(
                    self.borig(),
                    self.borig().cursor.y,
                    ci,
                    1,
                    self.editwincols(),
                    self.cfg.tabsize,
                    self.cfg.atblanks,
                )
            } else {
                crate::editor::back_chunks(
                    self.borig(),
                    self.borig().cursor.y,
                    ci,
                    1,
                    self.editwincols(),
                    self.cfg.tabsize,
                    self.cfg.atblanks,
                )
            };
            let dest = crate::display::compute_chunks(
                self.borig().line(y),
                self.editwincols(),
                self.cfg.tabsize,
                self.cfg.atblanks,
            );
            let c = &dest[i.min(dest.len() - 1)];
            let col =
                (c.left + relative).min(c.right.saturating_sub(usize::from(i + 1 < dest.len())));
            let x = crate::display::actual_x(self.borig().line(y), col, self.cfg.tabsize);
            self.goto_pos(y, x);
            self.b().goal = c.left + relative;
        } else {
            if down {
                self.move_down();
            } else {
                self.move_up();
            }
            self.b().goal = goal;
        }
    }
    pub fn prompt_input(&mut self, menu: u32, question: &str, initial: &str) -> Answer {
        self.prompt = Some(PromptState {
            menu,
            question: question.into(),
            answer: initial.into(),
            typing_x: initial.chars().count(),
            allow_editing: true,
        });
        let mut histpos = None;
        let original = initial.to_string();
        loop {
            self.draw();
            let event = self.input();
            if self.quit {
                self.prompt = None;
                return Answer::Cancel;
            }
            let k = match event {
                Event::Resize(_, _) => {
                    self.term.measure();
                    continue;
                }
                Event::Paste(s) => {
                    self.completion = None;
                    let p = self.prompt.as_mut().unwrap();
                    let mut a = p.answer.chars().collect::<Vec<_>>();
                    let s = s.replace(['\n', '\r'], "");
                    let n = s.chars().count();
                    a.splice(p.typing_x..p.typing_x, s.chars());
                    p.typing_x += n;
                    p.answer = a.into_iter().collect();
                    continue;
                }
                Event::Key(k) => k,
                _ => continue,
            };
            let Some(key) = terminal::convert_key(&k) else {
                continue;
            };
            let f = self.binding(menu, key);
            match f {
                Some(F::Cancel) => {
                    self.prompt = None;
                    return Answer::Cancel;
                }
                Some(F::Enter) => {
                    let answer = self.prompt.take().unwrap().answer;
                    return Answer::Text(answer);
                }
                Some(F::Help) => {
                    let saved = self.prompt.take();
                    self.show_help();
                    self.prompt = saved;
                    continue;
                }
                Some(F::ToggleNoHelp) => {
                    self.cfg.nohelp ^= true;
                    continue;
                }
                Some(F::Refresh) => {
                    self.term.measure();
                    continue;
                }
                Some(F::OlderItem | F::NewerItem) => {
                    let history = match menu {
                        MWHEREIS | MREPLACE | MFINDINHELP | MWHEREISFILE => &self.history.search,
                        MREPLACEWITH => &self.history.replace,
                        MEXECUTE => &self.history.execute,
                        _ => &self.history.file,
                    };
                    if history.is_empty() {
                        continue;
                    }
                    let at = histpos.unwrap_or(history.len());
                    let at = if f == Some(F::OlderItem) {
                        at.saturating_sub(1)
                    } else {
                        (at + 1).min(history.len())
                    };
                    histpos = Some(at);
                    let p = self.prompt.as_mut().unwrap();
                    p.answer = history.get(at).cloned().unwrap_or_else(|| original.clone());
                    p.typing_x = p.answer.chars().count();
                    continue;
                }
                Some(
                    F::Home
                    | F::End
                    | F::Left
                    | F::Right
                    | F::Backspace
                    | F::Delete
                    | F::Cut
                    | F::Paste
                    | F::Copy
                    | F::PrevWord
                    | F::NextWord
                    | F::Tab
                    | F::Verbatim,
                ) => {}
                Some(f) => {
                    self.browser_answer = self.prompt.take().map(|p| p.answer);
                    return Answer::Action(f);
                }
                None => {}
            }
            let p = self.prompt.as_mut().unwrap();
            let mut a = p.answer.chars().collect::<Vec<_>>();
            let x = p.typing_x;
            match (f, key) {
                (Some(F::Home), _) => p.typing_x = 0,
                (Some(F::End), _) => p.typing_x = a.len(),
                (Some(F::Left), _) => p.typing_x = x.saturating_sub(1),
                (Some(F::Right), _) => p.typing_x = (x + 1).min(a.len()),
                (Some(F::PrevWord), _) => {
                    let mut n = x;
                    while n > 0 && a[n - 1].is_whitespace() {
                        n -= 1;
                    }
                    while n > 0 && !a[n - 1].is_whitespace() {
                        n -= 1;
                    }
                    p.typing_x = n;
                }
                (Some(F::NextWord), _) => {
                    let mut n = x;
                    while n < a.len() && !a[n].is_whitespace() {
                        n += 1;
                    }
                    while n < a.len() && a[n].is_whitespace() {
                        n += 1;
                    }
                    p.typing_x = n;
                }
                (Some(F::Backspace), _) if x > 0 => {
                    a.remove(x - 1);
                    p.typing_x -= 1;
                }
                (Some(F::Delete), _) if x < a.len() => {
                    a.remove(x);
                }
                (Some(F::Cut), _) => {
                    a.truncate(x);
                }
                (Some(F::Paste), _) => {
                    let text = self
                        .cutbuffer
                        .iter()
                        .map(|l| l.iter().collect::<String>())
                        .collect::<Vec<_>>()
                        .join(" ");
                    let n = text.chars().count();
                    a.splice(x..x, text.chars());
                    p.typing_x += n;
                }
                (Some(F::Tab), _) if matches!(menu, MWRITEFILE | MINSERTFILE | MGOTODIR) => {
                    let value = a.iter().collect::<String>();
                    let completions = crate::files::complete_path(&value);
                    if completions.len() == 1 {
                        a = completions[0].chars().collect();
                        p.typing_x = a.len();
                    }
                }
                (_, Key::Char(c, false, false)) => {
                    a.insert(x, c);
                    p.typing_x += 1;
                }
                (_, Key::Space) => {
                    a.insert(x, ' ');
                    p.typing_x += 1;
                }
                _ => {}
            }
            p.answer = a.into_iter().collect();
        }
    }
    pub fn confirm(&mut self, question: &str, all: bool) -> Option<char> {
        self.prompt = Some(PromptState {
            menu: MYESNO,
            question: question.into(),
            answer: String::new(),
            typing_x: 0,
            allow_editing: false,
        });
        let answer = loop {
            self.draw();
            match self.input() {
                Event::Key(k) => match (k.code, k.modifiers.contains(M::CONTROL)) {
                    (KeyCode::Char('c'), true) => break None,
                    (KeyCode::Char(c), false)
                        if matches!(c.to_ascii_lowercase(), 'y' | 'n')
                            || (all && c.eq_ignore_ascii_case(&'a')) =>
                    {
                        break Some(c.to_ascii_lowercase())
                    }
                    _ => self.beep(),
                },
                Event::Resize(_, _) => self.term.measure(),
                _ => {}
            }
            if self.quit {
                break None;
            }
        };
        self.prompt = None;
        answer
    }
    fn mouse_event(&mut self, m: crossterm::event::MouseEvent) {
        use terminal::MouseKind::*;
        self.completion = None;
        if !self.cfg.mouse {
            return;
        }
        let Some(m) = terminal::convert_mouse(&m) else {
            return;
        };
        match m.kind {
            WheelUp => {
                for _ in 0..3 {
                    self.scroll_up();
                }
            }
            WheelDown => {
                for _ in 0..3 {
                    self.scroll_down();
                }
            }
            Click | Drag => {
                if m.row >= self.edit_top() && m.row < self.edit_top() + self.edit_rows() {
                    if let Some(&(y, left, _)) = self.visible_rows().get(m.row - self.edit_top()) {
                        let x = crate::display::actual_x(
                            self.borig().line(y),
                            left + m.col.saturating_sub(self.number_margin()),
                            self.cfg.tabsize,
                        );
                        if (m.kind == Drag || m.shift) && self.borig().mark.is_none() {
                            let p = self.borig().cursor;
                            self.b().mark = Some(p);
                        }
                        self.goto_pos(y, x);
                    }
                } else if m.row + 2 >= self.term.rows && !self.cfg.nohelp {
                    let hints = self.menu_hints(MMAIN);
                    let width = self.term.cols / hints.len().div_ceil(2).max(1);
                    let i = (m.col / width.max(1)) * 2 + usize::from(m.row + 1 == self.term.rows);
                    if let Some((key, _)) = hints.get(i) {
                        if let Some(k) = keys::key_from_string(key) {
                            if let Some(f) = self.binding(MMAIN, k) {
                                self.dispatch(f);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    fn anchor(&mut self, forward: bool) {
        let b = self.borig();
        let n = b.lines.len();
        let at = b.cursor.y;
        for i in 1..=n {
            let y = if forward {
                (at + i) % n
            } else {
                (at + n - i % n) % n
            };
            if b.lines[y].has_anchor {
                self.goto_pos(y, 0);
                return;
            }
        }
        self.statusbar("No anchors in this buffer");
    }
    pub fn toggle_recording(&mut self) {
        if self.playing_macro {
            return;
        }
        self.recording ^= true;
        if self.recording {
            self.macro_keys.clear();
        }
        self.statusbar(if self.recording {
            "Recording a macro..."
        } else {
            "Stopped recording"
        });
    }
    pub fn play_macro(&mut self) {
        if self.recording || self.playing_macro {
            return;
        }
        self.playing_macro = true;
        self.replay = self.macro_keys.clone().into();
        let mut count = 0;
        while let Some(e) = self.replay.pop_front() {
            count += 1;
            if count > 100000 {
                self.status(Importance::Ahem, "Macro recursion limit reached");
                break;
            }
            if self.quit {
                break;
            }
            self.handle_event(e);
        }
        self.playing_macro = false;
        self.replay.clear();
    }

    fn switch_buffer(&mut self, forward: bool) {
        if self.bufs.len() < 2 {
            self.statusbar("No more open file buffers");
            return;
        }
        self.remember_position();
        self.cur = if forward {
            (self.cur + 1) % self.bufs.len()
        } else {
            (self.cur + self.bufs.len() - 1) % self.bufs.len()
        };
        self.last_action = UndoType::Other;
        self.softmark = None;
    }
}

impl Editor {
    fn expand_binding(&mut self, index: usize) {
        // Build events first so a dialog can consume the rest of the expansion.
        let Some(text) = self.cfg.expansions.get(index).cloned() else {
            return;
        };
        let mut chars = text.chars().peekable();
        let mut events = Vec::new();
        while let Some(c) = chars.next() {
            if c == '{' {
                let mut command = String::new();
                for ch in chars.by_ref() {
                    if ch == '}' {
                        break;
                    }
                    command.push(ch);
                }
                let Some(f) = crate::config::function(&command) else {
                    self.status(
                        Importance::Ahem,
                        &format!("Unknown expansion function: {command}"),
                    );
                    return;
                };
                let Some(k) = keys::first_key_for(u32::MAX, f) else {
                    self.status(Importance::Ahem, &format!("Function has no key: {command}"));
                    return;
                };
                events.push(event_for_key(k));
            } else {
                events.push(Event::Key(KeyEvent::new(KeyCode::Char(c), M::NONE)));
            }
        }
        for e in events.into_iter().rev() {
            self.replay.push_front(e);
        }
        if !self.playing_macro {
            self.playing_macro = true;
            let mut count = 0;
            while let Some(e) = self.replay.pop_front() {
                count += 1;
                if count > 100000 {
                    self.status(Importance::Ahem, "Expansion recursion limit reached");
                    self.replay.clear();
                    break;
                }
                if self.quit {
                    break;
                }
                self.handle_event(e);
            }
            self.playing_macro = false;
        }
    }
}
fn event_for_key(k: Key) -> Event {
    if let Key::ShiftChar(c, ctrl, alt) = k {
        return Event::Key(KeyEvent::new(
            KeyCode::Char(c),
            M::SHIFT
                | (if ctrl { M::CONTROL } else { M::NONE })
                | (if alt { M::ALT } else { M::NONE }),
        ));
    }
    let (code, ctrl, alt) = match k {
        Key::ShiftChar(..) => unreachable!(),
        Key::Char(c, ctrl, alt) => (
            KeyCode::Char(if ctrl { c.to_ascii_lowercase() } else { c }),
            ctrl,
            alt,
        ),
        Key::Enter => (KeyCode::Enter, false, false),
        Key::Tab => (KeyCode::Tab, false, false),
        Key::Space => (KeyCode::Char(' '), false, false),
        Key::Left(c, a) => (KeyCode::Left, c, a),
        Key::Right(c, a) => (KeyCode::Right, c, a),
        Key::Up(c, a) => (KeyCode::Up, c, a),
        Key::Down(c, a) => (KeyCode::Down, c, a),
        Key::Home(c, a) => (KeyCode::Home, c, a),
        Key::End(c, a) => (KeyCode::End, c, a),
        Key::Del(c, a) => (KeyCode::Delete, c, a),
        Key::Backspace(a) => (KeyCode::Backspace, false, a),
        Key::PgUp => (KeyCode::PageUp, false, false),
        Key::PgDn => (KeyCode::PageDown, false, false),
        Key::Ins => (KeyCode::Insert, false, false),
        Key::ShTab => (KeyCode::BackTab, false, false),
        Key::F(n) => (KeyCode::F(n), false, false),
    };
    Event::Key(KeyEvent::new(
        code,
        (if ctrl { M::CONTROL } else { M::NONE }) | (if alt { M::ALT } else { M::NONE }),
    ))
}

impl Editor {
    pub fn menu_hints(&self, menu: u32) -> Vec<keys::Hint> {
        let mut result = Vec::new();
        let mut seen = Vec::new();
        let maximum = ((self.term.cols + 40) / 20) * 2;
        for &(f, tag, menus) in keys::FUNCS {
            if result.len() >= maximum {
                break;
            }
            if menus & menu == 0
                || seen.contains(&f)
                || (f == F::DiscardBuffer && !self.cfg.save_on_exit)
            {
                continue;
            }
            let bound = self
                .cfg
                .bindings
                .iter()
                .rev()
                .find(|&&(m, k, func)| {
                    m & menu != 0 && func == Some(f) && self.binding(menu, k) == Some(f)
                })
                .map(|&(_, k, _)| k);
            let key = bound.or_else(|| {
                keys::shortcut_keys(menu, f)
                    .into_iter()
                    .find(|&k| self.binding(menu, k) == Some(f))
            });
            if let Some(k) = key {
                result.push((keys::describe(k), tag));
                seen.push(f);
            }
        }
        result
    }
}
