use crate::{buffer::Buffer, defs::*, editor::Editor, keys, runtime::Answer};
use crossterm::event::Event;
impl Editor {
    pub fn show_help(&mut self) {
        let from = self.active_menu();
        let saved_prompt = self.prompt.take();
        let old = self.cur;
        let text=format!("MIT/Runo — nano 8.2 compatible key bindings\n\nThe top line shows the editor name, current filename, and Modified.\nThe bottom two lines list shortcuts: ^ means Ctrl and M- means Alt.\nEscape followed by a key also produces an Alt shortcut.\n\nType text to insert it. Arrow keys move the cursor.\n^X exits; ^O writes a file; ^S saves without asking for a filename.\n^R reads a file; ^T opens the command prompt.\n^G opens this help. ^X closes help.\n\n^F / ^W search forward; ^B / ^Q search backward.\nM-F / M-B repeat a search. ^\\ or M-R starts replacement.\nAt a search prompt, M-C toggles case, M-R regex, M-B direction.\n^/ or M-G jumps to a line and column (for example 12,8).\n\nM-A or ^6 sets/unsets the mark. Shift+arrows selects text.\n^K cuts the selection or current line. Consecutive cuts accumulate.\nM-6 copies. ^U pastes. M-Del erases a selection.\nM-U undoes an edit. M-E redoes it.\nM-}} indents lines; M-{{ unindents; M-3 comments/uncomments.\nM-] finds the matching bracket. M-D counts words.\nM-V inserts a verbatim key or hexadecimal Unicode code point.\n^J justifies a paragraph. M-J justifies the whole buffer.\n^] completes the current word from other words in this buffer.\n\nM-< / M-> switches buffers. ^X closes the current buffer.\nM-Ins toggles an anchor; M-PgUp / M-PgDn jumps between anchors.\n^T then M-: starts/stops macro recording; M-: replays the macro.\n^T then ^Z suspends the editor; use fg in your shell to resume.\n\nM-N toggles numbers; M-S soft wrap; M-X help lines; M-Z all bars.\nM-I toggles indentation; M-O tabs to spaces; M-M mouse support.\nM-P shows whitespace; M-Y syntax; M-L hard wrapping.\n\nIn file prompts, ^T opens the browser. Tab completes filenames.\nIn Write Out, M-D selects DOS format, M-M selects Mac format,\nM-A appends, M-P prepends, and M-B enables a backup file.\nIn Read File, M-F reads into a new buffer and M-N toggles conversion.\n\nAt ^T, enter a shell command to insert its output.\nM-\\ pipes the selection or buffer through the command.\n^T then ^T runs the spell checker; ^T then ^J justifies the buffer.\n\nCurrent menu: {from}\n");
        let mut b = Buffer::from_string(&text, false);
        b.help = true;
        self.bufs.push(b);
        self.cur = self.bufs.len() - 1;
        let view = self.cfg.view_mode;
        let soft = self.cfg.softwrap;
        let numbers = self.cfg.linenumbers;
        self.help_view = true;
        self.cfg.view_mode = true;
        self.cfg.softwrap = true;
        self.cfg.linenumbers = false;
        loop {
            self.draw();
            let e = self.input();
            if self.quit {
                break;
            }
            match e {
                Event::Key(k) => {
                    if let Some(key) = crate::terminal::convert_key(&k) {
                        if matches!(self.binding(MHELP, key), Some(keys::F::Exit)) {
                            break;
                        }
                    }
                    self.handle_event(Event::Key(k));
                    if !self.help_view {
                        break;
                    }
                }
                other => self.handle_event(other),
            }
        }
        self.bufs.pop();
        self.cur = old;
        self.help_view = false;
        self.cfg.view_mode = view;
        self.cfg.softwrap = soft;
        self.cfg.linenumbers = numbers;
        self.prompt = saved_prompt;
        self.wipe_status();
        let _ = Answer::Cancel;
    }
}
