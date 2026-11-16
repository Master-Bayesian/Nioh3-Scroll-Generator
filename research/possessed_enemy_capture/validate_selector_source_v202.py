"""Compare a version-pinned native trace to an independently seeded Rust run.

The model must be run before collection. No captured RNG state or descriptor
is supplied to it. Passing this command validates one normal-solo NG3 entry;
it never authorizes production support or substitutes for visual confirmation.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

from run_possessed_enemy_observer import arm_verified
from validate_assignment_origin_capture import normalized_identity, require, unwrap, validate as validate_causal

HERE = Path(__file__).resolve().parent
PROFILE_PATH = HERE / "selector_source_v202_locators.json"


def validate(capture: dict, cleanup: dict, prediction: dict, *, allow_owned_fixture: bool = False) -> dict:
    profile = json.loads(PROFILE_PATH.read_text(encoding="utf-8"))
    causal = validate_causal(capture, cleanup, profile=profile)
    p = unwrap(capture)
    metadata = capture["capture_metadata"]
    evidence_kind = metadata.get("evidence_kind")
    require(evidence_kind == "native_debugger_observation" or
            (allow_owned_fixture and evidence_kind == "owned_fixture"), "native evidence missing")
    require(p.get("state") == "captured", "capture state not complete")
    require(p.get("target_descriptor_selector") == 1 and p.get("variant") == "solo",
            "wrong declared descriptor branch/mode")
    require(metadata.get("phase") == "selector-source-v202", "wrong runner phase")
    require(metadata.get("target_seed") == p["target_seed"], "runner seed mismatch")
    require(metadata.get("run_id") == p["run_id"], "runner owner mismatch")
    require(normalized_identity(metadata["target"]) == normalized_identity(p["identity"]),
            "runner target mismatch")
    identity = p["identity"]
    require(type(identity["process_id"]) is int and identity["process_id"] > 0
            and p["pid"] == identity["process_id"]
            and metadata["requested_pid"] == p["pid"], "capture PID differs")
    module_base = int(p["module_base"], 16)
    if identity.get("module_base") is not None:
        require(module_base == int(identity["module_base"], 16), "capture module differs")
    require(metadata.get("mode_label") in (None, "normal-solo"), "wrong observed mode")
    require(metadata.get("read_only") is True and metadata.get("writes_game_memory") is False,
            "runner mode unverified")
    # The original Windows capture used CRLF before the existing .json LF
    # attribute normalized the new file for Git. Bind the exact same trusted
    # profile text in either line-ending representation; no field changes or
    # reordered/rewritten JSON are admitted by this compatibility allowance.
    profile_lf = PROFILE_PATH.read_bytes().replace(b"\r\n", b"\n")
    profile_forms = (profile_lf, profile_lf.replace(b"\n", b"\r\n"))
    require(metadata["source"]["profile_sha256"] in {
        hashlib.sha256(form).hexdigest().upper() for form in profile_forms
    }, "locator source differs")
    for key, source in (("phase_sha256", HERE / "selector_source_v202_ce.lua"),
                        ("lifecycle_sha256", HERE.parent / "owned_breakpoint_lifecycle_ce.lua")):
        require(metadata["source"][key] == hashlib.sha256(source.read_bytes()).hexdigest().upper(),
                "observer source differs")
    require(arm_verified(metadata["arm_verification"], p["run_id"], 4), "arm proof unverified")
    armed = unwrap(metadata["arm_verification"])
    expected_sites = {module_base + int(site["rva"], 16)
                      for site in profile["sites"].values()}
    require({int(value, 16) for value in armed["owned_breakpoints"]} == expected_sites,
            "arm sites differ from pinned profile")
    fresh = unwrap(metadata["fresh_phase_initialization"])
    require(fresh.get("initialized") is True and fresh.get("debugger_broken") is False
            and fresh.get("breakpoints") in ([], {}), "fresh phase unverified")
    cp = unwrap(cleanup)
    require(cp.get("debugger_broken") is False and cp["probe"].get("debugger_broken") is False,
            "debugger remains stopped or unknown")
    require(prediction.get("schema") == "nioh3-selector-wraith-offline-audit/v1", "wrong model schema")
    require(prediction.get("game_resource_version") == profile["version"], "wrong model game version")
    matching = [case for case in prediction["cases"]
                if case["seed"] == p["target_seed"] and case["variant"] == "solo"]
    require(len(matching) == 1, "missing/ambiguous seeded prediction")
    model = matching[0]
    require(model["model_inputs_complete"] is True and not model["missing_model_inputs"],
            "seeded model incomplete")
    require(model["descriptor_selector"] == 1 and model["playthrough"] == 3,
            "seeded prediction belongs to another branch")
    origins = [event for event in p["events"] if event["site"] == "origin_entry"]
    require(len(origins) == 1, "solo capture must contain exactly one source pass")
    origin = origins[0]
    require(origin["descriptor_selector"] == 1 and origin["selector"] == 0
            and origin["allow_class1"] == 0, "native descriptor branch/mode mismatch")
    request = bytes.fromhex(origin["global_request_hex"])
    require(len(request) == 12 and int.from_bytes(request[:4], "little") == model["seed"]
            and request[7] == 3 and request[9] == 0, "native request differs from seeded NG3 solo input")
    require(origin["descriptor_flags"] == model["descriptor_flags"], "descriptor flags differ")
    require(origin["context_key"] == model["auxiliary_mode"], "auxiliary context differs")
    require(origin["terrain"] == model["terrain"], "terrain differs")
    config = origin["tables"]["config_4543"]
    live_config = config["raw_hex"].lower() if config["present"] else None
    require(live_config == prediction["config_4543_hex"], "native/model configuration differs")
    require(origin["parent_rng"]["state"] == model["source_entry_state"],
            "seed-to-source-entry RNG mismatch")
    require(len(origin["descriptors"]) == len(model["occurrences"]), "seeded roster count differs")
    for native, expected in zip(origin["descriptors"], model["occurrences"]):
        raw = bytes.fromhex(native["raw_hex"])
        require(native["role"] == raw[8], "native role/raw mismatch")
        actual = (native["wave_index"], native["position"], native["spawn"], native["lookup"],
                  native["role"], native["selector_class"])
        desired = (expected["wave_index"], expected["position"], expected["spawn_key"],
                   expected["lookup_key"], expected["role"], expected["occurrence_selector_class"])
        require(actual == desired, "seeded roster identity/order differs")
        gate = native["eligibility"]
        eligible = (not gate["enemy_row_present"] or
                    (gate["subtype_row_present"] and gate["flags14"] & 1 == 0))
        require(type(expected["source_eligible"]) is bool and eligible == expected["source_eligible"],
                "native/model source eligibility differs")
        require(expected["unverified_possessed_prediction"] in ("yes", "no"), "unknown model state")
        require(causal["flags8f"][str(native["spawn"])] ==
                int(expected["unverified_possessed_prediction"] == "yes"), "seeded final flag differs")
    native_trials = [event for event in p["events"] if event["site"] == "trial_decision"]
    require(len(native_trials) == len(model["source_trials"]), "seeded source trial count differs")
    for native, expected in zip(native_trials, model["source_trials"]):
        require(native["source_return_rva"].upper() == "0X102E7C8", "trial source caller differs")
        require((native["selector"], native["descriptor"]["spawn"], native["parent_rng"]["state"],
                 native["ticket"], native["branch_will_set"]) ==
                (expected["occurrence_selector_class"], expected["spawn_key"], expected["state"],
                 expected["ticket"], expected["accepted"]), "seeded source trial differs")
    require(causal["state_after"] == model["final_state"], "seeded final RNG differs")
    native_parity = evidence_kind == "native_debugger_observation"
    return {"validated": True, "native_parity_validated": native_parity,
            "state": ("one_native_entry_matches_seeded_model" if native_parity else
                      "owned_fixture_matches_seeded_model"),
            "run_id": p["run_id"], "seed": p["target_seed"], "descriptor_selector": 1,
            "variant": "solo", "tasks": causal["tasks"], "trials": causal["trials"],
            "selected_spawn": causal["selected_spawn"], "flags8f": causal["flags8f"],
            "model_table_sha256": prediction["state_table_sha256"],
            "production_support_authorized": False,
            "still_required": ["independent entries and boundary coverage", "owner visual confirmation"]}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("--cleanup", type=Path, required=True)
    parser.add_argument("--prediction", type=Path, required=True)
    parser.add_argument("--allow-owned-fixture", action="store_true",
                        help="Exercise owned E2E evidence without certifying native parity")
    args = parser.parse_args()
    try:
        inputs = [json.loads(path.read_text(encoding="utf-8"))
                  for path in (args.capture, args.cleanup, args.prediction)]
        result = validate(*inputs, allow_owned_fixture=args.allow_owned_fixture)
    except (ValueError, KeyError, TypeError, IndexError) as error:
        print(json.dumps({"validated": False, "state": "rejected", "error": str(error)}))
        raise SystemExit(1) from error
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
