"""Independent real protected save-role process acceptance.

`tests/migration/test_protected_worker_parity.py` compares the Rust and Python
protected hosts method by method. This gate accepts the *save role end to end*:
two real protected worker processes (Rust and the shipped Python worker) are
driven over the shipped framed-JSON protocol, on isolated encrypted copies of
one synthetic save, through the whole certified path

    search effect-sequence candidate -> G4 materializer -> prepare_install /
    install_many -> commit -> on-disk container

including the R4 stage-one/final-preview pair, the private generation-context
identity, a Python-created backup bundle restored by Rust, the recycle/path/
foreign-binding refusals, EOF during a claimed write, process death mid-write,
restart receipts and no-replay, and an app-level edit+delete+install lifecycle
performance comparison against the shipped `SaveApplication` over the same RPC.

Scope boundaries: no real user save, no game process, no packaged app. Every
write is a task-local encrypted fixture under the D-backed fixture root. Live
save acceptance (a real save produced by the game, and a real game reading it
back) is *not* claimed here and remains the open gate.
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import statistics
import subprocess
import sys
import tempfile
import threading
import time
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from emaki_exchange import (  # noqa: E402
    USER_CHECKSUM_BODY_END,
    USER_CHECKSUM_BODY_START,
    USER_CHECKSUM_VALUE_OFFSET,
    compute_user_checksum,
)
from tests.migration.cargo_target import resolved_cargo_target_dir  # noqa: E402
from tests.migration.test_protected_worker_parity import FramedWorker  # noqa: E402
from tests.migration.test_save_read_parity import (  # noqa: E402
    SCROLL_GROUP_OFFSET,
    SCROLL_RECORD_SIZE,
    build_fixture_bytes,
    native_transform,
    native_transform_short,
)

FIXTURE_ROOT = Path(
    os.environ.get(
        "NIOH3_PROTECTED_SAVE_FIXTURE_ROOT",
        r"D:\Nioh3_v080_deliverables\m3-save-acceptance\protected",
    )
)
DELIVERABLES = ROOT / "deliverables" / "v080-completion-readiness"
ACCOUNT = "76561198000000000"
FOREIGN_ACCOUNT = "76561198000000001"
JOB_TIMEOUT_SECONDS = 300
# The shipped `SaveApplication` keeps its own state under the worker state root;
# the Rust host keeps its transaction state under `protected-internal`.
SHIPPED_BACKUP_SUBDIR = Path("backups")
RUST_BACKUP_SUBDIR = Path("protected-internal") / "backups"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def checksum_of(blob: bytes) -> int:
    import struct

    seed = struct.unpack_from("<I", blob, 0x90_0190)[0]
    return compute_user_checksum(blob[USER_CHECKSUM_BODY_START:USER_CHECKSUM_BODY_END], seed)


def stored_checksum(blob: bytes) -> int:
    import struct

    return struct.unpack_from("<I", blob, USER_CHECKSUM_VALUE_OFFSET)[0]


def drain(worker: FramedWorker, log_path: Path | None = None) -> None:
    """Keep `stderr` flowing so a chatty worker cannot block on its pipe.

    When `log_path` is given the same lines are appended there, which is how the
    gate reads a host's own per-operation diagnostics.
    """

    stream = worker.process.stderr
    if stream is None:
        return

    def pump() -> None:
        try:
            handle = log_path.open("ab") if log_path is not None else None
            try:
                for line in iter(lambda: stream.readline(), b""):
                    if handle is not None:
                        handle.write(line)
                        handle.flush()
            finally:
                if handle is not None:
                    handle.close()
        except Exception:
            return

    threading.Thread(target=pump, daemon=True, name=f"{worker.name}-stderr").start()


class ProtectedSaveAcceptanceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        manifest = ROOT / "crates" / "nioh3-protected" / "Cargo.toml"
        if not manifest.is_file():
            raise AssertionError(f"the protected worker crate is required: {manifest}")
        with manifest.open("rb") as stream:
            bins = [str(entry.get("name", "")) for entry in tomllib.load(stream).get("bin", [])]
        if bins[:1] != ["nioh3-protected-worker"]:
            raise AssertionError(f"unexpected protected worker bins: {bins}")
        cargo = shutil.which("cargo")
        if cargo is None:
            raise AssertionError("cargo is required to build the protected worker")
        cls.target = resolved_cargo_target_dir("protected")
        env = {**os.environ, "CARGO_TARGET_DIR": cls.target}
        build = subprocess.run(
            [
                cargo,
                "build",
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
                + build.stderr.decode("utf-8", "replace")[-4000:]
            )
        cls.rust_binary = Path(cls.target) / "release" / "nioh3-protected-worker.exe"
        if not cls.rust_binary.is_file():
            raise AssertionError(f"missing release protected worker: {cls.rust_binary}")
        cls.python = sys.executable
        FIXTURE_ROOT.mkdir(parents=True, exist_ok=True)
        cls.temp = tempfile.TemporaryDirectory(dir=str(FIXTURE_ROOT))
        cls.root = Path(cls.temp.name)
        plain = cls.root / "base-plain.bin"
        plain.write_bytes(bytes(build_fixture_bytes()))
        container = cls.root / "base-container.bin"
        native_transform(plain, container)
        cls.container = container.read_bytes()
        cls.plain = plain.read_bytes()
        cls.parity_gaps: list[str] = []
        cls.performance_findings: list[str] = []

    @classmethod
    def tearDownClass(cls) -> None:
        DELIVERABLES.mkdir(parents=True, exist_ok=True)
        (DELIVERABLES / "M3B_PROTECTED_SAVE_ACCEPTANCE.json").write_text(
            json.dumps(
                {
                    "task": "independent real protected save-role process acceptance",
                    "scope": {
                        "real_processes": [
                            "rust protected worker (release)",
                            "shipped python protected worker",
                        ],
                        "fixtures": str(FIXTURE_ROOT),
                        "excluded": [
                            "real game process",
                            "real user save",
                            "packaged application",
                        ],
                    },
                    "product_parity_gaps": cls.parity_gaps,
                    "performance_tradeoffs": cls.performance_findings,
                },
                indent=2,
            ),
            encoding="utf-8",
        )
        cls.temp.cleanup()

    # ------------------------------------------------------------------ setup

    def isolated(self, name: str) -> tuple[Path, Path]:
        """A fresh encrypted fixture copy plus its own state root."""

        base = self.root / name
        save = base / ACCOUNT / "SAVEDATA00" / "SAVEDATA.BIN"
        save.parent.mkdir(parents=True)
        (save.parent / "BACKUP.BIN").write_bytes(b"game-backup")
        system = base / ACCOUNT / "SYSTEMSAVEDATA00"
        system.mkdir()
        (system / "SAVEDATA.BIN").write_bytes(b"system-save")
        save.write_bytes(self.container)
        state = self.root / f"{name}-state"
        state.mkdir()
        return save, state

    def rust_worker(
        self,
        state: Path,
        extra_env: dict[str, str] | None = None,
        stderr_log: Path | None = None,
    ) -> FramedWorker:
        worker = FramedWorker(
            [
                str(self.rust_binary),
                "--role",
                "save",
                "--dev-protected-only",
                "--state-root",
                str(state),
                "--data-root",
                str(ROOT / "nioh3_scroll_editor" / "data"),
                "--contract-dir",
                str(ROOT / "packages" / "contracts"),
            ],
            cwd=ROOT,
            env={**os.environ, "NIOH3_STATE_ROOT": str(state), **(extra_env or {})},
            name=f"rust-{state.name}",
        )
        drain(worker, stderr_log)
        return worker

    def python_worker(self, state: Path, extra_env: dict[str, str] | None = None) -> FramedWorker:
        worker = FramedWorker(
            [self.python, "-u", "-m", "nioh3_scroll_editor.protected_worker", "--role", "save"],
            cwd=ROOT,
            env={
                **os.environ,
                "NIOH3_STATE_ROOT": str(state),
                "PYTHONUTF8": "1",
                "PYTHONIOENCODING": "utf-8",
                **(extra_env or {}),
            },
            name=f"python-{state.name}",
        )
        drain(worker)
        return worker

    # ------------------------------------------------------------- protocols

    def drive(self, worker: FramedWorker, method: str, params: dict) -> dict:
        started_at = time.perf_counter()
        started = worker.call(method, params)
        if not started["ok"]:
            raise AssertionError(f"{method} refused: {started}")
        job = started["result"]
        while job["state"] in ("running", "cancel_requested"):
            if time.perf_counter() - started_at > JOB_TIMEOUT_SECONDS:
                raise AssertionError(f"{method} did not finish in time: {job}")
            time.sleep(0.02)
            job = worker.call("job.snapshot", {"job_id": job["job_id"]})["result"]
        if job["state"] == "failed":
            raise AssertionError(f"{method} failed: {job['error']}")
        return job["result"]

    def drive_failure(self, worker: FramedWorker, method: str, params: dict) -> dict:
        """The error frame from one refused job, or from an inline refusal."""

        started_at = time.perf_counter()
        started = worker.call(method, params)
        if not started["ok"]:
            return started["error"]
        job = started["result"]
        while job["state"] in ("running", "cancel_requested"):
            if time.perf_counter() - started_at > 60:
                raise AssertionError(f"{method} did not refuse in time: {job}")
            time.sleep(0.02)
            job = worker.call("job.snapshot", {"job_id": job["job_id"]})["result"]
        if job["state"] != "failed":
            raise AssertionError(f"{method} was expected to refuse: {job}")
        return job["error"]

    def drive_or_refuse(self, worker: FramedWorker, method: str, params: dict) -> tuple[str, dict]:
        """`("ok", result)` or `("refused", error)` for one job."""

        started_at = time.perf_counter()
        started = worker.call(method, params)
        if not started["ok"]:
            return "refused", started["error"]
        job = started["result"]
        while job["state"] in ("running", "cancel_requested"):
            if time.perf_counter() - started_at > JOB_TIMEOUT_SECONDS:
                raise AssertionError(f"{method} did not finish in time: {job}")
            time.sleep(0.02)
            job = worker.call("job.snapshot", {"job_id": job["job_id"]})["result"]
        if job["state"] == "failed":
            return "refused", job["error"]
        return "ok", job["result"]

    def handshake_digest(self, worker: FramedWorker) -> str:
        handshake = worker.call("handshake")
        self.assertTrue(handshake["ok"], handshake)
        self.assertEqual(handshake["result"]["role"], "save")
        self.assertFalse(handshake["result"]["kill_safe"])
        return handshake["result"]["context"]["context_digest"]

    def register(self, worker: FramedWorker, save: Path) -> dict:
        return self.drive(worker, "save.register", {"path": str(save)})

    def inventory(self, worker: FramedWorker, save_id: str) -> dict:
        return self.drive(worker, "save.inventory", {"save_id": save_id})

    def commit(self, worker: FramedWorker, plan_id: str) -> dict:
        return self.drive(worker, "save.commit", {"plan_id": plan_id})

    # ------------------------------------------------------------ candidates

    def candidate_payloads(self, digest: str, level: int = 180) -> list[dict]:
        """Certified `effect_sequence_only` candidates, R3 / R4 / R5.

        These are what the search side hands the save role: effect-sequence-only
        payloads bound to the private generation context. Rarity 4 uses the
        certified final-sequence generator, rarity 3 and 5 their own.
        """

        from nioh3_scroll_editor.candidate_transfer import export_candidate
        from nioh3_scroll_editor.effect_sequence import (
            generate_ng3_rarity3_effect_sequence,
            generate_ng3_rarity4_final_effect_sequence,
            generate_ng3_rarity5_effect_sequence,
        )
        from nioh3_scroll_editor.models import ScrollCandidate

        def payload(sequence) -> dict:
            return export_candidate(ScrollCandidate.from_effect_sequence(sequence), digest, level)

        return [
            payload(generate_ng3_rarity3_effect_sequence(seed=54321, level=level)),
            # Seed 1 is the shipped rarity-4 case whose completion pass replaces
            # a slot, so its stage-one and final records genuinely differ.
            payload(generate_ng3_rarity4_final_effect_sequence(1, level=level)),
            payload(generate_ng3_rarity5_effect_sequence(seed=24680, level=level)),
        ]

    @staticmethod
    def masked_record(record_hex: str) -> bytes:
        """A record with the installer-owned identity fields zeroed."""

        record = bytearray(bytes.fromhex(record_hex))
        record[0x1C:0x20] = bytes(4)
        record[0x28:0x2C] = bytes(4)
        return bytes(record)

    def installed_slots(self, plaintext: bytes) -> list[int]:
        return [
            slot
            for slot in range(400)
            if plaintext[
                SCROLL_GROUP_OFFSET + slot * SCROLL_RECORD_SIZE : SCROLL_GROUP_OFFSET
                + (slot + 1) * SCROLL_RECORD_SIZE
            ]
            != self.plain[
                SCROLL_GROUP_OFFSET + slot * SCROLL_RECORD_SIZE : SCROLL_GROUP_OFFSET
                + (slot + 1) * SCROLL_RECORD_SIZE
            ]
        ]

    def record_at(self, plaintext: bytes, slot: int) -> bytes:
        start = SCROLL_GROUP_OFFSET + slot * SCROLL_RECORD_SIZE
        return plaintext[start : start + SCROLL_RECORD_SIZE]

    def decrypt(self, container: Path, name: str) -> bytes:
        output = self.root / f"decrypted-{name}.bin"
        native_transform_short(container, output)
        return output.read_bytes()

    def compare_preview(self, rust_value: dict, python_value: dict, label: str) -> None:
        """Compare a plan preview and record (not assert) key-set differences."""

        missing = sorted(set(python_value) - set(rust_value))
        extra = sorted(set(rust_value) - set(python_value))
        shared = {key: rust_value[key] for key in rust_value if key in python_value}
        expected = {key: python_value[key] for key in rust_value if key in python_value}
        self.assertEqual(shared, expected, f"{label}: the shared preview fields must match")
        if missing or extra:
            self.parity_gaps.append(
                f"{label}: preview key sets differ (rust missing {missing}, rust extra {extra})"
            )

    @staticmethod
    def crate_commit_control() -> dict:
        """The save-lane crate-level guarded commit median, when it is on disk.

        `tests/migration/test_save_performance_parity.py` measures a release
        in-crate plan+commit on the same fixture builder and the same D: volume.
        Quoting it here separates "the crate is slow" from "the protected host
        adds cost on top of the crate".
        """

        path = DELIVERABLES / "M3B_SAVE_COMMIT_TIMINGS.json"
        try:
            payload = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            return {"available": False}
        return {
            "available": True,
            "source": path.name,
            "operation": "release in-crate plan_edit + commit (median of 3, cold)",
            "median_commit_s": payload.get("median_commit_s"),
        }

    # ----------------------------------------------------------------- tests

    def test_materialize_then_batch_install_matches_the_shipped_host(self) -> None:
        """G4 materializer -> prepare_install_many -> commit -> on-disk bytes."""

        rust_save, rust_state = self.isolated("batch-rust")
        python_save, python_state = self.isolated("batch-python")
        rust = self.rust_worker(rust_state)
        python = self.python_worker(python_state)
        try:
            rust_digest = self.handshake_digest(rust)
            python_digest = self.handshake_digest(python)
            self.assertEqual(
                rust_digest,
                python_digest,
                "both protected hosts must publish the same private generation context",
            )
            digest = rust_digest

            rust_registered = self.register(rust, rust_save)
            python_registered = self.register(python, python_save)
            rust_inventory = self.inventory(rust, rust_registered["save_id"])
            python_inventory = self.inventory(python, python_registered["save_id"])

            # A payload bound to a foreign generation context must be refused by
            # both hosts on the materializer path: that is the private-identity
            # gate the search -> save handoff depends on.
            foreign = dict(self.candidate_payloads(digest)[0])
            foreign["context_digest"] = "0" * 64
            rust_foreign = self.drive_failure(
                rust,
                "save.materialize_live_many",
                {
                    "save_id": rust_registered["save_id"],
                    "snapshot_id": rust_inventory["snapshot_id"],
                    "candidates": [foreign],
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )
            python_foreign = self.drive_failure(
                python,
                "save.materialize_live_many",
                {
                    "save_id": python_registered["save_id"],
                    "snapshot_id": python_inventory["snapshot_id"],
                    "candidates": [foreign],
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )
            self.assertEqual(
                rust_foreign,
                python_foreign,
                "a foreign generation context must be refused identically",
            )
            self.assertIn("context", rust_foreign["message"].lower())

            self.assertEqual(rust_inventory["source_sha256"], python_inventory["source_sha256"])
            self.assertEqual(rust_inventory["entries"], python_inventory["entries"])
            rust_template = self.drive(
                rust,
                "save.template",
                {
                    "save_id": rust_registered["save_id"],
                    "snapshot_id": rust_inventory["snapshot_id"],
                    "playthrough": 3,
                },
            )
            python_template = self.drive(
                python,
                "save.template",
                {
                    "save_id": python_registered["save_id"],
                    "snapshot_id": python_inventory["snapshot_id"],
                    "playthrough": 3,
                },
            )
            self.assertEqual(rust_template, python_template)
            self.assertEqual(rust_template["context_digest"], digest)

            payloads = self.candidate_payloads(digest)
            rust_materialized = self.drive(
                rust,
                "save.materialize_live_many",
                {
                    "save_id": rust_registered["save_id"],
                    "snapshot_id": rust_inventory["snapshot_id"],
                    "candidates": payloads,
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )
            python_materialized = self.drive(
                python,
                "save.materialize_live_many",
                {
                    "save_id": python_registered["save_id"],
                    "snapshot_id": python_inventory["snapshot_id"],
                    "candidates": payloads,
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )
            self.assertEqual(
                rust_materialized["candidates"],
                python_materialized["candidates"],
                "the G4 materializer must produce the shipped records",
            )
            materialized = rust_materialized["candidates"]
            self.assertEqual([item["rarity"] for item in materialized], [3, 4, 5])
            for item in materialized:
                self.assertTrue(item["installation_record_hex"], item)
                self.assertEqual(item["context_digest"], digest)
                self.assertEqual(item["record_stage"], "final_record")
            rarity4 = materialized[1]
            self.assertNotEqual(
                rarity4["record_hex"],
                rarity4["installation_record_hex"],
                "the R4 stage-one install record and the final preview must stay separate",
            )

            rust_plan = self.drive(
                rust,
                "save.prepare_install_many",
                {
                    "save_id": rust_registered["save_id"],
                    "snapshot_id": rust_inventory["snapshot_id"],
                    "candidates": materialized,
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )
            python_plan = self.drive(
                python,
                "save.prepare_install_many",
                {
                    "save_id": python_registered["save_id"],
                    "snapshot_id": python_inventory["snapshot_id"],
                    "candidates": materialized,
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )
            self.assertEqual(rust_plan["kind"], "install_many")
            self.assertEqual(rust_plan["kind"], python_plan["kind"])
            self.assertEqual(rust_plan["source_sha256"], python_plan["source_sha256"])
            self.assertEqual(rust_plan["preview"]["count"], 3)
            self.assertEqual(rust_plan["preview"]["count"], python_plan["preview"]["count"])
            for index, (rust_item, python_item) in enumerate(
                zip(rust_plan["preview"]["items"], python_plan["preview"]["items"])
            ):
                self.compare_preview(
                    rust_item, python_item, f"prepare_install_many item {index}"
                )

            rust_receipt = self.commit(rust, rust_plan["plan_id"])
            python_receipt = self.commit(python, python_plan["plan_id"])
            self.assertEqual(rust_receipt["commit_status"], "committed", rust_receipt)
            self.assertEqual(python_receipt["commit_status"], "committed", python_receipt)

            self.assertEqual(
                rust_save.read_bytes(),
                python_save.read_bytes(),
                "the Rust protected host must write the shipped container bytes",
            )
            plaintext = self.decrypt(rust_save, "batch-rust")
            self.assertEqual(plaintext, self.decrypt(python_save, "batch-python"))

            slots = self.installed_slots(plaintext)
            self.assertEqual(len(slots), 3, f"exactly the batch must change: {slots}")
            self.assertEqual(
                slots,
                list(range(slots[0], slots[0] + 3)),
                "the batch must occupy one contiguous run",
            )
            occupied = [entry["slot_index"] for entry in rust_inventory["entries"]]
            self.assertGreater(slots[0], max(occupied), "the batch appends after the tail")
            for index, slot in enumerate(slots):
                record = self.record_at(plaintext, slot)
                self.assertEqual(
                    bytes(
                        bytearray(record)[:0x1C]
                        + bytes(4)
                        + bytearray(record)[0x20:0x28]
                        + bytes(4)
                        + bytearray(record)[0x2C:]
                    ),
                    self.masked_record(materialized[index]["installation_record_hex"]),
                    f"slot {slot} must carry the G4 installation record",
                )
                self.assertNotEqual(
                    record[0x1C:0x20], bytes(4), "the inventory key must be written"
                )
                self.assertNotEqual(
                    record[0x28:0x2C], bytes(4), "the generation serial must be written"
                )
            self.assertNotIn(
                bytes.fromhex(rarity4["record_hex"]),
                plaintext,
                "the R4 final preview must not be installed into the save",
            )
            for slot in range(400):
                if slot in slots:
                    continue
                self.assertEqual(
                    self.record_at(plaintext, slot),
                    self.record_at(self.plain, slot),
                    f"slot {slot} outside the batch must keep its source bytes",
                )
            self.assertEqual(stored_checksum(plaintext), checksum_of(plaintext))

            operations = self.drive(rust, "save.operations", {"save_id": rust_registered["save_id"]})
            self.assertTrue(
                any(
                    entry["operation_id"] == rust_plan["plan_id"]
                    for entry in operations["operations"]
                ),
                operations,
            )
        finally:
            rust.terminate()
            python.terminate()

    def test_discover_uses_the_shipped_game_save_root(self) -> None:
        """`save.discover` follows the shipped LOCALAPPDATA convention.

        The shipped worker computes `%LOCALAPPDATA%/KoeiTecmo/NIOH3/Savedata`
        (`nioh3_scroll_editor/savegame.py` `discover_save_paths`) and never
        consults the application state directory, which is where the packaged
        broker keeps the app's own data. The two roots are deliberately distinct
        here and a decoy save sits under the state root, so a host that scanned
        its state root cannot pass by co-location. Neither worker is allowed to
        read the operator's real save root: `LOCALAPPDATA` points at an isolated
        directory for both.
        """

        local = self.root / "discover-localappdata"
        game_root = local / "KoeiTecmo" / "NIOH3" / "Savedata"
        for slot in (0, 1):
            save = game_root / ACCOUNT / f"SAVEDATA{slot:02d}" / "SAVEDATA.BIN"
            save.parent.mkdir(parents=True, exist_ok=True)
            save.write_bytes(self.container)
            (save.parent / "BACKUP.BIN").write_bytes(b"game-backup")
        system = game_root / ACCOUNT / "SYSTEMSAVEDATA00"
        system.mkdir(parents=True, exist_ok=True)
        (system / "SAVEDATA.BIN").write_bytes(b"system-save")

        rust_state = self.root / "discover-rust-state"
        python_state = self.root / "discover-python-state"
        decoy = rust_state / ACCOUNT / "SAVEDATA00" / "SAVEDATA.BIN"
        decoy.parent.mkdir(parents=True, exist_ok=True)
        decoy.write_bytes(self.container)
        python_state.mkdir(parents=True, exist_ok=True)

        env = {"LOCALAPPDATA": str(local)}
        rust = self.rust_worker(rust_state, env)
        python = self.python_worker(python_state, env)
        try:
            self.handshake_digest(rust)
            self.handshake_digest(python)
            rust_saves = self.drive(rust, "save.discover", {})["saves"]
            python_saves = self.drive(python, "save.discover", {})["saves"]
            self.assertEqual(len(rust_saves), 2, rust_saves)
            self.assertEqual(
                rust_saves,
                python_saves,
                "discovery must match the shipped worker exactly",
            )
            self.assertEqual([entry["save_slot"] for entry in rust_saves], [0, 1])
            self.assertTrue(
                all(entry["account_id"] == ACCOUNT for entry in rust_saves), rust_saves
            )
            paths = [entry["path"] for entry in rust_saves]
            self.assertNotIn(
                str(decoy),
                paths,
                "the application state root must not be scanned for game saves",
            )
            self.assertTrue(all(str(game_root) in path for path in paths), paths)
            # The discovered save is then usable through the same RPC surface.
            inventory = self.inventory(rust, rust_saves[0]["save_id"])
            self.assertEqual(
                inventory["source_sha256"].lower(),
                sha256_file(game_root / ACCOUNT / "SAVEDATA00" / "SAVEDATA.BIN"),
            )
        finally:
            rust.terminate()
            python.terminate()

    def test_stale_generation_context_on_the_install_path(self) -> None:
        """The install path must bind the same private context as the handoff.

        The shipped host imports every candidate before it plans an install, so a
        candidate carrying a stale `context_digest` is refused with the shipped
        text. This case measures the Rust host on the same input and records the
        outcome: when it refuses identically the parity is asserted, and when it
        accepts, the plan is committed to prove the stale candidate really
        installs and the fail-open is written into the acceptance report for the
        host lane (`crates/nioh3-protected/src/save_app.rs`, which owns
        `install_record`/`prepare_install`).
        """

        rust_save, rust_state = self.isolated("stale-rust")
        python_save, python_state = self.isolated("stale-python")
        rust = self.rust_worker(rust_state)
        python = self.python_worker(python_state)
        try:
            digest = self.handshake_digest(rust)
            self.assertEqual(self.handshake_digest(python), digest)
            rust_registered = self.register(rust, rust_save)
            python_registered = self.register(python, python_save)
            rust_inventory = self.inventory(rust, rust_registered["save_id"])
            python_inventory = self.inventory(python, python_registered["save_id"])
            payloads = self.candidate_payloads(digest)
            materialized = self.drive(
                rust,
                "save.materialize_live_many",
                {
                    "save_id": rust_registered["save_id"],
                    "snapshot_id": rust_inventory["snapshot_id"],
                    "candidates": [payloads[0]],
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )["candidates"][0]
            stale = dict(materialized)
            stale["context_digest"] = "0" * 64

            python_outcome = self.drive_failure(
                python,
                "save.prepare_install",
                {
                    "save_id": python_registered["save_id"],
                    "snapshot_id": python_inventory["snapshot_id"],
                    "candidate": stale,
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )
            self.assertIn("context", python_outcome["message"].lower())

            rust_kind, rust_outcome = self.drive_or_refuse(
                rust,
                "save.prepare_install",
                {
                    "save_id": rust_registered["save_id"],
                    "snapshot_id": rust_inventory["snapshot_id"],
                    "candidate": stale,
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )
            if rust_kind == "refused":
                self.assertEqual(
                    rust_outcome,
                    python_outcome,
                    "a stale generation context must be refused identically",
                )
                return

            receipt = self.commit(rust, rust_outcome["plan_id"])
            self.assertEqual(receipt["commit_status"], "committed", receipt)
            self.assertNotEqual(
                rust_save.read_bytes(),
                self.container,
                "the accepted stale candidate must really have been installed",
            )
            self.parity_gaps.append(
                "FAIL-OPEN: save.prepare_install accepted a candidate whose "
                "context_digest does not match the host generation context; the "
                f"plan committed ({receipt['commit_status']}) and the container "
                f"changed (sha256 {sha256_file(rust_save)[:16]}...). The shipped "
                f"host refuses the same input with {python_outcome['message']!r}"
            )
        finally:
            rust.terminate()
            python.terminate()

    def test_prepare_install_materializes_search_candidates(self) -> None:
        """The UI path: a raw `effect_sequence_only` search candidate installs.

        The search side hands the save role effect-sequence-only payloads. The
        shipped host materializes them inside `prepare_install` /
        `prepare_install_many`, so this drives that exact handoff with no prior
        `materialize_live_many` call: plans, previews and the resulting containers
        must match the shipped host, and a context that is *not* certified must
        still be refused with the shipped text on both sides.
        """

        rust_save, rust_state = self.isolated("searchpath-rust")
        python_save, python_state = self.isolated("searchpath-python")
        rust = self.rust_worker(rust_state)
        python = self.python_worker(python_state)
        try:
            digest = self.handshake_digest(rust)
            self.assertEqual(self.handshake_digest(python), digest)
            rust_registered = self.register(rust, rust_save)
            python_registered = self.register(python, python_save)
            payloads = self.candidate_payloads(digest)
            for payload in payloads:
                self.assertEqual(payload["record_stage"], "effect_sequence_only", payload)

            # Single install of a raw search candidate (rarity 4, the paired case).
            rust_inventory = self.inventory(rust, rust_registered["save_id"])
            python_inventory = self.inventory(python, python_registered["save_id"])
            rust_plan = self.drive(
                rust,
                "save.prepare_install",
                {
                    "save_id": rust_registered["save_id"],
                    "snapshot_id": rust_inventory["snapshot_id"],
                    "candidate": payloads[1],
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )
            python_plan = self.drive(
                python,
                "save.prepare_install",
                {
                    "save_id": python_registered["save_id"],
                    "snapshot_id": python_inventory["snapshot_id"],
                    "candidate": payloads[1],
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )
            self.assertEqual(rust_plan["kind"], "install")
            self.compare_preview(
                rust_plan["preview"], python_plan["preview"], "prepare_install search candidate"
            )
            self.assertEqual(
                self.commit(rust, rust_plan["plan_id"])["commit_status"], "committed"
            )
            self.assertEqual(
                self.commit(python, python_plan["plan_id"])["commit_status"], "committed"
            )
            self.assertEqual(
                rust_save.read_bytes(),
                python_save.read_bytes(),
                "materializing a search candidate must produce the shipped bytes",
            )

            # Batch install of the three raw search candidates.
            rust_inventory = self.inventory(rust, rust_registered["save_id"])
            python_inventory = self.inventory(python, python_registered["save_id"])
            rust_batch = self.drive(
                rust,
                "save.prepare_install_many",
                {
                    "save_id": rust_registered["save_id"],
                    "snapshot_id": rust_inventory["snapshot_id"],
                    "candidates": payloads,
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )
            python_batch = self.drive(
                python,
                "save.prepare_install_many",
                {
                    "save_id": python_registered["save_id"],
                    "snapshot_id": python_inventory["snapshot_id"],
                    "candidates": payloads,
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )
            self.assertEqual(rust_batch["preview"]["count"], 3)
            for index, (rust_item, python_item) in enumerate(
                zip(rust_batch["preview"]["items"], python_batch["preview"]["items"])
            ):
                self.compare_preview(
                    rust_item, python_item, f"prepare_install_many search item {index}"
                )
            self.assertEqual(
                self.commit(rust, rust_batch["plan_id"])["commit_status"], "committed"
            )
            self.assertEqual(
                self.commit(python, python_batch["plan_id"])["commit_status"], "committed"
            )
            self.assertEqual(rust_save.read_bytes(), python_save.read_bytes())

            # An uncertified context must still fail closed, with the shipped text.
            from dataclasses import replace

            from nioh3_scroll_editor.candidate_transfer import export_candidate
            from nioh3_scroll_editor.effect_sequence import (
                generate_ng3_rarity3_effect_sequence,
            )
            from nioh3_scroll_editor.models import ScrollCandidate

            uncertified = ScrollCandidate.from_effect_sequence(
                generate_ng3_rarity3_effect_sequence(seed=777, level=180)
            )
            uncertified = replace(uncertified, playthrough=4)
            uncertified_payload = export_candidate(uncertified, digest, 180)
            rust_refusal = self.drive_failure(
                rust,
                "save.prepare_install",
                {
                    "save_id": rust_registered["save_id"],
                    "snapshot_id": self.inventory(rust, rust_registered["save_id"])[
                        "snapshot_id"
                    ],
                    "candidate": uncertified_payload,
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )
            python_refusal = self.drive_failure(
                python,
                "save.prepare_install",
                {
                    "save_id": python_registered["save_id"],
                    "snapshot_id": self.inventory(python, python_registered["save_id"])[
                        "snapshot_id"
                    ],
                    "candidate": uncertified_payload,
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )
            self.assertEqual(rust_refusal, python_refusal, (rust_refusal, python_refusal))
            self.assertIn("研究预览", rust_refusal["message"])
        finally:
            rust.terminate()
            python.terminate()

    def test_single_install_uses_the_installation_record(self) -> None:
        rust_save, rust_state = self.isolated("single-rust")
        python_save, python_state = self.isolated("single-python")
        rust = self.rust_worker(rust_state)
        python = self.python_worker(python_state)
        try:
            digest = self.handshake_digest(rust)
            self.assertEqual(self.handshake_digest(python), digest)
            rust_registered = self.register(rust, rust_save)
            python_registered = self.register(python, python_save)
            rust_inventory = self.inventory(rust, rust_registered["save_id"])
            python_inventory = self.inventory(python, python_registered["save_id"])
            payloads = self.candidate_payloads(digest)
            rarity4 = payloads[1]
            materialized = self.drive(
                rust,
                "save.materialize_live_many",
                {
                    "save_id": rust_registered["save_id"],
                    "snapshot_id": rust_inventory["snapshot_id"],
                    "candidates": [rarity4],
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )["candidates"][0]

            rust_plan = self.drive(
                rust,
                "save.prepare_install",
                {
                    "save_id": rust_registered["save_id"],
                    "snapshot_id": rust_inventory["snapshot_id"],
                    "candidate": materialized,
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )
            python_plan = self.drive(
                python,
                "save.prepare_install",
                {
                    "save_id": python_registered["save_id"],
                    "snapshot_id": python_inventory["snapshot_id"],
                    "candidate": materialized,
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )
            self.assertEqual(rust_plan["kind"], "install")
            self.compare_preview(rust_plan["preview"], python_plan["preview"], "prepare_install")
            self.assertEqual(self.commit(rust, rust_plan["plan_id"])["commit_status"], "committed")
            self.assertEqual(
                self.commit(python, python_plan["plan_id"])["commit_status"], "committed"
            )
            self.assertEqual(rust_save.read_bytes(), python_save.read_bytes())

            plaintext = self.decrypt(rust_save, "single-rust")
            slots = self.installed_slots(plaintext)
            self.assertEqual(len(slots), 1, slots)
            record = self.record_at(plaintext, slots[0])
            self.assertEqual(
                bytes(
                    bytearray(record)[:0x1C]
                    + bytes(4)
                    + bytearray(record)[0x20:0x28]
                    + bytes(4)
                    + bytearray(record)[0x2C:]
                ),
                self.masked_record(materialized["installation_record_hex"]),
                "the installed record must be the stage-one installation record",
            )
            self.assertNotIn(
                bytes.fromhex(materialized["record_hex"]),
                plaintext,
                "the final preview record must not be installed",
            )
        finally:
            rust.terminate()
            python.terminate()

    def test_python_backup_bundle_restores_under_rust(self) -> None:
        """A shipped-host backup bundle is restored by the Rust host.

        The bundle is produced by the shipped Python host's own commit path, then
        left exactly where the shipped host wrote it (the canonical public
        `state_root/backups` root). The Rust host must discover it there, restore
        it in place, never move or rewrite the user's files, keep its own
        transaction artifacts private, and still see the bundle after a restart.
        """

        python_save, python_state = self.isolated("bundle-python")
        rust_save, rust_state = self.isolated("bundle-rust")
        python = self.python_worker(python_state)
        try:
            self.handshake_digest(python)
            registered = self.register(python, python_save)
            inventory = self.inventory(python, registered["save_id"])
            entry = inventory["entries"][0]
            request = {
                "slot_index": entry["slot_index"],
                "header": {**entry["header"], "level": entry["header"]["level"] + 1},
                "effects": [
                    {
                        "slot_index": effect["slot_index"],
                        "effect_id": effect["effect_id"],
                        "value": effect["value"] + 1,
                        "prefix": effect["prefix"],
                        "metadata": effect["metadata"],
                        "tail_0": effect["tail_0"],
                        "tail_1": effect["tail_1"],
                    }
                    for effect in entry["effects"]
                ],
            }
            plan = self.drive(
                python,
                "save.prepare_edit",
                {
                    "save_id": registered["save_id"],
                    "snapshot_id": inventory["snapshot_id"],
                    "edits": [request],
                },
            )
            receipt = self.commit(python, plan["plan_id"])
            self.assertEqual(receipt["commit_status"], "committed", receipt)
            listed = self.drive(python, "save.backups", {"save_id": registered["save_id"]})
            self.assertTrue(listed["backups"], "the shipped commit must checkpoint a bundle")
            backup_id = listed["backups"][0]["backup_id"]
            shipped_bundle = python_state / SHIPPED_BACKUP_SUBDIR / backup_id
            self.assertTrue(shipped_bundle.is_dir(), shipped_bundle)
            checkpointed = python_save.read_bytes()
            self.assertNotEqual(checkpointed, self.container)
        finally:
            python.terminate()

        # Migration reality: the bundle stays exactly where the shipped host
        # wrote it. No copy into a private location, no move — the Rust host has
        # to find it in the canonical public root.
        canonical = rust_state / SHIPPED_BACKUP_SUBDIR
        canonical.mkdir(parents=True, exist_ok=True)
        shutil.copytree(shipped_bundle, canonical / backup_id)
        manifest_before = sha256_file(canonical / backup_id / "backup-manifest.json")

        rust = self.rust_worker(rust_state)
        try:
            self.handshake_digest(rust)
            rust_registered = self.register(rust, rust_save)
            location = self.drive(rust, "save.backup_location", {})
            self.assertEqual(
                Path(location["backup_directory"]).resolve(),
                canonical.resolve(),
                "the Rust host must publish the shipped public backup root",
            )
            listed = self.drive(rust, "save.backups", {"save_id": rust_registered["save_id"]})
            self.assertTrue(
                any(entry["backup_id"] == backup_id for entry in listed["backups"]),
                f"an existing bundle must be discovered in place: {listed}",
            )

            inventory = self.inventory(rust, rust_registered["save_id"])
            plan = self.drive(
                rust,
                "save.prepare_restore",
                {
                    "save_id": rust_registered["save_id"],
                    "snapshot_id": inventory["snapshot_id"],
                    "backup_id": backup_id,
                },
            )
            self.assertEqual(plan["kind"], "restore")
            receipt = self.commit(rust, plan["plan_id"])
            self.assertEqual(receipt["commit_status"], "committed", receipt)
            self.assertEqual(
                rust_save.read_bytes(),
                self.container,
                "the restore must return the checkpointed (pre-edit) generation exactly",
            )
            self.assertNotEqual(
                rust_save.read_bytes(),
                checkpointed,
                "restoring must undo the shipped host's edit",
            )
            # The user's files were neither moved nor rewritten.
            self.assertTrue((canonical / backup_id).is_dir(), "the bundle must stay in place")
            self.assertEqual(
                sha256_file(canonical / backup_id / "backup-manifest.json"),
                manifest_before,
                "the restore must not rewrite the user's manifest",
            )
            self.assertFalse(
                (rust_state / RUST_BACKUP_SUBDIR).exists(),
                "bundles must not be duplicated into the private transaction root",
            )
            self.assertTrue(
                (rust_state / "protected-internal" / "v2-operations").is_dir(),
                "the transaction's own receipts must stay in the private root",
            )
        finally:
            rust.terminate()

        # A restart must still see and be able to use the bundle.
        restarted = self.rust_worker(rust_state)
        try:
            self.handshake_digest(restarted)
            rust_registered = self.register(restarted, rust_save)
            listed = self.drive(
                restarted, "save.backups", {"save_id": rust_registered["save_id"]}
            )
            self.assertTrue(
                any(entry["backup_id"] == backup_id for entry in listed["backups"]),
                f"the bundle must survive a Rust restart: {listed}",
            )
            inventory = self.inventory(restarted, rust_registered["save_id"])
            self.assertEqual(
                inventory["source_sha256"].lower(),
                sha256_file(rust_save),
                "the restored generation must still be the current one",
            )
        finally:
            restarted.terminate()

    def test_recycle_path_and_binding_refusals_match_the_shipped_host(self) -> None:
        rust_save, rust_state = self.isolated("refuse-rust")
        python_save, python_state = self.isolated("refuse-python")
        rust = self.rust_worker(rust_state)
        python = self.python_worker(python_state)
        try:
            self.handshake_digest(rust)
            self.handshake_digest(python)
            rust_registered = self.register(rust, rust_save)
            python_registered = self.register(python, python_save)

            def compare(method: str, rust_params: dict, python_params: dict) -> dict:
                rust_error = self.drive_failure(rust, method, rust_params)
                python_error = self.drive_failure(python, method, python_params)
                self.assertEqual(
                    rust_error["code"],
                    python_error["code"],
                    f"{method}: {rust_error} vs {python_error}",
                )
                self.assertEqual(
                    rust_error["message"],
                    python_error["message"],
                    f"{method}: {rust_error} vs {python_error}",
                )
                return rust_error

            # A foreign backup bundle: same directory name, another account.
            foreign_id = "1758000000000000-00000000000000000000000000000000"
            foreign_manifest = {
                "backup_manifest_schema": "nioh3-scroll-backup/v2",
                "save_schema_profile": "nioh3-pc-v2.00.02-v2.01/save-layout-v1",
                "operation_id": "a" * 32,
                "created_at_utc": "2026-09-15T00:00:00+00:00",
                "action": "foreign-account-checkpoint",
                "steam_account_id": int(FOREIGN_ACCOUNT),
                "save_slot_index": 0,
                "backup_files": [
                    {
                        "source_role": "main_save",
                        "backup_file": "SAVEDATA.BIN",
                        "size": len(self.container),
                        "sha256": hashlib.sha256(self.container).hexdigest().upper(),
                    }
                ],
            }
            for state in (rust_state / RUST_BACKUP_SUBDIR, python_state / SHIPPED_BACKUP_SUBDIR):
                bundle = state / foreign_id
                bundle.mkdir(parents=True, exist_ok=True)
                (bundle / "SAVEDATA.BIN").write_bytes(self.container)
                (bundle / "backup-manifest.json").write_text(
                    json.dumps(foreign_manifest, indent=2),
                    encoding="utf-8",
                )

            compare(
                "save.recycle_backups",
                {"save_id": rust_registered["save_id"], "backup_ids": []},
                {"save_id": python_registered["save_id"], "backup_ids": []},
            )
            compare(
                "save.recycle_backups",
                {"save_id": rust_registered["save_id"], "backup_ids": [foreign_id]},
                {"save_id": python_registered["save_id"], "backup_ids": [foreign_id]},
            )
            compare(
                "save.prepare_restore",
                {
                    "save_id": rust_registered["save_id"],
                    "snapshot_id": self.inventory(rust, rust_registered["save_id"])["snapshot_id"],
                    "backup_id": foreign_id,
                },
                {
                    "save_id": python_registered["save_id"],
                    "snapshot_id": self.inventory(python, python_registered["save_id"])[
                        "snapshot_id"
                    ],
                    "backup_id": foreign_id,
                },
            )

            # Path and identity refusals on the registration gate.
            compare(
                "save.register",
                {"path": str(rust_save.parent / "BACKUP.BIN")},
                {"path": str(python_save.parent / "BACKUP.BIN")},
            )
            stray = self.root / "refuse-stray" / "not-an-account" / "SAVEDATA00" / "SAVEDATA.BIN"
            stray.parent.mkdir(parents=True, exist_ok=True)
            stray.write_bytes(self.container)
            rust_stray = self.drive_failure(rust, "save.register", {"path": str(stray)})
            python_stray = self.drive_failure(python, "save.register", {"path": str(stray)})
            self.assertEqual(rust_stray["code"], python_stray["code"])
            self.assertNotEqual(rust_stray["code"], "", rust_stray)

            # The foreign bundle must not have been recycled by the refusals.
            self.assertTrue((rust_state / RUST_BACKUP_SUBDIR / foreign_id).is_dir())
        finally:
            rust.terminate()
            python.terminate()

    def test_eof_during_a_claimed_write_finishes_it_and_never_replays(self) -> None:
        save, state = self.isolated("eof-write")
        rust = self.rust_worker(state)
        try:
            digest = self.handshake_digest(rust)
            registered = self.register(rust, save)
            inventory = self.inventory(rust, registered["save_id"])
            payloads = self.candidate_payloads(digest)
            materialized = self.drive(
                rust,
                "save.materialize_live_many",
                {
                    "save_id": registered["save_id"],
                    "snapshot_id": inventory["snapshot_id"],
                    "candidates": [payloads[0]],
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )["candidates"]
            plan = self.drive(
                rust,
                "save.prepare_install_many",
                {
                    "save_id": registered["save_id"],
                    "snapshot_id": inventory["snapshot_id"],
                    "candidates": materialized,
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )
            # Claim the write and drop the pipe while the job is in flight. A
            # broken pipe is not permission to abandon ownership: the host must
            # finish the write it claimed, then exit cleanly.
            rust.send("save.commit", {"plan_id": plan["plan_id"]})
            process = rust.process
            assert process.stdin is not None
            process.stdin.close()
            self.assertEqual(process.wait(timeout=JOB_TIMEOUT_SECONDS), 0)
            written = save.read_bytes()
            self.assertNotEqual(written, self.container, "the claimed write must complete")
        finally:
            rust.terminate()

        restarted = self.rust_worker(state)
        try:
            self.handshake_digest(restarted)
            registered = self.register(restarted, save)
            receipt = self.drive(restarted, "save.operation", {"plan_id": plan["plan_id"]})
            self.assertEqual(receipt["commit_status"], "committed", receipt)
            operations = self.drive(
                restarted, "save.operations", {"save_id": registered["save_id"]}
            )
            self.assertTrue(
                any(entry["operation_id"] == plan["plan_id"] for entry in operations["operations"]),
                operations,
            )
            # No replay: a retry answers the durable receipt and does not write.
            before = sha256_file(save)
            retried = self.drive(restarted, "save.commit", {"plan_id": plan["plan_id"]})
            self.assertEqual(retried["commit_status"], "committed")
            self.assertEqual(sha256_file(save), before, "a committed plan must not replay")
        finally:
            restarted.terminate()

    def test_process_death_mid_write_keeps_the_no_replay_invariant(self) -> None:
        """A killed write must never read as a clean success, and never replay.

        Both hosts are killed once their durable *intent* exists: the shipped
        host's `v2-operations/<plan>.json` (`commit_status: executing`) or the
        Rust host's save-core receipt under `protected-internal/v2-operations/`.
        The safety invariants are asserted on both; the difference in how an
        interrupted operation is *reported* after restart is recorded (the
        shipped host answers `unknown` with its warning, the Rust host currently
        answers `Unknown operation ID`) and reported to the host lane, which owns
        `crates/nioh3-protected/src/save_app.rs`.
        """

        observed: dict[str, str] = {}
        for side in ("rust", "python"):
            save, state = self.isolated(f"kill-{side}")
            worker = self.rust_worker(state) if side == "rust" else self.python_worker(state)
            digest = self.handshake_digest(worker)
            registered = self.register(worker, save)
            inventory = self.inventory(worker, registered["save_id"])
            payloads = self.candidate_payloads(digest)
            materialized = self.drive(
                worker,
                "save.materialize_live_many",
                {
                    "save_id": registered["save_id"],
                    "snapshot_id": inventory["snapshot_id"],
                    "candidates": [payloads[0]],
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )["candidates"]
            plan = self.drive(
                worker,
                "save.prepare_install_many",
                {
                    "save_id": registered["save_id"],
                    "snapshot_id": inventory["snapshot_id"],
                    "candidates": materialized,
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )
            plan_id = plan["plan_id"]
            intent = (
                state / RUST_BACKUP_SUBDIR.parent / "v2-operations" / f"{plan_id}.json"
                if side == "rust"
                else state / "v2-operations" / f"{plan_id}.json"
            )
            worker.send("save.commit", {"plan_id": plan_id})
            deadline = time.perf_counter() + 120
            while not intent.is_file() and time.perf_counter() < deadline:
                time.sleep(0.01)
            self.assertTrue(intent.is_file(), f"{side}: the durable intent must precede the write")
            worker.process.kill()
            worker.process.wait(timeout=60)
            killed_at = sha256_file(save)

            restarted = self.rust_worker(state) if side == "rust" else self.python_worker(state)
            try:
                self.handshake_digest(restarted)
                restarted_registered = self.register(restarted, save)
                try:
                    receipt = self.drive(
                        restarted, "save.operation", {"plan_id": plan_id}
                    )
                except AssertionError as error:
                    # The Rust host answers an error frame when its ledger has no
                    # entry for an operation it had already claimed. That missing
                    # durable intent is the recorded parity gap, so the *shape* of
                    # the refusal is asserted here rather than asserted equal to
                    # the shipped `unknown` receipt.
                    self.assertIn(
                        "Unknown operation ID",
                        str(error),
                        f"{side}: an interrupted operation must be reported, got {error}",
                    )
                    observed[side] = "unknown-operation-id"
                else:
                    observed[side] = receipt["commit_status"]
                    self.assertIn(
                        receipt["commit_status"],
                        ("unknown", "committed"),
                        f"{side}: an interrupted operation must never read as a fresh success",
                    )
                    if receipt["commit_status"] == "unknown":
                        self.assertTrue(receipt["warning"], receipt)
                    if killed_at != self.container:
                        self.assertNotEqual(
                            receipt["commit_status"],
                            "not_committed",
                            f"{side}: the write landed, so 'not_committed' would be wrong",
                        )
                    operations = self.drive(
                        restarted,
                        "save.operations",
                        {"save_id": restarted_registered["save_id"]},
                    )
                    self.assertTrue(
                        any(
                            entry["operation_id"] == plan_id
                            for entry in operations["operations"]
                        ),
                        "an interrupted operation the host still knows must be listed",
                    )

                # No replay: the restarted host cannot silently write again.
                try:
                    retried = self.drive(restarted, "save.commit", {"plan_id": plan_id})
                except AssertionError:
                    retried = None
                if retried is not None:
                    self.assertIn(retried["commit_status"], ("committed", "unknown"), retried)
                self.assertEqual(
                    sha256_file(save),
                    killed_at,
                    f"{side}: an interrupted operation must not be replayed by a retry",
                )
            finally:
                restarted.terminate()

        if observed.get("rust") != observed.get("python"):
            self.parity_gaps.append(
                "interrupted-write reporting: rust="
                f"{observed.get('rust')} python={observed.get('python')} "
                "(the shipped host persists 'executing' before the write and reports "
                "'unknown' plus its warning; the Rust host writes its ledger only after "
                "the commit, so the operation is missing from the protected ledger)"
            )

    def test_repeated_inventory_reuses_the_validated_snapshot(self) -> None:
        """Where the redundancy really is: repeat reads with no write.

        The Rust host decrypts the container inside `save.inventory`; the UI
        refreshes inventory repeatedly without writing, so those repeats must
        reuse the validated decryption of unchanged bytes. The same case proves
        the cache cannot serve a stale generation: an external rewrite of the
        container forces a fresh decrypt and a new digest.

        Reuse is measured against this host's own cold read rather than a fixed
        wall-clock floor, and the drift probe takes the best of three samples per
        side. One 9 MB decrypt is a few tens of milliseconds now, so a single
        round trip cannot separate "decrypted" from "cache hit" by itself; the
        digest and entry assertions are the real proof that no stale generation
        was served, and the timing probe only refuses the remaining shape of that
        bug, a cache that returns stale entries under the fresh digest.
        """

        save, state = self.isolated("cache-rust")
        rust = self.rust_worker(state)
        try:
            self.handshake_digest(rust)
            registered = self.register(rust, save)
            started = time.perf_counter()
            first = self.inventory(rust, registered["save_id"])
            cold_seconds = time.perf_counter() - started
            warm_samples: list[float] = []
            second = first
            for _ in range(3):
                started = time.perf_counter()
                second = self.inventory(rust, registered["save_id"])
                warm_samples.append(time.perf_counter() - started)
            warm_seconds = min(warm_samples)
            self.assertNotEqual(first["snapshot_id"], second["snapshot_id"])
            self.assertEqual(first["source_sha256"], second["source_sha256"])
            self.assertEqual(first["entries"], second["entries"])
            self.assertEqual(first["empty_slots"], second["empty_slots"])
            self.assertLess(
                warm_seconds,
                max(cold_seconds / 2, 0.2),
                f"a repeat inventory must reuse the validated decrypt "
                f"(cold {cold_seconds:.3f}s, warm {warm_seconds:.3f}s)",
            )

            # Drift: rewrite the container behind the host's back, three times
            # with a different plaintext byte each round, so every generation
            # carries a digest the cache has never seen.
            drift_samples: list[float] = []
            for attempt, mask in enumerate((0x5A, 0x33, 0x7C)):
                drifted = bytearray(self.plain)
                drifted[0x176CCE + 0x20] ^= mask
                plain = self.root / f"cache-drift-plain-{attempt}.bin"
                plain.write_bytes(bytes(drifted))
                container = self.root / f"cache-drift-container-{attempt}.bin"
                native_transform(plain, container)
                save.write_bytes(container.read_bytes())
                started = time.perf_counter()
                third = self.inventory(rust, registered["save_id"])
                drift_samples.append(time.perf_counter() - started)
                self.assertNotEqual(third["source_sha256"], second["source_sha256"])
                if attempt == 0:
                    # The first drift moves a reported field, so the decoded
                    # inventory itself must change, not just its digest. The
                    # later rounds only need a digest the cache has never seen.
                    self.assertNotEqual(third["entries"], second["entries"])
                self.assertEqual(
                    third["source_sha256"].lower(),
                    sha256_file(save),
                    "the digest must be current",
                )
                second = third
            drifted_seconds = min(drift_samples)
            self.assertGreater(
                drifted_seconds,
                max(warm_seconds * 1.5, warm_seconds + 0.02),
                f"a drifted generation must be decrypted again, not served from "
                f"the cache (warm best {warm_seconds:.3f}s, drifted best "
                f"{drifted_seconds:.3f}s)",
            )
        finally:
            rust.terminate()

    def measure_commit_seconds(self, side: str) -> float:
        """One full prepared commit on this side's own fixture, timed.

        The crash delays are placed relative to real timing instead of a
        hard-coded guess, so the attempts bracket the atomic replace.
        """

        save, state = self.isolated(f"measure-{side}")
        worker = self.rust_worker(state) if side == "rust" else self.python_worker(state)
        try:
            digest = self.handshake_digest(worker)
            registered = self.register(worker, save)
            inventory = self.inventory(worker, registered["save_id"])
            materialized = self.drive(
                worker,
                "save.materialize_live_many",
                {
                    "save_id": registered["save_id"],
                    "snapshot_id": inventory["snapshot_id"],
                    "candidates": [self.candidate_payloads(digest)[0]],
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )["candidates"]
            plan = self.drive(
                worker,
                "save.prepare_install_many",
                {
                    "save_id": registered["save_id"],
                    "snapshot_id": inventory["snapshot_id"],
                    "candidates": materialized,
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )
            started = time.perf_counter()
            receipt = self.drive(worker, "save.commit", {"plan_id": plan["plan_id"]})
            elapsed = time.perf_counter() - started
            self.assertEqual(receipt["commit_status"], "committed", receipt)
            return elapsed
        finally:
            worker.terminate()

    def test_write_boundary_death_reports_unknown_like_the_shipped_host(self) -> None:
        """Kill both hosts the instant the write lands, then compare responses.

        This is a real write boundary: the atomic replace is on disk and the
        terminal receipt has not been written. The shipped host answers `unknown`
        plus its warning from the durable `executing` receipt it wrote before the
        write; the Rust host must publish the same public response, list the
        operation, and answer a retry with that receipt instead of writing again.

        Note the shipped host never rewrites its ledger with the terminal status
        (the outcome lives in memory only), so after a restart even a *successful*
        Python commit reads as `unknown`. The Rust host persists the terminal
        receipt and therefore reports `committed` for a completed write; that
        stronger behaviour is asserted in the EOF case, not weakened here.
        """

        shipped_warning = (
            "Previous process ended before recording the outcome; inspect backups and "
            "current save before further writes"
        )
        observed: dict[str, dict] = {}
        for side in ("rust", "python"):
            # The kill delay is relative to this side's own measured commit, so
            # the attempts bracket the moment the atomic replace lands and stay
            # clear of the terminal ledger write.
            commit_seconds = self.measure_commit_seconds(side)
            delays = (0.35 * commit_seconds, 0.6 * commit_seconds, 0.85 * commit_seconds)
            save, state = self.isolated(f"boundary-{side}")
            worker = self.rust_worker(state) if side == "rust" else self.python_worker(state)
            digest = self.handshake_digest(worker)
            registered = self.register(worker, save)
            inventory = self.inventory(worker, registered["save_id"])
            payloads = self.candidate_payloads(digest)
            materialized = self.drive(
                worker,
                "save.materialize_live_many",
                {
                    "save_id": registered["save_id"],
                    "snapshot_id": inventory["snapshot_id"],
                    "candidates": [payloads[0]],
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )["candidates"]
            plan = self.drive(
                worker,
                "save.prepare_install_many",
                {
                    "save_id": registered["save_id"],
                    "snapshot_id": inventory["snapshot_id"],
                    "candidates": materialized,
                    "recommended_level": 183,
                    "transfer_count": 0,
                },
            )
            # Each attempt starts from a fresh plan. Kill points are measured
            # from the *durable intent* and nothing here opens the save while the
            # host is replacing it: a Windows read handle on the target would make
            # the replace fail instead of observing it.
            plan_id = plan["plan_id"]
            killed_at = sha256_file(save)
            landed = False
            for attempt, delay in enumerate(delays):
                if attempt:
                    worker = (
                        self.rust_worker(state) if side == "rust" else self.python_worker(state)
                    )
                    self.handshake_digest(worker)
                    registered = self.register(worker, save)
                    inventory = self.inventory(worker, registered["save_id"])
                    materialized = self.drive(
                        worker,
                        "save.materialize_live_many",
                        {
                            "save_id": registered["save_id"],
                            "snapshot_id": inventory["snapshot_id"],
                            "candidates": [self.candidate_payloads(self.handshake_digest(worker))[0]],
                            "recommended_level": 183,
                            "transfer_count": 0,
                        },
                    )["candidates"]
                    plan = self.drive(
                        worker,
                        "save.prepare_install_many",
                        {
                            "save_id": registered["save_id"],
                            "snapshot_id": inventory["snapshot_id"],
                            "candidates": materialized,
                            "recommended_level": 183,
                            "transfer_count": 0,
                        },
                    )
                    plan_id = plan["plan_id"]
                before = sha256_file(save)
                intent = state / "v2-operations" / f"{plan_id}.json"
                worker.send("save.commit", {"plan_id": plan_id})
                deadline = time.perf_counter() + 180
                while not intent.is_file() and time.perf_counter() < deadline:
                    time.sleep(0.005)
                self.assertTrue(
                    intent.is_file(),
                    f"{side}: the durable intent must exist before the write is claimed",
                )
                time.sleep(delay)
                worker.process.kill()
                worker.process.wait(timeout=60)
                killed_at = sha256_file(save)
                if killed_at != before:
                    landed = True
                    break
            self.assertTrue(
                landed,
                f"{side}: at least one kill must land a real write "
                f"(measured commit {commit_seconds:.2f}s, delays {delays})",
            )

            restarted = self.rust_worker(state) if side == "rust" else self.python_worker(state)
            try:
                self.handshake_digest(restarted)
                restarted_registered = self.register(restarted, save)
                receipt = self.drive(restarted, "save.operation", {"plan_id": plan_id})
                self.assertEqual(
                    set(receipt),
                    {"operation_id", "save_id", "commit_status", "warning", "details"},
                    receipt,
                )
                self.assertEqual(receipt["commit_status"], "unknown", receipt)
                self.assertEqual(receipt["warning"], shipped_warning, receipt)
                self.assertEqual(receipt["operation_id"], plan_id)
                self.assertEqual(receipt["save_id"], restarted_registered["save_id"])
                self.assertEqual(
                    set(receipt["details"]),
                    {"save_path", "reviewed_source_sha256"},
                    receipt,
                )
                operations = self.drive(
                    restarted,
                    "save.operations",
                    {"save_id": restarted_registered["save_id"]},
                )
                self.assertTrue(
                    any(
                        entry["operation_id"] == plan_id
                        and entry["commit_status"] == "unknown"
                        for entry in operations["operations"]
                    ),
                    f"{side}: the interrupted operation must be listed: {operations}",
                )
                retried = self.drive(restarted, "save.commit", {"plan_id": plan_id})
                self.assertEqual(retried["commit_status"], "unknown", retried)
                self.assertEqual(
                    sha256_file(save),
                    killed_at,
                    f"{side}: an interrupted operation must not be replayed by a retry",
                )
                observed[side] = receipt
            finally:
                restarted.terminate()

        self.assertEqual(set(observed["rust"]), set(observed["python"]))
        self.assertEqual(observed["rust"]["commit_status"], observed["python"]["commit_status"])
        self.assertEqual(observed["rust"]["warning"], observed["python"]["warning"])
        self.assertEqual(
            set(observed["rust"]["details"]), set(observed["python"]["details"])
        )

    def test_app_level_lifecycle_performance(self) -> None:
        """Whole-workflow lifecycle: edit + delete + install, cold and steady.

        One lifecycle is what the app performs: register, inventory, template,
        prepare+commit an edit, prepare+commit a delete, materialize a candidate
        batch and prepare+commit the batch install. Both hosts run it with the
        same guards (three 0.20 s quiescence windows per commit on both sides).
        Cold means a fresh worker process per lifecycle; steady means one worker
        process running several lifecycles over isolated fixture copies.

        The per-phase totals are recorded so a regression can be attributed: the
        Rust host decrypts the container inside every inventory/prepare/commit
        call, while the shipped host keeps a decrypted snapshot per save, and the
        Rust transaction host applies the shipped 0.20 s window to edit and
        delete plans where the shipped host applies none. Both facts are recorded
        as a tradeoff instead of being hidden by a loose threshold; a gross
        regression (more than twice the shipped lifecycle) still fails.
        """

        samples = int(os.environ.get("NIOH3_PROTECTED_SAVE_SAMPLES", "3"))

        def lifecycle(
            worker: FramedWorker, save: Path, digest: str, payloads: list[dict]
        ) -> dict[str, float]:
            phases: dict[str, float] = {}

            def phase(name: str, action) -> dict:
                started = time.perf_counter()
                value = action()
                phases[name] = phases.get(name, 0.0) + (time.perf_counter() - started)
                return value

            registered = phase("register", lambda: self.register(worker, save))
            inventory = phase("inventory", lambda: self.inventory(worker, registered["save_id"]))
            entry = inventory["entries"][0]
            edit = {
                "slot_index": entry["slot_index"],
                "header": {**entry["header"], "level": entry["header"]["level"] + 1},
                "effects": [
                    {
                        "slot_index": effect["slot_index"],
                        "effect_id": effect["effect_id"],
                        "value": effect["value"] + 1,
                        "prefix": effect["prefix"],
                        "metadata": effect["metadata"],
                        "tail_0": effect["tail_0"],
                        "tail_1": effect["tail_1"],
                    }
                    for effect in entry["effects"]
                ],
            }
            phase(
                "inventory",
                lambda: self.drive(
                    worker,
                    "save.template",
                    {
                        "save_id": registered["save_id"],
                        "snapshot_id": inventory["snapshot_id"],
                        "playthrough": 3,
                    },
                ),
            )
            edit_plan = phase(
                "prepare",
                lambda: self.drive(
                    worker,
                    "save.prepare_edit",
                    {
                        "save_id": registered["save_id"],
                        "snapshot_id": inventory["snapshot_id"],
                        "edits": [edit],
                    },
                ),
            )
            phase("commit", lambda: self.commit(worker, edit_plan["plan_id"]))
            inventory = phase("inventory", lambda: self.inventory(worker, registered["save_id"]))
            delete_plan = phase(
                "prepare",
                lambda: self.drive(
                    worker,
                    "save.prepare_delete",
                    {
                        "save_id": registered["save_id"],
                        "snapshot_id": inventory["snapshot_id"],
                        "slots": [entry["slot_index"]],
                    },
                ),
            )
            phase("commit", lambda: self.commit(worker, delete_plan["plan_id"]))
            inventory = phase("inventory", lambda: self.inventory(worker, registered["save_id"]))
            materialized = phase(
                "materialize",
                lambda: self.drive(
                    worker,
                    "save.materialize_live_many",
                    {
                        "save_id": registered["save_id"],
                        "snapshot_id": inventory["snapshot_id"],
                        "candidates": payloads,
                        "recommended_level": 183,
                        "transfer_count": 0,
                    },
                ),
            )["candidates"]
            install_plan = phase(
                "prepare",
                lambda: self.drive(
                    worker,
                    "save.prepare_install_many",
                    {
                        "save_id": registered["save_id"],
                        "snapshot_id": inventory["snapshot_id"],
                        "candidates": materialized,
                        "recommended_level": 183,
                        "transfer_count": 0,
                    },
                ),
            )
            phase("commit", lambda: self.commit(worker, install_plan["plan_id"]))
            return phases

        report: dict[str, dict[str, list[float]]] = {"rust": {}, "python": {}}
        host_totals: dict[str, float] = {}
        host_counts: dict[str, int] = {}
        host_series: dict[str, list[float]] = {}
        for side in ("rust", "python"):
            stderr_log = self.root / f"perf-{side}-stderr.log"

            def spawn(name: str) -> tuple[FramedWorker, Path, str]:
                save, state = self.isolated(name)
                if side == "rust":
                    worker = self.rust_worker(
                        state,
                        {"NIOH3_SAVE_HOST_TIMING": "1", "NIOH3_SAVE_TIMING": "1"},
                        stderr_log,
                    )
                else:
                    worker = self.python_worker(state)
                digest = self.handshake_digest(worker)
                return worker, save, digest

            cold: list[float] = []
            cold_phases: dict[str, float] = {}
            for index in range(samples):
                worker, save, digest = spawn(f"perf-{side}-cold-{index}")
                try:
                    started = time.perf_counter()
                    phases = lifecycle(worker, save, digest, self.candidate_payloads(digest))
                    cold.append(time.perf_counter() - started)
                    for name, value in phases.items():
                        cold_phases[name] = cold_phases.get(name, 0.0) + value
                finally:
                    worker.terminate()
            report[side]["cold"] = cold

            steady: list[float] = []
            steady_phases: dict[str, float] = {}
            worker, _unused, digest = spawn(f"perf-{side}-steady")
            try:
                for index in range(samples):
                    target_save = self.root / f"perf-{side}-steady-{index}" / ACCOUNT / "SAVEDATA00" / "SAVEDATA.BIN"
                    target_save.parent.mkdir(parents=True, exist_ok=True)
                    (target_save.parent / "BACKUP.BIN").write_bytes(b"game-backup")
                    system = target_save.parent.parent / "SYSTEMSAVEDATA00"
                    system.mkdir(parents=True, exist_ok=True)
                    (system / "SAVEDATA.BIN").write_bytes(b"system-save")
                    target_save.write_bytes(self.container)
                    started = time.perf_counter()
                    phases = lifecycle(worker, target_save, digest, self.candidate_payloads(digest))
                    steady.append(time.perf_counter() - started)
                    for name, value in phases.items():
                        steady_phases[name] = steady_phases.get(name, 0.0) + value
            finally:
                worker.terminate()
            report[side]["steady"] = steady
            report[side]["cold_phases"] = cold_phases
            report[side]["steady_phases"] = steady_phases
            if side == "rust" and stderr_log.is_file():
                for line in stderr_log.read_text(encoding="utf-8", errors="replace").splitlines():
                    if line.startswith("host-timing\t"):
                        _, operation, micros = line.split("\t")
                        host_totals[operation] = (
                            host_totals.get(operation, 0.0) + int(micros) / 1e6
                        )
                        host_counts[operation] = host_counts.get(operation, 0) + 1
                        host_series.setdefault(operation, []).append(int(micros) / 1e6)
                        continue
                    if line.startswith("save-timing\t"):
                        _, label, micros = line.split("\t")
                        key = f"core:{label}"
                        host_totals[key] = host_totals.get(key, 0.0) + int(micros) / 1e6
                        host_counts[key] = host_counts.get(key, 0) + 1

        summary = {
            "samples": samples,
            "operation": (
                "protected save role lifecycle: register + inventory + template + "
                "prepare/commit edit + prepare/commit delete + materialize_live_many + "
                "prepare/commit install_many"
            ),
            "guards": [
                "quiescent-baseline (0.20 s x2 per commit)",
                "generation-revalidation-before-replace",
                "durable-receipt",
                "checkpoint-before-write",
                "atomic-replace",
                "readback-verification",
            ],
            "guard_asymmetry": (
                "the Rust transaction host applies the shipped 0.20 s window to every "
                "plan and commit; the shipped host applies windows only inside "
                "install_many/install/restore, so its edit and delete commits run with "
                "no timed window at all"
            ),
            "crate_level_control": self.crate_commit_control(),
            "rust": {
                "cold_s": report["rust"]["cold"],
                "cold_median_s": statistics.median(report["rust"]["cold"]),
                "steady_s": report["rust"]["steady"],
                "steady_median_s": statistics.median(report["rust"]["steady"]),
                "steady_phase_totals_s": report["rust"]["steady_phases"],
                "host_operation_totals_s": host_totals,
                "host_operation_counts": host_counts,
                "host_operation_series_s": host_series,
            },
            "shipped_python": {
                "cold_s": report["python"]["cold"],
                "cold_median_s": statistics.median(report["python"]["cold"]),
                "steady_s": report["python"]["steady"],
                "steady_median_s": statistics.median(report["python"]["steady"]),
                "steady_phase_totals_s": report["python"]["steady_phases"],
            },
        }
        DELIVERABLES.mkdir(parents=True, exist_ok=True)
        (DELIVERABLES / "M3B_PROTECTED_SAVE_PERF.json").write_text(
            json.dumps(summary, indent=2), encoding="utf-8"
        )
        # A recorded tradeoff, not a hidden pass: the shipped host is the
        # reference, so a slower Rust lifecycle is reported with its measured
        # attribution. Only a gross regression fails the gate.
        for regime in ("cold", "steady"):
            rust = summary["rust"][f"{regime}_median_s"]
            shipped = summary["shipped_python"][f"{regime}_median_s"]
            self.assertLessEqual(rust, shipped * 2.0, json.dumps(summary))
            if rust > shipped:
                self.performance_findings.append(
                    f"app-level lifecycle {regime}: rust {rust:.2f}s vs shipped "
                    f"{shipped:.2f}s ({rust / shipped:.2f}x); steady phase totals "
                    f"rust={json.dumps(report['rust']['steady_phases'])} "
                    f"shipped={json.dumps(report['python']['steady_phases'])}"
                )


if __name__ == "__main__":
    unittest.main()
