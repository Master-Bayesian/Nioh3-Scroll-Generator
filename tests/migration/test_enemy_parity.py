"""Seed-to-roster/Wraith parity using shipped resources and native controls."""

import json
import os
from pathlib import Path
import shutil
import subprocess

import pytest

from nioh3_scroll_editor import auxiliary_generation as ag
from nioh3_scroll_editor.enemy_variant_generation import generate_enemy_variant
from nioh3_scroll_editor.possessed_generation import EnemyStateTables, generate_possessed
from tests.migration.cargo_target import resolved_cargo_target_dir


ROOT = Path(__file__).resolve().parents[2]
DATA = ROOT / "nioh3_scroll_editor/data"
NATIVE = json.loads((ROOT / "tests/fixtures/enemy_states_v201/native_controls.json").read_text())
SEEDS = list(dict.fromkeys(
    [0, 1, 5, 86872488, 156062997, 0xFFFFFFFF]
    + [((i * 2654435761) & 0x0FFFFFFF) or 1 for i in range(1, 513)]
))
CASES = [
    {"seed": seed, "playthrough": playthrough, "variant": variant}
    for playthrough in range(1, 6)
    for seed in (SEEDS if playthrough == 3 else SEEDS[:38])
    for variant in ("solo", "expedition")
]


@pytest.fixture(scope="module")
def rust_preview():
    cargo = shutil.which("cargo")
    assert cargo, "Rust is required for the enemy migration gate"
    env = dict(os.environ)
    env.setdefault("CARGO_TARGET_DIR", resolved_cargo_target_dir("v080-domain"))
    build = subprocess.run(
        [cargo, "build", "--release", "--locked", "--offline", "--manifest-path",
         str(ROOT / "crates/nioh3-data/Cargo.toml"), "--example", "enemy_preview"],
        cwd=ROOT, env=env, capture_output=True, text=True, encoding="utf-8", timeout=180,
    )
    assert build.returncode == 0, build.stderr
    executable = Path(env["CARGO_TARGET_DIR"]) / "release/examples" / (
        "enemy_preview.exe" if os.name == "nt" else "enemy_preview"
    )

    def run(cases, state_path=None, data_root=DATA, *, success=True):
        args = [str(executable), str(data_root)]
        if state_path is not None:
            args.append(str(state_path))
        result = subprocess.run(args, input=json.dumps(cases), cwd=ROOT,
                                capture_output=True, text=True, encoding="utf-8", timeout=60)
        if not success:
            assert result.returncode != 0, "Malformed resources were accepted"
            return result.stderr
        assert result.returncode == 0, result.stderr
        answers = json.loads(result.stdout)
        assert len(answers) == len(cases)
        return answers

    return run


@pytest.fixture(scope="module")
def reference_resources():
    return ag.load_default_auxiliary_generation_tables(), ag.load_default_r4_finalizer_resource()


def python_preview(case, resources, state_tables=None):
    tables, resource = resources
    seed, playthrough, variant = case["seed"], case["playthrough"], case["variant"]
    mode = ag.generate_auxiliary_mode(seed, resource=resource)
    terrain = ag.generate_terrain(seed, mode.value, tables=tables, resource=resource)
    desc = ag.generate_auxiliary_descriptor_flags(seed, mode.value, tables=tables, resource=resource)
    context = {
        "mode": mode.value, "mode_branch": mode.branch_class,
        "mode_draws": mode.random_draws, "mode_row_index": mode.selected_row_index,
        "terrain_row_index": terrain.selected_row_index, "terrain_value": terrain.value,
        "used_filtered_pool": terrain.used_filtered_pool,
        "selector": desc.selector, "flags": list(desc.flags),
        "descriptor_draws": desc.random_draws,
    }
    if desc.selector:
        return {"status": "unsupported_selector", "context": context}
    roster = generate_enemy_variant(seed, playthrough, variant=variant, tables=tables, resource=resource)
    wraith = generate_possessed(roster, tables=state_tables)
    trials = [{"selector": t["selector"], "spawn": t["spawn"], "ticket": t["ticket"],
               "state": t["after"], "draw": t["draw"], "accepted": t["accepted"]}
              for t in wraith.trace if t["reason"] == "source-first-success"]
    return {
        "status": "ok", "context": context,
        "roster": {
            "terrain": roster.terrain, "branch_class": roster.branch_class,
            "state_after_roster": roster.state_after_roster, "parent_draws": roster.parent_draws,
            "waves": [[{
                "wave_index": x.wave_index, "position": x.position,
                "spawn": x.native_spawn_key, "lookup": x.lookup_key,
                "role": x.role, "row": x.source_row_index, "selector": x.selector_class,
                "scratch": x.scratch_rule_key,
            } for x in wave] for wave in roster.waves],
        },
        "wraith": {
            "status": wraith.status,
            "states": [wraith.by_occurrence[(x.wave_index, x.position)] for x in roster.occurrences],
            "source_entry_state": wraith.source_entry_state, "source_entry_draw": wraith.source_entry_draw,
            "final_state": wraith.final_state,
            "final_draws": None if wraith.source_entry_draw is None else wraith.source_entry_draw + len(trials),
            "trials": trials,
        },
    }


def test_seed_to_enemy_chain_matches_retained_product(rust_preview, reference_resources):
    answers = rust_preview(CASES)
    branches, modes, terrains, guarded = set(), set(), set(), 0
    for case, actual in zip(CASES, answers):
        expected = python_preview(case, reference_resources)
        assert actual == expected, f"case={case}\nRust={actual}\nPython={expected}"
        context = expected["context"]
        branches.add(context["mode_branch"])
        modes.add(context["mode"])
        terrains.add(context["terrain_row_index"])
        guarded += expected["status"] == "unsupported_selector"
    assert branches == {0, 1, 2}
    assert len(modes) == 7 and len(terrains) == 20
    assert guarded > 0, "The unsupported-selector branch must be exercised"


def test_native_controls_independent_of_python_implementation(rust_preview):
    cases = [{"seed": c["seed"], "playthrough": 3, "variant": c["variant"]}
             for c in NATIVE["rosters"]]
    cases.append({"seed": 156062997, "playthrough": 3, "variant": "solo"})
    answers = rust_preview(cases)
    for expected, actual in zip(NATIVE["rosters"], answers):
        waves = actual["roster"]["waves"]
        assert [[{k: x[k] for k in ("spawn", "lookup", "role", "selector")} for x in wave]
                for wave in waves] == [
            [{k: x[k] for k in ("spawn", "lookup", "role", "selector")} for x in wave]
            for wave in expected["waves"]
        ]
        assert actual["wraith"]["states"] == [
            "yes" if x["flag"] else "no" for wave in expected["waves"] for x in wave
        ]
        assert actual["wraith"]["source_entry_state"] == NATIVE["run_d_source_entry_state"]
        assert actual["wraith"]["source_entry_draw"] == 35
        if expected["variant"] == "expedition":
            assert [{k: t[k] for k in ("spawn", "ticket", "state", "accepted")}
                    for t in actual["wraith"]["trials"]] == [
                {k: t[k] for k in ("spawn", "ticket", "state", "accepted")}
                for t in NATIVE["run_d_trials"]
            ]
    actual = answers[-1]
    captured = next(c for c in NATIVE["late_controls"] if c["seed"] == 156062997)
    flat = [x for wave in actual["roster"]["waves"] for x in wave]
    assert {x["spawn"]: (x["lookup"], x["role"]) for x in flat} == {
        x["spawn"]: (x["lookup"], int.from_bytes(bytes.fromhex(x["raw_hex"])[8:12], "little"))
        for x in captured["records"]
    }
    assert [x["spawn"] for x, state in zip(flat, actual["wraith"]["states"]) if state == "yes"] == [0xF40]


def test_partial_capture_stays_unknown(rust_preview, reference_resources, tmp_path):
    original = DATA / "enemy_states/pc_v2_01/native_tables.json"
    content = json.loads(original.read_text())
    content["enemy_index_complete"] = False
    content["eligibility_by_lookup"] = {}
    path = tmp_path / "partial.json"
    path.write_text(json.dumps(content), encoding="utf-8")
    cases = [{"seed": s, "playthrough": 3, "variant": "solo"} for s in (86872488, 156062997)]
    tables = EnemyStateTables.load(path)
    for case, actual in zip(cases, rust_preview(cases, path)):
        assert actual == python_preview(case, reference_resources, tables)
        assert set(actual["wraith"]["states"]) == {"unknown"}
        assert actual["wraith"]["source_entry_state"] is None


@pytest.mark.parametrize("mutation", ["identity", "unobserved_config", "bad_row", "missing_config"])
def test_invalid_state_resource_is_rejected(rust_preview, tmp_path, mutation):
    content = json.loads((DATA / "enemy_states/pc_v2_01/native_tables.json").read_text())
    if mutation == "identity":
        content["text_sha256"] = "0" * 64
    elif mutation == "unobserved_config":
        content["config_4543_lookup_observed"] = False
    elif mutation == "missing_config":
        del content["config_4543_hex"]
    else:
        terrain = next(iter(content["positions_by_terrain"].values()))
        terrain["rows_hex"][0] = "00"
    path = tmp_path / "invalid.json"
    path.write_text(json.dumps(content), encoding="utf-8")
    rust_preview([], path, success=False)
