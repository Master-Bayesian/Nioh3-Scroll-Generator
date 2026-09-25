"""Read-only parity gate for the M3 runtime adapter.

`crates/nioh3-runtime` ports the read surface the shipped runtime host uses: the
ownership state behind `RuntimeApplication.status()`, process discovery and
executable-identity checks, the approved native runtime profile, and the
read-before-dereference rules of `native.NativeBatchOracle.open`.

This gate compares that port with the shipped Python implementation on the same
inputs: synthetic ownership sequences are driven through a real
`RuntimeApplication`, the shipped research profile document is parsed by both
loaders, and the Windows probes are compared process-to-process on this
machine. Nothing here opens a writable handle, launches or closes a process,
touches a save, or mutates the game.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import nioh3_scroll_editor.native as native_module  # noqa: E402
from nioh3_scroll_editor.game_compatibility import (  # noqa: E402
    _file_version,
    verify_game_executable,
)
from nioh3_scroll_editor.native import (  # noqa: E402
    DEFAULT_NATIVE_RUNTIME_PROFILE,
    find_module_base,
    find_nioh3_pid,
    find_nioh3_pids,
    load_native_runtime_profile,
    native_runtime_profile_for_game_version,
)
from nioh3_scroll_editor.process_instance import process_creation_time  # noqa: E402
from nioh3_scroll_editor.runtime_application import (  # noqa: E402
    RuntimeApplication,
    running_game_identity,
)
from tests.migration.cargo_target import resolved_cargo_target_dir  # noqa: E402

CRATE = ROOT / "crates" / "nioh3-runtime"
PROFILE_DIR = ROOT / "nioh3_scroll_editor" / "data" / "game_versions"
PROFILE_V201 = PROFILE_DIR / "pc_v2_01.json"
PROBE = "runtime_read_probe"

STATUS_KEYS = (
    "override_state",
    "hit_count",
    "pending_remote_calls",
    "safe_to_shutdown",
    "error",
)

# Sites the shipped profile loader exposes, mapped to this probe's site names.
PROFILE_SITE_FIELDS = {
    "canonicalize": ("canonicalize_rva", "canonicalize_signature"),
    "completion_finalizer_wrapper": ("finalize_effect_rva", "finalize_effect_signature"),
    "descriptor_complete": ("descriptor_complete_rva", "descriptor_complete_signature"),
    "init_compact": ("init_compact_rva", None),
    "reset_compact": ("reset_compact_rva", None),
    "effective_level": ("effective_level_rva", None),
    "init_generation_context": ("init_generation_context_rva", None),
    "incomplete_record": ("incomplete_record_rva", None),
    "generate_effects": ("generate_effects_rva", None),
    "assemble_scroll": ("assemble_scroll_rva", None),
    "playthrough_vector": ("playthrough_vector_rva", None),
}

# `NativeBatchOracle.open` verifies canonicalize, the finalizer and the eight
# chain sites.
VERIFIED_SITE_COUNT = 10

# Nothing in this adapter may be able to change the inspected process.
FORBIDDEN_WRITE_CAPABILITIES = (
    "PROCESS_VM_WRITE",
    "PROCESS_VM_OPERATION",
    "PROCESS_TERMINATE",
    "PROCESS_SUSPEND_RESUME",
    "PROCESS_CREATE_THREAD",
    "WriteProcessMemory",
    "VirtualAllocEx",
    "VirtualProtectEx",
    "VirtualFreeEx",
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


def runtime_target_dir() -> str:
    """The v0.8.0 shared build cache.

    The repository volume has no room for another cargo target directory, and
    the shared cache keeps this gate from fighting a locked worker build.
    """

    explicit = os.environ.get("NIOH3_RUNTIME_CARGO_TARGET", "").strip()
    if explicit:
        return explicit
    return resolved_cargo_target_dir("m3-runtime-read")


def build_probe() -> Path:
    cargo = shutil.which("cargo")
    if cargo is None:
        raise AssertionError("cargo is required to build the runtime adapter probe")
    target = runtime_target_dir()
    completed = subprocess.run(
        [
            cargo,
            "build",
            "--locked",
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
            "the runtime adapter probe did not build: "
            + (completed.stderr.strip() or completed.stdout.strip())[-4000:]
        )
    binary = Path(target) / "debug" / "examples" / f"{PROBE}.exe"
    if not binary.is_file():
        raise AssertionError(f"the runtime adapter probe is missing at {binary}")
    return binary


def candidate_profile_dir(root: Path) -> Path:
    """A directory the shipped loader will resolve `pc_v2_01.json` from.

    `native_runtime_profile_for_game_version` builds its path from the module's
    `__file__`, so a scoped patch of that attribute lets the real shipped code
    read a candidate document without any copy of the product loader.
    """

    module_path = root / "nioh3_scroll_editor" / "native.py"
    directory = module_path.parent / "data" / "game_versions"
    directory.mkdir(parents=True, exist_ok=True)
    module_path.touch()
    return directory


def code_only(text: str) -> str:
    """Source with `//` comment lines removed.

    The adapter documents the process rights it deliberately never requests, so
    the write-capability audit has to look at code, not prose.
    """

    kept = []
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("//"):
            continue
        kept.append(line.split("//", 1)[0])
    return strip_string_literals("\n".join(kept))


def strip_string_literals(text: str) -> str:
    """Remove Rust string literals, so an operator message cannot shadow a call.

    `error.rs` names `WriteProcessMemory` and `VirtualProtectEx` inside the
    messages it renders. Naming an API in a message is not calling it, so the
    audit drops literals and looks at code.
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


class _Session:
    def __init__(self, hits: int) -> None:
        self._hits = hits

    def hit_count(self) -> int:
        return self._hits


class _FaultySession:
    """A session whose hit count fails exactly like an unreadable override."""

    def __init__(self, message: str) -> None:
        self._message = message

    def hit_count(self) -> int:
        raise OSError(self._message)


class _RetiredOracle:
    def __init__(self, pending: bool) -> None:
        self.remote_call_pending = pending


class _LiveAdd:
    def __init__(self, unsafe: bool) -> None:
        self._unsafe = unsafe

    def safe_to_shutdown(self) -> bool:
        return not self._unsafe


def shipped_status_steps(ops: list[dict]) -> list[dict]:
    """`RuntimeApplication.status()` over the ownership state only.

    The shipped host answers `runtime.status` without touching the game process,
    so this drives a real instance with stub owners instead of starting the
    protected host. No service, oracle, profile or override is constructed.
    """

    application = RuntimeApplication(service=object())
    steps: list[dict] = []
    for index, op in enumerate(ops):
        name = op["op"]
        if name == "retire":
            application.retired_oracles.append(_RetiredOracle(bool(op["pending"])))
        elif name == "live_add":
            application.live_add = _LiveAdd(bool(op["unsafe"]))
        elif name == "session":
            application.session = _Session(int(op["hits"]))
        elif name == "session_fault":
            application.session = _FaultySession(str(op["message"]))
        elif name == "session_stop":
            application.session = None
        else:
            raise AssertionError(f"unknown status op {name}")
        status = application.status()
        steps.append(
            {
                "step": index,
                "status": {key: status[key] for key in STATUS_KEYS},
                "retired_retained": len(application.retired_oracles),
            }
        )
    return steps


STATUS_SCENARIOS: dict[str, list[dict]] = {
    "retired_prune": [{"op": "retire", "pending": True}, {"op": "retire", "pending": False}],
    "live_add_ownership": [
        {"op": "live_add", "unsafe": True},
        {"op": "live_add", "unsafe": False},
    ],
    "override_lifecycle": [
        {"op": "session", "hits": 0},
        {"op": "session", "hits": 4},
        {"op": "session_stop"},
    ],
    "faulted_override": [
        {"op": "retire", "pending": True},
        {"op": "session_fault", "message": "device not ready"},
        {"op": "session_stop"},
        {"op": "live_add", "unsafe": True},
    ],
    "cleared_and_pending": [
        {"op": "retire", "pending": True},
        {"op": "retire", "pending": True},
        {"op": "session", "hits": 2},
        {"op": "session_stop"},
    ],
    "mixed_ownership": [
        {"op": "retire", "pending": False},
        {"op": "live_add", "unsafe": True},
        {"op": "session", "hits": 1},
        {"op": "session_stop"},
        {"op": "retire", "pending": False},
    ],
}


class RuntimeReadParityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.binary = build_probe()
        cls.temp = tempfile.TemporaryDirectory()
        cls.root = Path(cls.temp.name)
        cls.system_dll_copy = None

    @classmethod
    def tearDownClass(cls) -> None:
        cls.temp.cleanup()

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
                f"runtime_read_probe {' '.join(arguments)} failed: "
                + (completed.stdout + completed.stderr).strip()
            )
        return [line for line in completed.stdout.splitlines() if line]

    def probe_rows(self, *arguments: str) -> list[list[str]]:
        return [line.split("\t") for line in self.probe(*arguments)]

    def single_row(self, *arguments: str) -> list[str]:
        rows = self.probe_rows(*arguments)
        self.assertEqual(len(rows), 1, f"expected one row from {arguments}: {rows}")
        return rows[0]

    def status_snapshots(self, ops: list[dict]) -> list[tuple[int, dict]]:
        payload = json.dumps({"ops": ops}, separators=(",", ":"))
        snapshots = []
        for line in self.probe("--status-ops", payload):
            if not line.startswith("snapshot\t"):
                continue
            _, index, body = line.split("\t", 2)
            snapshots.append((int(index), json.loads(body)))
        return snapshots

    def write_profile(self, payload: dict) -> Path:
        directory = Path(tempfile.mkdtemp(dir=type(self).root))
        path = directory / "pc_v2_01.json"
        path.write_text(json.dumps(payload), encoding="utf-8")
        return path

    # -- runtime.status state machine -------------------------------------

    def test_status_state_machine_matches_the_shipped_runtime_application(self) -> None:
        for name, ops in STATUS_SCENARIOS.items():
            with self.subTest(scenario=name):
                rows = self.status_snapshots(ops)
                expected = shipped_status_steps(ops)
                self.assertEqual(len(rows), len(expected), rows)
                for position, ((step, actual), want) in enumerate(zip(rows, expected)):
                    self.assertEqual(step, want["step"], f"{name} step {position}")
                    for key in STATUS_KEYS:
                        self.assertEqual(
                            actual[key], want["status"][key], f"{name} step {position} {key}"
                        )
                    self.assertEqual(
                        actual["retired_retained"],
                        want["retired_retained"],
                        f"{name} step {position} retained oracles",
                    )

    def test_a_fresh_host_is_idle_and_safe_to_shutdown(self) -> None:
        rows = self.status_snapshots([{"op": "session_stop"}])
        self.assertEqual(len(rows), 1)
        step, rust = rows[0]
        self.assertEqual(step, 0)
        shipped = RuntimeApplication(service=object()).status()
        for key in STATUS_KEYS:
            self.assertEqual(rust[key], shipped[key], key)
        self.assertEqual(rust["override_state"], "stopped")
        self.assertEqual(rust["safe_to_shutdown"], True)
        self.assertEqual(rust["retired_retained"], 0)

    # -- approved profiles ------------------------------------------------

    def test_profile_document_matches_the_shipped_python_loader(self) -> None:
        profile = load_native_runtime_profile(PROFILE_V201)
        rows = self.probe_rows("--profile", str(PROFILE_V201))
        sites = {row[1]: row for row in rows if row[0] == "site"}
        self.assertEqual(len(sites), len(PROFILE_SITE_FIELDS))

        signatures = dict(profile.native_signatures)
        for name, (rva_field, signature_field) in PROFILE_SITE_FIELDS.items():
            self.assertIn(name, sites, name)
            expected_rva = getattr(profile, rva_field)
            self.assertEqual(int(sites[name][2], 16), expected_rva, f"{name} rva")
            if signature_field is None:
                expected_signature = signatures[expected_rva]
            else:
                expected_signature = getattr(profile, signature_field)
            self.assertEqual(sites[name][3], expected_signature.hex(), f"{name} signature")

        self.assertEqual(
            [row for row in rows if row[0] == "display"],
            [["display", profile.display_version]],
        )
        # The document keeps extra research-only sites; the port resolves the
        # same named subset the shipped loader consumes, and no less.
        payload = json.loads(PROFILE_V201.read_text(encoding="utf-8"))
        self.assertTrue(set(PROFILE_SITE_FIELDS) <= set(payload["text_sites"]))
        data = [row for row in rows if row[0] == "data"]
        self.assertEqual(len(data), 1)
        self.assertEqual(data[0][1], "playthrough_selector_pointer")
        self.assertEqual(
            int(data[0][2], 16),
            profile.playthrough_manager_pointer_rva,
            "playthrough selector data site",
        )

        digest = [row[1] for row in rows if row[0] == "digest"]
        self.assertEqual(len(digest), 1)
        self.assertRegex(digest[0], r"^[0-9a-f]{64}$")

    def test_profile_rejections_match_the_shipped_loader(self) -> None:
        payload = json.loads(PROFILE_V201.read_text(encoding="utf-8"))

        cases = []
        foreign = dict(payload, schema="other-schema/v1")
        cases.append((foreign, "PROFILE_SCHEMA"))
        unresolved = json.loads(json.dumps(payload))
        del unresolved["text_sites"]["init_compact"]["rva"]
        cases.append((unresolved, "PROFILE_UNRESOLVED"))
        unsigned = json.loads(json.dumps(payload))
        del unsigned["text_sites"]["generate_effects"]["captured_signature"]
        cases.append((unsigned, "PROFILE_MISSING_SIGNATURE"))

        for candidate, code in cases:
            with self.subTest(code=code):
                path = self.write_profile(candidate)
                with self.assertRaises(ValueError) as context:
                    load_native_runtime_profile(path)
                row = self.single_row("--profile", str(path))
                self.assertEqual(row[0], "error")
                self.assertEqual(row[1], code)
                self.assertEqual(row[2], str(context.exception))

    def test_version_selection_matches_the_shipped_approval_gate(self) -> None:
        payload = json.loads(PROFILE_V201.read_text(encoding="utf-8"))

        # v2.00.02 is hardcoded on both sides and needs no profile document.
        sites = {
            row[1]: row
            for row in self.probe_rows(
                "--profile-for-version", "2.0.0.2", str(type(self).root)
            )
            if row[0] == "site"
        }
        shipped_default = native_runtime_profile_for_game_version((2, 0, 0, 2))
        self.assertIs(shipped_default, DEFAULT_NATIVE_RUNTIME_PROFILE)
        for name, (rva_field, signature_field) in PROFILE_SITE_FIELDS.items():
            expected_rva = getattr(shipped_default, rva_field)
            self.assertEqual(int(sites[name][2], 16), expected_rva, f"{name} rva")
            expected_signature = (
                dict(shipped_default.native_signatures)[expected_rva]
                if signature_field is None
                else getattr(shipped_default, signature_field)
            )
            self.assertEqual(sites[name][3], expected_signature.hex(), f"{name} signature")

        # The shipped approved v2.01 document resolves on both sides.
        sites = {
            row[1]: row
            for row in self.probe_rows("--profile-for-version", "2.0.1.0", str(PROFILE_DIR))
            if row[0] == "site"
        }
        shipped_v201 = native_runtime_profile_for_game_version((2, 0, 1, 0))
        for name, (rva_field, _) in PROFILE_SITE_FIELDS.items():
            self.assertEqual(
                int(sites[name][2], 16), getattr(shipped_v201, rva_field), f"{name} rva"
            )

        # An unapproved document fails closed on both sides with the same text.
        unapproved = dict(payload, approval_status="pending")
        redirect = candidate_profile_dir(Path(tempfile.mkdtemp(dir=type(self).root)))
        (redirect / "pc_v2_01.json").write_text(json.dumps(unapproved), encoding="utf-8")
        with mock.patch.object(
            native_module,
            "__file__",
            str(redirect.parent.parent / "native.py"),
        ):
            with self.assertRaises(RuntimeError) as context:
                native_module.native_runtime_profile_for_game_version((2, 0, 1, 0))
        row = self.single_row("--profile-for-version", "2.0.1.0", str(redirect))
        self.assertEqual(row[0], "error")
        self.assertEqual(row[1], "PROFILE_NOT_APPROVED")
        self.assertEqual(row[2], str(context.exception))

        # An unknown executable version is rejected, not approximated.
        with self.assertRaises(ValueError) as context:
            native_runtime_profile_for_game_version((2, 0, 1, 1))
        row = self.single_row("--profile-for-version", "2.0.1.1", str(PROFILE_DIR))
        self.assertEqual(row[0], "error")
        self.assertEqual(row[1], "UNSUPPORTED_GAME_VERSION")
        self.assertEqual(row[2], str(context.exception))

    def test_site_ranges_are_validated_against_the_module_image(self) -> None:
        payload = json.loads(PROFILE_V201.read_text(encoding="utf-8"))
        for size in (0x1000, 0x2300000, 0x40000000):
            with self.subTest(size=hex(size)):
                rows = self.probe_rows("--bounds", str(PROFILE_V201), hex(size))
                bounds = {row[1]: row[2] for row in rows if row[0] == "bounds"}
                self.assertEqual(set(bounds), set(PROFILE_SITE_FIELDS))
                for name in PROFILE_SITE_FIELDS:
                    raw = payload["text_sites"][name]
                    end = int(str(raw["rva"]), 0) + len(bytes.fromhex(raw["captured_signature"]))
                    self.assertEqual(
                        bounds[name],
                        "ok" if end <= size else "RANGE_OUT_OF_BOUNDS",
                        f"{name} at {hex(size)}",
                    )
                self.assertEqual(
                    [row[1] for row in rows if row[0] == "module_size"], [hex(size)]
                )

    # -- read-only guarantees ---------------------------------------------

    def test_the_adapter_has_no_write_or_process_lifecycle_api(self) -> None:
        """The read adapter stays read-only.

        The mutation slice (`src/mutation/` and the probe's mutation modes) is
        allowed the exact write masks the shipped override and count code uses.
        That permission is not free: `test_runtime_mutation_parity.py` asserts
        the shipped mask values are reproduced exactly, audits every module for
        wider rights, and is the only place that may admit a write capability.
        This test keeps the read modules themselves free of any write or
        lifecycle capability in code.
        """

        for name in ("lib.rs", "error.rs", "status.rs", "profile.rs", "platform.rs"):
            text = code_only((CRATE / "src" / name).read_text(encoding="utf-8"))
            for capability in FORBIDDEN_WRITE_CAPABILITIES:
                with self.subTest(source=name, capability=capability):
                    self.assertNotIn(capability, text, f"{capability} in {name}")

    # -- Windows read API probes ------------------------------------------

    @unittest.skipUnless(sys.platform == "win32", "the process adapter is Windows-only")
    def test_process_creation_identity_matches_the_shipped_helper(self) -> None:
        pid = os.getpid()
        row = self.single_row("--creation-time", str(pid))
        expected = process_creation_time(pid)
        self.assertIsNotNone(expected)
        self.assertEqual(row, ["creation", expected])

    @unittest.skipUnless(sys.platform == "win32", "the process adapter is Windows-only")
    def test_process_discovery_matches_the_shipped_snapshot(self) -> None:
        found = sorted(
            int(row[1]) for row in self.probe_rows("--discover", "Nioh3.exe") if row[0] == "pid"
        )
        self.assertEqual(found, sorted(find_nioh3_pids()))

        single = self.single_row("--single", "Nioh3.exe")
        if not found:
            self.assertEqual(single, ["absent", "0"])
            with self.assertRaises(RuntimeError):
                find_nioh3_pid()
        elif len(found) == 1:
            self.assertEqual(single, ["pid", str(found[0])])
            self.assertEqual(find_nioh3_pid(), found[0])
        else:
            self.assertEqual(single[0], "error")
            self.assertEqual(single[1], "AMBIGUOUS_PROCESS")
            with self.assertRaises(RuntimeError):
                find_nioh3_pid()

    @unittest.skipUnless(sys.platform == "win32", "the process adapter is Windows-only")
    def test_executable_version_checks_match_the_shipped_helper(self) -> None:
        for path in (Path(sys.executable), self.system_dll_outside_windows_directory()):
            with self.subTest(path=str(path)):
                display = ".".join(str(part) for part in _file_version(path))
                self.assertEqual(
                    self.single_row("--file-version", str(path)), ["version", display]
                )
                status = verify_game_executable(path)
                self.assertEqual(
                    self.single_row("--verify-executable", str(path)),
                    ["state", status.state, display],
                )

    def system_dll_outside_windows_directory(self) -> Path:
        """A system DLL copied out of `%SystemRoot%`, where both sides agree.

        This is a fixture choice, not a claim about where a game may live: the
        version-parity differential needs two implementations that see the same
        bytes, and a copy outside `%SystemRoot%` gives that.
        `test_windows_directory_versions_fail_closed` pins the inside-the-directory
        difference itself.
        """

        cls = type(self)
        if cls.system_dll_copy is None:
            source = Path(os.environ.get("WINDIR", r"C:\Windows")) / "System32" / "ntdll.dll"
            if not source.is_file():
                self.skipTest("no system DLL to copy")
            destination = cls.root / "system-image.dll"
            shutil.copyfile(source, destination)
            cls.system_dll_copy = destination
        return cls.system_dll_copy

    @unittest.skipUnless(sys.platform == "win32", "the process adapter is Windows-only")
    def test_windows_directory_versions_fail_closed(self) -> None:
        """A bounded, machine-specific counterexample - not a rule.

        On this machine an unmanifested process is told the OS compatibility
        version for version resources under `%SystemRoot%`. That is not a
        general claim that a game can never live under `%SystemRoot%`, and it is
        not evidence that the ported version check is right: the game-version
        proof is the actual matching executable's resource plus its approved
        profile, which `test_version_selection_matches_the_shipped_approval_gate`
        and the executable checks above cover. This test only pins what the
        difference looks like and that it cannot select a supported version.
        """

        system_dll = Path(os.environ.get("WINDIR", r"C:\Windows")) / "System32" / "ntdll.dll"
        if not system_dll.is_file():
            self.skipTest("no system DLL to probe")

        shipped = _file_version(system_dll)
        row = self.single_row("--file-version", str(system_dll))
        rust = tuple(int(part) for part in row[1].split("."))
        self.assertEqual(
            shipped[2:], rust[2:], "the build and revision of one file must agree"
        )
        if shipped[:2] != rust[:2]:
            # The unmanifested probe is told the OS compatibility version.
            self.assertEqual(shipped[:2], (10, 0))
            self.assertEqual(rust[:2], (6, 2))
            self.assertNotIn(
                rust,
                ((2, 0, 0, 2), (2, 0, 1, 0)),
                "the compatibility version can never select a game profile",
            )
            self.assertEqual(
                self.single_row("--verify-executable", str(system_dll))[1],
                "unsupported",
                "an unreadable-under-manifest version still fails closed",
            )
        copied = self.system_dll_outside_windows_directory()
        self.assertEqual(
            self.single_row("--file-version", str(copied)),
            ["version", ".".join(str(part) for part in _file_version(copied))],
            "outside the Windows directory the two implementations agree exactly",
        )

    @unittest.skipUnless(sys.platform == "win32", "the process adapter is Windows-only")
    def test_an_unreadable_executable_is_not_reported_as_unsupported(self) -> None:
        missing = type(self).root / "absent-executable.exe"
        self.assertFalse(missing.is_file())
        self.assertEqual(verify_game_executable(missing).state, "unreadable")
        self.assertEqual(
            self.single_row("--verify-executable", str(missing)),
            ["state", "unreadable", "-"],
        )
        error = self.single_row("--file-version", str(missing))
        self.assertEqual(error[0], "error")
        self.assertEqual(error[1], "FILE_VERSION_UNREADABLE")

    @unittest.skipUnless(sys.platform == "win32", "the process adapter is Windows-only")
    def test_module_range_matches_the_shipped_lookup(self) -> None:
        pid = os.getpid()
        module_name = Path(sys.executable).name
        row = self.single_row("--module", str(pid), module_name)
        if row[0] == "error":
            self.assertEqual(row[1], "MODULE_NOT_FOUND")
            with self.assertRaises(RuntimeError):
                find_module_base(pid, module_name)
            return
        self.assertEqual(row[0], "module")
        self.assertEqual(int(row[1], 16), find_module_base(pid, module_name))
        self.assertGreater(int(row[2], 16), 0, "a loaded image has a non-zero size")

    @unittest.skipUnless(sys.platform == "win32", "the process adapter is Windows-only")
    def test_running_game_identity_reports_absence_without_a_generic_failure(self) -> None:
        row = self.single_row("--identify", "Nioh3.exe", "Nioh3.exe", str(PROFILE_DIR))
        if not find_nioh3_pids():
            self.assertEqual(row, ["absent", "0"])
            with self.assertRaises(RuntimeError):
                running_game_identity()
            return
        try:
            pid, profile, path = running_game_identity()
        except ValueError:
            # The running executable has no approved profile (PC v2.02 is still
            # a candidate): the shipped helper refuses, and so must the port,
            # by name rather than with a generic failure.
            self.assertEqual(row[:2], ["error", "PROFILE_NOT_APPROVED"])
            return
        self.assertEqual(row[0], "identity")
        self.assertEqual(int(row[1]), pid)
        self.assertEqual(row[3], path)
        self.assertEqual(int(row[6], 16), profile.canonicalize_rva)
        self.assertRegex(row[7], r"^[0-9a-f]{64}$")

    @unittest.skipUnless(sys.platform == "win32", "the process adapter is Windows-only")
    def test_signature_verification_is_read_only_and_reports_absence(self) -> None:
        # Every shipped runtime profile against whichever game is running: the
        # running image matches exactly one profile and refuses the others.
        rows = {
            path.name: self.single_row(
                "--verify-signatures", str(path), "Nioh3.exe", "Nioh3.exe"
            )
            for path in sorted(PROFILE_DIR.glob("pc_v*.json"))
            if ".research." not in path.name
        }
        self.assertIn(PROFILE_V201.name, rows)
        if not find_nioh3_pids():
            for name, row in rows.items():
                self.assertEqual(row, ["absent", "0"], name)
            return
        verified = [
            name
            for name, row in rows.items()
            if row == ["verified", str(VERIFIED_SITE_COUNT)]
        ]
        self.assertEqual(len(verified), 1, rows)
        for name, row in rows.items():
            if name not in verified:
                self.assertEqual(row[:2], ["error", "SIGNATURE_MISMATCH"], name)


if __name__ == "__main__":
    unittest.main()
