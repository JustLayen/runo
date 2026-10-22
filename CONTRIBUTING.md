# Contributing to MIT/Runo

Bug reports, compatibility fixes, and documentation improvements are welcome.
The supported development target is Linux with glibc
(`x86_64-unknown-linux-gnu`); the minimum Rust version is 1.82.

## Reporting a bug

Open a GitHub issue with:

- The output of `runo --version`, OS, terminal emulator, and `$TERM`.
- The command line, relevant configuration, and smallest example file.
- Exact keystrokes, expected result, and actual result.
- For nano compatibility issues, the GNU nano version and its result for
  the same steps.

Try `runo -I FILE` to determine whether configuration affects the problem.
Remove private file contents, paths, or command-history entries from reports.

## Development checks

Install Rust with the rustfmt and Clippy components, and Python 3.
From the project root:

```sh
cargo fetch --locked
python3 scripts/audit_licenses.py
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
python3 tests/pty_smoke.py target/release/runo
```

After fetching dependencies, Cargo checks can use `--offline`. The PTY
suite needs a Unix pseudo-terminal and common shell tools (`sh`, `tr`,
`sleep`); no third-party Python packages or installed nano are required.

Keep patches focused and describe the user-visible behavior they fix.
For editing or terminal bugs, add a regression case that demonstrates the
failure in `src/tests.rs` or `tests/pty_smoke.py` as appropriate. Update the
compatibility notes when support changes.

## Project map

- `src/buffer.rs`, `src/editing.rs`: text storage, editing, and undo/redo.
- `src/editor.rs`, `src/runtime.rs`, `src/keys.rs`: state, event loop, controls.
- `src/display.rs`, `src/render.rs`, `src/terminal.rs`: terminal display/input.
- `src/cli.rs`, `src/config.rs`, `src/syntax.rs`: options and configuration.
- `src/files.rs`, `src/browser.rs`, `src/locking.rs`, `src/session.rs`: files/state.
- `src/search.rs`, `src/posix.rs`, `src/actions.rs`: search and external tools.
- `scripts/audit_licenses.py`: locked Linux dependency-license inventory.

## Licensing contributions

Contributions are accepted under the project's [MIT License](LICENSE).
Submit original work or clearly identify compatible third-party material
and preserve its required notices. GNU nano is GPL-licensed: copying,
translating, or adapting its implementation is not a way to contribute
MIT-only code. Describe compatibility behavior and implement it independently.

For dependency updates, include `Cargo.lock`, review upstream license
changes, and refresh `THIRD_PARTY_NOTICES.md` only after that review:

```sh
python3 scripts/audit_licenses.py --write
python3 scripts/audit_licenses.py
```
