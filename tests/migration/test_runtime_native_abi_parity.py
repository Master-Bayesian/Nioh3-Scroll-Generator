"""Cross-language gate for the remote machine code the runtime crate emits.

The shipped emitters are the contract. Every wrapper `native.NativeBatchOracle`
can run and the one shim `live_add_native_transport` executes are compared byte
for byte with the shipped Python on identical values, because those bytes are
executed inside the game process by the oracle and by the live-add executor.

This gate also owns the scoped process-rights audit for the two new modules.
The blanket text ban is replaced by a contract that names the caller, the right
and the reason:

- no module may ever ask for `PROCESS_ALL_ACCESS`, an escalation right, or the
  lifecycle rights (`TerminateProcess`, `TerminateThread`, `SuspendThread`,
  `ResumeThread`, `CreateProcessW`, `OpenProcessToken`, `AdjustTokenPrivileges`);
- `CreateRemoteThread` and `SetThreadContext` exist only in the two modules whose
  shipped equivalent needs them: `mutation/oracle.rs` (the batch oracle) and
  `mutation/win_session.rs` (the debug binding);
- `DebugActiveProcess` exists only in that same debug binding, and it must always
  be paired with `DebugSetProcessKillOnExit(0)` so the target is never killed;
- the pure ABI module keeps no process capability at all.

No Nioh 3 process is touched here. The probe only computes bytes.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from nioh3_scroll_editor import native  # noqa: E402
from nioh3_scroll_editor.live_add_dispatch_code import build_dispatch_code  # noqa: E402
from nioh3_scroll_editor.live_add_profile import PC_V201 as LAYOUT  # noqa: E402
from tests.migration.cargo_target import resolved_cargo_target_dir  # noqa: E402

CRATE = ROOT / "crates" / "nioh3-runtime"
PROBE = "runtime_read_probe"
RECORD_SIZE = 0xE8

# Rights no module in this crate may ever request, regardless of reason.
FORBIDDEN_EVERYWHERE = (
    "PROCESS_ALL_ACCESS",
    "0x1F0FFF",
    "0x1f0fff",
    "TerminateProcess",
    "TerminateThread",
    "SuspendThread",
    "ResumeThread",
    "CreateProcessW",
    "OpenProcessToken",
    "AdjustTokenPrivileges",
)
# The modules whose shipped equivalent needs the two wider capabilities.
THREAD_CREATING_SOURCES = ("mutation/oracle.rs", "mutation/win_session.rs")
DEBUG_OWNER = "mutation/win_session.rs"
PURE_MODULE = "mutation/native_abi.rs"


class _Refused(Exception):
    """The probe reported a typed refusal rather than bytes."""


def code_only(text: str) -> str:
    """Drop line comments, so a documented ban is not read as a capability."""

    stripped = []
    for line in text.splitlines():
        marker = line.find("//")
        stripped.append(line if marker < 0 else line[:marker])
    return "\n".join(stripped)


def acceptance_target_dir() -> str:
    explicit = os.environ.get("NIOH3_RUNTIME_MUTATION_CARGO_TARGET", "").strip()
    if explicit:
        return explicit
    return resolved_cargo_target_dir("m3-runtime-acceptance")


def build_probe() -> Path:
    cargo = shutil.which("cargo")
    if cargo is None:
        raise AssertionError("cargo is required to build the ABI probe")
    target = acceptance_target_dir()
    completed = subprocess.run(
        [
            cargo,
            "build",
            "--offline",
            "--quiet",
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
            "the ABI probe did not build: "
            + (completed.stderr.strip() or completed.stdout.strip())[-4000:]
        )
    binary = Path(target) / "debug" / "examples" / f"{PROBE}.exe"
    if not binary.is_file():
        raise AssertionError(f"the ABI probe is missing at {binary}")
    return binary


class NativeAbiParityTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.probe = build_probe()

    def emit(self, request: dict) -> bytes:
        completed = subprocess.run(
            [str(self.probe), "--native-abi", json.dumps(request)],
            cwd=str(ROOT),
            capture_output=True,
            text=True,
            timeout=120,
            check=False,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        lines = completed.stdout.strip().splitlines()
        self.assertTrue(lines, "the probe printed nothing")
        fields = lines[0].split("\t")
        if fields[0] == "refused":
            raise _Refused(fields[2] if len(fields) > 2 else fields[-1])
        self.assertEqual(fields[0], "code", completed.stdout)
        return bytes.fromhex(fields[1])

    def assert_emits(self, request: dict, expected: bytes, label: str) -> None:
        self.assertEqual(self.emit(request), expected, label)

    def test_the_batch_wrapper_matches_the_shipped_bytes(self) -> None:
        cases = [
            (0x10000000, 0x10010000, 0x7FF012340000, 1),
            (0x20000000, 0x20010000, 0x7FF012340000, 2),
            (0x30000000, 0x30010000, 0x7FF012340000, 0x80),
            (0x7FF000000000, 0x7FF000100000, 0x7FF012340000, 0xFFFFFFFF),
        ]
        for source, destination, function, count in cases:
            expected = native.build_batch_wrapper(source, destination, function, count)
            self.assert_emits(
                {
                    "kind": "batch",
                    "source": source,
                    "destination": destination,
                    "function": function,
                    "count": count,
                },
                expected,
                f"batch {count}",
            )

    def test_the_seed_range_wrapper_matches_the_shipped_bytes(self) -> None:
        cases = [(0, 1, 1), (0x0BADF00D, 1, 8), (0xFFFFFF00, 0x10, 0x20)]
        for start, step, count in cases:
            expected = native.build_seed_range_wrapper(
                0x10000000, 0x10010000, 0x7FF012340000, start, step, count
            )
            self.assert_emits(
                {
                    "kind": "seed_range",
                    "source": 0x10000000,
                    "destination": 0x10010000,
                    "function": 0x7FF012340000,
                    "start_seed": start,
                    "seed_step": step,
                    "count": count,
                },
                expected,
                f"seed range {start}/{step}/{count}",
            )

    def test_the_effect_finalizer_wrappers_match_the_shipped_bytes(self) -> None:
        for index in range(7):
            for reveal in (True, False):
                expected = native.build_effect_finalizer_wrapper(
                    0x10000000, 0x10010000, 0x7FF022799A8, index, reveal
                )
                self.assert_emits(
                    {
                        "kind": "finalizer",
                        "source": 0x10000000,
                        "destination": 0x10010000,
                        "function": 0x7FF022799A8,
                        "effect_index": index,
                        "reveal": reveal,
                    },
                    expected,
                    f"finalizer {index}/{reveal}",
                )
        for count in (1, 2, 0x40):
            expected = native.build_effect_finalizer_batch_wrapper(
                0x10000000, 0x10010000, 0x7FF022799A8, count, 4, True
            )
            self.assert_emits(
                {
                    "kind": "finalizer_batch",
                    "source": 0x10000000,
                    "destination": 0x10010000,
                    "function": 0x7FF022799A8,
                    "count": count,
                    "effect_index": 4,
                    "reveal": True,
                },
                expected,
                f"finalizer batch {count}",
            )

    def test_the_explicit_playthrough_wrapper_matches_the_shipped_bytes(self) -> None:
        profile = native.DEFAULT_NATIVE_RUNTIME_PROFILE
        # The Rust side resolves the chain by site name from the approved
        # profile, so the shipped RVA table is named here on purpose.
        self.assertEqual(native.INIT_COMPACT_RVA, 0x1B6F650)
        self.assertEqual(native.RESET_COMPACT_RVA, 0x1BBADBC)
        self.assertEqual(native.EFFECTIVE_LEVEL_RVA, 0x3DB834)
        self.assertEqual(native.INIT_GENERATION_CONTEXT_RVA, 0x570DF8)
        self.assertEqual(native.INCOMPLETE_RECORD_RVA, 0x110BF30)
        self.assertEqual(native.GENERATE_EFFECTS_RVA, 0x577964)
        self.assertEqual(native.ASSEMBLE_SCROLL_RVA, 0x2277FE8)
        self.assertEqual(native.PLAYTHROUGH_VECTOR_RVA, 0x578CD4)
        self.assertEqual(native.PLAYTHROUGH_MANAGER_POINTER_RVA, 0x47494A0)
        for playthrough in (1, 3, 5):
            for mode in (0, 1):
                expected = native.build_explicit_playthrough_seed_range_wrapper(
                    0x10000000,
                    0x10010000,
                    0x7FF000000000,
                    0x1000,
                    1,
                    4,
                    playthrough,
                    mode,
                    profile,
                )
                self.assert_emits(
                    {
                        "kind": "explicit_playthrough",
                        "source": 0x10000000,
                        "destination": 0x10010000,
                        "module_base": 0x7FF000000000,
                        "start_seed": 0x1000,
                        "seed_step": 1,
                        "count": 4,
                        "playthrough": playthrough,
                        "generation_mode": mode,
                    },
                    expected,
                    f"explicit playthrough {playthrough}/{mode}",
                )

    def test_the_source_record_matches_the_shipped_bytes(self) -> None:
        template = bytes(range(RECORD_SIZE))
        for seed in (0x0BADF00D, 1):
            expected = native.build_source_record(
                template,
                seed=seed,
                rarity=4,
                level=180,
                recommended_level=183,
                transfer_count=2,
            )
            self.assert_emits(
                {
                    "kind": "source_record",
                    "template_hex": template.hex(),
                    "seed": seed,
                    "rarity": 4,
                    "level": 180,
                    "recommended_level": 183,
                    "transfer_count": 2,
                },
                expected,
                f"source record {seed:#x}",
            )

    def test_the_live_add_shim_matches_the_shipped_bytes(self) -> None:
        memory = 0x10000000
        resume = 0x7FF020BB2C
        original = bytes.fromhex(LAYOUT.dispatch_signature_hex)
        insertion = {
            "serial": 0x1122334455667788,
            "data": 0x200000000,
            "manager": 0x200010000,
            "function_address": 0x7FF054D294,
        }
        cases = [
            (None, 0, None, None, False),
            (0x7FF0227C4CC, memory + 0x600, memory + 0x400, None, False),
            (0x7FF0227C4CC, memory + 0x600, memory + 0x400, None, True),
            (0x7FF0227C4CC, memory + 0x600, memory + 0x400, insertion, False),
            (0x7FF0227C4CC, memory + 0x600, memory + 0x400, insertion, True),
        ]
        for leaf, argument, second_argument, branch, preserve in cases:
            expected = build_dispatch_code(
                memory,
                resume,
                original,
                leaf=leaf,
                argument=argument,
                second_argument=second_argument,
                insertion=dict(branch) if branch is not None else None,
                preserve_rarity5=preserve,
            )
            request = {
                "kind": "dispatch",
                "memory": memory,
                "resume": resume,
                "original_hex": original.hex(),
                "argument": argument,
                "preserve_rarity5": preserve,
            }
            if leaf is not None:
                request["leaf"] = leaf
            if second_argument is not None:
                request["second_argument"] = second_argument
            if branch is not None:
                request["insertion"] = branch
            self.assert_emits(
                request, expected, f"dispatch leaf={leaf} insert={branch is not None}"
            )

    def test_a_refusal_is_reported_for_every_bad_shape(self) -> None:
        refusals = [
            {"kind": "batch", "source": 1, "destination": 2, "function": 3, "count": 0},
            {
                "kind": "seed_range",
                "source": 1,
                "destination": 2,
                "function": 3,
                "start_seed": 0,
                "seed_step": 0,
                "count": 1,
            },
            {
                "kind": "finalizer",
                "source": 1,
                "destination": 2,
                "function": 3,
                "effect_index": 7,
                "reveal": True,
            },
            {
                "kind": "source_record",
                "template_hex": "00",
                "seed": 1,
                "rarity": 4,
                "level": 1,
                "recommended_level": 1,
            },
            {
                "kind": "explicit_playthrough",
                "source": 1,
                "destination": 2,
                "module_base": 3,
                "start_seed": 0,
                "seed_step": 1,
                "count": 1,
                "playthrough": 6,
            },
        ]
        for request in refusals:
            with self.subTest(kind=request["kind"]):
                with self.assertRaises(_Refused):
                    self.emit(request)


class ScopedRightsAuditTest(unittest.TestCase):
    """The rights contract, module by module, with the reason written down."""

    def source(self, relative: str) -> str:
        path = CRATE / "src" / relative
        self.assertTrue(path.is_file(), f"missing module {path}")
        return path.read_text(encoding="utf-8")

    def all_modules(self) -> list:
        files = sorted((CRATE / "src").rglob("*.rs"))
        self.assertGreater(len(files), 10, "the crate's modules are all scanned")
        return files

    def test_the_shipped_python_still_holds_the_facts_this_gate_names(self) -> None:
        override = (ROOT / "nioh3_scroll_editor" / "runtime_auxiliary_override.py").read_text(
            encoding="utf-8"
        )
        count = (ROOT / "nioh3_scroll_editor" / "runtime_count_edit.py").read_text(
            encoding="utf-8"
        )
        ships = (ROOT / "nioh3_scroll_editor" / "native.py").read_text(encoding="utf-8")
        debug = (ROOT / "nioh3_scroll_editor" / "windows_debug_session.py").read_text(
            encoding="utf-8"
        )
        for text in (override, count):
            self.assertNotIn("CreateRemoteThread", text)
            self.assertNotIn("PROCESS_ALL_ACCESS", text)
            self.assertNotIn("TerminateProcess", text)
            self.assertNotIn("DebugActiveProcess", text)
        self.assertIn("self.dll.CreateRemoteThread", ships)
        self.assertIn(
            "PROCESS_ACCESS = 0x0002 | 0x0008 | 0x0010 | 0x0020 | 0x0400", ships
        )
        self.assertIn("DebugActiveProcess", debug)
        self.assertIn("DebugSetProcessKillOnExit(False)", debug)

    def test_the_shipped_mask_and_layout_are_the_ones_declared(self) -> None:
        self.assertEqual(native.PROCESS_ACCESS, 0x0002 | 0x0008 | 0x0010 | 0x0020 | 0x0400)
        self.assertEqual(LAYOUT.dispatch_rva, 0x12E6840)
        self.assertEqual(LAYOUT.insertion_rva, 0x54D294)
        self.assertEqual(LAYOUT.capacity, 400)
        self.assertEqual(LAYOUT.record_size, 0xE8)
        self.assertEqual(LAYOUT.descriptor_size, 0xCC)
        abi = code_only(self.source(PURE_MODULE))
        self.assertIn(
            "pub const RUNTIME_ACCESS: u32 = 0x0002 | 0x0008 | 0x0010 | 0x0020 | 0x0400;",
            abi,
            "the one mask the two wider callers share is declared exactly once",
        )

    def test_no_module_asks_for_a_wider_right_than_its_reason(self) -> None:
        for path in self.all_modules():
            relative = path.relative_to(CRATE / "src").as_posix()
            text = code_only(path.read_text(encoding="utf-8"))
            with self.subTest(module=relative):
                for banned in FORBIDDEN_EVERYWHERE:
                    self.assertNotIn(banned, text, f"{relative} names {banned}")
                if relative not in THREAD_CREATING_SOURCES:
                    self.assertNotIn(
                        "CreateRemoteThread", text, f"{relative} creates a remote thread"
                    )
                    self.assertNotIn(
                        "SetThreadContext", text, f"{relative} writes a thread context"
                    )
                if relative != DEBUG_OWNER:
                    self.assertNotIn(
                        "DebugActiveProcess", text, f"{relative} takes debug ownership"
                    )

    def test_the_two_wider_callers_declare_the_shared_right(self) -> None:
        for relative in THREAD_CREATING_SOURCES:
            with self.subTest(module=relative):
                text = code_only(self.source(relative))
                self.assertIn("RUNTIME_ACCESS", text, f"{relative} must name the mask")

    def test_the_debug_owner_can_never_kill_the_target(self) -> None:
        text = code_only(self.source(DEBUG_OWNER))
        self.assertIn("DebugActiveProcess", text, "the shipped debug view is ported")
        self.assertIn(
            "DebugSetProcessKillOnExit(0)",
            text,
            "the debugger must never kill the target on exit",
        )

    def test_the_abi_module_is_pure(self) -> None:
        text = code_only(self.source(PURE_MODULE))
        for capability in (
            "OpenProcess",
            "ReadProcessMemory",
            "WriteProcessMemory",
            "VirtualAllocEx",
            "CreateRemoteThread",
            "DebugActiveProcess",
            "SetThreadContext",
        ):
            self.assertNotIn(capability, text, f"native_abi.rs names {capability}")


if __name__ == "__main__":
    unittest.main()
