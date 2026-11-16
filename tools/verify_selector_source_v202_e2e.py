"""Run the real seeded Rust probe, Lua observer and CLI validator in owned E2E.

No process is opened. The Lua adapter simulates memory and game instructions.
Raw fixtures, CLI outputs, deliberate faults and file hashes are retained.
Native parity remains unverified even when every owned control passes.
"""
from __future__ import annotations

import argparse
import copy
import ctypes
import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RESEARCH = ROOT / "research" / "possessed_enemy_capture"
sys.path.insert(0, str(RESEARCH))
from assignment_origin_reference import threshold_4543  # noqa: E402
from run_possessed_enemy_observer import lua_literal  # noqa: E402


def save(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest().upper()


class OwnedLua:
    def __init__(self, library: Path):
        self.lib = ctypes.CDLL(str(library.resolve()))
        signatures = {
            "luaL_newstate": ([], ctypes.c_void_p),
            "luaL_openlibs": ([ctypes.c_void_p], None),
            "luaL_loadstring": ([ctypes.c_void_p, ctypes.c_char_p], ctypes.c_int),
            "lua_pcallk": ([ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_int,
                            ctypes.c_ssize_t, ctypes.c_void_p], ctypes.c_int),
            "lua_tolstring": ([ctypes.c_void_p, ctypes.c_int, ctypes.POINTER(ctypes.c_size_t)], ctypes.c_void_p),
            "lua_close": ([ctypes.c_void_p], None),
        }
        for name, (args, result) in signatures.items():
            function = getattr(self.lib, name)
            function.argtypes, function.restype = args, result

    def run(self, script: str) -> dict:
        state = self.lib.luaL_newstate()
        if not state:
            raise MemoryError("Owned Lua state allocation failed")
        try:
            self.lib.luaL_openlibs(state)
            code = self.lib.luaL_loadstring(state, script.encode("utf-8"))
            if not code:
                code = self.lib.lua_pcallk(state, 0, 1, 0, 0, None)
            size = ctypes.c_size_t()
            pointer = self.lib.lua_tolstring(state, -1, ctypes.byref(size))
            text = ctypes.string_at(pointer, size.value).decode("utf-8") if pointer else "<no Lua output>"
            if code:
                raise RuntimeError(f"Owned Lua execution failed ({code}): {text}")
            return json.loads(text)
        finally:
            self.lib.lua_close(state)


def fixture(runtime: OwnedLua, profile: dict, model: dict, config_hex: str, fault: str = "") -> dict:
    values = {
        "TEST_MODEL": model, "TEST_PROFILE": profile, "TEST_CONFIG_HEX": config_hex,
        "TEST_THRESHOLD": threshold_4543(bytes.fromhex(config_hex)), "TEST_FAULT": fault,
        "TEST_COLLECTOR_PATH": (RESEARCH / "selector_source_v202_ce.lua").as_posix(),
        "NIOH3_BREAKPOINT_LIFECYCLE_PATH": (ROOT / "research" / "owned_breakpoint_lifecycle_ce.lua").as_posix(),
    }
    script = "\n".join(f"{key}={lua_literal(value)}" for key, value in values.items())
    script += "\nreturn assert(loadfile(" + lua_literal((RESEARCH / "selector_source_v202_fixture.lua").as_posix()) + "))()"
    return runtime.run(script)


def envelopes(observed: dict, profile: dict) -> tuple[dict, dict]:
    probe = observed["probe"]
    sites = [f"0x{0x140000000 + int(site['rva'], 16):X}" for site in profile["sites"].values()]
    arm = {"result": {"active": True, "run_id": probe["run_id"], "debugger_broken": False,
                      "owned_breakpoints": sites, "breakpoints": sites}}
    capture = {"capture_metadata": {
        "evidence_kind": "owned_fixture", "phase": "selector-source-v202", "run_id": probe["run_id"],
        "target_seed": probe["target_seed"], "target": probe["identity"], "mode_label": "normal-solo",
        "requested_pid": probe["pid"],
        "read_only": True, "writes_game_memory": False,
        "source": {"profile_sha256": digest(RESEARCH / "selector_source_v202_locators.json"),
                   "phase_sha256": digest(RESEARCH / "selector_source_v202_ce.lua"),
                   "lifecycle_sha256": digest(ROOT / "research" / "owned_breakpoint_lifecycle_ce.lua")},
        "arm_verification": arm, "fresh_phase_initialization": {"result": {
            "initialized": True, "debugger_broken": False, "breakpoints": []}},
    }, "bridge_result": {"result": probe}}
    cleanup = {"cleanup_metadata": {
        "verified": not observed["breakpoints"] and not probe["cleanup_pending"],
        "run_id": probe["run_id"], "target": probe["identity"],
    }, "bridge_result": {"result": {
        "probe": {key: probe[key] for key in ("active", "cleanup_pending", "owned_breakpoints", "run_id", "schema")}
                 | {"debugger_broken": False},
        "breakpoints": observed["breakpoints"], "debugger_broken": False,
    }}}
    return capture, cleanup


def cli_case(directory: Path, capture: dict, cleanup: dict, prediction_path: Path,
             expected_error: str | None = None, *, allow_owned: bool = True) -> dict:
    directory.mkdir()
    save(directory / "capture.json", capture)
    save(directory / "cleanup.json", cleanup)
    command = [sys.executable, str(RESEARCH / "validate_selector_source_v202.py"),
               str(directory / "capture.json"), "--cleanup", str(directory / "cleanup.json"),
               "--prediction", str(prediction_path)]
    if allow_owned:
        command.append("--allow-owned-fixture")
    result = subprocess.run(command, capture_output=True, text=True, cwd=ROOT, check=False)
    (directory / "stdout.json").write_text(result.stdout, encoding="utf-8")
    (directory / "stderr.log").write_text(result.stderr, encoding="utf-8")
    parsed = json.loads(result.stdout)
    if expected_error is None:
        if result.returncode != 0 or parsed.get("validated") is not True:
            raise AssertionError(f"{directory.name}: {result.stdout} {result.stderr}")
        if parsed.get("native_parity_validated") is not False or parsed.get("production_support_authorized") is not False:
            raise AssertionError("Owned fixture incorrectly certified native/product parity")
    elif result.returncode != 1 or parsed.get("validated") is not False or expected_error not in parsed.get("error", ""):
        raise AssertionError(f"{directory.name}: expected {expected_error!r}, received {result.stdout} {result.stderr}")
    return {"name": directory.name, "exit_code": result.returncode, "expected_error": expected_error, "pass": True}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-directory", type=Path, required=True)
    parser.add_argument("--lua-library", type=Path, required=True)
    args = parser.parse_args()
    output = args.output_directory.resolve()
    output.mkdir(parents=True, exist_ok=False)
    prediction_path = output / "seeded-model.json"
    command = ["cargo", "run", "--release", "--locked", "--offline", "--manifest-path",
               str(ROOT / "crates" / "nioh3-worker" / "Cargo.toml"), "--example", "selector_wraith_audit",
               "--", str(ROOT / "data"), "2.0.2.0", str(prediction_path)]
    build = subprocess.run(command, capture_output=True, text=True, cwd=ROOT, check=False)
    (output / "rust-probe.log").write_text(build.stdout + build.stderr, encoding="utf-8")
    build.check_returncode()
    explicit_seeds = output / "explicit-seeds.json"
    explicit_prediction = output / "explicit-seeds-model.json"
    save(explicit_seeds, [100000029, 100000439])
    explicit_command = command[:-1] + [str(explicit_prediction), str(explicit_seeds)]
    explicit_run = subprocess.run(explicit_command, capture_output=True, text=True, cwd=ROOT, check=False)
    (output / "explicit-seeds-probe.log").write_text(explicit_run.stdout + explicit_run.stderr, encoding="utf-8")
    explicit_run.check_returncode()
    explicit_result = json.loads(explicit_prediction.read_text(encoding="utf-8"))
    if {(case["seed"], case["variant"]) for case in explicit_result["cases"]} != {
        (seed, mode) for seed in (100000029, 100000439) for mode in ("solo", "expedition")
    } or len(explicit_result["cases"]) != 4:
        raise AssertionError("Explicit seeded-probe request was not preserved")
    prediction = json.loads(prediction_path.read_text(encoding="utf-8"))
    profile = json.loads((RESEARCH / "selector_source_v202_locators.json").read_text(encoding="utf-8"))
    runtime = OwnedLua(args.lua_library)
    checks = [{"name": "explicit-seed-input-through-rust-cli", "pass": True}]
    chosen = None
    for model in prediction["cases"]:
        if model["descriptor_selector"] != 1 or model["variant"] != "solo":
            continue
        observed = fixture(runtime, profile, model, prediction["config_4543_hex"])
        capture, cleanup = envelopes(observed, profile)
        name = f"seed-{model['seed']}"
        checks.append(cli_case(output / name, capture, cleanup, prediction_path))
        if model["seed"] == 100000439:
            chosen = model, capture, cleanup
    if chosen is None:
        raise AssertionError("Recognizable test seed absent from seeded probe")
    model, original, original_cleanup = chosen
    crlf_profile_capture = copy.deepcopy(original)
    profile_lf = (RESEARCH / "selector_source_v202_locators.json").read_bytes().replace(b"\r\n", b"\n")
    crlf_profile_capture["capture_metadata"]["source"]["profile_sha256"] = hashlib.sha256(
        profile_lf.replace(b"\n", b"\r\n")
    ).hexdigest().upper()
    checks.append(cli_case(output / "identical-profile-with-windows-line-endings", crlf_profile_capture,
                           original_cleanup, prediction_path))
    unavailable_base_capture, unavailable_base_cleanup = copy.deepcopy((original, original_cleanup))
    identity = copy.deepcopy(original["bridge_result"]["result"]["identity"])
    unavailable_base_capture["capture_metadata"]["target"] = dict(identity, module_base=None)
    unavailable_base_capture["bridge_result"]["result"]["identity"] = {
        key: value for key, value in identity.items() if key != "module_base"
    }
    unavailable_base_cleanup["cleanup_metadata"]["target"] = dict(identity, module_base=None)
    checks.append(cli_case(output / "bridge-omits-optional-module-base", unavailable_base_capture,
                           unavailable_base_cleanup, prediction_path))
    for fault in ("executable", "signature", "ancestry-signature", "foreign-breakpoint"):
        observed = fixture(runtime, profile, model, prediction["config_4543_hex"], fault)
        save(output / f"admission-{fault}.json", observed)
        if observed["bootstrap_ok"] is not False or observed["continued"] != 0:
            raise AssertionError(f"Unsafe preflight admitted: {fault}")
        expected_inventory = [0xDEAD] if fault == "foreign-breakpoint" else []
        if observed["breakpoints"] != expected_inventory:
            raise AssertionError(f"Preflight modified foreign/owned resources: {fault}")
        checks.append({"name": f"admission-{fault}", "pass": True})
    for fault in ("descriptor-selector", "request-playthrough", "preassigned-flag", "trial-thread",
                  "cleanup-failure", "partial-arm", "budget-timer", "timeout"):
        observed = fixture(runtime, profile, model, prediction["config_4543_hex"], fault)
        capture, cleanup = envelopes(observed, profile)
        error = ("missed/extra native LCG trial" if fault == "trial-thread" else
                 "cleanup unverified" if fault == "cleanup-failure" else
                 "incomplete capture stop" if fault == "timeout" else "observer active or failed")
        checks.append(cli_case(output / f"observer-{fault}", capture, cleanup, prediction_path, error))
        if fault == "preassigned-flag":
            events = observed["probe"]["events"]
            if not events or events[0]["descriptors"][0]["flag"] != 1:
                raise AssertionError("Preassigned-flag counterexample was discarded")
    checks.append(cli_case(output / "owned-is-not-native", original, original_cleanup,
                           prediction_path, "native evidence missing", allow_owned=False))
    mutations = {
        "wrong-profile-source": (lambda c, q: c["capture_metadata"]["source"].update(profile_sha256="0" * 64), "locator source differs"),
        "wrong-version": (lambda c, q: c["bridge_result"]["result"]["identity"].update(image_size=77814240), "wrong executable identity"),
        "missing-link": (lambda c, q: c["bridge_result"]["result"]["events"].pop(), "missing/duplicate object-copy/link events"),
        "unknown-cleanup": (lambda c, q: q["bridge_result"]["result"].update(breakpoints=None), "breakpoint inventory is unknown"),
        "stopped-debugger": (lambda c, q: q["bridge_result"]["result"].update(debugger_broken=True), "debugger remains stopped"),
        "wrong-arm-owner": (lambda c, q: c["capture_metadata"]["arm_verification"]["result"].update(run_id="foreign"), "arm proof unverified"),
    }
    for name, (mutate, expected_error) in mutations.items():
        capture, cleanup = copy.deepcopy((original, original_cleanup))
        mutate(capture, cleanup)
        checks.append(cli_case(output / name, capture, cleanup, prediction_path, expected_error))
    shifted = copy.deepcopy(prediction)
    for case in shifted["cases"]:
        if case["seed"] == model["seed"] and case["variant"] == "solo":
            case["source_entry_state"] ^= 1
    shifted_path = output / "wrong-seeded-entry.json"
    save(shifted_path, shifted)
    checks.append(cli_case(output / "captured-replay-is-not-seeded-proof", original, original_cleanup,
                           shifted_path, "seed-to-source-entry RNG mismatch"))
    sources = [Path(__file__).resolve(), RESEARCH / "selector_source_v202_ce.lua",
               RESEARCH / "selector_source_v202_fixture.lua", RESEARCH / "selector_source_v202_locators.json",
               RESEARCH / "run_possessed_enemy_observer.py", RESEARCH / "validate_selector_source_v202.py",
               RESEARCH / "validate_assignment_origin_capture.py", ROOT / "research" / "owned_breakpoint_lifecycle_ce.lua",
               ROOT / "crates" / "nioh3-worker" / "examples" / "selector_wraith_audit.rs"]
    report = {"pass": True, "scope": "Owned Lua/CLI/Rust E2E only; no game process or save access",
              "native_parity_validated": False, "production_support_authorized": False,
              "source_sha256": {str(path.relative_to(ROOT)): digest(path) for path in sources},
              "lua_library_sha256": digest(args.lua_library), "checks": checks}
    save(output / "e2e-report.json", report)
    save(output / "artifact-manifest.json", {str(path.relative_to(output)): digest(path)
                                             for path in sorted(output.rglob("*")) if path.is_file()})
    print(json.dumps({"pass": True, "checks": len(checks), "evidence": str(output), "native_parity_validated": False}))


if __name__ == "__main__":
    main()
