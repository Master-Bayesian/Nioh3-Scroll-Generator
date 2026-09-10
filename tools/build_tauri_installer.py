"""Bundle one verified portable directory as a single Tauri NSIS installer."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
from uuid import uuid4


def resource_map(portable: Path) -> dict[str, str]:
    files = {}
    for source in sorted(portable.rglob('*')):
        if not source.is_file() or source.name == 'Nioh3Studio.exe' and source.parent == portable:
            continue
        files[str(source.resolve())] = source.relative_to(portable).as_posix()
    return files


def bundle_config(portable: Path) -> dict:
    return {
        'mainBinaryName': 'Nioh3Studio',
        'bundle': {
            'active': True,
            'targets': ['nsis'],
            'publisher': 'MasterBayesian and Saber_Li',
            'homepage': 'https://github.com/Master-Bayesian/Nioh3-Scroll-Generator',
            'shortDescription': 'Nioh 3 scroll search and editing studio',
            'longDescription': 'Search, inspect, add, edit and back up Nioh 3 scrolls.',
            'resources': resource_map(portable),
            'windows': {
                'webviewInstallMode': {'type': 'downloadBootstrapper'},
                'nsis': {
                    'installMode': 'currentUser',
                    'languages': ['SimpChinese', 'English', 'Japanese'],
                    'compression': 'lzma',
                },
            },
        },
    }


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('portable', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    portable = args.portable.resolve(strict=True)
    output = args.output.resolve()
    manifest = json.loads((portable / 'build-manifest.json').read_text(encoding='utf-8'))
    version = json.loads((root / 'package.json').read_text(encoding='utf-8'))['version']
    if manifest.get('schema') != 'nioh3-tauri-manifest/v1' or manifest.get('version') != version:
        raise ValueError('Portable manifest version differs from the release source')
    main_binary = portable / 'Nioh3Studio.exe'
    if not main_binary.is_file() or main_binary.read_bytes()[:2] != b'MZ':
        raise ValueError('Portable main executable is missing or invalid')

    cargo_binary = root / 'apps/tauri/src-tauri/target/release/Nioh3Studio.exe'
    shutil.copy2(main_binary, cargo_binary)
    temporary = root / '.codex_tmp' / f'tauri-installer-{uuid4().hex}.json'
    temporary.parent.mkdir(parents=True, exist_ok=True)
    temporary.write_text(json.dumps(bundle_config(portable), indent=2) + '\n', encoding='utf-8')
    bundle_directory = root / 'apps/tauri/src-tauri/target/release/bundle/nsis'
    before = {path.resolve() for path in bundle_directory.glob('*.exe')} if bundle_directory.exists() else set()
    npm = 'npm.cmd' if os.name == 'nt' else 'npm'
    try:
        subprocess.run(
            [npm, 'exec', '--', 'tauri', 'bundle', '--ci', '--bundles', 'nsis', '--config', str(temporary)],
            cwd=root / 'apps/tauri', check=True,
        )
    finally:
        temporary.unlink(missing_ok=True)
    candidates = [path for path in bundle_directory.glob('*.exe') if path.resolve() not in before]
    if len(candidates) != 1:
        # Tauri can overwrite the same versioned output during a deliberate rebuild.
        candidates = sorted(bundle_directory.glob('*.exe'), key=lambda path: path.stat().st_mtime_ns, reverse=True)[:1]
    if len(candidates) != 1:
        raise RuntimeError('Tauri did not produce exactly one NSIS installer')
    output.parent.mkdir(parents=True, exist_ok=True)
    if output.exists():
        raise FileExistsError('Choose a new installer output path')
    shutil.copy2(candidates[0], output)
    if output.read_bytes()[:2] != b'MZ' or output.stat().st_size > 60 * 1024 * 1024:
        output.unlink(missing_ok=True)
        raise ValueError('Installer is invalid or exceeds the 60 MiB release budget')
    digest = sha256(output)
    output.with_suffix(output.suffix + '.sha256').write_text(f'{digest}  {output.name}\n', encoding='ascii')
    print(json.dumps({'installer': str(output), 'size': output.stat().st_size, 'sha256': digest}))


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(f'ERROR: {error}', file=sys.stderr)
        raise
