# Preparing a GitHub release

The initial release version is `0.1.0`; use the Git tag `v0.1.0`. This guide
describes maintainer actions; packaging the project does not publish it.

## Repository setup

Create a GitHub repository with a short description such as:

> MIT-licensed terminal text editor in Rust with GNU nano-style controls.

Suggested topics: `rust`, `terminal`, `text-editor`, `nano`, `mit-license`.
The repository URL can then be added as `package.repository` in `Cargo.toml`.
`Cargo.lock` belongs in version control; `target/`, temporary saves, and
Python caches do not. The Cargo package uses an explicit file allowlist.

## Release checks

1. Update `Cargo.toml` and `CHANGELOG.md` for the intended version. Keep
   README versions and compatibility notes current; let Cargo update the
   root package entry in `Cargo.lock` when the version changes.
2. Run the checks from [CONTRIBUTING.md](../CONTRIBUTING.md). The Linux CI
   also tests Rust 1.82 and checks the source package.
3. Check the source archive with `cargo package --locked`. Its contents
   can be previewed using `cargo package --locked --list`.
4. Build and package the supported binary:

   ```sh
   cargo build --release --locked --target x86_64-unknown-linux-gnu
   python3 scripts/package_release.py
   ```

   Packaging requires Python 3.11 or newer. The script creates
   `target/dist/runo-0.1.0-x86_64-unknown-linux-gnu.tar.gz`
   and a matching `.sha256` file. Each archive contains the executable,
   README files, example configuration, MIT license, dependency notices,
   and license-review notes. It uses the actual host target and the version
   embedded in the executable to catch accidental cross-target/version mixes.

5. After committing and tagging the intended revision, create a GitHub
   release for the tag and attach both files. Summarize the changelog and
   link the known limitations. Stable CI also produces a downloadable
   binary-bundle artifact for its tested revision.

The binary is dynamically linked. Build on an appropriately old glibc
baseline for the Linux distributions you intend to support, and state that
baseline in the release description. The CI bundle is built on Ubuntu 24.04;
do not describe it as a universally portable or static Linux binary.

Retain `LICENSE` and `THIRD_PARTY_NOTICES.md` when redistributing binaries.
System libraries and nano syntax definitions are not included in the bundle.
