"""Assemble a Tauri portable directory without Electron, Node or Chromium."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('workers', type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    output = args.output.resolve()
    if output.exists():
        raise FileExistsError('Choose a new output directory')
    output.mkdir(parents=True)
    version = json.loads((root / 'package.json').read_text(encoding='utf-8'))['version']
    shutil.copy2(root / 'apps/tauri/src-tauri/target/release/nioh3-studio.exe', output / 'Nioh3Studio.exe')
    (output / 'worker').mkdir()
    for name in ('nioh3-search-worker.exe', 'nioh3-protected-worker.exe'):
        shutil.copy2(args.workers / name, output / 'worker' / name)
    (output / 'packages/contracts').mkdir(parents=True)
    for path in (root / 'packages/contracts').glob('*.schema.json'):
        shutil.copy2(path, output / 'packages/contracts' / path.name)
    shutil.copytree(args.workers / 'licenses', output / 'licenses')
    shutil.copy2(root / 'third_party/nioh_savefile_decrypt/LICENSE', output / 'licenses/Nioh-Savedata-Decryption-Tool-LICENSE')
    javascript, pending = [], list(json.loads((root / 'package.json').read_text(encoding='utf-8'))['dependencies']) + ['@tauri-apps/api']
    visited = set()
    while pending:
        name = pending.pop()
        if name in visited:
            continue
        visited.add(name)
        source = root / 'node_modules' / name
        info = json.loads((source / 'package.json').read_text(encoding='utf-8'))
        destination = output / 'licenses/javascript' / (name.replace('/', '_') + '-' + info['version'])
        notices = []
        for path in source.iterdir():
            if path.is_file() and path.name.lower().startswith(('license', 'copying', 'notice')):
                destination.mkdir(parents=True, exist_ok=True)
                shutil.copy2(path, destination / path.name)
                notices.append((destination / path.name).relative_to(output).as_posix())
        if not notices:
            raise ValueError('JavaScript license missing: ' + name)
        javascript.append({'name': name, 'version': info['version'], 'license': info.get('license'), 'notices': notices})
        pending.extend(info.get('dependencies', {}))
    metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--manifest-path', str(root / 'apps/tauri/src-tauri/Cargo.toml'), '--format-version', '1', '--locked', '--filter-platform', 'x86_64-pc-windows-msvc'], text=True, encoding='utf-8'))
    dependencies = []
    used = {node['id'] for node in metadata['resolve']['nodes']}
    for package in metadata['packages']:
        if package['id'] not in used or package['source'] is None:
            continue
        source = Path(package['manifest_path']).parent
        destination = output / 'licenses/rust' / f"{package['name']}-{package['version']}"
        notices = []
        for path in source.iterdir():
            if path.is_file() and path.name.lower().startswith(('license', 'copying', 'notice')):
                destination.mkdir(parents=True, exist_ok=True)
                shutil.copy2(path, destination / path.name)
                notices.append((destination / path.name).relative_to(output).as_posix())
        if not notices:
            supplement = root / 'third_party/rust-notices' / f"{package['name']}-{package['version']}"
            if not supplement.is_dir():
                raise ValueError('Rust license missing: ' + package['name'])
            shutil.copytree(supplement, destination)
            notices = [p.relative_to(output).as_posix() for p in destination.iterdir() if p.is_file()]
        dependencies.append({'name': package['name'], 'version': package['version'], 'license': package['license'], 'source': f"https://crates.io/api/v1/crates/{package['name']}/{package['version']}/download", 'notices': notices})
    (output / 'dependency-manifest.json').write_text(json.dumps({'javascript': javascript, 'rust': dependencies, 'pythonBuildEnvironment': json.loads((args.workers / 'python-build-environment.json').read_text(encoding='utf-8'))}, indent=2) + '\n', encoding='utf-8')
    (output / 'README.txt').write_text('Nioh 3 Studio ' + version + '\n\nExtract the complete archive, then run Nioh3Studio.exe.\nThe shared Microsoft Edge WebView2 Runtime is required.\nNo Python, Node, Electron or Cheat Engine installation is required.\nKeep the worker and packages folders beside the EXE.\n', encoding='utf-8')
    files = []
    for path in sorted(output.rglob('*')):
        if path.is_file():
            files.append({'path': path.relative_to(output).as_posix(), 'size': path.stat().st_size, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()})
    git = lambda *args: subprocess.check_output(['git', *args], cwd=root, text=True, encoding='utf-8').strip()
    manifest = {'schema': 'nioh3-tauri-manifest/v1', 'version': version, 'git': {'commit': git('rev-parse', 'HEAD'), 'dirty': bool(git('status', '--porcelain'))}, 'files': files}
    (output / 'build-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    report = {'path': str(output), 'version': version, 'fileCount': len(files) + 1, 'installedBytes': sum(f['size'] for f in files), 'exeBytes': (output / 'Nioh3Studio.exe').stat().st_size}
    print(json.dumps(report))


if __name__ == '__main__':
    main()
