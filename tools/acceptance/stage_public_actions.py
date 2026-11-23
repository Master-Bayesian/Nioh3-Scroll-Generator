"""Collect previously sealed public scenes for a local desktop acceptance build."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from verify_loadout_release import copy_verified_scene, seal, verify_scene_directory


def stage(scenes, kernels, app_commit, output, replayed):
    headers = {hashlib.sha256(path.read_bytes()).hexdigest(): path for path in kernels}
    evidence = []
    with tempfile.TemporaryDirectory(dir=output.parent) as tmp:
        staged = Path(tmp) / "loadout"
        staged.mkdir()
        prepared = [(scene, None, None) for scene in scenes]
        prepared.extend(replayed)
        for scene, search, build in prepared:
            contract = json.loads((scene / "runtime-contract.json").read_bytes())
            target = staged / scene.name
            if search is None:
                before = verify_scene_directory(scene)
                copy_verified_scene(scene, target, before)
                header = headers[contract["kernel_layout"]["generated_main_sha256"]]
            else:
                shutil.copytree(scene, target)
                # Some accepted development searches used a newer plan/binary than
                # the original seal. Compare the actual replayed payload before sealing.
                for f in target.iterdir():
                    if f.read_bytes() != (search / "bundle" / f.name).read_bytes():
                        raise ValueError(f"Search did not use {scene / f.name}")
                for backend in ("cpu", "cuda"):
                    result = json.loads(
                        (search / backend / "SEARCH_RESULT.json").read_bytes()
                    )
                    if not result["complete"] or not result["progress"]["answered"]:
                        raise ValueError(f"Incomplete {backend} reference for {scene}")
                header = build / "main_gen.inc"
                if (target / "main_cpu.dll").read_bytes() != (
                    build / "main_cpu.dll"
                ).read_bytes():
                    raise ValueError(
                        f"CPU binary differs from the compiled pair for {scene}"
                    )
                shutil.copy2(build / "main_device.dll", target)
            seal(target, header, app_commit, contract["native_source_commit"])
            after = verify_scene_directory(target, app_commit)
            if not after["layout"]["empty_actor_buffers_and_seeds"]:
                raise ValueError(f"Private actor data in {scene}")
            if search is None and before["files"] != after["files"]:
                raise ValueError(f"Runtime payload changed for {scene}")
            evidence.append(
                {
                    "scene": target.name,
                    "source": str(scene),
                    "replayed_search": str(search) if search else None,
                    "rebuilt_gpu_changed": search is not None
                    and (scene / "main_device.dll").read_bytes()
                    != (target / "main_device.dll").read_bytes(),
                    "build_contract": after["contract_sha256"],
                }
            )
        if output.exists():
            raise FileExistsError(output)
        staged.rename(output)
    return evidence


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--scene", type=Path, action="append", required=True)
    p.add_argument("--kernel", type=Path, action="append", required=True)
    p.add_argument(
        "--replayed",
        type=Path,
        nargs=3,
        action="append",
        default=[],
        metavar=("PUBLIC_SCENE", "ACCEPTED_SEARCH", "MATCHED_KERNEL_BUILD"),
    )
    p.add_argument("--app-commit", required=True)
    p.add_argument("--output", type=Path, required=True)
    args = p.parse_args()
    print(
        json.dumps(
            stage(args.scene, args.kernel, args.app_commit, args.output, args.replayed),
            indent=2,
        )
    )
