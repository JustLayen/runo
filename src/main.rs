mod actions;
mod browser;
mod buffer;
mod cli;
mod config;
mod defs;
mod display;
mod editing;
mod editor;
mod files;
mod help;
mod keys;
mod locking;
mod posix;
mod render;
mod runtime;
mod search;
mod session;
mod signals;
mod syntax;
mod terminal;

use defs::*;
use std::io::{self, Read};
fn main() {
    if let Err(e) = start() {
        eprintln!("MIT/Runo: {e}");
        std::process::exit(1);
    }
}
fn start() -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        libc::setlocale(libc::LC_ALL, c"".as_ptr());
    }
    let mut opts = cli::parse(std::env::args().skip(1).collect()).map_err(io::Error::other)?;
    if let Some(dir) = &opts.cfg.operdir {
        std::env::set_current_dir(dir)?;
        opts.cfg.operdir = Some(std::env::current_dir()?.to_string_lossy().into());
    }
    let mut buffers = Vec::new();
    let mut positions = Vec::new();
    for (name, position) in &opts.files {
        let result = if name.is_empty() {
            Ok(buffer::Buffer::new_empty())
        } else if name == "-" {
            let mut s = String::new();
            io::stdin().read_to_string(&mut s)?;
            let mut b = buffer::Buffer::from_string(&s, !opts.cfg.noconvert);
            if !opts.cfg.nonewlines {
                b.add_magic();
            }
            b.serial = 1;
            b.next_serial = 2;
            Ok(b)
        } else {
            files::read_buffer(name, &opts.cfg)
        };
        match result {
            Ok(b) => {
                buffers.push(b);
                positions.push(position.clone());
            }
            Err(e) => opts.errors.push(format!("{name}: {e}")),
        }
    }
    if buffers.is_empty() {
        buffers.push(buffer::Buffer::new_empty());
        positions.push(None);
    }
    if !terminal::is_tty() {
        use std::os::fd::AsRawFd;
        let tty = std::fs::File::open("/dev/tty").map_err(|_| {
            io::Error::other("Standard input is not a terminal; use '-' to read piped input")
        })?;
        if unsafe { libc::dup2(tty.as_raw_fd(), 0) } < 0 {
            return Err(io::Error::last_os_error().into());
        }
    }
    let old_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        eprint!("\x1b[0m\x1b[?25h\x1b[?2004l\x1b[?1049l");
        old_hook(info);
    }));
    signals::install();
    let mut term = terminal::Terminal::new(opts.cfg.mouse)?;
    term.set_flow_control(opts.cfg.preserve);
    let mut ed = editor::Editor::new(opts.cfg, term, buffers);
    ed.load_session();
    ed.lock_buffers();
    for (i, spec) in positions.iter().enumerate() {
        ed.cur = i;
        if let Some(spec) = spec {
            let mut rest = spec.as_str();
            while let Some(c) = rest.chars().next() {
                match c {
                    'c' => ed.search_case = true,
                    'C' => ed.search_case = false,
                    'r' => ed.search_regexp = true,
                    'R' => ed.search_regexp = false,
                    _ => break,
                }
                rest = &rest[c.len_utf8()..];
            }
            if let Some(s) = rest.strip_prefix('/') {
                ed.last_search = s.into();
                ed.last_line();
                ed.repeat_search(false);
            } else if let Some(s) = rest.strip_prefix('?') {
                ed.last_search = s.into();
                ed.first_line();
                ed.repeat_search(true);
            } else {
                ed.goto_spec(spec);
            }
        }
    }
    ed.cur = 0;
    if let Some(error) = opts.errors.first() {
        ed.status(
            Importance::Alert,
            &format!(
                "{error}{}",
                if opts.errors.len() > 1 {
                    format!(
                        " ({} other configuration/read errors)",
                        opts.errors.len() - 1
                    )
                } else {
                    String::new()
                }
            ),
        );
    } else if let Some(name) = &ed.borig().name {
        if !std::path::Path::new(name).exists() {
            ed.statusbar("New File");
        } else {
            let n = ed.borig().lines.len().saturating_sub(1);
            ed.statusbar(&format!(
                "Read {n} {}",
                if n == 1 { "line" } else { "lines" }
            ));
        }
    }
    ed.run();
    ed.term.stop();
    let status = ed.exit_status;
    drop(ed);
    if status != 0 {
        std::process::exit(status);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
