// terminal.rs — raw mode, screen sizing, input polling and output painting.

use crossterm::event::{
    poll, read, DisableMouseCapture, EnableMouseCapture, Event, KeyEvent, KeyEventKind, MouseEvent,
};
use crossterm::execute;
use std::io::{self, Stdout, Write};
use std::time::Duration;

use crate::keys::Key;

pub struct Terminal {
    pub out: Stdout,
    pub rows: usize,
    pub cols: usize,
    pub mouse: bool,
    pub suspended: bool,
    pub active: bool,
    pub preserve: bool,
}
extern "C" {
    fn isatty(fd: i32) -> i32;
}

pub fn is_tty() -> bool {
    unsafe { isatty(0) != 0 }
}

impl Terminal {
    pub fn new(mouse: bool) -> io::Result<Terminal> {
        let out = io::stdout();
        let mut t = Terminal {
            out,
            rows: 24,
            cols: 80,
            mouse,
            suspended: false,
            active: false,
            preserve: false,
        };
        t.start()?;
        t.measure();
        Ok(t)
    }

    pub fn start(&mut self) -> io::Result<()> {
        crossterm::terminal::enable_raw_mode()?;
        self.active = true;
        execute!(self.out, crossterm::terminal::EnterAlternateScreen)?;
        execute!(self.out, crossterm::event::EnableBracketedPaste)?;
        if self.mouse {
            execute!(self.out, EnableMouseCapture)?;
        }
        execute!(self.out, crossterm::cursor::Hide)?;
        Ok(())
    }

    pub fn stop(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        let _ = execute!(self.out, crossterm::event::DisableBracketedPaste);
        let _ = write!(self.out, "\x1b[0m\x1b[{};1H\x1b[K", self.rows);
        let _ = crossterm::terminal::disable_raw_mode();
        if self.mouse {
            let _ = execute!(self.out, DisableMouseCapture);
        }
        let _ = execute!(
            self.out,
            crossterm::cursor::Show,
            crossterm::terminal::LeaveAlternateScreen
        );
        let _ = self.out.flush();
    }

    /// Temporarily leave raw mode (before culdozering to suspend).
    pub fn leave_for_suspend(&mut self) {
        let _ = execute!(
            self.out,
            DisableMouseCapture,
            crossterm::event::DisableBracketedPaste,
            crossterm::terminal::LeaveAlternateScreen
        );
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = execute!(self.out, crossterm::cursor::Show);
        let _ = self.out.flush();
    }

    /// Put the terminal back together after the suspension.
    pub fn resume_after_suspend(&mut self) {
        let _ = crossterm::terminal::enable_raw_mode();
        let _ = execute!(
            self.out,
            crossterm::terminal::EnterAlternateScreen,
            crossterm::event::EnableBracketedPaste
        );
        if self.mouse {
            let _ = execute!(self.out, EnableMouseCapture);
        }
        self.set_flow_control(self.preserve);
        self.measure();
        self.suspended = true;
    }

    pub fn set_flow_control(&mut self, on: bool) {
        self.preserve = on;
        if on {
            unsafe {
                let mut attr = std::mem::MaybeUninit::<libc::termios>::uninit();
                if libc::tcgetattr(0, attr.as_mut_ptr()) == 0 {
                    let mut attr = attr.assume_init();
                    attr.c_iflag |= libc::IXON;
                    libc::tcsetattr(0, libc::TCSANOW, &attr);
                }
            }
        }
    }
    pub fn measure(&mut self) {
        if let Ok((c, r)) = crossterm::terminal::size() {
            if r > 0 && c > 0 {
                self.rows = r as usize;
                self.cols = c as usize;
            }
        }
    }

    /// Collect one event, blocking forever.  Returns None on weirdness.
    pub fn next_event(&mut self) -> Event {
        loop {
            match read() {
                Ok(ev) => {
                    // Ignore key-release events from kitty terminals.
                    if let Event::Key(e) = &ev {
                        if e.kind == KeyEventKind::Release {
                            continue;
                        }
                    }
                    return ev;
                }
                Err(_) => continue,
            }
        }
    }

    /// Check whether some event is waiting for at most the given time.
    pub fn poll_event(&self, timeout: Duration) -> Option<Event> {
        if poll(timeout).ok()? {
            read().ok()
        } else {
            None
        }
    }

    pub fn set_mouse(&mut self, on: bool) {
        if on != self.mouse {
            let res = if on {
                execute!(self.out, EnableMouseCapture)
            } else {
                execute!(self.out, DisableMouseCapture)
            };
            res.ok();
            self.mouse = on;
        }
    }

    /// Write a raw string straight out (for the bell, and such).
    pub fn write_raw(&mut self, s: &str) {
        let out = &mut self.out;
        let _ = out.write_all(s.as_bytes());
        let _ = out.flush();
    }
}

/// Convert a crossterm key event into Runo's key type.
pub fn convert_key(ev: &KeyEvent) -> Option<Key> {
    use crossterm::event::KeyCode::Char as KChar;
    use crossterm::event::KeyModifiers as M;

    let ctrl = ev.modifiers.contains(M::CONTROL);
    let alt = ev.modifiers.contains(M::ALT);

    let key = match ev.code {
        KChar(c) => {
            if alt && ev.modifiers.contains(M::SHIFT) && c.is_ascii_alphabetic() {
                return Some(Key::ShiftChar(c.to_ascii_uppercase(), ctrl, alt));
            }
            if c == '\r' {
                return Some(Key::Enter);
            }
            if c == '\t' {
                return Some(if ev.modifiers.contains(M::SHIFT) {
                    Key::ShTab
                } else {
                    Key::Tab
                });
            }
            if c == '\n' || c == 0x0B as char {
                // Ctrl+J or Ctrl+K arriving as raw control codes.
                let code = c as u32 as u8;
                return Some(Key::Char(
                    ((code + 0x40) as char).to_ascii_lowercase(),
                    true,
                    alt,
                ));
            }
            if ctrl && matches!(c, '4' | '5' | '7') {
                return Some(Key::Char(
                    match c {
                        '4' => '\\',
                        '5' => ']',
                        _ => '_',
                    },
                    true,
                    alt,
                ));
            }
            if ctrl && c != ' ' {
                return Some(Key::Char(c.to_ascii_lowercase(), true, alt));
            }
            if (c as u32) < 0x20 {
                // A raw control character without the ctrl flag set.
                let code = c as u32 as u8;
                return Some(Key::Char(
                    ((code + 0x40) as char).to_ascii_lowercase(),
                    true,
                    alt,
                ));
            }
            if c == ' ' && !ctrl && !alt {
                return Some(Key::Space);
            }
            return Some(Key::Char(c, false, alt));
        }
        crossterm::event::KeyCode::Enter => Key::Enter,
        crossterm::event::KeyCode::Backspace => Key::Backspace(alt),
        crossterm::event::KeyCode::Delete => Key::Del(ctrl, alt),
        crossterm::event::KeyCode::Left => Key::Left(ctrl, alt),
        crossterm::event::KeyCode::Right => Key::Right(ctrl, alt),
        crossterm::event::KeyCode::Up => Key::Up(ctrl, alt),
        crossterm::event::KeyCode::Down => Key::Down(ctrl, alt),
        crossterm::event::KeyCode::Home => Key::Home(ctrl, alt),
        crossterm::event::KeyCode::End => Key::End(ctrl, alt),
        crossterm::event::KeyCode::PageUp => Key::PgUp,
        crossterm::event::KeyCode::PageDown => Key::PgDn,
        crossterm::event::KeyCode::Insert => Key::Ins,
        crossterm::event::KeyCode::BackTab => Key::ShTab,
        crossterm::event::KeyCode::Tab => {
            if ev.modifiers.contains(M::SHIFT) {
                Key::ShTab
            } else {
                Key::Tab
            }
        }
        crossterm::event::KeyCode::F(n) => Key::F(n),
        _ => return None,
    };
    Some(key)
}

impl Drop for Terminal {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Paint helpers: build ANSI output for a full-screen frame.
pub struct Painter {
    pub buf: String,
}

impl Painter {
    pub fn new(_rows: usize, _cols: usize) -> Painter {
        Painter { buf: String::new() }
    }

    pub fn goto(&mut self, row: usize, col: usize) {
        self.buf.push_str(&format!("\x1b[{};{}H", row + 1, col + 1));
    }

    pub fn sgr(&mut self, s: &str) {
        self.buf.push_str(&format!("\x1b[{}m", s));
    }

    pub fn reset(&mut self) {
        self.sgr("0");
    }

    pub fn clr_eol(&mut self) {
        self.buf.push_str("\x1b[K");
    }

    pub fn wipescreen(&mut self) {
        // Clear everything, no alternate-screen games beyond enter above.
        self.buf.push_str("\x1b[2J\x1b[H");
    }

    pub fn flush(&mut self, out: &mut Stdout) {
        out.write_all(self.buf.as_bytes()).ok();
        out.flush().ok();
        self.buf.clear();
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MouseKind {
    Click,
    Release,
    Drag,
    WheelUp,
    WheelDown,
    Move,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MouseAction {
    pub kind: MouseKind,
    pub row: usize,
    pub col: usize,
    pub shift: bool,
}

/// Handle a mouse event: return (kind, row, col, shift).
pub fn convert_mouse(ev: &MouseEvent) -> Option<MouseAction> {
    use crossterm::event::MouseButton;
    use crossterm::event::MouseEventKind::*;
    use MouseKind as K;
    let kind = match ev.kind {
        Down(MouseButton::Left) => K::Click,
        Up(MouseButton::Left) => K::Release,
        Drag(_) => K::Drag,
        ScrollUp => K::WheelUp,
        ScrollDown => K::WheelDown,
        Moved => K::Move,
        _ => return None,
    };
    Some(MouseAction {
        kind,
        row: ev.row as usize,
        col: ev.column as usize,
        shift: ev.modifiers.contains(crossterm::event::KeyModifiers::SHIFT),
    })
}
