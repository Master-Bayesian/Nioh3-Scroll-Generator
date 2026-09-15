"""Differential acceptance for the Rust protected save host (M3 role host).

This gate drives the real Rust protected process and the shipped Python
protected worker over the shipped framed-JSON protocol, against the *same*
isolated encrypted fixture, and compares the protected save surface they serve.

It does not skip. A missing crate, a missing Cargo toolchain, a missing shipped
save component, or a stale binary is a failure, never a silent pass. The binary
is rebuilt from the current candidate before every run.

No user save and no game process is involved; every write is a task-local copy
under the D-backed fixture root, and only `save.*` methods that do not require a
running game are exercised.
"""

from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import tomllib
import unittest

from jsonschema import Draft7Validator


ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from tests.migration.test_save_read_parity import (  # noqa: E402
    build_fixture_bytes,
    native_transform,
    native_transform_short,
)
from tests.migration.cargo_target import resolved_cargo_target_dir  # noqa: E402

SCHEMA_DIR = ROOT / "packages" / "contracts"
MAX_FRAME_BYTES = 4 * 1024 * 1024
FIXTURE_ROOT = Path(
    os.environ.get(
        "NIOH3_PROTECTED_FIXTURE_ROOT",
        r"D:\Nioh3_v080_deliverables\m3-protected-host\fixtures",
    )
)
RESPONSE_VALIDATOR = Draft7Validator(
    json.loads((SCHEMA_DIR / "protected-response.schema.json").read_text(encoding="utf-8"))
)


class FramedWorker:
    """Framed protected-protocol client for one worker subprocess."""

    def __init__(self, argv: list[str], *, cwd: Path, env: dict[str, str], name: str) -> None:
        self.process = subprocess.Popen(
            argv,
            cwd=str(cwd),
            env=env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.name = name
        self.counter = 0

    def send(self, method: str, params: dict | None = None) -> str:
        self.counter += 1
        request_id = f"m3h-{self.counter}"
        body = json.dumps(
            {
                "protocol": 1,
                "id": request_id,
                "method": method,
                "params": params if params is not None else {},
            },
            separators=(",", ":"),
        ).encode("utf-8")
        assert self.process.stdin is not None
        self.process.stdin.write(struct.pack("<I", len(body)) + body)
        self.process.stdin.flush()
        return request_id

    def read_frame(self) -> dict:
        assert self.process.stdout is not None
        header = self.process.stdout.read(4)
        if len(header) != 4:
            raise EOFError(f"{self.name} closed stdout")
        (size,) = struct.unpack("<I", header)
        if size > MAX_FRAME_BYTES:
            raise ValueError(f"{self.name} frame exceeds the contract limit: {size}")
        body = self.process.stdout.read(size)
        if len(body) != size:
            raise EOFError(f"{self.name} frame truncated")
        frame = json.loads(body.decode("utf-8"))
        errors = sorted(RESPONSE_VALIDATOR.iter_errors(frame), key=lambda error: list(error.path))
        if errors:
            raise AssertionError(
                f"{self.name} frame violates protected-response.schema.json: "
                + "; ".join(f"{list(error.path)}: {error.message}" for error in errors)
            )
        return frame

    def call(self, method: str, params: dict | None = None) -> dict:
        self.send(method, params)
        return self.read_frame()

    def terminate(self) -> int:
        assert self.process.stdin is not None
        self.process.stdin.close()
        try:
            return self.process.wait(timeout=60)
        except subprocess.TimeoutExpired:
            self.process.kill()
            raise


def develop_protected_manifest() -> Path:
    """Return the protected crate manifest; a missing crate fails the gate."""

    manifest = ROOT / "crates" / "nioh3-protected" / "Cargo.toml"
    if not manifest.is_file():
        raise AssertionError(
            "the development protected worker crate is required by this gate and was not "
            f"found at {manifest}"
        )
    with manifest.open("rb") as stream:
        payload = tomllib.load(stream)
    bins = [str(entry.get("name", "")) for entry in payload.get("bin", [])]
    if bins[:1] != ["nioh3-protected-worker"]:
        raise AssertionError(f"unexpected protected worker bins: {bins}")
    return manifest


class ProtectedSaveParityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        manifest = develop_protected_manifest()
        cargo = shutil.which("cargo")
        if cargo is None:
            raise AssertionError("cargo is required to build and run the protected worker")
        env = dict(os.environ)
        env["CARGO_TARGET_DIR"] = resolved_cargo_target_dir("protected")
        build = subprocess.run(
            [
                cargo,
                "build",
                "--offline",
                "--manifest-path",
                str(manifest),
                "--bin",
                "nioh3-protected-worker",
            ],
            cwd=str(ROOT),
            env=env,
            capture_output=True,
            timeout=3600,
        )
        if build.returncode != 0:
            raise AssertionError(
                "the protected worker did not build from the current candidate: "
                + build.stderr.decode("utf-8", "replace")[-4000:]
            )
        cls.target = env["CARGO_TARGET_DIR"]
        cls.rust_binary = Path(cls.target) / "debug" / "nioh3-protected-worker.exe"
        cls.python = sys.executable
        FIXTURE_ROOT.mkdir(parents=True, exist_ok=True)
        cls.temp = tempfile.TemporaryDirectory(dir=str(FIXTURE_ROOT))
        cls.root = Path(cls.temp.name)
        cls.container = cls.root / "encrypted.bin"
        plain = cls.root / "plain.bin"
        plain.write_bytes(bytes(build_fixture_bytes()))
        native_transform(plain, cls.container)
        cls.fixture = cls.container.read_bytes()

    @classmethod
    def tearDownClass(cls) -> None:
        cls.temp.cleanup()

    def fixture_at(self, name: str) -> Path:
        """Create a fresh copy of the encrypted fixture and return its path."""

        base = self.root / name
        save = base / "76561198000000000" / "SAVEDATA00" / "SAVEDATA.BIN"
        save.parent.mkdir(parents=True)
        (save.parent / "BACKUP.BIN").write_bytes(b"game-backup")
        system = base / "76561198000000000" / "SYSTEMSAVEDATA00"
        system.mkdir()
        (system / "SAVEDATA.BIN").write_bytes(b"system-save")
        save.write_bytes(self.fixture)
        return save

    def state_at(self, name: str) -> Path:
        state = self.root / name
        state.mkdir()
        return state

    def isolated(self, name: str) -> tuple[Path, Path]:
        """A fresh state root plus a fresh copy of the encrypted fixture."""

        return self.fixture_at(name), self.state_at(f"{name}-state")

    def rust_worker(
        self,
        state: Path,
        role: str = "save",
        extra_env: dict[str, str] | None = None,
    ) -> FramedWorker:
        argv = [
            str(self.rust_binary),
            "--role",
            role,
            "--dev-protected-only",
            "--state-root",
            str(state),
            "--data-root",
            str(ROOT / "nioh3_scroll_editor" / "data"),
            "--contract-dir",
            str(SCHEMA_DIR),
        ]
        env = dict(os.environ)
        env["NIOH3_STATE_ROOT"] = str(state)
        env.update(extra_env or {})
        return FramedWorker(argv, cwd=ROOT, env=env, name="rust protected worker")

    def python_worker(
        self,
        state: Path,
        role: str = "save",
        extra_env: dict[str, str] | None = None,
    ) -> FramedWorker:
        argv = [
            self.python,
            "-u",
            "-m",
            "nioh3_scroll_editor.protected_worker",
            "--role",
            role,
        ]
        env = dict(os.environ)
        env["NIOH3_STATE_ROOT"] = str(state)
        env["PYTHONUTF8"] = "1"
        env["PYTHONIOENCODING"] = "utf-8"
        env.update(extra_env or {})
        return FramedWorker(argv, cwd=ROOT, env=env, name="python protected worker")

    def drive(self, worker: FramedWorker, method: str, params: dict) -> dict:
        """Start one protected job and poll it to a terminal state."""

        started = worker.call(method, params)
        if not started["ok"]:
            raise AssertionError(f"{method} refused: {started}")
        job = started["result"]
        while job["state"] in ("running", "cancel_requested"):
            snapshot = worker.call("job.snapshot", {"job_id": job["job_id"]})
            if not snapshot["ok"]:
                raise AssertionError(f"job.snapshot refused: {snapshot}")
            job = snapshot["result"]
        if job["state"] == "failed":
            raise AssertionError(f"{method} failed: {job['error']}")
        return job["result"]

    def test_handshake_register_inventory_and_template_match(self) -> None:
        # One shared, read-only fixture so identity hashes are comparable; the
        # two workers still own separate state roots.
        shared_save = self.fixture_at("read-shared")
        rust_state = self.state_at("read-rust-state")
        python_state = self.state_at("read-python-state")
        rust = self.rust_worker(rust_state)
        python = self.python_worker(python_state)
        try:
            rust_handshake = rust.call("handshake")
            python_handshake = python.call("handshake")
            self.assertTrue(rust_handshake["ok"], rust_handshake)
            self.assertTrue(python_handshake["ok"], python_handshake)
            self.assertEqual(rust_handshake["result"]["role"], "save")
            self.assertEqual(rust_handshake["result"]["kill_safe"], False)
            self.assertEqual(
                rust_handshake["result"]["contract_digest"],
                python_handshake["result"]["contract_digest"],
            )
            self.assertEqual(
                rust_handshake["result"]["context"],
                python_handshake["result"]["context"],
                "the protected host must publish the shipped generation identity",
            )

            rust_registered = self.drive(rust, "save.register", {"path": str(shared_save)})
            python_registered = self.drive(python, "save.register", {"path": str(shared_save)})
            self.assertEqual(rust_registered["path"], python_registered["path"])
            self.assertEqual(rust_registered["account_id"], python_registered["account_id"])
            self.assertEqual(rust_registered["save_slot"], python_registered["save_slot"])
            self.assertEqual(rust_registered["save_id"], python_registered["save_id"])

            rust_id = rust_registered["save_id"]
            python_id = python_registered["save_id"]
            rust_inventory = self.drive(rust, "save.inventory", {"save_id": rust_id})
            python_inventory = self.drive(python, "save.inventory", {"save_id": python_id})
            self.assertEqual(rust_inventory["source_sha256"], python_inventory["source_sha256"])
            self.assertEqual(rust_inventory["account_id"], python_inventory["account_id"])
            self.assertEqual(rust_inventory["empty_slots"], python_inventory["empty_slots"])
            self.assertEqual(
                rust_inventory["entries"],
                python_inventory["entries"],
                "protected inventory entries must match the shipped worker exactly",
            )

            rust_template = self.drive(
                rust,
                "save.template",
                {"save_id": rust_id, "snapshot_id": rust_inventory["snapshot_id"], "playthrough": 3},
            )
            python_template = self.drive(
                python,
                "save.template",
                {
                    "save_id": python_id,
                    "snapshot_id": python_inventory["snapshot_id"],
                    "playthrough": 3,
                },
            )
            self.assertEqual(rust_template, python_template)
        finally:
            self.assertEqual(rust.terminate(), 0)
            self.assertEqual(python.terminate(), 0)

    def test_delete_commit_receipt_and_restart_match(self) -> None:
        rust_save, rust_state = self.isolated("write-rust")
        python_save, python_state = self.isolated("write-python")
        rust = self.rust_worker(rust_state)
        python = self.python_worker(python_state)
        try:
            self.assertTrue(rust.call("handshake")["ok"])
            self.assertTrue(python.call("handshake")["ok"])
            rust_registered = self.drive(rust, "save.register", {"path": str(rust_save)})
            python_registered = self.drive(python, "save.register", {"path": str(python_save)})
            rust_id = rust_registered["save_id"]
            python_id = python_registered["save_id"]
            rust_inventory = self.drive(rust, "save.inventory", {"save_id": rust_id})
            python_inventory = self.drive(python, "save.inventory", {"save_id": python_id})
            self.assertTrue(rust_inventory["entries"], "fixture must carry an occupied scroll")
            slot_index = rust_inventory["entries"][0]["slot_index"]
            self.assertEqual(slot_index, python_inventory["entries"][0]["slot_index"])

            rust_plan = self.drive(
                rust,
                "save.prepare_delete",
                {
                    "save_id": rust_id,
                    "snapshot_id": rust_inventory["snapshot_id"],
                    "slots": [slot_index],
                },
            )
            python_plan = self.drive(
                python,
                "save.prepare_delete",
                {
                    "save_id": python_id,
                    "snapshot_id": python_inventory["snapshot_id"],
                    "slots": [slot_index],
                },
            )
            self.assertEqual(rust_plan["kind"], python_plan["kind"])
            self.assertEqual(rust_plan["source_sha256"], python_plan["source_sha256"])

            rust_receipt = self.drive(rust, "save.commit", {"plan_id": rust_plan["plan_id"]})
            python_receipt = self.drive(python, "save.commit", {"plan_id": python_plan["plan_id"]})
            self.assertEqual(rust_receipt["commit_status"], "committed", rust_receipt)
            self.assertEqual(rust_receipt["warning"], python_receipt["warning"])
            self.assertEqual(rust_receipt["commit_status"], python_receipt["commit_status"])

            rust_operation = self.drive(
                rust, "save.operation", {"plan_id": rust_plan["plan_id"]}
            )
            python_operation = self.drive(
                python, "save.operation", {"plan_id": python_plan["plan_id"]}
            )
            self.assertEqual(rust_operation["commit_status"], "committed")
            self.assertEqual(rust_operation["commit_status"], python_operation["commit_status"])

            rust_operations = self.drive(rust, "save.operations", {"save_id": rust_id})
            self.assertTrue(
                any(
                    entry["operation_id"] == rust_plan["plan_id"]
                    for entry in rust_operations["operations"]
                ),
                "the committed operation must appear in the ledger",
            )

            # A no-replay retry returns the durable receipt rather than writing.
            retried = self.drive(rust, "save.commit", {"plan_id": rust_plan["plan_id"]})
            self.assertEqual(retried["commit_status"], "committed")

            rust_failure = self.drive_failure(rust, "save.operation", {"plan_id": "f" * 32})
            python_failure = self.drive_failure(python, "save.operation", {"plan_id": "f" * 32})
            # A failed protected job answers OPERATION_FAILED, not the inline
            # handler's OPERATION_REJECTED.
            self.assertEqual(rust_failure["code"], "OPERATION_FAILED")
            self.assertEqual(rust_failure["code"], python_failure["code"])
            self.assertEqual(rust_failure["message"], "Unknown operation ID")
            self.assertEqual(rust_failure["message"], python_failure["message"])
        finally:
            self.assertEqual(rust.terminate(), 0)
            self.assertEqual(python.terminate(), 0)

        # A restarted host must reconcile the same durable receipt.
        restarted = self.rust_worker(rust_state)
        try:
            handshake = restarted.call("handshake")
            self.assertTrue(handshake["ok"], handshake)
            receipt = self.drive(
                restarted,
                "save.operation",
                {"plan_id": rust_plan["plan_id"]},
            )
            self.assertEqual(receipt["commit_status"], "committed")
        finally:
            self.assertEqual(restarted.terminate(), 0)

    def _edit_request(self, entry: dict) -> dict:
        """A schema-valid local edit derived from one inventory entry."""

        header = dict(entry["header"])
        return {
            "slot_index": entry["slot_index"],
            "header": header,
            "effects": [
                {
                    "slot_index": effect["slot_index"],
                    "effect_id": effect["effect_id"],
                    "value": effect["value"],
                    "prefix": effect["prefix"],
                    "metadata": effect["metadata"],
                    "tail_0": effect["tail_0"],
                    "tail_1": effect["tail_1"],
                }
                for effect in entry["effects"]
            ],
        }

    def test_broker_only_sources_and_edit_lifecycle_match(self) -> None:
        rust_save, rust_state = self.isolated("edit-rust")
        python_save, python_state = self.isolated("edit-python")
        rust = self.rust_worker(rust_state)
        python = self.python_worker(python_state)
        try:
            self.assertTrue(rust.call("handshake")["ok"])
            self.assertTrue(python.call("handshake")["ok"])
            rust_registered = self.drive(rust, "save.register", {"path": str(rust_save)})
            python_registered = self.drive(python, "save.register", {"path": str(python_save)})
            rust_id = rust_registered["save_id"]
            python_id = python_registered["save_id"]
            rust_inventory = self.drive(rust, "save.inventory", {"save_id": rust_id})
            python_inventory = self.drive(python, "save.inventory", {"save_id": python_id})
            self.assertTrue(rust_inventory["entries"], "fixture must carry an occupied scroll")
            entry = rust_inventory["entries"][0]
            self.assertEqual(entry["slot_index"], python_inventory["entries"][0]["slot_index"])

            # Broker-only source handoffs are protocol services too.
            rust_live = self.drive(
                rust,
                "save.live_add_source",
                {"save_id": rust_id, "snapshot_id": rust_inventory["snapshot_id"]},
            )
            python_live = self.drive(
                python,
                "save.live_add_source",
                {"save_id": python_id, "snapshot_id": python_inventory["snapshot_id"]},
            )
            # The two peers own separate fixture copies, so compare the shape
            # and the save basename rather than the absolute path.
            self.assertEqual(set(rust_live), set(python_live))
            self.assertEqual(
                os.path.basename(rust_live["save_path"]),
                os.path.basename(python_live["save_path"]),
            )
            self.assertEqual(os.path.basename(rust_live["save_path"]), "SAVEDATA.BIN")
            rust_count = self.drive(
                rust,
                "save.count_edit_source",
                {
                    "save_id": rust_id,
                    "snapshot_id": rust_inventory["snapshot_id"],
                    "slot_index": entry["slot_index"],
                },
            )
            python_count = self.drive(
                python,
                "save.count_edit_source",
                {
                    "save_id": python_id,
                    "snapshot_id": python_inventory["snapshot_id"],
                    "slot_index": entry["slot_index"],
                },
            )
            self.assertEqual(rust_count["count_source"]["source_sha256"],
                             python_count["count_source"]["source_sha256"])
            self.assertEqual(rust_count["count_source"]["record_hex"],
                             python_count["count_source"]["record_hex"])

            rust_plan = self.drive(
                rust,
                "save.prepare_edit",
                {
                    "save_id": rust_id,
                    "snapshot_id": rust_inventory["snapshot_id"],
                    "edits": [self._edit_request(entry)],
                },
            )
            python_plan = self.drive(
                python,
                "save.prepare_edit",
                {
                    "save_id": python_id,
                    "snapshot_id": python_inventory["snapshot_id"],
                    "edits": [self._edit_request(python_inventory["entries"][0])],
                },
            )
            self.assertEqual(rust_plan["kind"], python_plan["kind"])
            self.assertEqual(rust_plan["source_sha256"], python_plan["source_sha256"])
            self.assertEqual(
                rust_plan["preview"]["changes"][0]["changed_offsets"],
                python_plan["preview"]["changes"][0]["changed_offsets"],
            )

            rust_receipt = self.drive(rust, "save.commit", {"plan_id": rust_plan["plan_id"]})
            python_receipt = self.drive(python, "save.commit", {"plan_id": python_plan["plan_id"]})
            self.assertEqual(rust_receipt["commit_status"], "committed", rust_receipt)
            self.assertEqual(rust_receipt["commit_status"], python_receipt["commit_status"])

            # Both committed saves must now read back as the same inventory; the
            # encrypted container bytes are not compared because each host
            # encrypts through its own writer.
            rust_after = self.drive(rust, "save.inventory", {"save_id": rust_id})
            python_after = self.drive(python, "save.inventory", {"save_id": python_id})
            self.assertEqual(rust_after["entries"], python_after["entries"])
            self.assertEqual(rust_after["source_sha256"], python_after["source_sha256"])
        finally:
            self.assertEqual(rust.terminate(), 0)
            self.assertEqual(python.terminate(), 0)

    def test_no_game_refuses_count_prepare_without_faking_success(self) -> None:
        # The count source is produced by a real protected save worker against
        # one shared, read-only fixture; no write happens because both runtime
        # hosts must refuse before any plan is fabricated.
        shared_save = self.fixture_at("nogame-shared")
        save_worker = self.rust_worker(self.state_at("nogame-save-state"))
        rust_runtime = self.rust_worker(self.state_at("nogame-rust"), role="runtime")
        python_runtime = self.python_worker(self.state_at("nogame-python"), role="runtime")
        try:
            self.assertTrue(save_worker.call("handshake")["ok"])
            registered = self.drive(save_worker, "save.register", {"path": str(shared_save)})
            inventory = self.drive(
                save_worker, "save.inventory", {"save_id": registered["save_id"]}
            )
            entry = inventory["entries"][0]
            source = self.drive(
                save_worker,
                "save.count_edit_source",
                {
                    "save_id": registered["save_id"],
                    "snapshot_id": inventory["snapshot_id"],
                    "slot_index": entry["slot_index"],
                },
            )["count_source"]

            self.assertTrue(rust_runtime.call("handshake")["ok"])
            self.assertTrue(python_runtime.call("handshake")["ok"])

            # Neither host may invent a count plan while no game is running.
            rust_failure = self.drive_failure(
                rust_runtime,
                "runtime.count_prepare",
                {"source": source, "new_count": 5},
            )
            python_failure = self.drive_failure(
                python_runtime,
                "runtime.count_prepare",
                {"source": source, "new_count": 5},
            )
            for failure in (rust_failure, python_failure):
                self.assertEqual(failure["code"], "OPERATION_FAILED", failure)
                self.assertTrue(failure["message"].strip())
        finally:
            self.assertEqual(save_worker.terminate(), 0)
            self.assertEqual(rust_runtime.terminate(), 0)
            self.assertEqual(python_runtime.terminate(), 0)

    def test_settings_pointer_and_refusals_match(self) -> None:
        """`save.data_directory` is the rotation pointer for the one-file cache.

        Its value is what lets the packaged app keep its state outside the
        package, so the Rust host must resolve and persist the same pointer the
        shipped host does, and refuse the same malformed selections.
        """

        # An isolated per-user location: `set`/`reset` write the settings
        # pointer there, never into the real user profile.
        local_app_data = self.root / "settings-localappdata"
        local_app_data.mkdir()
        env = {"LOCALAPPDATA": str(local_app_data)}
        rust_state = self.state_at("settings-rust")
        python_state = self.state_at("settings-python")
        selected = self.root / "settings-selected-data"
        rust = self.rust_worker(rust_state, extra_env=env)
        python = self.python_worker(python_state, extra_env=env)
        try:
            self.assertTrue(rust.call("handshake")["ok"])
            self.assertTrue(python.call("handshake")["ok"])

            # `inspect` reports the worker's own state root and needs no restart.
            rust_inspect = self.drive(rust, "save.data_directory", {"action": "inspect", "path": None})
            python_inspect = self.drive(
                python, "save.data_directory", {"action": "inspect", "path": None}
            )
            self.assertEqual(rust_inspect["data_directory"], str(rust_state))
            self.assertEqual(python_inspect["data_directory"], str(python_state))
            self.assertIs(rust_inspect["restart_required"], False)
            self.assertIs(python_inspect["restart_required"], False)

            # An explicit directory is persisted identically on both peers.
            rust_set = self.drive(
                rust, "save.data_directory", {"action": "set", "path": str(selected)}
            )
            python_set = self.drive(
                python, "save.data_directory", {"action": "set", "path": str(selected)}
            )
            self.assertEqual(
                os.path.normcase(str(Path(rust_set["data_directory"]).resolve())),
                os.path.normcase(str(Path(python_set["data_directory"]).resolve())),
            )
            self.assertIs(rust_set["restart_required"], True)
            self.assertEqual(
                Path(rust_set["data_directory"]).resolve(),
                selected.resolve(),
            )
            pointer = local_app_data / "Nioh3ScrollGenerator" / "settings.json"
            self.assertTrue(pointer.is_file(), "the pointer must be written to the user location")
            payload = json.loads(pointer.read_text(encoding="utf-8"))
            self.assertEqual(payload["schema"], "nioh3-scroll-generator-settings/v1")
            self.assertEqual(Path(payload["data_root"]).resolve(), selected.resolve())

            # `reset` returns to the default per-user directory on both peers.
            rust_reset = self.drive(
                rust, "save.data_directory", {"action": "reset", "path": None}
            )
            python_reset = self.drive(
                python, "save.data_directory", {"action": "reset", "path": None}
            )
            self.assertEqual(
                os.path.normcase(str(Path(rust_reset["data_directory"]).resolve())),
                os.path.normcase(str(Path(python_reset["data_directory"]).resolve())),
            )
            self.assertIs(rust_reset["restart_required"], True)
            self.assertEqual(
                Path(rust_reset["data_directory"]).resolve(),
                (local_app_data / "Nioh3ScrollGenerator").resolve(),
            )

            # Refusals are the shipped ones, and they are job failures.
            for action, path, expected in (
                ("set", "relative/dir", "data_root must be an absolute path"),
                ("set", None, "Invalid data directory action"),
            ):
                rust_failure = self.drive_failure(
                    rust, "save.data_directory", {"action": action, "path": path}
                )
                python_failure = self.drive_failure(
                    python, "save.data_directory", {"action": action, "path": path}
                )
                self.assertEqual(rust_failure["code"], "OPERATION_FAILED", rust_failure)
                self.assertEqual(rust_failure["message"], python_failure["message"])
                self.assertEqual(rust_failure["message"], expected)

            file_path = self.root / "settings-not-a-directory"
            file_path.write_text("not a directory", encoding="utf-8")
            rust_failure = self.drive_failure(
                rust, "save.data_directory", {"action": "set", "path": str(file_path)}
            )
            python_failure = self.drive_failure(
                python, "save.data_directory", {"action": "set", "path": str(file_path)}
            )
            self.assertEqual(rust_failure["message"], python_failure["message"])
            self.assertEqual(rust_failure["message"], "data_root points to a file")
        finally:
            # Closing stdin after the pointer was written must let both hosts
            # exit cleanly: EOF is not a licence to abandon protected state.
            self.assertEqual(rust.terminate(), 0)
            self.assertEqual(python.terminate(), 0)

    @staticmethod
    def _bogus_template() -> dict:
        """A schema-valid template that belongs to no running core."""

        return {
            "template_hex": "00" * 0xE8,
            "source_sha256": "0" * 64,
            "context_digest": "0" * 64,
            "save_fingerprint": "0" * 64,
        }

    @staticmethod
    def _empty_criteria() -> dict:
        """A schema-valid criteria object with no requested field."""

        return {
            "primary_effect_ids": [],
            "required_secondary_ids": [],
            "required_secondary_id_groups": [],
            "grace_effect_id": None,
            "auxiliary": {
                "required_terrain_effect_keys": [],
                "required_terrain_effect_key_groups": [],
                "required_special_rule_keys": [],
                "required_special_rule_key_groups": [],
                "required_enemy_lookup_keys": [],
                "required_enemy_lookup_key_groups": [],
            },
        }

    def test_runtime_role_matches_the_shipped_host_without_a_game(self) -> None:
        """The protected runtime role over everything that needs no game.

        `runtime.start_override` and the three scan methods are deliberately
        never driven with a valid profile here: with a real game attached
        Python would arm a native hook or start a scan, and this gate must not
        touch a game process. The scan methods are instead driven through their
        template-identity gate, which both hosts evaluate *before* the game is
        opened, so the refusal is comparable and side-effect free. The routing
        audit below proves each remaining method has its own implementation.
        """

        rust = self.rust_worker(self.state_at("runtime-rust"), role="runtime")
        python = self.python_worker(self.state_at("runtime-python"), role="runtime")
        try:
            rust_handshake = rust.call("handshake")
            python_handshake = python.call("handshake")
            self.assertTrue(rust_handshake["ok"], rust_handshake)
            self.assertTrue(python_handshake["ok"], python_handshake)
            self.assertEqual(rust_handshake["result"]["role"], "runtime")
            self.assertEqual(rust_handshake["result"]["kill_safe"], False)
            self.assertEqual(
                rust_handshake["result"]["context"],
                python_handshake["result"]["context"],
            )

            # `runtime.status` is answered inline and must publish the shipped
            # ownership object, including its exact key set.
            rust_status = rust.call("runtime.status")
            python_status = python.call("runtime.status")
            self.assertTrue(rust_status["ok"], rust_status)
            self.assertEqual(rust_status["result"], python_status["result"])
            self.assertEqual(
                set(rust_status["result"]),
                {
                    "override_state",
                    "hit_count",
                    "pending_remote_calls",
                    "safe_to_shutdown",
                    "error",
                },
            )
            self.assertEqual(rust_status["result"]["override_state"], "stopped")
            self.assertIs(rust_status["result"]["safe_to_shutdown"], True)

            # Stopping an override the host does not own is a no-op that still
            # answers the ownership object.
            self.assertEqual(
                self.drive(rust, "runtime.stop_override", {}),
                self.drive(python, "runtime.stop_override", {}),
            )

            # An expired candidate is refused identically by both hosts, and it
            # is a job failure rather than an inline protocol refusal.
            for worker in (rust, python):
                failure = self.drive_failure(
                    worker, "runtime.export", {"candidate_id": "f" * 64}
                )
                self.assertEqual(failure["code"], "OPERATION_FAILED", failure)
                self.assertEqual(
                    failure["message"], "Native candidate expired; generate again"
                )

            # The scan methods reach their template-identity gate before the
            # game is opened, so a template from another core is comparable.
            scan_requests = {
                "runtime.generate": {
                    "template": self._bogus_template(),
                    "seed": 10032001,
                    "playthrough": 3,
                    "rarity": 4,
                    "level": 180,
                    "recommended_level": 183,
                    "title_screen_confirmed": True,
                },
                "runtime.search": {
                    "template": self._bogus_template(),
                    "seed": 10032001,
                    "playthrough": 3,
                    "rarity": 4,
                    "level": 180,
                    "recommended_level": 183,
                    "criteria": self._empty_criteria(),
                    "max_seeds": 4,
                    "title_screen_confirmed": True,
                },
                "runtime.capture_grace": {
                    "template": self._bogus_template(),
                    "playthrough": 3,
                    "rarity": 4,
                    "level": 180,
                    "recommended_level": 183,
                    "title_screen_confirmed": True,
                },
            }
            for method, params in scan_requests.items():
                rust_failure = self.drive_failure(rust, method, params)
                python_failure = self.drive_failure(python, method, params)
                self.assertEqual(rust_failure["code"], "OPERATION_FAILED", rust_failure)
                self.assertEqual(
                    rust_failure["message"], python_failure["message"], method
                )
                self.assertEqual(
                    rust_failure["message"], "Template context differs from the running core"
                )

            # `shutdown` reports ownership; with nothing armed both agree.
            rust_shutdown = rust.call("shutdown")
            python_shutdown = python.call("shutdown")
            self.assertTrue(rust_shutdown["ok"], rust_shutdown)
            self.assertEqual(rust_shutdown["result"], python_shutdown["result"])
        finally:
            self.assertEqual(rust.terminate(), 0)
            self.assertEqual(python.terminate(), 0)

    def drive_failure(self, worker: FramedWorker, method: str, params: dict) -> dict:
        """Start one protected job and require it to fail, returning its error."""

        started = worker.call(method, params)
        if not started["ok"]:
            return started["error"]
        job = started["result"]
        while job["state"] in ("running", "cancel_requested"):
            snapshot = worker.call("job.snapshot", {"job_id": job["job_id"]})
            self.assertTrue(snapshot["ok"], snapshot)
            job = snapshot["result"]
        self.assertEqual(job["state"], "failed", job)
        return job["error"]


class ProtectedRuntimeScanTests(unittest.TestCase):
    """The protected runtime scan loops over the real framed protocol.

    The host is built with `--features test-fake`, which binds a deterministic
    batch oracle in place of the native one and can answer from a scripted row
    table. Everything else is the shipped path: the real binary, the real
    contract validation, the real job machine, the real preview composition and
    the real cache writer. No game process is involved, and the packaged host
    never builds the feature.
    """

    EFFECT_START = 0x34
    EFFECT_STRIDE = 0x18

    @classmethod
    def setUpClass(cls) -> None:
        manifest = ROOT / "crates" / "nioh3-protected" / "Cargo.toml"
        if not manifest.is_file():
            raise AssertionError(f"the protected crate is missing: {manifest}")
        cargo = shutil.which("cargo")
        if cargo is None:
            raise AssertionError("cargo is required to build and run the protected worker")
        env = dict(os.environ)
        env["CARGO_TARGET_DIR"] = resolved_cargo_target_dir("protected-scan")
        build = subprocess.run(
            [
                cargo,
                "build",
                "--offline",
                "--features",
                "test-fake",
                "--manifest-path",
                str(manifest),
                "--bin",
                "nioh3-protected-worker",
            ],
            cwd=str(ROOT),
            env=env,
            capture_output=True,
            timeout=3600,
        )
        if build.returncode != 0:
            raise AssertionError(
                "the scripted protected worker did not build: "
                + build.stderr.decode("utf-8", "replace")[-4000:]
            )
        cls.binary = Path(env["CARGO_TARGET_DIR"]) / "debug" / "nioh3-protected-worker.exe"
        FIXTURE_ROOT.mkdir(parents=True, exist_ok=True)
        cls.temp = tempfile.TemporaryDirectory(dir=str(FIXTURE_ROOT))
        cls.root = Path(cls.temp.name)

    @classmethod
    def tearDownClass(cls) -> None:
        cls.temp.cleanup()

    def worker(
        self,
        name: str,
        script: Path | None = None,
    ) -> tuple[FramedWorker, Path]:
        state = self.root / f"{name}-state"
        state.mkdir(parents=True, exist_ok=True)
        argv = [
            str(self.binary),
            "--role",
            "runtime",
            "--dev-protected-only",
            "--state-root",
            str(state),
            "--data-root",
            str(ROOT / "nioh3_scroll_editor" / "data"),
            "--contract-dir",
            str(SCHEMA_DIR),
        ]
        env = dict(os.environ)
        env["NIOH3_STATE_ROOT"] = str(state)
        if script is not None:
            env["NIOH3_PROTECTED_ORACLE_SCRIPT"] = str(script)
        else:
            env.pop("NIOH3_PROTECTED_ORACLE_SCRIPT", None)
        return FramedWorker(argv, cwd=ROOT, env=env, name=f"runtime {name}"), state

    def save_fixture(self, name: str) -> tuple[Path, Path]:
        """A fresh encrypted fixture plus its isolated state root."""

        base = self.root / name
        save = base / "76561198000000000" / "SAVEDATA00" / "SAVEDATA.BIN"
        save.parent.mkdir(parents=True, exist_ok=True)
        plain = base / "plain.bin"
        plain.write_bytes(bytes(build_fixture_bytes()))
        # The shipped tool silently fails on a long absolute output path, which
        # a nested D-backed fixture root reaches immediately.
        native_transform_short(plain, save)
        state = self.root / f"{name}-save-state"
        state.mkdir(parents=True, exist_ok=True)
        return save, state

    def product_template(self, save: Path, state: Path) -> dict:
        """The playthrough-3 template a real save would install into."""

        argv = [
            sys.executable,
            "-u",
            "-m",
            "nioh3_scroll_editor.protected_worker",
            "--role",
            "save",
        ]
        env = dict(os.environ)
        env["NIOH3_STATE_ROOT"] = str(state)
        env["PYTHONUTF8"] = "1"
        env["PYTHONIOENCODING"] = "utf-8"
        worker = FramedWorker(argv, cwd=ROOT, env=env, name="template source")
        try:
            self.assertTrue(worker.call("handshake")["ok"])
            registered = drive_job(self, worker, "save.register", {"path": str(save)})
            inventory = drive_job(
                self, worker, "save.inventory", {"save_id": registered["save_id"]}
            )
            return drive_job(
                self,
                worker,
                "save.template",
                {
                    "save_id": registered["save_id"],
                    "snapshot_id": inventory["snapshot_id"],
                    "playthrough": 3,
                },
            )
        finally:
            self.assertEqual(worker.terminate(), 0)

    def record_hex(
        self,
        template_hex: str,
        seed: int,
        rarity: int,
        effects: list[tuple[int, int]],
        completed: bool,
    ) -> str:
        """One deterministic native row: template plus chosen slot identifiers."""

        record = bytearray.fromhex(template_hex)
        record[0x20:0x24] = seed.to_bytes(4, "little")
        record[0x30] = rarity
        record[0x31] = rarity
        for slot, effect_id in effects:
            offset = self.EFFECT_START + slot * self.EFFECT_STRIDE + 4
            record[offset : offset + 4] = effect_id.to_bytes(4, "little")
        if completed:
            record[self.EFFECT_START + 4 * self.EFFECT_STRIDE + 0x0E] = 0x04
        return bytes(record).hex()

    def test_the_real_scan_loops_run_over_the_protected_protocol(self) -> None:
        save, save_state = self.save_fixture("scan")
        template = self.product_template(save, save_state)
        self.assertTrue(template["template_hex"])

        # Two rows: the requested seed and one later in the search range.
        first_seed = 0x0002_0001
        later_seed = first_seed + 2
        effects = [(0, 0x0000_0100), (1, 0x0000_0200), (2, 0x0000_0300),
                   (3, 0x0000_0400), (4, 0x0000_0500)]
        script = self.root / "oracle-script.json"
        script.write_text(
            json.dumps(
                {
                    "template_hex": template["template_hex"],
                    "rarity": 4,
                    "level": 180,
                    "recommended_level": 183,
                    "transfer_count": 0,
                    "rows": [
                        {
                            "seed": seed,
                            "stage_one_hex": self.record_hex(
                                template["template_hex"], seed, 4, effects, completed=False
                            ),
                            "completed_hex": self.record_hex(
                                template["template_hex"], seed, 4, effects, completed=True
                            ),
                        }
                        for seed in (first_seed, later_seed)
                    ],
                }
            ),
            encoding="utf-8",
        )

        worker, state = self.worker("scan", script)
        try:
            handshake = worker.call("handshake")
            self.assertTrue(handshake["ok"], handshake)
            digest = handshake["result"]["context"]["context_digest"]
            request_template = {
                "template_hex": template["template_hex"],
                "source_sha256": template["source_sha256"],
                "context_digest": digest,
                "save_fingerprint": template["save_fingerprint"],
            }

            # `runtime.generate`: one seed, one native generation, one completion
            # pass, and a payload whose two records stay separate.
            generated = drive_job(
                self,
                worker,
                "runtime.generate",
                {
                    "template": request_template,
                    "seed": first_seed,
                    "playthrough": 3,
                    "rarity": 4,
                    "level": 180,
                    "recommended_level": 183,
                    "title_screen_confirmed": True,
                },
            )
            candidate = generated["candidate"]
            self.assertIsNotNone(candidate, generated)
            self.assertEqual(candidate["seed"], first_seed)
            self.assertEqual(candidate["rarity"], 4)
            self.assertEqual(candidate["record_stage"], "final_record")
            self.assertEqual(candidate["evidence"], "native_finalized_generation")
            self.assertEqual(candidate["effects"][0]["effect_id"], 0x0100)
            self.assertEqual(candidate["effects"][4]["effect_id"], 0x0500)
            self.assertIsNone(candidate["effects"][0]["roll_percent"])

            # The retained transfer is the two-record pair: the completed record
            # and the stage-one record the save still has to receive.
            exported = drive_job(
                self,
                worker,
                "runtime.export",
                {"candidate_id": candidate["candidate_id"]},
            )
            self.assertEqual(exported["record_stage"], "final_record")
            self.assertNotEqual(exported["installation_record_hex"], exported["record_hex"])
            stage_one = self.record_hex(
                template["template_hex"], first_seed, 4, effects, completed=False
            )
            completed = self.record_hex(
                template["template_hex"], first_seed, 4, effects, completed=True
            )
            self.assertEqual(exported["installation_record_hex"], stage_one)
            self.assertEqual(exported["record_hex"], completed)

            # `runtime.search`: the loop walks the requested range and accepts the
            # only seed the scripted oracle resolves.
            searched = drive_job(
                self,
                worker,
                "runtime.search",
                {
                    "template": request_template,
                    "seed": first_seed,
                    "playthrough": 3,
                    "rarity": 4,
                    "level": 180,
                    "recommended_level": 183,
                    "max_seeds": 8,
                    "criteria": {
                        "primary_effect_ids": [],
                        "required_secondary_ids": [],
                        "required_secondary_id_groups": [],
                        "grace_effect_id": None,
                        "auxiliary": {
                            "required_terrain_effect_keys": [],
                            "required_terrain_effect_key_groups": [],
                            "required_special_rule_keys": [],
                            "required_special_rule_key_groups": [],
                            "required_enemy_lookup_keys": [],
                            "required_enemy_lookup_key_groups": [],
                        },
                    },
                    "title_screen_confirmed": True,
                },
            )
            self.assertIsNotNone(searched["candidate"], searched)
            self.assertEqual(searched["candidate"]["seed"], first_seed)

            # `runtime.capture_grace`: 65,536 probes over the scripted oracle,
            # persisted under the state root for a later game-closed search.
            captured = drive_job(
                self,
                worker,
                "runtime.capture_grace",
                {
                    "template": request_template,
                    "playthrough": 3,
                    "rarity": 4,
                    "level": 180,
                    "recommended_level": 183,
                    "title_screen_confirmed": True,
                },
            )
            self.assertEqual(
                captured, {"captured": True, "playthrough": 3, "rarity": 4}
            )
            cache = (
                state
                / "grace-output-maps"
                / (
                    f"{template['save_fingerprint'].lower()}-{digest.lower()[:16]}"
                    "-p3-r4-draw1.json"
                )
            )
            self.assertTrue(cache.is_file(), "the measured map must be persisted")
            payload = json.loads(cache.read_text(encoding="utf-8"))
            self.assertEqual(payload["rarity"], 4)
            self.assertEqual(payload["effect_slot"], 5)
            self.assertEqual(len(payload["ranges"]), 1)
            self.assertEqual(payload["ranges"][0]["start"], 0)
            self.assertEqual(payload["ranges"][0]["end"], 0xFFFF)
        finally:
            self.assertEqual(worker.terminate(), 0)

    def _filled_template(self, template_hex: str, rarity: int) -> str:
        """The product template with a deterministic effect area.

        The scripted oracle answers an unscripted seed through the shipped
        `build_source_record`, which copies the template's effect area, so a
        populated template is what turns a measured map into deterministic
        rows without hand-writing any record bytes.
        """

        record = bytearray.fromhex(template_hex)
        record[0x30] = rarity
        record[0x31] = rarity
        for slot in range(7):
            offset = self.EFFECT_START + slot * self.EFFECT_STRIDE + 4
            record[offset : offset + 4] = (0x0100 * (slot + 1)).to_bytes(4, "little")
        return bytes(record).hex()

    def test_the_accelerated_map_branches_run_over_the_protected_protocol(self) -> None:
        """`prepare_maps` plus the accelerated Grace and draw-1 primary branches.

        Both searches run the shipped loops over the measured maps the host
        itself built a moment earlier: nothing here is a handwritten candidate.
        """

        save, save_state = self.save_fixture("accelerated")
        product = self.product_template(save, save_state)
        # Rarity 4 hides the stage Grace in slot 5, so the populated slot 5 is
        # the result the measured map has to report.
        filled = self._filled_template(product["template_hex"], 4)

        raw = self.root / "unused-script.json"
        raw.write_text(
            json.dumps({"template_hex": filled, "rarity": 4}), encoding="utf-8"
        )
        worker, state = self.worker("accelerated", raw)
        try:
            handshake = worker.call("handshake")
            self.assertTrue(handshake["ok"], handshake)
            digest = handshake["result"]["context"]["context_digest"]
            request_template = {
                "template_hex": filled,
                "source_sha256": product["source_sha256"],
                "context_digest": digest,
                "save_fingerprint": product["save_fingerprint"],
            }

            # Measure the Grace map for this context.
            captured = drive_job(
                self,
                worker,
                "runtime.capture_grace",
                {
                    "template": request_template,
                    "playthrough": 3,
                    "rarity": 4,
                    "level": 180,
                    "recommended_level": 183,
                    "title_screen_confirmed": True,
                },
            )
            self.assertEqual(captured["captured"], True)

            # The accelerated Grace search must reuse that map, enumerate its
            # seeds, and publish the stage-one record beside the completed one.
            searched = drive_job(
                self,
                worker,
                "runtime.search",
                {
                    "template": request_template,
                    "seed": 0,
                    "playthrough": 3,
                    "rarity": 4,
                    "level": 180,
                    "recommended_level": 183,
                    "max_seeds": 1,
                    "criteria": {
                        "primary_effect_ids": [],
                        "required_secondary_ids": [],
                        "required_secondary_id_groups": [],
                        "grace_effect_id": 0x0500,
                        "auxiliary": {
                            "required_terrain_effect_keys": [],
                            "required_terrain_effect_key_groups": [],
                            "required_special_rule_keys": [],
                            "required_special_rule_key_groups": [],
                            "required_enemy_lookup_keys": [],
                            "required_enemy_lookup_key_groups": [],
                        },
                    },
                    "title_screen_confirmed": True,
                },
            )
            candidate = searched["candidate"]
            self.assertIsNotNone(candidate, searched)
            self.assertEqual(candidate["rarity"], 4)
            self.assertEqual(candidate["record_stage"], "final_record")
            # The measured map says every first-draw bucket resolves to slot 5.
            exported = drive_job(
                self,
                worker,
                "runtime.export",
                {"candidate_id": candidate["candidate_id"]},
            )
            self.assertIsNotNone(
                exported["installation_record_hex"],
                "the accelerated completion pass must publish the stage-one record",
            )
            cache = (
                state
                / "grace-output-maps"
                / (
                    f"{product['save_fingerprint'].lower()}-{digest.lower()[:16]}"
                    "-p3-r4-draw1.json"
                )
            )
            self.assertTrue(cache.is_file(), "the search must reuse a measured map")
        finally:
            self.assertEqual(worker.terminate(), 0)

    def test_the_primary_first_draw_branch_measures_its_own_map(self) -> None:
        """`primary_first_output_map`: a draw-1 primary search at playthrough 2."""

        save, save_state = self.save_fixture("primary")
        product = self.product_template(save, save_state)
        template = bytearray.fromhex(self._filled_template(product["template_hex"], 5))
        # Category 2 needs the 0x516D record type.
        template[0:2] = (0x516D).to_bytes(2, "little")
        filled = bytes(template).hex()

        script = self.root / "primary-script.json"
        script.write_text(
            json.dumps({"template_hex": filled, "rarity": 5}), encoding="utf-8"
        )
        worker, state = self.worker("primary", script)
        try:
            handshake = worker.call("handshake")
            self.assertTrue(handshake["ok"], handshake)
            digest = handshake["result"]["context"]["context_digest"]
            searched = drive_job(
                self,
                worker,
                "runtime.search",
                {
                    "template": {
                        "template_hex": filled,
                        "source_sha256": product["source_sha256"],
                        "context_digest": digest,
                        "save_fingerprint": product["save_fingerprint"],
                    },
                    "seed": 0,
                    "playthrough": 2,
                    "rarity": 5,
                    "level": 180,
                    "recommended_level": 183,
                    # The shipped host prepares measured maps only for a bounded
                    # search (`max_seeds > 1`), so this value is what puts the
                    # draw-1 primary capture on the path at all.
                    "max_seeds": 4,
                    "criteria": {
                        "primary_effect_ids": [0x0100],
                        "required_secondary_ids": [],
                        "required_secondary_id_groups": [],
                        "grace_effect_id": None,
                        "auxiliary": {
                            "required_terrain_effect_keys": [],
                            "required_terrain_effect_key_groups": [],
                            "required_special_rule_keys": [],
                            "required_special_rule_key_groups": [],
                            "required_enemy_lookup_keys": [],
                            "required_enemy_lookup_key_groups": [],
                        },
                    },
                    "title_screen_confirmed": True,
                },
            )
            candidate = searched["candidate"]
            self.assertIsNotNone(candidate, searched)
            self.assertEqual(candidate["rarity"], 5)
            self.assertEqual(candidate["playthrough"], 2)
            # The primary map was measured and cached for this draw-1 context.
            maps = list((state / "primary-effect-maps").glob("*-p2-r5-draw1.json"))
            self.assertEqual(len(maps), 1, "the draw-1 primary map must be persisted")
        finally:
            self.assertEqual(worker.terminate(), 0)

    def test_the_joint_rarity_five_branch_solves_the_measured_intersection(self) -> None:
        """`primary_output_map`: rarity 5, a selected Grace and a primary target.

        This is the branch that needs both measured maps at once: the draw-1
        Grace map conditions the draw-2 primary capture, and the search then
        enumerates the exact intersection of the two constraints.
        """

        save, save_state = self.save_fixture("joint")
        product = self.product_template(save, save_state)
        # Rarity 5 carries the final Grace in slot 6.
        filled = self._filled_template(product["template_hex"], 5)
        script = self.root / "joint-script.json"
        script.write_text(
            json.dumps({"template_hex": filled, "rarity": 5}), encoding="utf-8"
        )
        worker, state = self.worker("joint", script)
        try:
            handshake = worker.call("handshake")
            self.assertTrue(handshake["ok"], handshake)
            digest = handshake["result"]["context"]["context_digest"]
            request_template = {
                "template_hex": filled,
                "source_sha256": product["source_sha256"],
                "context_digest": digest,
                "save_fingerprint": product["save_fingerprint"],
            }
            searched = drive_job(
                self,
                worker,
                "runtime.search",
                {
                    "template": request_template,
                    "seed": 0,
                    "playthrough": 3,
                    "rarity": 5,
                    "level": 180,
                    "recommended_level": 183,
                    "max_seeds": 4,
                    "criteria": {
                        "primary_effect_ids": [0x0100],
                        "required_secondary_ids": [],
                        "required_secondary_id_groups": [],
                        "grace_effect_id": 0x0600,
                        "auxiliary": {
                            "required_terrain_effect_keys": [],
                            "required_terrain_effect_key_groups": [],
                            "required_special_rule_keys": [],
                            "required_special_rule_key_groups": [],
                            "required_enemy_lookup_keys": [],
                            "required_enemy_lookup_key_groups": [],
                        },
                    },
                    "title_screen_confirmed": True,
                },
            )
            candidate = searched["candidate"]
            self.assertIsNotNone(candidate, searched)
            self.assertEqual(candidate["rarity"], 5)
            # A joint search reports the solver trial it accepted, which the
            # seed scans never do.
            self.assertIsNotNone(candidate["cursor"])
            # Both measured maps are cached for this context, under the draw-2
            # name the joint path uses.
            self.assertEqual(
                len(list((state / "grace-output-maps").glob("*-p3-r5-draw1.json"))), 1
            )
            draw2 = list(
                (state / "primary-effect-maps").glob("*-p3-r5-grace-00000600-draw2.json")
            )
            self.assertEqual(
                len(draw2), 1, "the Grace-conditioned draw-2 map must be persisted"
            )
        finally:
            self.assertEqual(worker.terminate(), 0)

    def test_a_retired_oracle_keeps_the_runtime_unsafe_to_shut_down(self) -> None:
        """A native call that may still be outstanding retains ownership."""

        save, save_state = self.save_fixture("retire")
        product = self.product_template(save, save_state)
        filled = self._filled_template(product["template_hex"], 4)
        script = self.root / "retire-script.json"
        script.write_text(
            json.dumps(
                {"template_hex": filled, "rarity": 4, "remote_call_pending": True}
            ),
            encoding="utf-8",
        )
        worker, _state = self.worker("retire", script)
        try:
            handshake = worker.call("handshake")
            self.assertTrue(handshake["ok"], handshake)
            digest = handshake["result"]["context"]["context_digest"]
            before = worker.call("runtime.status")
            self.assertEqual(before["result"]["pending_remote_calls"], 0)
            self.assertIs(before["result"]["safe_to_shutdown"], True)

            drive_job(
                self,
                worker,
                "runtime.generate",
                {
                    "template": {
                        "template_hex": filled,
                        "source_sha256": product["source_sha256"],
                        "context_digest": digest,
                        "save_fingerprint": product["save_fingerprint"],
                    },
                    "seed": 0x0003_0001,
                    "playthrough": 3,
                    "rarity": 4,
                    "level": 180,
                    "recommended_level": 183,
                    "title_screen_confirmed": True,
                },
            )

            after = worker.call("runtime.status")
            self.assertGreaterEqual(after["result"]["pending_remote_calls"], 1)
            self.assertIs(after["result"]["safe_to_shutdown"], False)
            # The shipped host refuses a shutdown that would abandon the owner.
            refused = worker.call("shutdown")
            self.assertEqual(refused["result"]["safe_to_shutdown"], False)
        finally:
            self.assertEqual(worker.terminate(), 0)

    def test_the_scan_refuses_a_foreign_context_and_a_missing_game(self) -> None:
        save, save_state = self.save_fixture("scan-refusals")
        template = self.product_template(save, save_state)

        # A template from another core is refused before anything is opened.
        worker, _state = self.worker("foreign")
        try:
            self.assertTrue(worker.call("handshake")["ok"])
            foreign = {
                "template_hex": template["template_hex"],
                "source_sha256": template["source_sha256"],
                "context_digest": "0" * 64,
                "save_fingerprint": template["save_fingerprint"],
            }
            failure = self.drive_failure(
                worker,
                "runtime.generate",
                {
                    "template": foreign,
                    "seed": 1,
                    "playthrough": 3,
                    "rarity": 4,
                    "level": 180,
                    "recommended_level": 183,
                    "title_screen_confirmed": True,
                },
            )
            self.assertEqual(failure["code"], "OPERATION_FAILED", failure)
            self.assertEqual(
                failure["message"], "Template context differs from the running core"
            )
        finally:
            self.assertEqual(worker.terminate(), 0)

        # With the matching context but no scripted oracle, the same binary must
        # fall through to the real identity check: this is the shipped
        # process-absence failure, not a "does not serve" refusal.
        worker, _state = self.worker("missing-game")
        try:
            handshake = worker.call("handshake")
            self.assertTrue(handshake["ok"], handshake)
            digest = handshake["result"]["context"]["context_digest"]
            request_template = {
                "template_hex": template["template_hex"],
                "source_sha256": template["source_sha256"],
                "context_digest": digest,
                "save_fingerprint": template["save_fingerprint"],
            }
            for method, params in (
                (
                    "runtime.generate",
                    {
                        "template": request_template,
                        "seed": 1,
                        "playthrough": 3,
                        "rarity": 4,
                        "level": 180,
                        "recommended_level": 183,
                        "title_screen_confirmed": True,
                    },
                ),
                (
                    "runtime.capture_grace",
                    {
                        "template": request_template,
                        "playthrough": 3,
                        "rarity": 4,
                        "level": 180,
                        "recommended_level": 183,
                        "title_screen_confirmed": True,
                    },
                ),
            ):
                failure = self.drive_failure(worker, method, params)
                self.assertEqual(failure["code"], "OPERATION_FAILED", failure)
                self.assertTrue(failure["message"].strip())
                self.assertNotIn("does not serve", failure["message"])
                self.assertNotIn("is not a method", failure["message"])
        finally:
            self.assertEqual(worker.terminate(), 0)

    def drive_failure(self, worker: FramedWorker, method: str, params: dict) -> dict:
        """Start one protected job and require it to fail, returning its error."""

        started = worker.call(method, params)
        if not started["ok"]:
            return started["error"]
        job = started["result"]
        while job["state"] in ("running", "cancel_requested"):
            snapshot = worker.call("job.snapshot", {"job_id": job["job_id"]})
            self.assertTrue(snapshot["ok"], snapshot)
            job = snapshot["result"]
        self.assertEqual(job["state"], "failed", job)
        return job["error"]


def drive_job(test: unittest.TestCase, worker: FramedWorker, method: str, params: dict) -> dict:
    """Start one protected job and poll it to a successful terminal state."""

    started = worker.call(method, params)
    if not started["ok"]:
        raise AssertionError(f"{method} refused: {started}")
    job = started["result"]
    while job["state"] in ("running", "cancel_requested"):
        snapshot = worker.call("job.snapshot", {"job_id": job["job_id"]})
        if not snapshot["ok"]:
            raise AssertionError(f"job.snapshot refused: {snapshot}")
        job = snapshot["result"]
    if job["state"] != "completed":
        raise AssertionError(f"{method} did not complete: {job}")
    return job["result"]


class ProtectedRuntimeRoutingAudit(unittest.TestCase):
    """The runtime role must implement every reviewed method by name.

    A missing game may only block the native call itself. It must not let a
    ported capability be answered by one blanket "not implemented" refusal, and
    a genuinely absent host-side loop must name its own dependency so an
    operator can tell a port gap from a rejected request. This audit is static
    and touches no process, so it runs even when no build cache is warm.
    """

    RUNTIME_METHODS = (
        "status",
        "stop_override",
        "start_override",
        "export",
        "generate",
        "search",
        "capture_grace",
        "live_add_prepare",
        "live_add_execute",
        "live_add_status",
        "live_add_recover",
        "live_add_cancel",
        "live_batch_prepare",
        "live_batch_execute",
        "live_batch_status",
        "live_batch_cancel",
        "count_prepare",
        "count_execute",
        "count_status",
        "count_recover",
    )

    def setUp(self) -> None:
        sources = [
            ROOT / "crates" / "nioh3-protected" / "src" / "runtime_app.rs",
            ROOT / "crates" / "nioh3-protected" / "src" / "scan.rs",
            ROOT / "crates" / "nioh3-protected" / "src" / "maps.rs",
            ROOT / "crates" / "nioh3-protected" / "src" / "grace_capture.rs",
        ]
        for path in sources:
            if not path.is_file():
                raise AssertionError(f"the protected runtime role is missing: {path}")
        self.source_path = sources[0]
        self.source = sources[0].read_text(encoding="utf-8")
        self.every_source = "\n".join(path.read_text(encoding="utf-8") for path in sources)

    def temp_dir(self, name: str) -> Path:
        """A directory under the task fixture root."""

        FIXTURE_ROOT.mkdir(parents=True, exist_ok=True)
        directory = Path(tempfile.mkdtemp(prefix=f"{name}-", dir=str(FIXTURE_ROOT)))
        self.addCleanup(shutil.rmtree, directory, True)
        return directory

    def release_binary(self) -> Path:
        """The production-default protected worker, built without test features."""

        manifest = ROOT / "crates" / "nioh3-protected" / "Cargo.toml"
        cargo = shutil.which("cargo")
        if cargo is None:
            raise AssertionError("cargo is required to build the release worker")
        env = dict(os.environ)
        env["CARGO_TARGET_DIR"] = resolved_cargo_target_dir("protected")
        build = subprocess.run(
            [
                cargo,
                "build",
                "--locked",
                "--offline",
                "--release",
                "--manifest-path",
                str(manifest),
                "--bin",
                "nioh3-protected-worker",
            ],
            cwd=str(ROOT),
            env=env,
            capture_output=True,
            timeout=3600,
        )
        if build.returncode != 0:
            raise AssertionError(
                "the release protected worker did not build: "
                + build.stderr.decode("utf-8", "replace")[-3000:]
            )
        binary = Path(env["CARGO_TARGET_DIR"]) / "release" / "nioh3-protected-worker.exe"
        if not binary.is_file():
            raise AssertionError(f"the release protected worker is missing: {binary}")
        return binary

    def test_no_blanket_runtime_refusal_remains(self) -> None:
        self.assertNotIn(
            "requires the native game oracle",
            self.source,
            "the runtime role must not answer every native method with one shared stub",
        )
        self.assertNotIn(
            "fn native_pending",
            self.source,
            "the single blanket 'pending' helper must be gone",
        )

    def test_every_contract_method_has_its_own_arm(self) -> None:
        for method in self.RUNTIME_METHODS:
            self.assertIn(
                f'"{method}"',
                self.source,
                f"runtime.{method} has no dedicated dispatch arm",
            )

    def test_the_real_adapters_are_constructed_not_shelled(self) -> None:
        for symbol in (
            "WindowsMutationHost",
            "OverrideSession::auxiliary",
            "OverrideSession::challenge",
            "ChallengeOverrideProfile",
            "roster_roles",
            "LiveAddApplication",
            "NativeLiveAddExecutor",
            "NativeDebugTransport",
            "SaveBackupAdapter",
            "DomainCatalogPolicy",
            "LiveAddBatch",
            "identify_running_game",
        ):
            self.assertIn(
                symbol,
                self.source,
                f"the protected runtime role no longer constructs {symbol}",
            )

    def test_the_scan_loops_are_implemented_not_stubbed(self) -> None:
        # The seed scan and the live Grace capture must be driven by the ported
        # loops over a batch oracle, including every accelerated branch and the
        # measured-map machinery, and the host must name the product oracle.
        for symbol in (
            "scan_next_candidate",
            "scan_seed_range",
            "scan_primary_candidates",
            "scan_grace_accelerated",
            "build_live_grace_output_map",
            "prepare_maps",
            "ConstraintIntersection",
            "GraceSeedCursor",
            "first_u16_ranges_for_grace",
            "build_primary_first_draw_output_map",
            "build_primary_output_map",
            "NativeOracle",
            "OracleHandle",
            "HostAuxiliarySource",
        ):
            self.assertIn(
                symbol,
                self.every_source,
                f"the runtime role no longer drives {symbol}",
            )
        for absent in ("fn scan_unported", "which this build does not include"):
            self.assertNotIn(
                absent,
                self.source,
                "the pre-flight-only placeholder must be gone",
            )

    def test_the_scripted_oracle_is_excluded_from_the_default_build(self) -> None:
        """The production-default build must not be able to take a test oracle.

        A script path, a fake oracle or a helper transport must be unreachable
        without an explicit development feature, so a packaged host cannot be
        steered by the environment.
        """

        manifest = (ROOT / "crates" / "nioh3-protected" / "Cargo.toml").read_text(
            encoding="utf-8"
        )
        # The feature exists, is named for testing, and is not a default.
        self.assertIn('test-fake = []', manifest)
        self.assertNotIn("default =", manifest)
        # The only entry point that can select the scripted oracle requires it.
        example = manifest.split("[[example]]", 1)[1].split("[features]", 1)[0]
        self.assertIn('required-features = ["test-fake"]', example)

        lib = (ROOT / "crates" / "nioh3-protected" / "src" / "lib.rs").read_text(
            encoding="utf-8"
        )
        index = lib.find("pub mod scan_bench_api;")
        self.assertNotEqual(index, -1)
        self.assertIn(
            '#[cfg(feature = "test-fake")]',
            lib[max(0, index - 80) : index],
            "the bench surface must be feature-gated in lib.rs",
        )

        # The script environment variable is only honoured inside a
        # feature-gated block.
        source = self.every_source
        for position in range(len(source)):
            if source.startswith("NIOH3_PROTECTED_ORACLE_SCRIPT", position):
                window = source[max(0, position - 400) : position]
                self.assertIn(
                    'feature = "test-fake"',
                    window,
                    "the scripted-oracle environment variable must sit under a "
                    "test-fake gate",
                )

    def test_the_release_worker_ignores_the_script_environment(self) -> None:
        """Empirical half: the real release binary fails closed, not on a script."""

        release = self.release_binary()
        state = self.temp_dir("script-env-state")
        script = self.temp_dir("script-env") / "oracle.json"
        template = "00" * 0xE8
        script.write_text(
            json.dumps({"template_hex": template, "rarity": 4}), encoding="utf-8"
        )
        env = dict(os.environ)
        env["NIOH3_STATE_ROOT"] = str(state)
        env["NIOH3_PROTECTED_ORACLE_SCRIPT"] = str(script)
        worker = FramedWorker(
            [
                str(release),
                "--role",
                "runtime",
                "--dev-protected-only",
                "--state-root",
                str(state),
                "--data-root",
                str(ROOT / "nioh3_scroll_editor" / "data"),
                "--contract-dir",
                str(SCHEMA_DIR),
            ],
            cwd=ROOT,
            env=env,
            name="release runtime worker",
        )
        try:
            handshake = worker.call("handshake")
            self.assertTrue(handshake["ok"], handshake)
            digest = handshake["result"]["context"]["context_digest"]
            started = worker.call(
                "runtime.generate",
                {
                    "template": {
                        "template_hex": template,
                        "source_sha256": "0" * 64,
                        "context_digest": digest,
                        "save_fingerprint": "a" * 64,
                    },
                    "seed": 1,
                    "playthrough": 3,
                    "rarity": 4,
                    "level": 180,
                    "recommended_level": 183,
                    "title_screen_confirmed": True,
                },
            )
            self.assertTrue(started["ok"], started)
            job = started["result"]
            deadline = time.monotonic() + 120
            while job["state"] in ("running", "cancel_requested"):
                self.assertLess(time.monotonic(), deadline, "the job never finished")
                snapshot = worker.call("job.snapshot", {"job_id": job["job_id"]})
                self.assertTrue(snapshot["ok"], snapshot)
                job = snapshot["result"]
            self.assertEqual(
                job["state"],
                "failed",
                "a release build must not satisfy generation from the script "
                f"environment: {job}",
            )
            message = job["error"]["message"]
            self.assertNotIn(
                "oracle script",
                message,
                "the release build must not even read the script path",
            )
        finally:
            self.assertEqual(worker.terminate(), 0)

    def test_the_runtime_role_never_calls_a_terminating_api(self) -> None:
        # A protected host owns a game process; it must never terminate or
        # suspend it, whatever the request says.
        for banned in (
            "TerminateProcess",
            "TerminateThread",
            "SuspendThread",
            "ResumeThread",
            "WinExec",
        ):
            self.assertNotIn(banned, self.source, f"{banned} must not appear")
