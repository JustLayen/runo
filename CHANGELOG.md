# Changelog

User-visible changes are recorded here. Cargo/package versions use three
components; the terminal branding currently shows only major and minor.

## 0.1.0 — initial release preparation

- Rust terminal editor targeting GNU nano 8.2-style controls on Linux/glibc.
- Multiple buffers, UTF-8 editing, selection, cut/copy/paste, undo/redo,
  search, and interactive POSIX-regex replacement.
- File browser, line-ending conversion, backups, advisory locks, and
  external-modification detection.
- Soft wrap, line numbers, nanorc-style configuration and syntax highlighting,
  macros, word completion, and external-tool integration.
- Session history, cursor-position persistence, terminal restoration, and
  emergency saving on termination signals.
- Rust unit tests, pseudo-terminal integration checks, and a reproducible
  dependency-license inventory.
- English/Russian project documentation, GitHub Actions CI, and binary
  archive packaging with license notices.

See [compatibility notes](docs/compatibility.md) for known limitations.
