"""Stage a retained-memory first-read WebView fixture using the normal catalog binder."""

import argparse
from hashlib import sha256
import json
from pathlib import Path
import shutil
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from tools.package_tauri import bind_installed_actions


def stage(session, root, scene_id):
    data = root / "data"
    descriptors = [
        json.loads(p.read_bytes()) for p in (data / "loadout").glob("*/bundle.json")
    ]
    bind_installed_actions(data / "loadout/catalog.json", descriptors)
    fixture = root / "webview"
    fixture.mkdir(exist_ok=True)
    shutil.copytree(data, fixture / "host-data", dirs_exist_ok=True)
    relative = Path(
        "local/KoeiTecmo/NIOH3/Savedata/76561198408378493/SAVEDATA01/SAVEDATA.BIN"
    )
    target = fixture / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(session / "samurai-high/SAVEDATA.BIN", target)
    shutil.copyfile(
        session / "samurai-high/read/ordinary-read-memory.json", fixture / "memory.json"
    )
    reference = root / "NATIVE_APP_REFERENCE.json"
    if not reference.is_file():
        raise ValueError(
            "Compare both production searches with the native oracle first"
        )
    doc = dict(
        scene_id=scene_id,
        fixture=relative.as_posix(),
        source_sha256=sha256(target.read_bytes()).hexdigest(),
        standalone_reference=str(reference.resolve()),
        required_pieces=6,
        ranking_only=True,
        manual_refresh=True,
        equipped=dict(total=28707, physical=28707, elemental=0),
        observation_scope="Ordinary first read of the retained Samurai role, all eligible windows land, extra martial effect inactive.",
    )
    (fixture / "FIXTURE.json").write_text(json.dumps(doc, indent=2), encoding="utf-8")


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("session", type=Path)
    p.add_argument("root", type=Path)
    p.add_argument("scene_id")
    args = p.parse_args()
    stage(args.session, args.root, args.scene_id)
