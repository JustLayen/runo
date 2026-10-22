# MIT/Runo

**A small terminal text editor in Rust, with familiar GNU nano-style controls.**

[Русский](README.ru.md) · [Compatibility](docs/compatibility.md) ·
[Contributing](CONTRIBUTING.md) · [Changelog](CHANGELOG.md)

MIT/Runo aims to bring the editing experience of **GNU nano 8.2** to an
MIT-licensed Rust implementation. Open a file, type, save with `Ctrl+S`, and
exit with `Ctrl+X`. An installed copy of nano is not required.

**Status: early release, 0.1.0.** The compatibility target is nano 8.2;
complete behavioral or visual parity is not claimed. The terminal title
shows `MIT/Runo 0.1`. The supported platform is **Linux with glibc**, with
license checks and CI targeting `x86_64-unknown-linux-gnu`.

## Features

- Multiple file buffers, selection, cut/copy/paste, and undo/redo.
- Forward/backward search and interactive replacement, including POSIX
  extended regular expressions and replacement groups `\1`–`\9`.
- UTF-8 text, wide characters, combining marks, and Unix/DOS/Mac line endings.
- Line numbers, soft wrapping, indentation, paragraph justification, and
  word completion.
- Syntax highlighting from a supported subset of `.nanorc` definitions,
  configurable key bindings, and keyboard macros.
- File browser, backups, advisory lock files, and external-change detection.
- Shell command output and buffer piping; external spell checker, formatter,
  and linter integration.
- Mouse support, bracketed paste, cursor-position history, and emergency
  saves on SIGHUP/SIGTERM.

## Build and install

You need **Rust 1.82 or newer**, Cargo, a C linker/build toolchain, and a
UTF-8 terminal on Linux with glibc. On Debian/Ubuntu, `build-essential`
provides the native build tools. Install Rust using [rustup](https://rustup.rs/)
if your distribution's compiler is older than 1.82.

From the downloaded or cloned project directory:

```sh
cargo build --release --locked
./target/release/runo notes.txt
```

To install the `runo` command into Cargo's binary directory (usually
`~/.cargo/bin`, which should be in your `PATH`):

```sh
cargo install --path . --locked
runo --version
```

## Usage

```sh
runo                         # Start with an empty buffer
runo notes.txt               # Open or create a file
runo +12,8 notes.txt          # Start at line 12, column 8
runo -lS first.txt second.txt # Line numbers, soft wrap, two buffers
printf 'hello\n' | runo -     # Read stdin, then edit in the controlling terminal
runo -I notes.txt            # Ignore configuration files
runo --help                  # List command-line options
```

### Essential shortcuts

In the editor, `^` means Ctrl and `M-` means Alt. You can also press
Escape followed by a key instead of holding Alt.

| Action | Shortcut |
| --- | --- |
| Help | `Ctrl+G` |
| Save | `Ctrl+S` |
| Write out / choose filename | `Ctrl+O` |
| Close current buffer / exit | `Ctrl+X` |
| Search forward / backward | `Ctrl+W` / `Ctrl+Q` |
| Replace | `Alt+R` |
| Go to line and column | `Alt+G` |
| Set / unset selection mark | `Alt+A` |
| Cut / copy / paste | `Ctrl+K` / `Alt+6` / `Ctrl+U` |
| Undo / redo | `Alt+U` / `Alt+E` |
| Previous / next buffer | `Alt+<` / `Alt+>` |
| Read a file | `Ctrl+R` |
| Execute a command | `Ctrl+T` |

Shift+arrow keys also select text. `Ctrl+G` provides the longer shortcut
reference and help for the active prompt.

## Configuration

Runo reads settings in this order; later settings override earlier ones:

1. `/etc/nanorc`.
2. The first existing file of `~/.nanorc` and
   `$XDG_CONFIG_HOME/nano/nanorc`.
3. The first existing file of `~/.runorc` and
   `$XDG_CONFIG_HOME/runo/runorc`.
4. Command-line options.

`XDG_CONFIG_HOME` defaults to `~/.config`. Use `-I` to skip configuration,
or `-f FILE` to read only the specified configuration file.

For example, put this in `~/.runorc`:

```text
set linenumbers
set softwrap
set autoindent
set tabsize 4
set tabstospaces
```

See [examples/runorc](examples/runorc) for more settings and an optional
include for system-installed nano syntax definitions. These definitions are
not bundled; nano's syntax-definition packages and external tools are optional.
`--magic` needs the `file` command; spell checking, formatting, and linting
need the corresponding configured programs.

With `set historylog` or `set positionlog`, Runo stores state in
`$XDG_STATE_HOME/runo/` (default `~/.local/state/runo/`), separately from nano.
Restricted mode does not read or write those state files.

## Compatibility and limitations

Files must contain valid UTF-8. The interface is currently English-only.
Configuration support, lock-file conflict handling, completion, and some
editing details differ from nano. macOS, Windows, and musl-based Linux
builds are not currently supported or tested.

See [the compatibility notes](docs/compatibility.md) for the implemented
behavior, known differences, and scope of testing.

## Development

Python 3 is needed for dependency-license and pseudo-terminal checks;
both scripts use only the standard library.

```sh
cargo fetch --locked
python3 scripts/audit_licenses.py
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
python3 tests/pty_smoke.py target/release/runo
```

The suite includes 17 Rust tests (with 1,800 randomized editing operations)
and 25 real pseudo-terminal scenarios. GitHub Actions runs the checks on
stable Rust and tests the minimum supported Rust version. See
[CONTRIBUTING.md](CONTRIBUTING.md) for reporting bugs and making changes,
and [the release guide](docs/releasing.md) for preparing an archive.

## License and acknowledgements

Runo is distributed under the [MIT License](LICENSE). It is an independent 
clean-room implementation written in Rust based on public documentation and 
observed editor behavior, without using or porting GNU nano source code.

GNU nano is the compatibility reference. MIT/Runo is an independent project
and is not affiliated with or endorsed by the GNU nano project.

Dependency license texts are preserved in 
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). The scope and findings of 
the license review are documented in [LICENSE-AUDIT.md](LICENSE-AUDIT.md).
