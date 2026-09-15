"""Assemble a Tauri portable directory without Electron, Node or Chromium.

The default worker backend is the Rust worker (`rust`), which is the shipped
graph: it requires the staged Rust workers plus their `worker-backend.json`,
stages the helper DLLs and generation tables the Rust workers resolve from
`data_root.parent().parent()`, records an explicit worker build environment, and
refuses a workers directory that still contains PyInstaller output.
`--worker-backend python` keeps the previous PyInstaller graph for development,
parity and oracle use, and is no longer the shipped product.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

WORKER_BACKENDS = ('python', 'rust')


def cargo_target_dir(root: Path, workspace: str) -> Path:
    """The directory holding one workspace's release binaries.

    Cargo shares an explicit `CARGO_TARGET_DIR` across every workspace, so the
    host and launcher EXEs must be resolved from there when it is set (a
    candidate build keeps its target tree off the checkout volume); otherwise
    each workspace keeps its own `target/` directory.
    """

    configured = os.environ.get('CARGO_TARGET_DIR', '').strip()
    if configured:
        return Path(configured) / 'release'
    return root / workspace / 'target' / 'release'

# The Rust license inventory has to cover every crate the package actually
# ships, not only the two whose binaries sit at the top level. The worker crates
# are separate workspaces, so their dependency graphs are enumerated explicitly
# for the rust backend instead of being covered by accident through the host.
HOST_CRATES = ('apps/tauri/src-tauri/Cargo.toml', 'apps/launcher/Cargo.toml')
WORKER_CRATES = ('crates/nioh3-worker/Cargo.toml', 'crates/nioh3-protected/Cargo.toml')


def rust_license_manifests(backend: str) -> tuple:
    """The manifests whose locked graphs the package must attribute.

    The worker crates are standalone workspaces, so only the Rust graph ships
    them; the Python graph keeps the host-only set it has always used.
    """

    return HOST_CRATES + (WORKER_CRATES if backend == 'rust' else ())


def rust_dependency_packages(root: Path, manifests) -> dict:
    """Every third-party package the given manifests lock in for this target."""

    packages = {}
    for crate in manifests:
        metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--manifest-path', str(root / crate), '--format-version', '1', '--locked', '--filter-platform', 'x86_64-pc-windows-msvc'], text=True, encoding='utf-8'))
        used = {node['id'] for node in metadata['resolve']['nodes']}
        for package in metadata['packages']:
            if package['id'] in used and package['source'] is not None:
                packages[package['id']] = package
    return packages


def collect_rust_licenses(root: Path, output: Path, packages: dict) -> list:
    """Copy each dependency's own notices and describe them for the manifest."""

    dependencies = []
    for package in packages.values():
        if package['source'] is None:
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
    return dependencies


def dependency_manifest(
    backend: str,
    javascript: list,
    rust: list,
    *,
    worker_manifest: dict | None = None,
    worker_layout: dict | None = None,
    python_environment: dict | None = None,
) -> dict:
    """Build dependency-manifest.json for one worker backend.

    The Rust graph ships no Python interpreter, so it records its own build
    environment and states explicitly that no Python build environment exists;
    it must never carry the PyInstaller one by omission.
    """

    manifest = {'javascript': javascript, 'rust': rust}
    if backend == 'rust':
        manifest['workerBuildEnvironment'] = worker_manifest
        manifest['workerRuntimeLayout'] = worker_layout
        manifest['pythonBuildEnvironment'] = None
    else:
        manifest['pythonBuildEnvironment'] = python_environment
    return manifest


def stage_rust_runtime(root: Path, output: Path) -> dict:
    """Stage the assets a Rust worker resolves from its application root.

    `Engine::load` derives `application_root = data_root.parent().parent()`, so
    a data root of `<runtime>/worker/runtime/nioh3_scroll_editor/data` makes the
    worker look for helpers in `<runtime>/worker/runtime/bin`.
    """

    runtime = output / 'worker' / 'runtime'
    (runtime / 'bin').mkdir(parents=True, exist_ok=True)
    helpers = [
        'nioh3_seed_accelerator.dll',
        'nioh3_effect_preimage_accelerator.dll',
        'nioh3_seed_accelerator.build.json',
    ]
    staged = []
    for name in helpers:
        source = root / 'bin' / name
        if not source.is_file():
            raise FileNotFoundError('Rust worker runtime asset is missing: ' + str(source))
        shutil.copy2(source, runtime / 'bin' / name)
        staged.append('worker/runtime/bin/' + name)
    data_source = root / 'nioh3_scroll_editor' / 'data'
    data_destination = runtime / 'nioh3_scroll_editor' / 'data'
    shutil.copytree(data_source, data_destination)
    for path in sorted(data_destination.rglob('*')):
        if path.is_file():
            staged.append(path.relative_to(output).as_posix())
    return {
        'applicationRoot': 'worker/runtime',
        'dataRoot': 'worker/runtime/nioh3_scroll_editor/data',
        'contractDir': 'packages/contracts',
        'staged': staged,
    }


def worker_environment(workers: Path, backend: str) -> tuple[dict, dict]:
    """The explicit worker build environment for the chosen backend."""

    if backend == 'rust':
        manifest_path = workers / 'worker-backend.json'
        if not manifest_path.is_file():
            raise FileNotFoundError(
                'the Rust graph needs worker-backend.json; run '
                'tools/stage_rust_workers.py before packaging'
            )
        if (workers / 'python-build-environment.json').is_file():
            raise ValueError(
                'refusing to package a Rust worker backend beside PyInstaller output'
            )
        recorded = json.loads(manifest_path.read_text(encoding='utf-8'))
        if recorded.get('backend') != 'rust':
            raise ValueError('worker-backend.json does not describe a rust backend')
        return recorded, {}
    return {}, json.loads(
        (workers / 'python-build-environment.json').read_text(encoding='utf-8')
    )


def assemble_layout(
    root: Path, output: Path, workers: Path, backend: str
) -> dict:
    """Copy the runtime into the portable layout for one worker backend."""

    output.mkdir(parents=True)
    shutil.copy2(
        cargo_target_dir(root, 'apps/tauri/src-tauri') / 'nioh3-studio.exe',
        output / 'Nioh3Studio.exe',
    )
    (output / 'launcher').mkdir()
    shutil.copy2(
        cargo_target_dir(root, 'apps/launcher') / 'Nioh3Launcher.exe',
        output / 'launcher/Nioh3Launcher.exe',
    )
    (output / 'worker').mkdir()
    for name in ('nioh3-search-worker.exe', 'nioh3-protected-worker.exe'):
        shutil.copy2(workers / name, output / 'worker' / name)
    (output / 'packages/contracts').mkdir(parents=True)
    for path in (root / 'packages/contracts').glob('*.schema.json'):
        shutil.copy2(path, output / 'packages/contracts' / path.name)
    shutil.copytree(workers / 'licenses', output / 'licenses')
    shutil.copy2(
        root / 'third_party/nioh_savefile_decrypt/LICENSE',
        output / 'licenses/Nioh-Savedata-Decryption-Tool-LICENSE',
    )
    if backend == 'rust':
        # The packaged host selects its worker graph from this file, so it has to
        # travel inside the package at the path the broker reads. Without it the
        # broker would keep the shipped worker and the staged Rust binary would be
        # present but never launched.
        shutil.copy2(workers / 'worker-backend.json', output / 'worker' / 'worker-backend.json')
        return stage_rust_runtime(root, output)
    return {}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('workers', type=Path)
    parser.add_argument('--worker-backend', choices=WORKER_BACKENDS, default='rust')
    parser.add_argument('--root', type=Path, help='override the repository root (tests)')
    args = parser.parse_args()
    root = (args.root or Path(__file__).resolve().parents[1]).resolve()
    output = args.output.resolve()
    if output.exists():
        raise FileExistsError('Choose a new output directory')
    version = json.loads((root / 'package.json').read_text(encoding='utf-8'))['version']
    worker_manifest, python_environment = worker_environment(args.workers, args.worker_backend)
    layout = assemble_layout(root, output, args.workers, args.worker_backend)
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
    dependencies = collect_rust_licenses(
        root,
        output,
        rust_dependency_packages(root, rust_license_manifests(args.worker_backend)),
    )
    manifest = dependency_manifest(
        args.worker_backend,
        javascript,
        dependencies,
        worker_manifest=worker_manifest,
        worker_layout=layout,
        python_environment=python_environment,
    )
    (output / 'dependency-manifest.json').write_text(
        json.dumps(manifest, indent=2) + '\n', encoding='utf-8'
    )
    (output / 'README.txt').write_text('Nioh 3 Studio ' + version + '\n\nThis directory is the verified internal runtime and signed updater payload.\nPlayers launch the outer Nioh3Studio-' + version + '-win-x64.exe directly; no installation or manual extraction is needed.\nThe shared Microsoft Edge WebView2 Runtime is required.\nNo Python, Node, Electron or Cheat Engine installation is required.\nKeep this internal runtime intact.\n', encoding='utf-8')
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
