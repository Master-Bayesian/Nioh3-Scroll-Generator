"""Project the existing reviewed armor model into a read-only planner asset.

Run through run_python_tests.ps1 with --research-root pointing to the existing
armor-remodel research checkout. No game access or new parameter inference.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
from pathlib import Path
import sys


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--research-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    source = args.research_root / "tools/armor_remodel_batch_v202.py"
    spec = importlib.util.spec_from_file_location("reviewed_armor_model", source)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    model = module.ArmorModel()
    validation = model.validate_live()
    asset = {
        "schema": "nioh3-loadout-armor-requirements/v1",
        "game_version": "2.0.2.0",
        "provenance": model.provenance,
        "validation": validation,
        "stage_caps": list(module.STAGE_CAPS.values()),
        "requirement_coefficients": [row[0] for row in model.curve],
        "items": {str(key): list(item.requirements) for key, item in sorted(model.armor.items())},
    }
    args.output.write_text(json.dumps(asset, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"Projected {len(model.armor)} armor rows; existing native validation replayed.")


if __name__ == "__main__":
    main()
