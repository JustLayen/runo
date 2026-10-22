# License review: MIT/Runo 0.1.0

Review date: **2026-10-22**. Project license: [MIT](LICENSE).

## Application source

MIT/Runo is an independent clean-room implementation of a terminal text editor 
compatible with the user experience and interface of GNU nano. It is written from
scratch in Rust based on public documentation and observed CLI behavior. It does 
not incorporate or derive from GNU nano source code.

## Cargo dependencies

The locked dependency graph for **`x86_64-unknown-linux-gnu`** contains
**23 third-party packages**. Every package offers MIT at the crate level,
and MIT is selected where those licenses offer alternatives. There are also
**file-level Unicode and CC0 terms**; the complete dependency inventory is
not accurately described as "MIT only". The check reads the declared license
expressions and upstream license files of the versions in `Cargo.lock`.

The original notices and SHA-256 hashes of their upstream files are
preserved in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

To reproduce the check:

```sh
cargo fetch --locked
python3 scripts/audit_licenses.py
```

The script uses `cargo metadata --offline --locked --filter-platform`.
Dependencies must already be available in Cargo's cache. It accepts only
an explicit set of license expressions offering MIT, checks for a license
text, and compares the resulting inventory with the checked-in notices.
An unknown expression, changed notice, or unreviewed nested license file
fails the check. Reviewed file-level exceptions are pinned to their package
versions so upgrades prompt another review.

After updating dependencies, review the upstream licensing before refreshing:

```sh
python3 scripts/audit_licenses.py --write
python3 scripts/audit_licenses.py
```

`signal-hook` uses the legacy alternative-license spelling `Apache-2.0/MIT`
and includes a separate `LICENSE-MIT` file.

### File-level exceptions

- `regex-syntax 0.8.11`: `src/unicode_tables/LICENSE-UNICODE` covers the
  Unicode data tables. It requires preservation of the copyright and
  permission notice, which is included verbatim in the dependency notices.
- `rustix 0.38.44`: the header of `src/backend/linux_raw/vdso.rs` identifies
  a translation of Linux's vDSO parser under CC0-1.0. The original licensing
  header and a hash of the complete upstream file are preserved. The CC0
  dedication is permissive and does not require relicensing Runo.

The inventory does not cover dependencies enabled only for other platforms
or arbitrary feature combinations. A package appearing in `Cargo.lock` does
not necessarily mean it is compiled for the supported Linux target. Root
crate license declarations and this additional inspection do not establish
the complete authorship history of every upstream file.

## Runtime and optional external components

- The inspected release executable links to the system C library, loader,
  and GCC runtime (`libc.so.6`, `ld-linux-x86-64.so.2`, `libgcc_s.so.1`).
  GNU libc has its own LGPL terms; the GCC runtime has its own terms and
  Runtime Library Exception. These system libraries are not bundled.
- Rust's standard library, toolchain, and runtime components are outside
  the Cargo inventory and retain their own licensing terms.
- `/bin/sh`, `file`, spell checkers, formatters, and linters are external
  programs, not bundled dependencies.
- `.nanorc` definitions are read from the user's system. The example's
  `/usr/share/nano/*.nanorc` include is optional; no such definitions are
  distributed by this project. External files retain their own licenses.

Source and binary distributions should retain `LICENSE` and
`THIRD_PARTY_NOTICES.md`. The release packaging script includes both.
Redistributing additional runtime libraries or syntax files requires
accounting for the terms of those additional files.
