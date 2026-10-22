use crate::{
    defs::*,
    keys::{self, F},
    syntax::{self, Rule, Syntax},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn expand_home(s: &str) -> PathBuf {
    if s == "~" {
        return PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    }
    if let Some(rest) = s.strip_prefix("~/") {
        return PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(rest);
    }
    PathBuf::from(s)
}

pub fn words(mut line: &str) -> Vec<String> {
    let mut out = Vec::new();
    while !line.trim_start().is_empty() {
        line = line.trim_start();
        if line.starts_with('#') && out.is_empty() {
            break;
        }
        if let Some(s) = line.strip_prefix('"') {
            // Nanorc parameters can contain literal quotes. A quoted regex
            // ends at a quote followed by whitespace, another regex, or EOL.
            let end = s
                .char_indices()
                .find(|&(i, c)| {
                    c == '"' && (i + 1 == s.len() || s[i + 1..].starts_with(char::is_whitespace))
                })
                .map(|(i, _)| i)
                .unwrap_or(s.len());
            out.push(s[..end].to_string());
            line = if end < s.len() { &s[end + 1..] } else { "" };
        } else {
            let end = line.find(char::is_whitespace).unwrap_or(line.len());
            out.push(line[..end].to_string());
            line = &line[end..];
        }
    }
    out
}

pub fn set(cfg: &mut Config, name: &str, value: Option<&str>, on: bool) -> Result<(), String> {
    macro_rules! flags { ($($n:literal => $f:ident),* $(,)?) => { match name { $($n => {cfg.$f=on; return Ok(());}),* _=>{} } }; }
    flags! {
        "smarthome"=>smarthome,"tabstospaces"=>tabs_to_spaces,"cutfromcursor"=>cut_from_cursor,
        "zap"=>zap,"jumpyscrolling"=>jumpyscrolling,"linenumbers"=>linenumbers,
        "softwrap"=>softwrap,"atblanks"=>atblanks,"breaklonglines"=>break_long_lines,
        "autoindent"=>autoindent,"bookstyle"=>bookstyle,"trimblanks"=>trimblanks,
        "constantshow"=>constantshow,"quickblank"=>quickblank,"mouse"=>mouse,
        "view"=>view_mode,"restricted"=>restricted,"nohelp"=>nohelp,"emptyline"=>emptyline,
        "zero"=>zero,"minibar"=>minibar,"stateflags"=>stateflags,"indicator"=>indicator,
        "saveonexit"=>save_on_exit,"backup"=>backup,"nonewlines"=>nonewlines,
        "noconvert"=>noconvert,"multibuffer"=>multibuffer,"newbuffer"=>multibuffer,
        "boldtext"=>bold,"wordbounds"=>wordbounds,"afterends"=>afterends,
        "historylog"=>historylog,"positionlog"=>positionlog,"locking"=>locking,
        "showcursor"=>showcursor,"rebinddelete"=>rebinddelete,"preserve"=>preserve,"colonparsing"=>colonparsing,"magic"=>magic,"unix"=>unix,"casesensitive"=>search_case,"regexp"=>search_regexp
    }
    if name.ends_with("color") {
        cfg.colors.insert(
            name.to_string(),
            syntax::color(value.ok_or("Missing color")?),
        );
        return Ok(());
    }
    let val = value.ok_or_else(|| format!("Option {name} requires an argument"))?;
    match name {
        "wordchars" => cfg.wordchars = Some(val.into()),
        "punct" => cfg.punct = val.into(),
        "brackets" => cfg.brackets = val.into(),
        "whitespace" => {
            if val.chars().count() != 2 {
                return Err("Whitespace requires two characters".into());
            }
            cfg.whitespace_chars = val.into();
        }
        "tabsize" => {
            cfg.tabsize = val
                .parse::<usize>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or("Invalid tab size")?
        }
        "fill" => cfg.fill = val.parse().map_err(|_| "Invalid fill width")?,
        "guidestripe" => cfg.guidestripe = Some(val.parse().map_err(|_| "Invalid guide stripe")?),
        "quotestr" => {
            syntax::compile(val, false).map_err(|e| e.to_string())?;
            cfg.quotestr = val.to_string();
        }
        "matchbrackets" => {
            if val.chars().count() % 2 != 0 || val.chars().any(char::is_whitespace) {
                return Err("Invalid bracket pairs".into());
            }
            cfg.matchbrackets = val.into();
        }
        "backupdir" => cfg.backup_dir = Some(expand_home(val).to_string_lossy().into()),
        "operatingdir" => {
            cfg.operdir = if val.is_empty() {
                None
            } else {
                Some(expand_home(val).to_string_lossy().into())
            }
        }
        "speller" => cfg.speller = Some(vec![val.to_string()]),
        _ => return Err(format!("Unknown option: {name}")),
    }
    Ok(())
}

pub fn menu(s: &str) -> Option<u32> {
    Some(match s {
        "main" => MMAIN,
        "search" => MWHEREIS,
        "replace" => MREPLACE,
        "replacewith" => MREPLACEWITH,
        "gotoline" => MGOTOLINE,
        "writeout" => MWRITEFILE,
        "insert" => MINSERTFILE,
        "execute" => MEXECUTE,
        "help" => MHELP,
        "browser" => MBROWSER,
        "whereisfile" => MWHEREISFILE,
        "gotodir" => MGOTODIR,
        "yesno" => MYESNO,
        "all" => u32::MAX,
        _ => return None,
    })
}
pub fn function(s: &str) -> Option<F> {
    Some(match s {
        "flipreplace" => F::FlipReplace,
        "flipgoto" => F::FlipGoto,
        "flippipe" => F::PipeToggle,
        "flipnewbuffer" => F::NewBufferToggle,
        "flipexecute" => F::FlipExecute,
        "flipconvert" => F::NoConvert,
        "dosformat" => F::DosFormat,
        "macformat" => F::MacFormat,
        "append" => F::Append,
        "prepend" => F::Prepend,
        "backup" => F::Backup,
        "discardbuffer" => F::DiscardBuffer,
        "browser" => F::ToFiles,
        "firstfile" => F::BrowserFirst,
        "lastfile" => F::BrowserLast,
        "older" => F::OlderItem,
        "newer" => F::NewerItem,
        "findbracket" => F::FindBracket,
        "beginpara" => F::ParaBegin,
        "endpara" => F::ParaEnd,
        "toprow" => F::TopRow,
        "bottomrow" => F::BottomRow,
        "cutfromcursor" => F::ToggleCutFromCursor,
        "breaklonglines" => F::ToggleBreakLong,
        "nosyntax" => F::ToggleNoSyntax,
        "recordmacro" => F::RecordMacro,
        "runmacro" => F::RunMacro,
        "complete" => F::Complete,
        "anchor" => F::Anchor,
        "prevanchor" => F::PrevAnchor,
        "nextanchor" => F::NextAnchor,
        "formatter" => F::Formatter,
        "linter" => F::Linter,
        "help" => F::Help,
        "cancel" => F::Cancel,
        "exit" => F::Exit,
        "writeout" => F::WriteOut,
        "savefile" => F::SaveFile,
        "insert" => F::ReadFile,
        "whereis" => F::SearchForward,
        "wherewas" => F::SearchBackward,
        "findnext" => F::FindNext,
        "findprevious" => F::FindPrev,
        "replace" => F::Replace,
        "cut" => F::Cut,
        "copy" => F::Copy,
        "paste" => F::Paste,
        "execute" => F::Execute,
        "justify" => F::Justify,
        "fulljustify" => F::FullJustify,
        "location" => F::Location,
        "gotoline" => F::GoToLine,
        "undo" => F::Undo,
        "redo" => F::Redo,
        "mark" => F::Mark,
        "back" => F::Left,
        "forward" => F::Right,
        "prevword" => F::PrevWord,
        "nextword" => F::NextWord,
        "home" => F::Home,
        "end" => F::End,
        "up" => F::Up,
        "down" => F::Down,
        "scrollup" => F::ScrollUp,
        "scrolldown" => F::ScrollDown,
        "prevblock" => F::PrevBlock,
        "nextblock" => F::NextBlock,
        "pageup" => F::PageUp,
        "pagedown" => F::PageDown,
        "firstline" => F::FirstLine,
        "lastline" => F::LastLine,
        "prevbuf" => F::PrevFile,
        "nextbuf" => F::NextFile,
        "tab" => F::Tab,
        "enter" => F::Enter,
        "backspace" => F::Backspace,
        "delete" => F::Delete,
        "chopwordleft" => F::ChopWordLeft,
        "chopwordright" => F::ChopWordRight,
        "cutrestoffile" => F::CutTillEof,
        "wordcount" => F::WordCount,
        "verbatim" => F::Verbatim,
        "indent" => F::Indent,
        "unindent" => F::Unindent,
        "comment" => F::Comment,
        "zap" => F::Zap,
        "speller" => F::SpellCheck,
        "suspend" => F::Suspend,
        "refresh" => F::Refresh,
        "center" => F::Cycle,
        "gotodir" => F::GotoDir,
        "tofiles" => F::ToFiles,
        "linenumbers" => F::ToggleLinenumbers,
        "softwrap" => F::ToggleSoftwrap,
        "nohelp" => F::ToggleNoHelp,
        "constantshow" => F::ToggleConstant,
        "autoindent" => F::ToggleAutoindent,
        "mouse" => F::ToggleMouse,
        "smarthome" => F::ToggleSmartHome,
        "tabstospaces" => F::ToggleTabsToSpaces,
        "whitespacedisplay" => F::ToggleWhitespace,
        "zero" => F::ToggleZero,
        "casesens" => F::CaseSens,
        "regexp" => F::Regexp,
        "backwards" => F::Backwards,
        _ => return None,
    })
}

fn glob(pattern: &Path) -> Vec<PathBuf> {
    let name = pattern.file_name().unwrap_or_default().to_string_lossy();
    if !name.contains(['*', '?']) {
        return vec![pattern.to_owned()];
    }
    let re = format!(
        "^{}$",
        regex::escape(&name)
            .replace("\\*", ".*")
            .replace("\\?", ".")
    );
    let re = regex::Regex::new(&re).unwrap();
    let mut paths = fs::read_dir(pattern.parent().unwrap_or(Path::new(".")))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| re.is_match(&p.file_name().unwrap_or_default().to_string_lossy()))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

pub fn read(cfg: &mut Config, path: &Path, depth: usize, errors: &mut Vec<String>) {
    if depth > 16 {
        errors.push(format!("{}: include nesting too deep", path.display()));
        return;
    }
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            errors.push(format!("{}: {e}", path.display()));
            return;
        }
    };
    let mut current = None;
    for (num, line) in text.lines().enumerate() {
        let w = words(line);
        if w.is_empty() {
            continue;
        }
        let result: Result<(), String> = (|| {
            match w[0].as_str() {
                "set" | "unset" => {
                    if w.len() < 2 {
                        return Err("Missing option".into());
                    }
                    set(cfg, &w[1], w.get(2).map(String::as_str), w[0] == "set")?;
                }
                "include" => {
                    for p in w.iter().skip(1) {
                        let p = expand_home(p);
                        let p = if p.is_absolute() {
                            p
                        } else {
                            path.parent().unwrap_or(Path::new(".")).join(p)
                        };
                        for p in glob(&p) {
                            read(cfg, &p, depth + 1, errors);
                        }
                    }
                }
                "bind" | "unbind" => {
                    let unbind = w[0] == "unbind";
                    let k = keys::key_from_string(w.get(1).ok_or("Missing key")?)
                        .ok_or("Invalid key")?;
                    let m = menu(w.get(if unbind { 2 } else { 3 }).ok_or("Missing menu")?)
                        .ok_or("Invalid menu")?;
                    let f = if unbind {
                        None
                    } else if line.trim_start().strip_prefix("bind").is_some_and(|s| {
                        s.trim_start()
                            .split_once(char::is_whitespace)
                            .is_some_and(|(_, s)| s.trim_start().starts_with('"'))
                    }) {
                        cfg.expansions.push(w[2].clone());
                        Some(F::Expansion(cfg.expansions.len() - 1))
                    } else {
                        Some(
                            function(&w[2])
                                .ok_or_else(|| format!("Unsupported function: {}", w[2]))?,
                        )
                    };
                    cfg.bindings.push((m, k, f));
                }
                "syntax" => {
                    cfg.syntaxes.push(Syntax {
                        name: w.get(1).ok_or("Missing syntax name")?.clone(),
                        ..Default::default()
                    });
                    current = Some(cfg.syntaxes.len() - 1);
                    for p in w.iter().skip(2) {
                        if let Ok(r) = syntax::compile(p, false) {
                            cfg.syntaxes.last_mut().unwrap().filenames.push(r);
                        }
                    }
                }
                "header" | "color" | "icolor" | "comment" | "tabgives" | "formatter" | "linter" => {
                    let s = &mut cfg.syntaxes[current.ok_or("No syntax defined")?];
                    match w[0].as_str() {
                        "header" => {
                            for p in w.iter().skip(1) {
                                s.headers
                                    .push(syntax::compile(p, false).map_err(|e| e.to_string())?);
                            }
                        }
                        "comment" => s.comment = w.get(1).cloned(),
                        "tabgives" => s.tab = w.get(1).cloned(),
                        "formatter" => s.formatter = w.get(1).cloned(),
                        "linter" => s.linter = w.get(1).cloned(),
                        _ => {
                            let c = syntax::color(w.get(1).ok_or("Missing color")?);
                            let patterns = quoted_patterns(line);
                            if line.contains("start=") {
                                if patterns.len() >= 2 {
                                    s.rules.push(Rule {
                                        color: c,
                                        start: syntax::compile(&patterns[0], w[0] == "icolor")
                                            .map_err(|e| e.to_string())?,
                                        end: Some(
                                            syntax::compile(&patterns[1], w[0] == "icolor")
                                                .map_err(|e| e.to_string())?,
                                        ),
                                    });
                                }
                            } else {
                                for p in patterns {
                                    s.rules.push(Rule {
                                        color: c.clone(),
                                        start: syntax::compile(&p, w[0] == "icolor")
                                            .map_err(|e| e.to_string())?,
                                        end: None,
                                    });
                                }
                            }
                        }
                    }
                }
                "magic" => {
                    let s = &mut cfg.syntaxes[current.ok_or("No syntax defined")?];
                    for pattern in w.iter().skip(1) {
                        s.magic
                            .push(syntax::compile(pattern, false).map_err(|e| e.to_string())?);
                    }
                }
                "extendsyntax" => {
                    let n = w.get(1).ok_or("Missing syntax name")?;
                    let s = cfg
                        .syntaxes
                        .iter_mut()
                        .find(|s| &s.name == n)
                        .ok_or("Unknown syntax")?;
                    match w.get(2).map(String::as_str) {
                        Some("comment") => s.comment = w.get(3).cloned(),
                        Some("tabgives") => s.tab = w.get(3).cloned(),
                        Some("formatter") => s.formatter = w.get(3).cloned(),
                        Some("linter") => s.linter = w.get(3).cloned(),
                        Some("header") | Some("magic") => {
                            let field = if w[2] == "header" {
                                &mut s.headers
                            } else {
                                &mut s.magic
                            };
                            for p in w.iter().skip(3) {
                                field.push(syntax::compile(p, false).map_err(|e| e.to_string())?);
                            }
                        }
                        Some("color") | Some("icolor") => {
                            let color = syntax::color(w.get(3).ok_or("Missing color")?);
                            for p in quoted_patterns(line) {
                                s.rules.push(Rule {
                                    color: color.clone(),
                                    start: syntax::compile(&p, w[2] == "icolor")
                                        .map_err(|e| e.to_string())?,
                                    end: None,
                                });
                            }
                        }
                        _ => return Err("Unsupported syntax extension".into()),
                    }
                }
                _ => return Err(format!("Unknown command: {}", w[0])),
            }
            Ok(())
        })();
        if let Err(e) = result {
            errors.push(format!("{}:{}: {e}", path.display(), num + 1));
        }
    }
}

fn quoted_patterns(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in line.char_indices() {
        if c == '"' {
            if let Some(s) = start {
                if i + 1 == line.len() || line[i + 1..].starts_with(char::is_whitespace) {
                    out.push(line[s..i].to_string());
                    start = None;
                }
            } else {
                start = Some(i + 1);
            }
        }
    }
    out
}

pub fn load(cfg: &mut Config, explicit: Option<&str>, ignore: bool) -> Vec<String> {
    let mut errors = Vec::new();
    if ignore {
        return errors;
    }
    if let Some(p) = explicit {
        read(cfg, &expand_home(p), 0, &mut errors);
        return errors;
    }
    if Path::new("/etc/nanorc").exists() {
        read(cfg, Path::new("/etc/nanorc"), 0, &mut errors);
    }
    let home = expand_home("~");
    let xdg = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    for group in [
        [home.join(".nanorc"), xdg.join("nano/nanorc")],
        [home.join(".runorc"), xdg.join("runo/runorc")],
    ] {
        if let Some(p) = group.iter().find(|p| p.exists()) {
            read(cfg, p, 0, &mut errors);
        }
    }
    errors
}
