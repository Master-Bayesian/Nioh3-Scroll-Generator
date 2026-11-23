"""Assemble a Tauri portable directory without Electron, Node or Chromium.

It needs the staged Rust workers plus their `worker-backend.json`, stages the
helper DLLs and generation tables the workers resolve from `data_root.parent()`,
and records the worker build environment.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

# Both the command-line packager and imported tests resolve the same project module.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from tools.verify_tauri_native_dependencies import verify_runtime_native_dependencies
from tools.verify_loadout_release import CONTRACT, POOLED_FILES, POOL_ROOT, identity, verify_scene_directory


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

# The license inventory covers every crate the package ships. The worker crates
# are separate workspaces, so their graphs are listed explicitly.
LICENSED_CRATES = (
    'apps/tauri/src-tauri/Cargo.toml',
    'apps/launcher/Cargo.toml',
    'crates/nioh3-worker/Cargo.toml',
    'crates/nioh3-protected/Cargo.toml',
)


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


def stage_absorption_variants(scenes: Path, *, followed_by_defense: bool = False) -> list[dict]:
    """The same sealed single contact can score any supported immediate 312 grant."""
    variants = []
    ending, condition, event = (
        ("-absorb-defense", "absorption_then_defense_damage", "absorption_then_defense")
        if followed_by_defense else ("-absorb", "amrita_absorption_damage", "amrita_absorption")
    )
    for source in sorted(scenes.glob("*/bundle.json")):
        descriptor = json.loads(source.read_bytes())
        old_condition = (descriptor.get("scene") or {}).get("buff")
        suffix = {"ninja_evade_damage": "-evade", "samurai_deflect_damage": "-deflect-damage"}.get(old_condition)
        if suffix is None or descriptor.get("diagnostic") or not descriptor.get("resource_action"):
            continue
        original = source.parent
        verify_scene_directory(original)
        plan = json.loads((original / "event_plan.json").read_bytes())
        stem = descriptor["id"].removesuffix(suffix)
        if len(stem) + len(ending) > 64:
            stem = stem[:min(42, 64 - len(ending) - 13)] + "-" + hashlib.sha256(stem.encode("ascii")).hexdigest()[:12]
        scene_id = stem + ending
        destination = scenes / scene_id
        descriptor["id"] = scene_id
        descriptor["scene"]["buff"] = condition
        plan["triggered_damage"] = event
        contract = json.loads((original / CONTRACT).read_bytes())
        contract["scene_id"] = scene_id
        changed = {}
        for name, content in (("bundle.json", descriptor), ("event_plan.json", plan)):
            changed[name] = (json.dumps(content, ensure_ascii=False, indent=2) + "\n").encode()
            contract["files"][name] = identity(changed[name])
        if destination.exists():
            verify_scene_directory(destination)
            if json.loads((destination / CONTRACT).read_bytes()) != contract:
                raise ValueError(f"Conflicting staged absorption scene: {scene_id}")
        else:
            # The original check binds the kernel and immutable layout. Recheck the
            # derivative under that same layout before making it visible to the picker.
            with tempfile.TemporaryDirectory(dir=scenes) as temporary:
                stage = Path(temporary)
                shutil.copytree(original, stage, dirs_exist_ok=True)
                for name, raw in changed.items():
                    (stage / name).write_bytes(raw)
                (stage / CONTRACT).write_text(json.dumps(contract, indent=2, sort_keys=True) + "\n", encoding="utf-8")
                verify_scene_directory(stage, scene_id=scene_id)
                stage.rename(destination)
        variants.append(descriptor)
    return variants


def bind_installed_actions(catalog_path: Path, descriptors: list[dict]) -> None:
    """Expose supplied scenes under their installed owner action, including charge variants."""
    catalog = json.loads(catalog_path.read_bytes())
    by_action = {(row["resource"], row["action"], row["parameter_sha256"]): row
                 for row in catalog["selections"]}
    if len(by_action) != len(catalog["selections"]):
        raise ValueError("Duplicate action identity in the packaged catalog")
    changed = False
    for scene in descriptors:
        action = scene.get("resource_action")
        if action is None or scene.get("diagnostic"):
            continue
        row = by_action.get((action["resource"], action["action"], action["parameter_sha256"]))
        if row is None or row["school"] != scene["school"]:
            raise ValueError(f"Scene {scene['id']} has no matching catalog action")
        if scene["id"] in row["scene_ids"]:
            continue
        selected_class = (scene["selection_weapon_class"] if "selection_weapon_class" in scene
                          else scene["weapon_class"])
        if row["weapon_class"] not in (None, selected_class):
            raise ValueError(f"Scene {scene['id']} disagrees with the catalog weapon type")
        row["scene_ids"].append(scene["id"])
        row["plan_ready"] = True
        row["weapon_class"] = selected_class
        row["stance"] = scene["scene"]["stance"]
        if row["attack_id"] is None:
            row["attack_id"] = scene["scene"]["attack_id"]
        changed = True
    if changed:
        catalog_path.write_bytes(json.dumps(catalog, ensure_ascii=False, separators=(",", ":")).encode("utf-8"))


def stage_rust_runtime(root: Path, output: Path, loadout_runtime: Path | None = None) -> dict:
    """Stage the assets a Rust worker resolves from its application root.

    `Engine::load` derives `application_root = data_root.parent()`, so a data
    root of `<runtime>/worker/runtime/data` makes the worker look for helpers
    in `<runtime>/worker/runtime/bin`.
    """

    runtime = output / 'worker' / 'runtime'
    (runtime / 'bin').mkdir(parents=True, exist_ok=True)
    helpers = [
        'nioh3_seed_accelerator.dll',
        'nioh3_effect_preimage_accelerator.dll',
        'nioh3_seed_accelerator.build.json',
        'nioh3_effect_preimage_accelerator.build.json',
    ]
    staged = []
    for name in helpers:
        source = root / 'bin' / name
        if not source.is_file():
            raise FileNotFoundError('Rust worker runtime asset is missing: ' + str(source))
        shutil.copy2(source, runtime / 'bin' / name)
        staged.append('worker/runtime/bin/' + name)
    data_source = root / 'data'
    data_destination = runtime / 'data'
    shutil.copytree(data_source, data_destination)
    if loadout_runtime is not None:
        supplied = loadout_runtime.resolve(strict=True)
        sources = ([supplied] if (supplied / 'bundle.json').is_file() else
                   sorted(path for path in supplied.iterdir() if path.name != 'catalog.json'))
        if not sources:
            raise ValueError("Loadout runtime contains no scenes")
    else:
        sources = []
    descriptors = []
    for source in sources:
        descriptor = json.loads((source / "bundle.json").read_bytes())
        scene_id = descriptor["id"]
        if not isinstance(scene_id, str) or not scene_id.isascii() or not scene_id.replace("-", "").isalnum() or len(scene_id) > 64:
            raise ValueError("Invalid loadout scene ID")
        if source != supplied and source.name != scene_id:
            raise ValueError("Loadout directory and descriptor disagree")
        target = data_destination / "loadout" / scene_id
        if target.exists() and descriptor != json.loads((target / "bundle.json").read_bytes()):
            raise ValueError("Loadout runtime does not match the packaged scene")
        # A descriptor only identifies the menu entry. It does not bind address
        # spaces, source programs or the two native binaries. Validate the whole
        # immutable bundle, then replace the scene rather than merge leftovers.
        verified = verify_scene_directory(source, scene_id=target.name)
        staged_scene = target.with_name(target.name + ".staging")
        shutil.copytree(source, staged_scene)
        # An installed scene can resolve sealed assets from its runtime's shared pool.
        # Carry those same bytes into the destination pool when repackaging it.
        for name in POOLED_FILES:
            if not (staged_scene / name).is_file():
                key = Path(POOL_ROOT) / verified["files"][name]["sha256"] / name
                destination = data_destination / key
                if not destination.is_file():
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(source.parent.parent / key, destination)
        verify_scene_directory(staged_scene, scene_id=target.name)
        if target.exists():
            shutil.rmtree(target)
        staged_scene.rename(target)
        if descriptor.get("resource_action") and not descriptor.get("diagnostic"):
            descriptors.append(descriptor)
    scenes = data_destination / "loadout"
    descriptors.extend(stage_absorption_variants(scenes))
    descriptors.extend(stage_absorption_variants(scenes, followed_by_defense=True))
    if descriptors:
        bind_installed_actions(scenes / "catalog.json", descriptors)
    if scenes.is_dir():
        duplicates = {}
        for scene in sorted(scenes.iterdir()):
            if scene.is_dir():
                for name in POOLED_FILES:
                    source = scene / name
                    if source.is_file():
                        digest = hashlib.sha256(source.read_bytes()).hexdigest()
                        duplicates.setdefault((digest, name), []).append(source)
        for (digest, name), paths in duplicates.items():
            if len(paths) > 1:
                pool = data_destination / POOL_ROOT / digest
                pool.mkdir(parents=True, exist_ok=True)
                paths[0].replace(pool / name)
                for duplicate in paths[1:]:
                    duplicate.unlink()
        for scene in sorted(scenes.iterdir()):
            if scene.is_dir():
                verify_scene_directory(scene)
    for path in sorted(data_destination.rglob('*')):
        if path.is_file():
            staged.append(path.relative_to(output).as_posix())
    return {
        'applicationRoot': 'worker/runtime',
        'dataRoot': 'worker/runtime/data',
        'contractDir': 'packages/contracts',
        'staged': staged,
    }


def worker_environment(workers: Path) -> dict:
    """The recorded Rust worker build environment."""

    manifest_path = workers / 'worker-backend.json'
    if not manifest_path.is_file():
        raise FileNotFoundError(
            'the package needs worker-backend.json; run '
            'tools/stage_rust_workers.py before packaging'
        )
    recorded = json.loads(manifest_path.read_text(encoding='utf-8'))
    if recorded.get('backend') != 'rust':
        raise ValueError('worker-backend.json does not describe a rust backend')
    return recorded


def assemble_layout(root: Path, output: Path, workers: Path, loadout_runtime: Path | None = None) -> dict:
    """Copy the runtime into the portable layout."""

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
    # The packaged host launches the workers this file declares.
    shutil.copy2(workers / 'worker-backend.json', output / 'worker' / 'worker-backend.json')
    return stage_rust_runtime(root, output, loadout_runtime)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('workers', type=Path)
    parser.add_argument('--loadout-runtime', type=Path, help='one sealed PC202 scene or a directory of sealed scenes')
    parser.add_argument('--root', type=Path, help='override the repository root (tests)')
    args = parser.parse_args()
    root = (args.root or Path(__file__).resolve().parents[1]).resolve()
    output = args.output.resolve()
    if output.exists():
        raise FileExistsError('Choose a new output directory')
    version = json.loads((root / 'package.json').read_text(encoding='utf-8'))['version']
    worker_manifest = worker_environment(args.workers)
    layout = assemble_layout(root, output, args.workers, args.loadout_runtime)
    native = verify_runtime_native_dependencies(output)
    (output / 'native-dependencies.json').write_text(json.dumps(native, indent=2) + '\n', encoding='utf-8')
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
        rust_dependency_packages(root, LICENSED_CRATES),
    )
    manifest = {
        'javascript': javascript,
        'rust': dependencies,
        'workerBuildEnvironment': worker_manifest,
        'workerRuntimeLayout': layout,
    }
    (output / 'dependency-manifest.json').write_text(
        json.dumps(manifest, indent=2) + '\n', encoding='utf-8'
    )
    (output / 'README.txt').write_text('Nioh 3 Studio ' + version + '\n\nThis directory is the verified internal runtime and signed updater payload.\nPlayers launch the outer Nioh3Studio-' + version + '-win-x64.exe directly; no installation or manual extraction is needed.\nThe shared Microsoft Edge WebView2 Runtime is required.\nNo Python, Node, Electron or Cheat Engine installation is required.\nKeep this internal runtime intact.\n', encoding='utf-8')
    files = []
    for path in sorted(output.rglob('*')):
        if path.is_file():
            files.append({'path': path.relative_to(output).as_posix(), 'size': path.stat().st_size, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()})
    def git(*args):
        return subprocess.check_output(['git', *args], cwd=root, text=True, encoding='utf-8').strip()
    manifest = {'schema': 'nioh3-tauri-manifest/v1', 'version': version, 'git': {'commit': git('rev-parse', 'HEAD'), 'dirty': bool(git('status', '--porcelain'))}, 'files': files}
    (output / 'build-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    report = {'path': str(output), 'version': version, 'fileCount': len(files) + 1, 'installedBytes': sum(f['size'] for f in files), 'exeBytes': (output / 'Nioh3Studio.exe').stat().st_size}
    print(json.dumps(report))


if __name__ == '__main__':
    main()
