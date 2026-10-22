#!/usr/bin/env python3
"""Bundle a tested Linux/glibc release executable with documentation and notices.

Build first: cargo build --release --locked --target x86_64-unknown-linux-gnu
Requires Python 3.11+ (tomllib); no third-party Python packages.
"""
import argparse
import hashlib
from pathlib import Path
import subprocess
import sys
import tarfile
import tomllib

ROOT = Path(__file__).resolve().parent.parent
TARGET = 'x86_64-unknown-linux-gnu'
DOCUMENTS = [
    'LICENSE', 'THIRD_PARTY_NOTICES.md', 'LICENSE-AUDIT.md',
    'README.md', 'README.ru.md', 'CHANGELOG.md', 'CONTRIBUTING.md',
    'docs/compatibility.md', 'docs/releasing.md', 'examples/runorc',
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path,
                        default=ROOT / 'target' / TARGET / 'release' / 'runo',
                        help='Path to the tested release executable')
    args = parser.parse_args()
    try:
        rustc = subprocess.run(['rustc', '-vV'], check=True, capture_output=True, text=True)
        if f'host: {TARGET}' not in rustc.stdout.splitlines():
            raise ValueError(f'Packaging is currently supported only on {TARGET}')
        version = tomllib.loads((ROOT / 'Cargo.toml').read_text(encoding='utf-8'))['package']['version']
        binary = args.binary.resolve()
        # The terminal's display version intentionally omits the patch component;
        # the executable also reports the full package version for release tooling.
        result = subprocess.run([str(binary), '--version'], check=True,
                                capture_output=True, text=True)
        if f'Package version: {version}' not in result.stdout.splitlines():
            raise ValueError('Executable/package version mismatch; rebuild the release binary')
        subprocess.run([sys.executable, str(ROOT / 'scripts/audit_licenses.py')], check=True)
        files = [(binary, 'runo')] + [(ROOT / name, name) for name in DOCUMENTS]
        for source, _ in files:
            if not source.is_file():
                raise ValueError(f'Missing release file: {source}')
        name = f'runo-{version}-{TARGET}'
        destination = ROOT / 'target' / 'dist'
        destination.mkdir(parents=True, exist_ok=True)
        archive = destination / f'{name}.tar.gz'
        with tarfile.open(archive, 'w:gz') as bundle:
            for source, relative in files:
                info = bundle.gettarinfo(str(source), arcname=f'{name}/{relative}')
                info.uid = info.gid = 0
                info.uname = info.gname = ''
                info.mode = 0o755 if relative == 'runo' else 0o644
                with source.open('rb') as stream:
                    bundle.addfile(info, stream)
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        checksum = archive.with_name(archive.name + '.sha256')
        checksum.write_text(f'{digest}  {archive.name}\n', encoding='utf-8')
    except (OSError, ValueError, subprocess.CalledProcessError) as exc:
        print(f'Release packaging failed: {exc}', file=sys.stderr)
        if isinstance(exc, subprocess.CalledProcessError) and exc.stderr:
            print(exc.stderr, file=sys.stderr)
        return 1
    print(f'Archive: {archive.relative_to(ROOT)}')
    print(f'Checksum: {checksum.relative_to(ROOT)}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
