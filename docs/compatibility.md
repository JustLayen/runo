# GNU nano compatibility

[Back to README](../README.md)

Runo targets the controls and observable behavior of **GNU nano 8.2**.
Version `0.1.0` is an early implementation, not a claim of full 1:1 parity.
The supported platform is Linux with glibc; CI and the Cargo license
inventory cover `x86_64-unknown-linux-gnu`.

## Implemented behavior

| Area | Current support |
| --- | --- |
| Startup | Short/long options, multiple files, `+LINE,COLUMN`, relative/negative positions, `+/TEXT`, `+?TEXT`, search flags, stdin via `-` |
| Terminal | Full-screen rendering, resize, mouse, bracketed paste, terminal restoration on exit/panic/suspend |
| Display | Selection and search highlighting, line numbers, soft wrap, minibar, zero mode, guide stripe, whitespace display |
| Search | Forward/backward wraparound, case toggle, POSIX ERE, interactive replacement, groups `\1`–`\9`, replacement in a selection |
| Files | Unix/DOS/Mac formats, append/prepend, writing a selection, backups, numbered backup directories, overwrite confirmation, external-change detection |
| Editing | Cut/copy/paste, undo/redo, indentation, comments, justification, hard wrap, word completion, anchors, macros |
| Navigation | Multiple buffers, file browser, browser search, go-to-directory, unambiguous filename completion |
| External tools | Command output insertion, buffer/selection pipes, command cancellation, speller, formatter, linter |
| Configuration | `include`, `set`/`unset`, `bind`/`unbind`, command expansions, syntax/header/magic/color/icolor, multiline rules, partial `extendsyntax` |
| State | Search/replace/command history, cursor positions and anchors, advisory lock header, emergency saves on SIGHUP/SIGTERM |

Runo also implements view/restricted/operating-directory controls. These
are editor behavior settings, not an operating-system security boundary.

## Known differences

- **UTF-8 only.** Invalid byte sequences are rejected; arbitrary-byte editing
  as supported by nano is not implemented.
- **English interface.** There is no localization. Help text is written for
  Runo, and browser layout, scrollbar, messages, and screen details differ.
- **Partial nanorc support.** Unknown commands/options are reported as errors.
  Search, replacement, highlighting, and quote recognition use the system
  libc's POSIX regular expressions. Loading system nano definitions can
  expose unsupported directives.
- **Lock conflicts.** Runo warns about an existing lock; nano's complete
  owner/stale-lock dialog is not implemented.
- **Linter output.** Diagnostics open in another buffer, without interactive
  diagnostic-by-diagnostic navigation.
- **Filename completion.** Only an unambiguous match completes; there is no
  candidate list.
- **Editing edge cases.** Anchor behavior during complex cuts/undo, some undo
  grouping, and selection movement do not reproduce every nano detail.
- **Platforms.** Windows, macOS, and musl-based Linux builds are not currently
  supported or tested.

## Verification scope

The current suite has 17 Rust tests and 25 pseudo-terminal scenarios. It
covers editing-history invariants (including 1,800 randomized operations),
Unicode, line endings, POSIX search, syntax state, navigation, configuration,
file operations, command cancellation, and emergency saves.

There is no complete differential test matrix against nano 8.2. Earlier
development notes mention spot checks against nano 9.2; those cannot establish
8.2 parity. When reporting a compatibility issue, include the nano version,
terminal, configuration, exact keystrokes, and expected/actual outcome.

## References

- [GNU nano v8 manual](https://www.nano-editor.org/dist/v8/nano.html)
- [nanorc v8 reference](https://www.nano-editor.org/dist/v8/nanorc.5.html)

The v8 documentation pages can change within that release series; they are
not necessarily an immutable reference for exactly version 8.2.
