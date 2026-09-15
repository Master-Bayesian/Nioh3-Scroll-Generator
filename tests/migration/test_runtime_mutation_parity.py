"""Cross-language gate for the M3 protected mutation core.

Two things are compared with the shipped Python implementation on the same
inputs: the trampoline builders, byte for byte, because those bytes are executed
by the game from a temporary hook; and the count-edit receipt state machine,
step by step, through a real `RuntimeCountEditor` with an injected record
adapter and the Rust editor with the equivalent injected adapter.

This file also owns the process-rights audit, because the read adapter must stay
read-only while the mutation modules may use exactly the write masks the shipped
code uses and nothing wider. No Nioh 3 process is touched: every scenario is
injected.
"""
from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from nioh3_scroll_editor.live_add_operations import canonical  # noqa: E402
from nioh3_scroll_editor.runtime_auxiliary_override import (  # noqa: E402
    DESCRIPTOR_COMPLETE_BYTES,
    PROCESS_ACCESS,
    RuntimeAuxiliaryOverrideProfile,
    _enemy_role_by_lookup_key,
    build_override_trampoline,
)
from nioh3_scroll_editor.runtime_challenge_override import (  # noqa: E402
    ChallengeOverrideProfile,
    build_challenge_trampoline,
)
from nioh3_scroll_editor.runtime_count_edit import RuntimeCountEditor  # noqa: E402
from tests.migration.cargo_target import resolved_cargo_target_dir  # noqa: E402

CRATE = ROOT / "crates" / "nioh3-runtime"
PROBE = "runtime_read_probe"
RECORD_SIZE = 0xE8
SERIAL = 0x1122334455667788
SEED = 0x0BADF00D

# Modules that must stay read-only, and the capabilities that may never appear
# anywhere in the crate.
READ_ONLY_SOURCES = ("lib.rs", "error.rs", "status.rs", "profile.rs", "platform.rs")
MUTATION_SOURCES = (
    "mutation/memory.rs",
    "mutation/session.rs",
    "mutation/count.rs",
    "mutation/trampoline.rs",
    "mutation/inventory.rs",
    "mutation/descriptor.rs",
    "mutation/evidence.rs",
    "mutation/operations.rs",
    "mutation/live_add.rs",
    "mutation/live_batch.rs",
    "mutation/live_fakes.rs",
)
READ_ONLY_FORBIDDEN = (
    "WriteProcessMemory",
    "VirtualAllocEx",
    "VirtualFreeEx",
    "VirtualProtectEx",
    "FlushInstructionCache",
    "PROCESS_VM_WRITE",
    "PROCESS_VM_OPERATION",
    "PROCESS_TERMINATE",
)
FORBIDDEN_EVERYWHERE = (
    "PROCESS_ALL_ACCESS",
    "0x1F0FFF",
    "0x1f0fff",
    "CreateRemoteThread",
    "TerminateProcess",
    "TerminateThread",
    "SuspendThread",
    "ResumeThread",
    "SetThreadContext",
    "DebugActiveProcess",
    "CreateProcessW",
    "OpenProcessToken",
)


def acceptance_target_dir() -> str:
    """An explicit acceptance cache wins; otherwise the shared resolver."""

    explicit = os.environ.get("NIOH3_RUNTIME_MUTATION_CARGO_TARGET", "").strip()
    if explicit:
        return explicit
    return resolved_cargo_target_dir("m3-runtime-acceptance")


def build_probe() -> Path:
    cargo = shutil.which("cargo")
    if cargo is None:
        raise AssertionError("cargo is required to build the mutation probe")
    target = acceptance_target_dir()
    completed = subprocess.run(
        [
            cargo,
            "build",
            "--locked",
            "--offline",
            "--quiet",
            "--features",
            "test-fake",
            "--manifest-path",
            str(CRATE / "Cargo.toml"),
            "--example",
            PROBE,
        ],
        cwd=str(ROOT),
        env={**os.environ, "CARGO_TARGET_DIR": target},
        capture_output=True,
        text=True,
        timeout=3600,
        check=False,
    )
    if completed.returncode != 0:
        raise AssertionError(
            "the mutation probe did not build: "
            + (completed.stderr.strip() or completed.stdout.strip())[-4000:]
        )
    binary = Path(target) / "debug" / "examples" / f"{PROBE}.exe"
    if not binary.is_file():
        raise AssertionError(f"the mutation probe is missing at {binary}")
    return binary


def record_bytes(count: int = 2) -> bytes:
    record = bytearray(RECORD_SIZE)
    record[0x00] = 0x04
    record[0x02] = 0x11
    record[0x0E] = 0
    record[0x18] = 0x03
    record[0x10] = 0x5A
    record[0x20:0x24] = SEED.to_bytes(4, "little")
    record[0x28:0x30] = SERIAL.to_bytes(8, "little")
    record[0x30] = 4
    record[0x33] = count
    return bytes(record)


class _ScenarioMemory:
    """The injected record adapter both editors are compared through."""

    def __init__(self, record: bytes, fault: str) -> None:
        self.record = bytearray(record)
        self.fault = fault
        self.pid = 4321
        self.creation_time = "134338049984156850"
        self.manager = 0x100000000
        self.data = 0x200000000
        self.address = 0x200001000
        self.serial = int.from_bytes(record[0x28:0x30], "little")

    def capture(self, serial: int) -> dict:
        # The shipped editor passes the serial as a decimal string.
        serial = int(serial)
        if serial != self.serial:
            raise ValueError("Scroll instance is no longer in the current inventory")
        return {
            "pid": self.pid,
            "creation_time": self.creation_time,
            "manager": self.manager,
            "data": self.data,
            "address": self.address,
            "record_hex": bytes(self.record).hex(),
            "serial": self.serial,
        }

    def write(self, expected: dict, desired: int) -> bytes:
        if self.fault == "write_fail":
            raise OSError("injected write failure")
        previous = bytes(self.record)
        if self.fault != "quiet":
            self.record[0x33] = desired
        if self.fault == "readback":
            return previous
        return bytes(self.record)


def shipped_count_steps(
    record: bytes, new_count: int, fault: str, steps: list[str]
) -> list[tuple[str, str]]:
    root = Path(tempfile.mkdtemp(prefix="nioh3-mutation-parity-"))
    try:
        # The shipped backup manifest derives the account id from the path, so
        # the fixture keeps the `<account>/SAVEDATA00/SAVEDATA.BIN` shape.
        save_directory = root / str(0x1111222233334444) / "SAVEDATA00"
        save_directory.mkdir(parents=True, exist_ok=True)
        save = save_directory / "SAVEDATA.BIN"
        save.write_bytes(b"scenario-save")
        editor = RuntimeCountEditor(root, memory=_ScenarioMemory(record, fault))
        source = {
            "record_hex": record.hex(),
            "save_path": str(save),
            "source_sha256": hashlib.sha256(b"scenario-save").hexdigest(),
        }
        prepared = editor.prepare(source, new_count)["count_edit"]
        rows: list[tuple[str, str]] = []
        for step in steps:
            if step == "execute":
                value = editor.execute(prepared["operation_id"], prepared["plan_digest"])
            elif step == "recover":
                value = editor.recover(prepared["operation_id"])
            else:
                value = editor.status(prepared["operation_id"])
            status = value["count_edit"] if "count_edit" in value else value
            rows.append((status["state"], status.get("error") or "-"))
        return rows
    finally:
        shutil.rmtree(root, ignore_errors=True)


def code_only(text: str) -> str:
    kept = []
    for line in text.splitlines():
        if line.strip().startswith("//"):
            continue
        kept.append(line.split("//", 1)[0])
    return strip_string_literals("\n".join(kept))


def strip_string_literals(text: str) -> str:
    """Remove Rust string literals.

    A read-only module may *name* a write API inside an operator message, which
    is what `error.rs` does; it may never call one. Dropping the literals keeps
    the audit pointed at code.
    """

    out = []
    in_string = False
    escaped = False
    for character in text:
        if in_string:
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == '"':
                in_string = False
            continue
        if character == '"':
            in_string = True
            continue
        out.append(character)
    return "".join(out)


def parse_mask(text: str) -> int:
    """Evaluate a hex or decimal mask expression that only uses `|` and `+`."""

    if not re.fullmatch(r"[\s0-9a-fxX|+]+", text):
        raise AssertionError(f"refusing to evaluate an unexpected mask: {text!r}")
    return int(eval(text))  # noqa: S307 - the character set is allowlisted above


class RuntimeMutationParityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.binary = build_probe()
        cls.record = record_bytes()

    def probe(self, *arguments: str) -> list[str]:
        completed = subprocess.run(
            [str(type(self).binary), *arguments],
            cwd=str(ROOT),
            capture_output=True,
            text=True,
            timeout=600,
            check=False,
        )
        if completed.returncode != 0:
            raise AssertionError(
                f"runtime_read_probe {' '.join(arguments[:2])} failed: "
                + (completed.stdout + completed.stderr).strip()
            )
        return [line for line in completed.stdout.splitlines() if line]

    def test_challenge_trampoline_matches_the_shipped_builder(self) -> None:
        cases = [
            (0x01020304, 5, 0x140001000, 0x150000000, b"\x48\x89\x5c\x24\x08"),
            (0xFFFFFFFF, 1, 0x140002000, 0x160000000, b"\x48\x89\x5c\x24\x08"),
            (0, 7, 0x7FF000000000, 0x7FF100000000, b"\x90\x90\x90\x90\x90"),
        ]
        for seed, capacity, return_address, counter, original in cases:
            with self.subTest(seed=hex(seed), capacity=capacity):
                request = {
                    "kind": "challenge",
                    "seed": seed,
                    "capacity": capacity,
                    "return_address": hex(return_address),
                    "counter_address": hex(counter),
                    "original_bytes": original.hex(),
                }
                rust = self.probe("--trampoline", json.dumps(request))
                self.assertEqual(rust[0].split("\t")[0], "code")
                expected = build_challenge_trampoline(
                    ChallengeOverrideProfile(seed=seed, capacity=capacity),
                    return_address=return_address,
                    counter_address=counter,
                    original_instruction=original,
                )
                self.assertEqual(rust[0].split("\t")[1], expected.hex())

    def test_auxiliary_trampoline_matches_the_shipped_builder(self) -> None:
        roles = _enemy_role_by_lookup_key()
        capable = sorted(roles)[:2]
        cases = [
            {"enemy_keys": (), "special_rule_keys": (9, 8, 7), "terrain_value": 2},
            {"enemy_keys": (), "special_rule_keys": None, "terrain_value": 5},
            {"enemy_keys": (), "special_rule_keys": (1, 2, 3), "terrain_value": None},
            {
                "enemy_keys": tuple(capable),
                "special_rule_keys": (0x1234, 0x5678, 0x9ABC),
                "terrain_value": 0xFF,
            },
            {"enemy_keys": (capable[0],), "special_rule_keys": None, "terrain_value": None},
        ]
        for index, case in enumerate(cases):
            with self.subTest(case=index):
                return_address = 0x140000000 + index * 0x1000
                counter = 0x150000000 + index * 0x1000
                profile = RuntimeAuxiliaryOverrideProfile(
                    seed=SEED + index,
                    enemy_keys=tuple(case["enemy_keys"]),
                    special_rule_keys=case["special_rule_keys"],
                    terrain_value=case["terrain_value"],
                )
                request = {
                    "kind": "auxiliary",
                    "seed": profile.seed,
                    "enemy_groups": [
                        {"lookup_key": key, "role": roles[key]} for key in case["enemy_keys"]
                    ],
                    "special_rule_keys": list(case["special_rule_keys"])
                    if case["special_rule_keys"]
                    else None,
                    "terrain_value": case["terrain_value"],
                    "return_address": hex(return_address),
                    "counter_address": hex(counter),
                    "original_bytes": DESCRIPTOR_COMPLETE_BYTES.hex(),
                }
                rust = self.probe("--trampoline", json.dumps(request))
                expected = build_override_trampoline(
                    profile,
                    return_address=return_address,
                    counter_address=counter,
                    original_instruction=DESCRIPTOR_COMPLETE_BYTES,
                )
                self.assertEqual(rust[0].split("\t")[1], expected.hex())

    def test_canonical_json_and_digest_match_the_shipped_helper(self) -> None:
        values = [
            {"b": 1, "a": "plain"},
            {"path": "C:\\Users\\名字\\SAVEDATA.BIN", "flag": True, "none": None},
            {
                "operation_id": "0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0",
                "target": {"pid": 4321, "address": 0x200001000, "serial": SERIAL},
                "counts": [0, 7],
            },
            {"emoji": "\U0001F600", "control": "\u0001"},
        ]
        for value in values:
            with self.subTest(value=str(value)[:40]):
                rows = self.probe("--canonical", json.dumps(value))
                self.assertEqual(rows[0], "canonical\t" + canonical(value).decode("utf-8"))
                self.assertEqual(
                    rows[1], "digest\t" + hashlib.sha256(canonical(value)).hexdigest()
                )

    def test_count_scenario_states_match_the_shipped_editor(self) -> None:
        scripts = [
            (2, "none", ["execute"]),
            (2, "none", ["execute", "execute"]),
            (2, "none", ["recover"]),
            (2, "quiet", ["execute", "recover"]),
            (2, "quiet", ["execute", "execute", "recover"]),
            (2, "readback", ["execute", "recover"]),
            (2, "write_fail", ["execute", "recover"]),
            (7, "none", ["execute", "status"]),
        ]
        for new_count, fault, steps in scripts:
            with self.subTest(fault=fault, steps=steps, new_count=new_count):
                request = {
                    "record_hex": self.record.hex(),
                    "new_count": new_count,
                    "fault": fault,
                    "steps": steps,
                }
                rust = [
                    (row.split("\t")[2], row.split("\t", 3)[3])
                    for row in self.probe("--count-scenario", json.dumps(request))
                    if row.startswith("step\t")
                ]
                expected = shipped_count_steps(self.record, new_count, fault, steps)
                self.assertEqual(
                    [state for state, _ in rust],
                    [state for state, _ in expected],
                    "the receipt state sequence must match",
                )
                for (rust_state, rust_error), (_, python_error) in zip(rust, expected):
                    if fault == "write_fail" and rust_state == "uncertain":
                        # The failing write is injected on each side, so the two
                        # adapters carry their own text; the state, not the text,
                        # is the contract here.
                        self.assertNotEqual(rust_error, "-")
                        self.assertNotEqual(python_error, "-")
                        continue
                    self.assertEqual(rust_error, python_error, rust_state)

    def test_the_read_adapter_stays_read_only(self) -> None:
        for name in READ_ONLY_SOURCES:
            text = code_only((CRATE / "src" / name).read_text(encoding="utf-8"))
            for capability in READ_ONLY_FORBIDDEN:
                with self.subTest(source=name, capability=capability):
                    self.assertNotIn(capability, text, f"{capability} in {name}")

    def test_the_mutation_modules_declare_exactly_the_shipped_write_rights(self) -> None:
        memory = (CRATE / "src" / "mutation" / "memory.rs").read_text(encoding="utf-8")
        override_expression = re.search(r"pub const OVERRIDE_ACCESS: u32 = ([^;]+);", memory)
        count_expression = re.search(r"pub const COUNT_WRITE_ACCESS: u32 = ([^;]+);", memory)
        self.assertIsNotNone(override_expression)
        self.assertIsNotNone(count_expression)
        self.assertEqual(parse_mask(override_expression.group(1)), PROCESS_ACCESS)

        count_source = (ROOT / "nioh3_scroll_editor" / "runtime_count_edit.py").read_text(
            encoding="utf-8"
        )
        shipped_count_access = re.search(
            r"OpenProcess\(([^,]+), False, reader\.pid\)", count_source
        )
        self.assertIsNotNone(shipped_count_access)
        self.assertEqual(
            parse_mask(count_expression.group(1)),
            parse_mask(shipped_count_access.group(1)),
            "the count write must request exactly the shipped mask",
        )

    def test_no_module_asks_for_wider_process_rights(self) -> None:
        """The ban is scoped to what the shipped mutation paths actually need.

        Verified from the shipped source: neither
        `runtime_auxiliary_override.py` nor `runtime_count_edit.py` references
        `CreateRemoteThread`, `TerminateProcess` or `PROCESS_ALL_ACCESS` - the
        override installs a hook the game calls itself, and the count edit
        writes one byte with `0x1000 | 0x8 | 0x20`. So no thread creation,
        termination or suspension belongs in this crate today.

        The batch generation oracle in `native.py` *does* use
        `CreateRemoteThread` with `PROCESS_ACCESS = 0x0002 | 0x0008 | 0x0010 |
        0x0020 | 0x0400`. That is a different subsystem (`runtime.generate`,
        `runtime.search`) and a future slice; porting it needs an explicit,
        reviewed declaration of that mask, which
        `test_thread_creation_is_absent_and_its_future_mask_is_recorded`
        records instead of leaving the ban to be relaxed ad hoc.
        """

        for name in READ_ONLY_SOURCES + MUTATION_SOURCES:
            text = code_only((CRATE / "src" / name).read_text(encoding="utf-8"))
            for capability in FORBIDDEN_EVERYWHERE:
                with self.subTest(source=name, capability=capability):
                    self.assertNotIn(capability, text, f"{capability} in {name}")

    def test_thread_creation_is_absent_and_its_future_mask_is_recorded(self) -> None:
        native_source = (ROOT / "nioh3_scroll_editor" / "native.py").read_text(
            encoding="utf-8"
        )
        self.assertIn("CreateRemoteThread", native_source, "the shipped oracle uses one")
        self.assertIn("PROCESS_ACCESS = 0x0002 | 0x0008 | 0x0010 | 0x0020 | 0x0400", native_source)

        for module in ("runtime_auxiliary_override.py", "runtime_count_edit.py"):
            source = (ROOT / "nioh3_scroll_editor" / module).read_text(encoding="utf-8")
            with self.subTest(module=module):
                self.assertNotIn("CreateRemoteThread", source)
                self.assertNotIn("PROCESS_ALL_ACCESS", source)


if __name__ == "__main__":
    unittest.main()
