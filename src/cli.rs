use crate::{config, defs::*};
use std::path::Path;

pub struct Options {
    pub cfg: Config,
    pub files: Vec<(String, Option<String>)>,
    pub errors: Vec<String>,
}
pub fn help() -> String {
    format!("Usage: runo [OPTIONS] [[+LINE[,COLUMN]|+/TEXT|+?TEXT] FILE]...\n\nMIT/Runo {} — terminal text editor with nano 8.2 key bindings\n\n  -h, --help                 Show this help\n  -V, --version              Show version\n  -A, --smarthome            Smart Home key\n  -B, --backup               Make backup files\n  -C, --backupdir=DIR        Store numbered backups in DIR\n  -D, --boldtext             Use bold instead of inverse video\n  -E, --tabstospaces         Convert typed tabs to spaces\n  -F, --multibuffer          Read files into new buffers\n  -G, --locking              Enable file locking\n  -H, --historylog           Remember searches and commands\n  -I, --ignorercfiles        Skip initialization files\n  -J, --guidestripe=COL      Draw a guide stripe\n  -K, --rawsequences         Use terminal key sequences\n  -L, --nonewlines           Do not add a final newline\n  -M, --trimblanks           Trim trailing blanks when wrapping\n  -N, --noconvert            Keep carriage returns literally\n  -O, --bookstyle            Paragraphs start at indented lines\n  -P, --positionlog          Remember cursor positions\n  -Q, --quotestr=REGEX       Quote prefix for justification\n  -R, --restricted           Restrict filesystem operations\n  -S, --softwrap             Wrap lines on screen\n  -T, --tabsize=COLS         Tab width (default 8)\n  -U, --quickblank           Clear messages after one key\n  -W, --wordbounds           Treat punctuation as word characters\n  -X, --wordchars=CHARS      Additional word characters\n  -Y, --syntax=NAME          Select syntax definition\n  -Z, --zap                  Let Delete erase a selection\n  -a, --atblanks             Soft wrap at whitespace\n  -b, --breaklonglines       Hard wrap long input lines\n  -c, --constantshow         Always show cursor position\n  -d, --rebinddelete         Swap Delete and Backspace\n  -e, --emptyline            Blank line below the title\n  -f, --rcfile=FILE          Read this initialization file\n  -g, --showcursor           Show cursor in viewers\n  -i, --autoindent           Automatically indent new lines\n  -j, --jumpyscrolling       Scroll in half screens\n  -k, --cutfromcursor        Cut from cursor to line end\n  -l, --linenumbers          Display line numbers\n  -m, --mouse                Enable mouse\n  -n, --noread               Start named empty buffers\n  -o, --operatingdir=DIR     Restrict files to DIR\n  -p, --preserve             Preserve flow control keys\n  -q, --indicator            Show a scrollbar\n  -r, --fill=COLS            Hard wrap / justification width\n  -s, --speller=COMMAND      External spell checker\n  -t, --saveonexit           Save on exit without asking\n  -u, --unix                 Write Unix line endings\n  -v, --view                 Read-only mode\n  -w, --nowrap               Disable hard wrapping\n  -x, --nohelp               Hide bottom help lines\n  -y, --afterends            Word navigation stops at word ends\n  -z, --listsyntaxes         List available syntax names\n  -!, --magic                Detect syntax from file MIME type\n  -%, --stateflags           Show editing state flags\n  -_, --minibar              Compact status bar\n  -0, --zero                 Use the whole terminal\n\nUse FILE '-' to read standard input. Press ^G for key help.\n", VERSION)
}

fn short(c: char) -> Option<(&'static str, bool)> {
    Some(match c {
        'h' => ("help", false),
        'V' => ("version", false),
        'A' => ("smarthome", false),
        'B' => ("backup", false),
        'C' => ("backupdir", true),
        'D' => ("boldtext", false),
        'E' => ("tabstospaces", false),
        'F' => ("multibuffer", false),
        'G' => ("locking", false),
        'H' => ("historylog", false),
        'I' => ("ignorercfiles", false),
        'J' => ("guidestripe", true),
        'K' => ("rawsequences", false),
        'L' => ("nonewlines", false),
        'M' => ("trimblanks", false),
        'N' => ("noconvert", false),
        'O' => ("bookstyle", false),
        'P' => ("positionlog", false),
        'Q' => ("quotestr", true),
        'R' => ("restricted", false),
        'S' => ("softwrap", false),
        'T' => ("tabsize", true),
        'U' => ("quickblank", false),
        'W' => ("wordbounds", false),
        'X' => ("wordchars", true),
        'Y' => ("syntax", true),
        'Z' => ("zap", false),
        'a' => ("atblanks", false),
        'b' => ("breaklonglines", false),
        'c' => ("constantshow", false),
        'd' => ("rebinddelete", false),
        'e' => ("emptyline", false),
        'f' => ("rcfile", true),
        'g' => ("showcursor", false),
        'i' => ("autoindent", false),
        'j' => ("jumpyscrolling", false),
        'k' => ("cutfromcursor", false),
        'l' => ("linenumbers", false),
        'm' => ("mouse", false),
        'n' => ("noread", false),
        'o' => ("operatingdir", true),
        'p' => ("preserve", false),
        'q' => ("indicator", false),
        'r' => ("fill", true),
        's' => ("speller", true),
        't' => ("saveonexit", false),
        'u' => ("unix", false),
        'v' => ("view", false),
        'w' => ("nowrap", false),
        'x' => ("nohelp", false),
        'y' => ("afterends", false),
        'z' => ("listsyntaxes", false),
        '!' => ("magic", false),
        '%' => ("stateflags", false),
        '_' => ("minibar", false),
        '0' => ("zero", false),
        _ => return None,
    })
}

pub fn parse(args: Vec<String>) -> Result<Options, String> {
    let mut parsed = Vec::new();
    let mut files = Vec::new();
    let mut position = None;
    let mut i = 0;
    let mut ended = false;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--" && !ended {
            ended = true;
            i += 1;
            continue;
        }
        if !ended && arg.starts_with("--") {
            let (name, inline) = arg[2..]
                .split_once('=')
                .map_or((&arg[2..], None), |(a, b)| (a, Some(b.to_string())));
            let takes = matches!(
                name,
                "backupdir"
                    | "guidestripe"
                    | "quotestr"
                    | "tabsize"
                    | "wordchars"
                    | "syntax"
                    | "rcfile"
                    | "operatingdir"
                    | "fill"
                    | "speller"
            );
            let val = if takes {
                if inline.is_some() {
                    inline
                } else {
                    i += 1;
                    Some(
                        args.get(i)
                            .ok_or_else(|| format!("--{name} requires an argument"))?
                            .clone(),
                    )
                }
            } else {
                if inline.is_some() {
                    return Err(format!("--{name} does not take an argument"));
                }
                None
            };
            parsed.push((name.to_string(), val));
        } else if !ended && arg.starts_with('-') && arg != "-" {
            let chars = arg[1..].char_indices();
            for (at, c) in chars {
                let (name, takes) = short(c).ok_or_else(|| format!("Unknown option: -{c}"))?;
                let val = if takes {
                    let rest = &arg[1 + at + c.len_utf8()..];
                    if !rest.is_empty() {
                        Some(rest.to_string())
                    } else {
                        i += 1;
                        Some(
                            args.get(i)
                                .ok_or_else(|| format!("-{c} requires an argument"))?
                                .clone(),
                        )
                    }
                } else {
                    None
                };
                parsed.push((name.into(), val));
                if takes {
                    break;
                }
            }
        } else if !ended && arg.starts_with('+') {
            position = Some(arg[1..].to_string());
        } else {
            files.push((arg.clone(), position.take()));
        }
        i += 1;
    }
    if parsed.iter().any(|(n, _)| n == "help") {
        print!("{}", help());
        std::process::exit(0);
    }
    if parsed.iter().any(|(n, _)| n == "version") {
        println!(
            "{} {}\nPackage version: {}\nRust implementation; nano 8.2 compatibility target\nLicense: {}",
            NAME,
            VERSION,
            env!("CARGO_PKG_VERSION"),
            env!("CARGO_PKG_LICENSE")
        );
        std::process::exit(0);
    }
    let mut cfg = Config::default();
    let rc = parsed
        .iter()
        .rev()
        .find(|(n, _)| n == "rcfile")
        .and_then(|(_, v)| v.as_deref());
    let errors = config::load(
        &mut cfg,
        rc,
        parsed.iter().any(|(n, _)| n == "ignorercfiles"),
    );
    for (name, val) in parsed {
        match name.as_str() {
            "rcfile" | "ignorercfiles" => {}
            "syntax" => cfg.syntax_name = val,
            "nowrap" => cfg.break_long_lines = false,
            "rawsequences" => {}
            "showcursor" => cfg.showcursor = true,
            "listsyntaxes" => {
                for s in &cfg.syntaxes {
                    println!("{}", s.name);
                }
                std::process::exit(0);
            }
            "noread" => cfg.noread = true,
            "rebinddelete" => cfg.rebinddelete = true,
            "preserve" => cfg.preserve = true,
            _ => config::set(&mut cfg, &name, val.as_deref(), true)?,
        }
    }
    if position.is_some() {
        files.push((String::new(), position));
    }
    if cfg.colonparsing {
        for (name, pos) in &mut files {
            if pos.is_some() || Path::new(name).exists() {
                continue;
            }
            if let Some((base, last)) = name.rsplit_once(':') {
                if last.parse::<isize>().is_ok() {
                    if Path::new(base).exists() {
                        *pos = Some(last.into());
                        *name = base.into();
                    } else if let Some((base, line)) = base.rsplit_once(':') {
                        if line.parse::<isize>().is_ok() && Path::new(base).exists() {
                            *pos = Some(format!("{line},{last}"));
                            *name = base.into();
                        }
                    }
                }
            }
        }
    }
    Ok(Options { cfg, files, errors })
}
