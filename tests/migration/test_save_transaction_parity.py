"""Guarded save-transaction gate for the M3-b Rust lifecycle.

Every write runs against a task-local copy under a D-backed temporary root (the
workspace drive is nearly full), and the shipped encrypt/decrypt component is the
only oracle used to build the containers. No user save and no game process is
ever involved.

The gate covers the semantics the shipped Python transaction layer enforces:
quiescent baseline, identity drift, backup-before-write, durable replace,
readback, receipt recording, discard, restore from a recorded backup, and refusal
to replay an operation id that already has a receipt.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from emaki_exchange import USER_SAVE_SIZE  # noqa: E402
from tests.migration.test_save_read_parity import (  # noqa: E402
    build_fixture_bytes,
    native_transform,
    native_transform_short,
)
from tests.migration.cargo_target import resolved_cargo_target_dir  # noqa: E402
from tests.migration.save_restore_fixture import (  # noqa: E402
    assert_generations_distinct,
    generation_bytes,
    generation_digests,
    read_generation,
    write_backup_bundle,
    write_generation,
)
from tests.migration.test_save_read_parity import (  # noqa: E402
    OWN_ACCOUNT,
    SAVE_RECORD_TYPE,
    _pack_record,
)

SAVE_CRYPTO = ROOT / "bin" / "Nioh_Savefile_decrypt.exe"
FIXTURE_ROOT = Path(
    os.environ.get(
        "NIOH3_SAVE_FIXTURE_ROOT",
        r"D:\Nioh3_v080_deliverables\m3-save-acceptance",
    )
)
RESTORE_FAULT_HARNESS = ROOT / "tests" / "migration" / "restore_fault_harness" / "Cargo.toml"


def run_host(target: str, arguments: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(
        [
            "cargo",
            "run",
            "--offline",
            "--quiet",
            "--manifest-path",
            str(ROOT / "crates" / "nioh3-save" / "Cargo.toml"),
            "--example",
            "save_transaction",
            "--",
            *arguments,
        ],
        cwd=str(ROOT),
        env={**os.environ, "CARGO_TARGET_DIR": target},
        capture_output=True,
        text=True,
        check=False,
    )


def run_restore_harness(target: str, arguments: list[str]) -> subprocess.CompletedProcess:
    """Drive a Restore plan through public save-crate APIs only."""

    return subprocess.run(
        [
            "cargo",
            "run",
            "--offline",
            "--quiet",
            "--manifest-path",
            str(RESTORE_FAULT_HARNESS),
            "--",
            *arguments,
        ],
        cwd=str(ROOT),
        env={**os.environ, "CARGO_TARGET_DIR": target},
        capture_output=True,
        text=True,
        check=False,
    )


class SaveTransactionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.target = resolved_cargo_target_dir()
        FIXTURE_ROOT.mkdir(parents=True, exist_ok=True)
        cls.temp = tempfile.TemporaryDirectory(dir=str(FIXTURE_ROOT))
        cls.root = Path(cls.temp.name)
        cls.save_path = cls.root / "76561198000000000" / "SAVEDATA00" / "SAVEDATA.BIN"
        cls.save_path.parent.mkdir(parents=True)
        (cls.save_path.parent / "BACKUP.BIN").write_bytes(b"game-backup")
        system = cls.root / "76561198000000000" / "SYSTEMSAVEDATA00"
        system.mkdir()
        (system / "SAVEDATA.BIN").write_bytes(b"system-save")
        cls.state_root = cls.root / "state"
        cls.container = cls.root / "encrypted.bin"
        plain = cls.root / "plain.bin"
        plain.write_bytes(bytes(build_fixture_bytes()))
        native_transform(plain, cls.container)
        cls.save_path.write_bytes(cls.container.read_bytes())
        cls.baseline = hashlib.sha256(cls.save_path.read_bytes()).hexdigest()

    @classmethod
    def tearDownClass(cls) -> None:
        cls.temp.cleanup()

    def setUp(self) -> None:
        """Every case starts from the exact baseline generation."""

        type(self).save_path.write_bytes(type(self).container.read_bytes())
        (type(self).save_path.parent / "BACKUP.BIN").write_bytes(b"game-backup")
        (
            type(self).root / "76561198000000000" / "SYSTEMSAVEDATA00" / "SAVEDATA.BIN"
        ).write_bytes(b"system-save")

    @classmethod
    def backup_count(cls) -> int:
        directory = cls.state_root / "backups"
        if not directory.is_dir():
            return 0
        return len(list(directory.iterdir()))

    def test_edit_commit_records_backup_and_readback(self) -> None:
        rebuilt = type(self).root / "edited_plain.bin"
        edited = bytearray(bytes(build_fixture_bytes()))
        edited[0x176CCE + 0x20 : 0x176CCE + 0x24] = (0xAABBCCDD).to_bytes(4, "little")
        rebuilt.write_bytes(bytes(edited))
        edited_container = type(self).root / "edited_container.bin"
        native_transform(rebuilt, edited_container)

        backups_before = type(self).backup_count()
        plan = run_host(
            type(self).target,
            [
                "plan",
                "--state-root",
                str(type(self).state_root),
                "--save-path",
                str(type(self).save_path),
                "--kind",
                "edit",
                "--source-sha256",
                type(self).baseline,
                "--write-file",
                str(edited_container),
            ],
        )
        self.assertEqual(plan.returncode, 0, plan.stderr)
        plan_id = plan.stdout.strip().splitlines()[0]

        commit = run_host(
            type(self).target,
            [
                "commit",
                "--state-root",
                str(type(self).state_root),
                "--save-path",
                str(type(self).save_path),
                "--kind",
                "edit",
                "--plan-id",
                plan_id,
            ],
        )
        self.assertEqual(commit.returncode, 0, commit.stderr)
        self.assertEqual(
            type(self).save_path.read_bytes(),
            edited_container.read_bytes(),
            "the commit must install exactly the prepared bytes",
        )
        receipt_path = type(self).state_root / "v2-operations" / f"{plan_id}.json"
        self.assertTrue(receipt_path.is_file(), "a receipt must be recorded")
        receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
        self.assertEqual(receipt["outcome"], "committed")
        self.assertEqual(
            receipt["installed_sha256"],
            hashlib.sha256(edited_container.read_bytes()).hexdigest(),
        )
        backups = sorted(
            (type(self).state_root / "backups").iterdir(), key=lambda path: path.name
        )
        self.assertEqual(
            len(backups),
            backups_before + 1,
            "exactly one new checkpoint must be written",
        )
        self.assertEqual(
            (backups[-1] / "SAVEDATA.BIN").read_bytes(),
            type(self).container.read_bytes(),
            "the checkpoint must hold the quiet generation",
        )
        manifest = json.loads(
            (backups[-1] / "backup-manifest.json").read_text(encoding="utf-8")
        )
        self.assertEqual(manifest["backup_manifest_schema"], "nioh3-scroll-backup/v2")
        self.assertEqual(manifest["action"], "edit")
        self.assertEqual(manifest["steam_account_id"], int(type(self).save_path.parent.parent.name))
        self.assertEqual(manifest["save_slot_index"], 0)

    def test_stale_identity_is_refused(self) -> None:
        stale = "0" * 64
        planned = run_host(
            type(self).target,
            [
                "plan",
                "--state-root",
                str(type(self).state_root),
                "--save-path",
                str(type(self).save_path),
                "--kind",
                "delete",
                "--source-sha256",
                stale,
                "--write-file",
                str(type(self).container),
            ],
        )
        self.assertNotEqual(planned.returncode, 0, "a stale identity must be refused")
        self.assertIn("digest mismatch", planned.stderr)

    def test_external_change_between_plan_and_commit_is_refused(self) -> None:
        plan = run_host(
            type(self).target,
            [
                "plan",
                "--state-root",
                str(type(self).state_root),
                "--save-path",
                str(type(self).save_path),
                "--kind",
                "install",
                "--source-sha256",
                type(self).baseline,
                "--write-file",
                str(type(self).container),
            ],
        )
        self.assertEqual(plan.returncode, 0, plan.stderr)
        plan_id = plan.stdout.strip().splitlines()[0]
        # The game (or anything else) touches the system save before the commit.
        (type(self).root / "76561198000000000" / "SYSTEMSAVEDATA00" / "SAVEDATA.BIN").write_bytes(
            b"changed-by-someone-else"
        )
        commit = run_host(
            type(self).target,
            [
                "commit",
                "--state-root",
                str(type(self).state_root),
                "--save-path",
                str(type(self).save_path),
                "--kind",
                "install",
                "--plan-id",
                plan_id,
            ],
        )
        self.assertNotEqual(commit.returncode, 0, "drift must abort the write")
        self.assertIn("changed after the operation was prepared", commit.stderr)

    def test_discard_writes_no_file_and_no_backup(self) -> None:
        before = type(self).save_path.read_bytes()
        backups_before = type(self).backup_count()
        plan = run_host(
            type(self).target,
            [
                "plan",
                "--state-root",
                str(type(self).state_root),
                "--save-path",
                str(type(self).save_path),
                "--kind",
                "install",
                "--source-sha256",
                type(self).baseline,
                "--write-file",
                str(type(self).container),
            ],
        )
        plan_id = plan.stdout.strip().splitlines()[0]
        discarded = run_host(
            type(self).target,
            [
                "discard",
                "--state-root",
                str(type(self).state_root),
                "--save-path",
                str(type(self).save_path),
                "--kind",
                "install",
                "--plan-id",
                plan_id,
            ],
        )
        self.assertEqual(discarded.returncode, 0, discarded.stderr)
        self.assertEqual(type(self).save_path.read_bytes(), before)
        self.assertEqual(
            type(self).backup_count(),
            backups_before,
            "a discarded plan must not leave a checkpoint",
        )
        # The same plan id must not be committable afterwards.
        commit = run_host(
            type(self).target,
            [
                "commit",
                "--state-root",
                str(type(self).state_root),
                "--save-path",
                str(type(self).save_path),
                "--kind",
                "install",
                "--plan-id",
                plan_id,
            ],
        )
        self.assertNotEqual(commit.returncode, 0, "a discarded plan must not commit")

    def test_restore_from_a_recorded_backup(self) -> None:
        # Record a checkpoint by committing, then restore it.
        entry = bytearray(bytes(build_fixture_bytes()))
        entry[0x176CCE + 0x20 : 0x176CCE + 0x24] = (0x11223344).to_bytes(4, "little")
        staged_plain = type(self).root / "restore_plain.bin"
        staged_plain.write_bytes(bytes(entry))
        staged_container = type(self).root / "restore_container.bin"
        native_transform(staged_plain, staged_container)
        plan = run_host(
            type(self).target,
            [
                "plan",
                "--state-root",
                str(type(self).state_root),
                "--save-path",
                str(type(self).save_path),
                "--kind",
                "edit",
                "--source-sha256",
                hashlib.sha256(type(self).save_path.read_bytes()).hexdigest(),
                "--write-file",
                str(staged_container),
            ],
        )
        self.assertEqual(plan.returncode, 0, plan.stderr)
        plan_id = plan.stdout.strip().splitlines()[0]
        commit = run_host(
            type(self).target,
            [
                "commit",
                "--state-root",
                str(type(self).state_root),
                "--save-path",
                str(type(self).save_path),
                "--kind",
                "edit",
                "--plan-id",
                plan_id,
            ],
        )
        self.assertEqual(commit.returncode, 0, commit.stderr)
        receipt = json.loads(
            (type(self).state_root / "v2-operations" / f"{plan_id}.json").read_text(
                encoding="utf-8"
            )
        )
        backup_id = receipt["backup_id"]
        bundles_before = {
            entry.name for entry in (type(self).state_root / "backups").iterdir()
        }
        restored = run_host(
            type(self).target,
            [
                "restore",
                "--state-root",
                str(type(self).state_root),
                "--save-path",
                str(type(self).save_path),
                "--backup-id",
                backup_id,
            ],
        )
        self.assertEqual(restored.returncode, 0, restored.stderr)
        self.assertEqual(
            type(self).save_path.read_bytes(),
            type(self).container.read_bytes(),
            "restore must return the checkpointed generation",
        )
        # The shipped host also records the generation it is about to replace as
        # its own automatic checkpoint, so a restore stays undoable and the
        # selected bundle is never reused as that checkpoint.
        bundles_after = {
            entry.name for entry in (type(self).state_root / "backups").iterdir()
        }
        self.assertIn(backup_id, bundles_after, "the selected backup must survive")
        created = bundles_after - bundles_before
        self.assertEqual(len(created), 1, sorted(created))
        checkpoint = type(self).state_root / "backups" / created.pop()
        manifest = json.loads((checkpoint / "backup-manifest.json").read_text("utf-8"))
        self.assertEqual(manifest["action"], "pre-restore-checkpoint")
        main = next(
            entry
            for entry in manifest["backup_files"]
            if entry["source_role"] == "main_save"
        )
        self.assertEqual(
            (checkpoint / main["backup_file"]).read_bytes(),
            staged_container.read_bytes(),
            "the checkpoint must hold the complete pre-restore generation",
        )
        journal = json.loads((checkpoint / "restore-journal.json").read_text("utf-8"))
        self.assertEqual(journal["schema"], "nioh3-save-restore-journal/v1")
        self.assertEqual(journal["state"], "committed")
        self.assertEqual(journal["source_backup_directory"], backup_id)

    def test_duplicate_operation_id_is_not_replayed(self) -> None:
        plan = run_host(
            type(self).target,
            [
                "plan",
                "--state-root",
                str(type(self).state_root),
                "--save-path",
                str(type(self).save_path),
                "--kind",
                "edit",
                "--source-sha256",
                hashlib.sha256(type(self).save_path.read_bytes()).hexdigest(),
                "--write-file",
                str(type(self).container),
            ],
        )
        plan_id = plan.stdout.strip().splitlines()[0]
        first = run_host(
            type(self).target,
            [
                "commit",
                "--state-root",
                str(type(self).state_root),
                "--save-path",
                str(type(self).save_path),
                "--kind",
                "edit",
                "--plan-id",
                plan_id,
            ],
        )
        self.assertEqual(first.returncode, 0, first.stderr)
        second = run_host(
            type(self).target,
            [
                "commit",
                "--state-root",
                str(type(self).state_root),
                "--save-path",
                str(type(self).save_path),
                "--kind",
                "edit",
                "--plan-id",
                plan_id,
            ],
        )
        self.assertNotEqual(second.returncode, 0, "a committed plan must not replay")
        self.assertIn("already-committed", second.stderr)


class _ProductFixture(unittest.TestCase):
    """Product operations, journals and the ledger.

    Every case drives the Rust host from a fresh process against a task-local
    fixture copy. The shipped Python installer is the oracle for the realized
    bytes wherever the two contracts are the same.

    This class holds the shared fixture and helpers. Its concrete subclasses
    below own the actual cases.
    """

    @classmethod
    def setUpClass(cls) -> None:
        cls.target = resolved_cargo_target_dir()
        FIXTURE_ROOT.mkdir(parents=True, exist_ok=True)
        cls.temp = tempfile.TemporaryDirectory(dir=str(FIXTURE_ROOT))
        cls.root = Path(cls.temp.name)
        cls.account = str(OWN_ACCOUNT)
        cls.save_path = cls.root / cls.account / "SAVEDATA00" / "SAVEDATA.BIN"
        cls.save_path.parent.mkdir(parents=True)
        cls.state_root = cls.root / "state"
        cls.blob = bytes(build_fixture_bytes())
        cls.plain = cls.root / "base-plain.bin"
        cls.plain.write_bytes(cls.blob)
        cls.container = cls.root / "base-container.bin"
        native_transform_short(cls.plain, cls.container)
        cls.baseline = hashlib.sha256(cls.container.read_bytes()).hexdigest()
        restore_b = bytearray(cls.blob)
        restore_b[0x176CCE + 0x20 : 0x176CCE + 0x24] = (0xB0B0B0B0).to_bytes(
            4, "little"
        )
        restore_b_path = cls.root / "restore-b-plain.bin"
        restore_b_path.write_bytes(bytes(restore_b))
        restore_b_container = cls.root / "restore-b-container.bin"
        native_transform_short(restore_b_path, restore_b_container)
        restore_c = bytearray(cls.blob)
        restore_c[0x176CCE + 0x20 : 0x176CCE + 0x24] = (0xC0C0C0C0).to_bytes(
            4, "little"
        )
        restore_c_path = cls.root / "restore-c-plain.bin"
        restore_c_path.write_bytes(bytes(restore_c))
        restore_c_container = cls.root / "restore-c-container.bin"
        native_transform_short(restore_c_path, restore_c_container)
        cls.restore_a = generation_bytes(cls.container.read_bytes(), "generation-a")
        cls.restore_b = generation_bytes(restore_b_container.read_bytes(), "generation-b")
        cls.restore_c = generation_bytes(restore_c_container.read_bytes(), "generation-c")
        assert_generations_distinct(cls.restore_a, cls.restore_b, cls.restore_c)

    @classmethod
    def tearDownClass(cls) -> None:
        cls.temp.cleanup()

    def setUp(self) -> None:
        self.save_path.write_bytes(self.container.read_bytes())
        (self.save_path.parent / "BACKUP.BIN").write_bytes(b"game-backup")
        system = self.root / self.account / "SYSTEMSAVEDATA00"
        system.mkdir(parents=True, exist_ok=True)
        (system / "SAVEDATA.BIN").write_bytes(b"system-save")
        self.clear_state()

    def clear_state(self) -> None:
        for child in ("v2-plans", "v2-operations", "backups"):
            directory = self.state_root / child
            if directory.is_dir():
                for path in sorted(directory.rglob("*"), reverse=True):
                    if path.is_file():
                        path.unlink()
                    elif path.is_dir():
                        path.rmdir()

    def write_container(self, blob: bytes, name: str) -> Path:
        plain = self.root / f"{name}.bin"
        plain.write_bytes(blob)
        container = self.root / f"{name}-container.bin"
        native_transform_short(plain, container)
        return container

    def host(self, *arguments: str) -> subprocess.CompletedProcess:
        return run_host(self.target, list(arguments))

    def plan_id(self, completed: subprocess.CompletedProcess) -> str:
        self.assertEqual(completed.returncode, 0, completed.stderr)
        # Product plan commands print the plan id first and may follow it with
        # realized-outcome fields, so the id is always the first line.
        return completed.stdout.strip().splitlines()[0]

    def receipt(self, plan_id: str) -> dict:
        path = self.state_root / "v2-operations" / f"{plan_id}.json"
        self.assertTrue(path.is_file(), "a receipt must exist")
        return json.loads(path.read_text(encoding="utf-8"))

    def backup_file_count(self) -> int:
        """Files under the managed backups root, so an empty leftover dir is fine."""

        directory = self.state_root / "backups"
        if not directory.is_dir():
            return 0
        return sum(1 for path in directory.rglob("*") if path.is_file())

    def edit_spec(self, slot_index: int, effects: list[dict] | None = None) -> Path:
        spec = {
            "edits": [
                {
                    "slot_index": slot_index,
                    "header": {
                        "playthrough": 3,
                        "level": 185,
                        "recommended_level": 195,
                        "seed": 0x0BADF00D,
                        "rarity": 5,
                        "transfer_count": 2,
                    },
                    "effects": effects if effects is not None else [{"slot_index": 0, "value": 4242}],
                }
            ]
        }
        spec_path = self.root / f"spec-{slot_index}.json"
        spec_path.write_text(json.dumps(spec), encoding="utf-8")
        return spec_path

    def plan_edit(self, slot_index: int, source: str | None = None) -> str:
        completed = self.host(
            "plan-edit",
            "--state-root",
            str(self.state_root),
            "--save-path",
            str(self.save_path),
            "--source-sha256",
            source or self.baseline,
            "--spec-file",
            str(self.edit_spec(slot_index)),
        )
        return self.plan_id(completed)

    def commit(self, plan_id: str) -> subprocess.CompletedProcess:
        return self.host(
            "commit",
            "--state-root",
            str(self.state_root),
            "--save-path",
            str(self.save_path),
            "--plan-id",
            plan_id,
        )

    def decrypt_installed(self, name: str) -> bytes:
        path = self.root / f"installed-{name}.bin"
        native_transform_short(self.save_path, path)
        return path.read_bytes()

    def decode_with_rust_codec(self, container: Path, name: str) -> bytes:
        """Decode a container with the ported Rust codec, byte for byte.

        No shipped process runs and nothing is rewritten, so the returned bytes
        are exactly what the Rust writer encrypted. That is what makes a stored
        checksum assertion here independent of the oracle's own decode path.
        """

        output = self.root / f"rust-decoded-{name}.bin"
        completed = self.host(
            "decrypt-container",
            "--state-root",
            str(self.state_root),
            "--save-path",
            str(self.save_path),
            "--container-file",
            str(container),
            "--output-file",
            str(output),
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        return output.read_bytes()

    def encode_with_rust_codec(self, clear: Path, name: str) -> Path:
        """Encode a clear save with the ported Rust codec, rewriting nothing."""

        container = self.root / f"rust-encoded-{name}.bin"
        completed = self.host(
            "encrypt-container",
            "--state-root",
            str(self.state_root),
            "--save-path",
            str(self.save_path),
            "--write-file",
            str(clear),
            "--output-file",
            str(container),
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        return container

    def decode_with_the_shipped_tool(self, container: Path, name: str) -> bytes:
        path = self.root / f"oracle-decoded-{name}.bin"
        native_transform_short(container, path)
        return path.read_bytes()

    def assert_container_carries_the_derived_checksum(self, container: Path, label: str) -> None:
        """Prove the field inside `container` is the fold of its own body.

        Four anchors, so the assertion cannot be satisfied by normalization:

        1. the container was written by the Rust transaction, not the oracle;
        2. the Rust codec decodes it byte-preservingly, so `stored` is the exact
           value the Rust writer encrypted;
        3. `stored` equals the fold computed by the shipped Python
           (`emaki_exchange.compute_user_checksum`), an independent
           implementation of the same rule;
        4. the shipped tool's own decode of the same container keeps that field
           (it only zeroes the trailing 8 bytes), and a corrupted field is shown
           to fail the same assertion below.
        """

        import struct

        from emaki_exchange import (
            USER_CHECKSUM_BODY_END,
            USER_CHECKSUM_BODY_START,
            USER_CHECKSUM_VALUE_OFFSET,
            compute_user_checksum,
        )

        byte_preserving = self.decode_with_rust_codec(container, f"{label}-rust")
        seed = struct.unpack_from("<I", byte_preserving, 0x90_0190)[0]
        derived = compute_user_checksum(
            byte_preserving[USER_CHECKSUM_BODY_START:USER_CHECKSUM_BODY_END], seed
        )
        stored = struct.unpack_from("<I", byte_preserving, USER_CHECKSUM_VALUE_OFFSET)[0]
        self.assertEqual(
            stored,
            derived,
            f"{label}: the Rust-written container must store the fold of its own body",
        )

        # The oracle keeps that field too; its real behaviour is pinned by
        # `test_the_shipped_tool_preserves_the_checksum_and_drops_only_the_trailer`.
        oracle = self.decode_with_the_shipped_tool(container, f"{label}-oracle")
        self.assertEqual(
            oracle[USER_CHECKSUM_VALUE_OFFSET : USER_CHECKSUM_VALUE_OFFSET + 4],
            byte_preserving[USER_CHECKSUM_VALUE_OFFSET : USER_CHECKSUM_VALUE_OFFSET + 4],
            f"{label}: the shipped tool must not rewrite the user checksum",
        )
        differing = {
            index
            for index, (left, right) in enumerate(zip(oracle, byte_preserving, strict=True))
            if left != right
        }
        self.assertLessEqual(
            differing,
            set(range(USER_SAVE_SIZE - 8, USER_SAVE_SIZE)),
            f"{label}: the shipped tool must not rewrite anything but the trailing 8 bytes",
        )
        self.assertNotIn(USER_CHECKSUM_VALUE_OFFSET, differing)

        # Negative control: the same assertion must fail on a wrong stored
        # checksum, so a gate that stopped comparing would be caught.
        corrupted = bytearray(container.read_bytes())
        corrupted[USER_CHECKSUM_VALUE_OFFSET] ^= 0x01
        corrupted_path = self.root / f"{label}-corrupt-container.bin"
        corrupted_path.write_bytes(bytes(corrupted))
        corrupt_plain = self.decode_with_rust_codec(corrupted_path, f"{label}-corrupt")
        corrupt_stored = struct.unpack_from("<I", corrupt_plain, USER_CHECKSUM_VALUE_OFFSET)[0]
        corrupt_derived = compute_user_checksum(
            corrupt_plain[USER_CHECKSUM_BODY_START:USER_CHECKSUM_BODY_END],
            struct.unpack_from("<I", corrupt_plain, 0x90_0190)[0],
        )
        self.assertNotEqual(
            corrupt_stored,
            corrupt_derived,
            f"{label}: a wrong stored checksum must fail the fold assertion",
        )


class SaveProductTransactionTests(_ProductFixture):
    """Realized product operations and the durable journal."""

    def test_edit_commit_installs_the_composed_record(self) -> None:
        from nioh3_scroll_editor.savegame import (
            LocalEffectEdit,
            SCROLL_GROUP_OFFSET,
            SCROLL_RECORD_SIZE,
            patch_local_scroll_header,
            patch_local_scroll_record,
        )

        slot_index = 1
        plan_id = self.plan_edit(slot_index)
        commit = self.commit(plan_id)
        self.assertEqual(commit.returncode, 0, commit.stderr)
        receipt = self.receipt(plan_id)
        self.assertEqual(receipt["outcome"], "committed")
        self.assertEqual(
            receipt["installed_sha256"],
            hashlib.sha256(self.save_path.read_bytes()).hexdigest(),
        )

        offset = SCROLL_GROUP_OFFSET + slot_index * SCROLL_RECORD_SIZE
        current = self.blob[offset : offset + SCROLL_RECORD_SIZE]
        expected = patch_local_scroll_header(
            current,
            playthrough=3,
            level=185,
            recommended_level=195,
            seed=0x0BADF00D,
            rarity=5,
            transfer_count=2,
        )
        expected = patch_local_scroll_record(expected, [LocalEffectEdit(slot_index=0, value=4242)])
        realized = self.decrypt_installed("edit")
        self.assertEqual(
            realized[offset : offset + SCROLL_RECORD_SIZE],
            expected,
            "the Rust edit must install the reference-composed record",
        )
        self.assertEqual(realized[:6], b"RNNUSR")

    def test_install_matches_the_shipped_python_installer(self) -> None:
        from nioh3_scroll_editor.savegame import SaveCrypto, SaveInstaller, default_crypto_tool

        candidate = bytes(
            _pack_record(
                record_type=SAVE_RECORD_TYPE,
                account_id=OWN_ACCOUNT,
                level=180,
                recommended_level=190,
                seed=0x5EED1234,
                inventory_key=0,
                serial=0,
                rarity=5,
                transfer_count=0,
            )
        )
        plan_id = self.plan_id(
            self.host(
                "plan-install",
                "--state-root",
                str(self.state_root),
                "--save-path",
                str(self.save_path),
                "--source-sha256",
                self.baseline,
                "--record-hex",
                candidate.hex(),
                "--transfer-count",
                "3",
            )
        )
        commit = self.commit(plan_id)
        self.assertEqual(commit.returncode, 0, commit.stderr)
        rust_installed = self.save_path.read_bytes()

        reference_root = self.root / "reference"
        reference_save = reference_root / self.account / "SAVEDATA00" / "SAVEDATA.BIN"
        reference_save.parent.mkdir(parents=True)
        reference_save.write_bytes(self.container.read_bytes())
        (reference_save.parent / "BACKUP.BIN").write_bytes(b"game-backup")
        reference_system = reference_root / self.account / "SYSTEMSAVEDATA00"
        reference_system.mkdir(parents=True)
        (reference_system / "SAVEDATA.BIN").write_bytes(b"system-save")
        installer = SaveInstaller(
            save_path=reference_save,
            crypto=SaveCrypto(default_crypto_tool(ROOT)),
            state_root=self.root / "reference-state",
        )
        installer.install(candidate, transfer_count=3)
        self.assertEqual(
            rust_installed,
            reference_save.read_bytes(),
            "the Rust install must reproduce the shipped installer bytes",
        )

    def test_delete_clears_only_the_selected_records(self) -> None:
        from nioh3_scroll_editor.savegame import SCROLL_GROUP_OFFSET, SCROLL_RECORD_SIZE

        plan_id = self.plan_id(
            self.host(
                "plan-delete",
                "--state-root",
                str(self.state_root),
                "--save-path",
                str(self.save_path),
                "--source-sha256",
                self.baseline,
                "--slot",
                "1",
            )
        )
        commit = self.commit(plan_id)
        self.assertEqual(commit.returncode, 0, commit.stderr)
        realized = self.decrypt_installed("delete")
        offset = SCROLL_GROUP_OFFSET + 1 * SCROLL_RECORD_SIZE
        self.assertEqual(realized[offset : offset + SCROLL_RECORD_SIZE], bytes(SCROLL_RECORD_SIZE))
        untouched = SCROLL_GROUP_OFFSET
        self.assertEqual(
            realized[untouched : untouched + SCROLL_RECORD_SIZE],
            self.blob[untouched : untouched + SCROLL_RECORD_SIZE],
            "a delete must not compact or rewrite other records",
        )

    def test_checkpoint_precedes_the_write(self) -> None:
        plan_id = self.plan_edit(1)
        commit = self.commit(plan_id)
        self.assertEqual(commit.returncode, 0, commit.stderr)
        backup_id = self.receipt(plan_id)["backup_id"]
        bundle = self.state_root / "backups" / backup_id
        self.assertEqual((bundle / "SAVEDATA.BIN").read_bytes(), self.container.read_bytes())
        self.assertEqual((bundle / "BACKUP.BIN").read_bytes(), b"game-backup")
        self.assertEqual((bundle / "SYSTEMSAVEDATA.BIN").read_bytes(), b"system-save")
        manifest = json.loads((bundle / "backup-manifest.json").read_text(encoding="utf-8"))
        self.assertEqual(manifest["backup_manifest_schema"], "nioh3-scroll-backup/v2")
        self.assertEqual(manifest["steam_account_id"], OWN_ACCOUNT)
        self.assertEqual(manifest["save_slot_index"], 0)

    def test_backup_listing_and_recycle(self) -> None:
        plan_id = self.plan_edit(1)
        self.commit(plan_id)
        backup_id = self.receipt(plan_id)["backup_id"]
        listed = self.host(
            "backups",
            "--state-root",
            str(self.state_root),
            "--save-path",
            str(self.save_path),
            "--account-id",
            self.account,
            "--save-slot",
            "0",
        )
        self.assertEqual(listed.returncode, 0, listed.stderr)
        self.assertIn(backup_id, listed.stdout)
        foreign = self.host(
            "backups",
            "--state-root",
            str(self.state_root),
            "--save-path",
            str(self.save_path),
            "--account-id",
            self.account,
            "--save-slot",
            "1",
        )
        self.assertNotIn(backup_id, foreign.stdout)
        recycled = self.host(
            "recycle",
            "--state-root",
            str(self.state_root),
            "--save-path",
            str(self.save_path),
            "--backup-id",
            backup_id,
        )
        self.assertEqual(recycled.returncode, 0, recycled.stderr)
        self.assertFalse((self.state_root / "backups" / backup_id).exists())

    def test_restore_returns_the_checkpointed_generation(self) -> None:
        plan_id = self.plan_edit(1)
        self.commit(plan_id)
        backup_id = self.receipt(plan_id)["backup_id"]
        restored = self.host(
            "restore",
            "--state-root",
            str(self.state_root),
            "--save-path",
            str(self.save_path),
            "--backup-id",
            backup_id,
        )
        self.assertEqual(restored.returncode, 0, restored.stderr)
        self.assertEqual(self.save_path.read_bytes(), self.container.read_bytes())

    def test_operation_id_is_not_replayed(self) -> None:
        plan_id = self.plan_edit(1)
        first = self.commit(plan_id)
        self.assertEqual(first.returncode, 0, first.stderr)
        second = self.commit(plan_id)
        self.assertNotEqual(second.returncode, 0, "a committed plan must not replay")

    def test_edit_refuses_a_save_that_moved(self) -> None:
        plan_id = self.plan_edit(1)
        mutated = bytearray(self.blob)
        offset = 0x176CCE + 1 * 0xE8
        mutated[offset + 0x20 : offset + 0x24] = (0x99887766).to_bytes(4, "little")
        self.save_path.write_bytes(self.write_container(bytes(mutated), "mutated").read_bytes())
        commit = self.commit(plan_id)
        self.assertNotEqual(commit.returncode, 0, "a drifted save must refuse the commit")

    def test_discard_writes_nothing(self) -> None:
        before = self.save_path.read_bytes()
        backups_before = self.backup_file_count()
        plan_id = self.plan_edit(1)
        discarded = self.host(
            "discard",
            "--state-root",
            str(self.state_root),
            "--save-path",
            str(self.save_path),
            "--plan-id",
            plan_id,
        )
        self.assertEqual(discarded.returncode, 0, discarded.stderr)
        self.assertEqual(self.save_path.read_bytes(), before)
        self.assertEqual(
            self.backup_file_count(),
            backups_before,
            "a discarded plan must not write a checkpoint",
        )
        self.assertNotEqual(self.commit(plan_id).returncode, 0)

    def test_ledger_lists_committed_operations(self) -> None:
        plan_id = self.plan_edit(1)
        self.commit(plan_id)
        listed = self.host(
            "operations",
            "--state-root",
            str(self.state_root),
            "--save-path",
            str(self.save_path),
        )
        self.assertEqual(listed.returncode, 0, listed.stderr)
        self.assertIn(plan_id, listed.stdout)
        self.assertIn("committed", listed.stdout)

    def test_product_writes_carry_the_derived_checksum(self) -> None:
        """The installed save's checksum field is our derived value, not a stub.

        A codec gate that merely excludes the field would leave it unchecked, and
        an oracle that rewrote the field would make the check tautological. Both
        are avoided: the Rust-written container is decoded byte-preservingly by
        the Rust codec and the stored field is compared to the shipped Python
        fold. A wrong stored field is shown to fail the same comparison.
        """

        from tests.migration.test_save_read_parity import _pack_record

        plan_id = self.plan_edit(1)
        self.assertEqual(self.commit(plan_id).returncode, 0)
        self.assert_container_carries_the_derived_checksum(self.save_path, "edit")

        candidate = bytes(
            _pack_record(
                record_type=SAVE_RECORD_TYPE,
                account_id=OWN_ACCOUNT,
                level=180,
                recommended_level=190,
                seed=0x5EED7777,
                inventory_key=0,
                serial=0,
                rarity=5,
                transfer_count=0,
            )
        )
        install_plan = self.plan_id(
            self.host(
                "plan-install",
                "--state-root",
                str(self.state_root),
                "--save-path",
                str(self.save_path),
                "--source-sha256",
                hashlib.sha256(self.save_path.read_bytes()).hexdigest(),
                "--record-hex",
                candidate.hex(),
                "--transfer-count",
                "3",
            )
        )
        installed = self.commit(install_plan)
        self.assertEqual(installed.returncode, 0, installed.stderr)
        self.assert_container_carries_the_derived_checksum(self.save_path, "install")

    def test_the_shipped_tool_preserves_the_stored_checksum(self) -> None:
        """Pin the oracle behaviour the handoff described incorrectly.

        The handoff claimed the shipped tool rewrites the trailing user-checksum
        field when it decrypts, which would make every decrypt-then-fold
        assertion tautological. Craft a clear save whose stored field is
        deliberately *not* the fold, encode it with the Rust codec, and decode
        the container with the oracle: the field comes back byte for byte, so
        the fold assertion above really is checking our value.

        The final 8 bytes of a user save are a separate matter: the body region
        is 0x900058 bytes, which is not a multiple of the 16-byte block, so the
        last 8 bytes are outside the transformed body in *both* directions and
        in both implementations. That is why they read back as zeros and why a
        byte-exact round-trip fixture keeps them zero.
        """

        import struct

        from emaki_exchange import (
            USER_CHECKSUM_BODY_END,
            USER_CHECKSUM_BODY_START,
            USER_CHECKSUM_SEED_OFFSET,
            USER_CHECKSUM_VALUE_OFFSET,
            compute_user_checksum,
        )

        crafted = (0x0BADF00D).to_bytes(4, "little")
        blob = bytearray(self.blob)
        blob[USER_CHECKSUM_VALUE_OFFSET : USER_CHECKSUM_VALUE_OFFSET + 4] = crafted
        # The last 8 bytes sit outside the transformed body; make them non-zero
        # to show that no implementation carries them through.
        blob[USER_SAVE_SIZE - 8 : USER_SAVE_SIZE] = bytes(range(0xD0, 0xD8))
        seed = struct.unpack_from("<I", blob, USER_CHECKSUM_SEED_OFFSET)[0]
        fold = compute_user_checksum(
            bytes(blob[USER_CHECKSUM_BODY_START:USER_CHECKSUM_BODY_END]), seed
        )
        self.assertNotEqual(
            fold, 0x0BADF00D, "the crafted value must not accidentally be the fold"
        )

        clear = self.root / "checksum-clear.bin"
        clear.write_bytes(bytes(blob))
        container = self.encode_with_rust_codec(clear, "checksum")

        byte_preserving = self.decode_with_rust_codec(container, "checksum-rust")
        self.assertEqual(
            byte_preserving[USER_CHECKSUM_VALUE_OFFSET : USER_CHECKSUM_VALUE_OFFSET + 4],
            crafted,
            "the Rust codec must return the field it encrypted",
        )
        self.assertEqual(
            byte_preserving[USER_SAVE_SIZE - 8 : USER_SAVE_SIZE],
            bytes(8),
            "the last 8 bytes are outside the transformed body",
        )

        oracle = self.decode_with_the_shipped_tool(container, "checksum-oracle")
        self.assertEqual(
            oracle[USER_CHECKSUM_VALUE_OFFSET : USER_CHECKSUM_VALUE_OFFSET + 4],
            crafted,
            "the shipped tool must keep the stored user checksum, not rewrite it",
        )
        self.assertEqual(
            oracle[USER_CHECKSUM_SEED_OFFSET : USER_CHECKSUM_SEED_OFFSET + 4],
            bytes(blob[USER_CHECKSUM_SEED_OFFSET : USER_CHECKSUM_SEED_OFFSET + 4]),
            "the shipped tool must keep the checksum seed slot",
        )
        self.assertEqual(
            oracle[USER_CHECKSUM_BODY_START:USER_CHECKSUM_BODY_END],
            byte_preserving[USER_CHECKSUM_BODY_START:USER_CHECKSUM_BODY_END],
            "the oracle and the Rust codec must agree over the checksummed body",
        )

    def test_an_external_writer_inside_the_quiescence_window_is_refused(self) -> None:
        """The shipped 0.20 s multi-file window is real and it catches a writer.

        The probe schedules its writer half a window after a measured fingerprint
        pass, so the write lands inside the window instead of racing it. Three
        cases, all on `BACKUP.BIN` (a related file, never the write target):

        - an explicit 1.2 s window catches the writer, and the guard takes at
          least that window to return;
        - the default window with no writer passes, and still waits at least the
          shipped 0.20 s;
        - a write that lands before the guard starts is not a false positive.
        """

        window_state = self.root / "window-state"
        before = self.save_path.read_bytes()

        def probe(*extra: str) -> list[str]:
            completed = self.host(
                "quiescence-probe",
                "--state-root",
                str(window_state),
                "--save-path",
                str(self.save_path),
                *extra,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            return completed.stdout.strip().split("\t")

        refused = probe("--quiescence-ms", "1200", "--writer-role", "backup")
        self.assertEqual(refused[0], "refused", refused)
        self.assertGreaterEqual(
            int(refused[1]), 1200, "the guard must wait the window it was given"
        )
        self.assertIn("writer_delay_ms=", refused[2])

        (self.save_path.parent / "BACKUP.BIN").write_bytes(b"game-backup")
        guarded = probe()
        self.assertEqual(guarded[0], "guarded", guarded)
        self.assertGreaterEqual(
            int(guarded[1]), 180, "the default window is SAVE_QUIESCENCE_SECONDS"
        )

        pre_written = probe("--writer-role", "backup", "--writer-delay-ms", "0")
        self.assertEqual(
            pre_written[0], "guarded", "a write before the guard starts is not drift"
        )

        self.assertEqual(
            self.save_path.read_bytes(),
            before,
            "a refused quiescence guard must not write the main save",
        )
        self.assertFalse(
            window_state.exists(),
            "a refusal during preparation must not create state or a receipt",
        )


class SaveRestoreRedTests(_ProductFixture):
    """Restore-specific counterexamples from the verified Pro review.

    A is the selected old backup, B is the current target at prepare time, and
    C is an external writer's generation.  Every role is distinct before the
    host runs, so a no-op restore cannot satisfy the byte assertions.
    """

    backup_id = "20260920-restore-source-a"

    def prepare_restore_result(
        self, backup_id: str | None = None
    ) -> subprocess.CompletedProcess:
        return run_restore_harness(
            self.target,
            [
                "prepare",
                "--state-root",
                str(self.state_root),
                "--save-path",
                str(self.save_path),
                "--backup-id",
                backup_id or self.backup_id,
            ],
        )

    def prepare_restore(self, backup_id: str | None = None) -> str:
        completed = self.prepare_restore_result(backup_id)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        return completed.stdout.strip().splitlines()[0]

    def commit_restore(
        self, plan_id: str, point: str | None = None
    ) -> subprocess.CompletedProcess:
        arguments = [
            "commit",
            "--state-root",
            str(self.state_root),
            "--save-path",
            str(self.save_path),
            "--plan-id",
            plan_id,
        ]
        if point is not None:
            arguments.extend(("--point", point))
        return run_restore_harness(self.target, arguments)

    def commit_restore_by_id(self, plan_id: str) -> subprocess.CompletedProcess:
        return run_restore_harness(
            self.target,
            [
                "commit-by-id",
                "--state-root",
                str(self.state_root),
                "--save-path",
                str(self.save_path),
                "--plan-id",
                plan_id,
            ],
        )

    def reset_restore_case(self) -> Path:
        self.clear_state()
        write_generation(self.save_path, self.restore_b)
        assert_generations_distinct(self.restore_a, self.restore_b, self.restore_c)
        self.assert_generation(self.restore_b, "precondition: target must be generation B")
        return write_backup_bundle(
            self.state_root,
            self.backup_id,
            self.restore_a,
            account_id=int(self.account),
        )

    def assert_generation(self, expected: dict[str, bytes], message: str) -> None:
        actual = read_generation(self.save_path)
        mismatched = [role for role in expected if actual[role] != expected[role]]
        self.assertFalse(
            mismatched,
            f"{message}; mismatched_roles={mismatched}; "
            f"actual={generation_digests(actual)}; expected={generation_digests(expected)}",
        )

    def assert_refused_without_target_change(self, completed: subprocess.CompletedProcess) -> None:
        self.assertNotEqual(completed.returncode, 0, completed.stdout + completed.stderr)
        self.assert_generation(
            self.restore_b, "a refused restore must leave all three B roles unchanged"
        )

    def assert_invalid_source_refused(self) -> None:
        prepared = self.prepare_restore_result()
        if prepared.returncode == 0:
            plan_id = prepared.stdout.strip().splitlines()[0]
            refused = self.commit_restore(plan_id)
        else:
            refused = prepared
        self.assert_refused_without_target_change(refused)

    def restore_journal(self) -> dict[str, object]:
        journals = list((self.state_root / "backups").glob("*/restore-journal.json"))
        self.assertEqual(len(journals), 1, journals)
        return json.loads(journals[0].read_text(encoding="utf-8"))

    def assert_restore_ledger(
        self,
        plan_id: str,
        *,
        journal_state: str,
        receipt_outcome: str | None,
        replacement_started: bool,
        final_state: str,
    ) -> None:
        journal = self.restore_journal()
        self.assertEqual(journal["operation_id"], plan_id)
        self.assertEqual(journal["state"], journal_state)
        self.assertEqual(
            set(journal["targets"]), {"main_save", "game_backup", "system_save"}
        )
        role_results = journal["role_results"]
        self.assertEqual(len(role_results), 3)
        self.assertEqual(
            {entry["role"] for entry in role_results},
            {"main_save", "game_backup", "system_save"},
        )
        self.assertTrue(
            all(entry["replacement_started"] is replacement_started for entry in role_results)
        )
        self.assertTrue(all(entry["final_state"] == final_state for entry in role_results))

        receipt_path = self.state_root / "v2-operations" / f"{plan_id}.json"
        if receipt_outcome is None:
            self.assertFalse(receipt_path.exists())
        else:
            self.assertTrue(receipt_path.is_file())
            receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
            self.assertEqual(receipt["outcome"], receipt_outcome)

    def raw_receipt(self, plan_id: str) -> dict:
        path = self.state_root / "v2-operations" / f"{plan_id}.json"
        self.assertTrue(path.is_file(), "the intent receipt must survive on disk")
        return json.loads(path.read_text(encoding="utf-8"))

    def receipt_exists(self, plan_id: str) -> bool:
        return (self.state_root / "v2-operations" / f"{plan_id}.json").is_file()

    def commit_restore_crash(
        self, plan_id: str, cut: str, point: str | None = None
    ) -> subprocess.CompletedProcess:
        """Run a commit that exits the child inside the restore loop.

        `cut` is `<before-replace|after-replace>:<role>`; the process terminates
        after the durable per-role marker and before any recovery path, so the
        parent observes a real crash rather than an in-process rollback.
        """

        arguments = [
            "commit",
            "--state-root",
            str(self.state_root),
            "--save-path",
            str(self.save_path),
            "--plan-id",
            plan_id,
            "--crash-cut",
            cut,
        ]
        if point is not None:
            arguments.extend(("--point", point))
        return run_restore_harness(self.target, arguments)

    def commit_restore_fault_crash(self, plan_id: str, point: str) -> subprocess.CompletedProcess:
        """Run a commit that exits after the injected stage fault returns."""

        return run_restore_harness(
            self.target,
            [
                "commit",
                "--state-root",
                str(self.state_root),
                "--save-path",
                str(self.save_path),
                "--plan-id",
                plan_id,
                "--point",
                point,
                "--crash",
            ],
        )

    def classify_restore(self, plan_id: str) -> dict:
        completed = run_restore_harness(
            self.target,
            [
                "classify",
                "--state-root",
                str(self.state_root),
                "--save-path",
                str(self.save_path),
                "--plan-id",
                plan_id,
            ],
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        parsed: dict = {}
        for line in completed.stdout.strip().splitlines():
            key, _, value = line.partition("=")
            parsed[key] = value
        return parsed

    RESTORE_ROLE_CUTS = (
        "before-replace:system_save",
        "after-replace:system_save",
        "before-replace:game_backup",
        "after-replace:game_backup",
        "before-replace:main_save",
        "after-replace:main_save",
    )

    RESTORE_JOURNAL_CUTS = (
        "journal-create",
        "journal-write",
        "journal-flush",
        "journal-replace",
    )

    def assert_crash_cut_exits(self, completed: subprocess.CompletedProcess, cut: str) -> None:
        self.assertEqual(
            completed.returncode,
            9,
            f"{cut}: the cut must terminate the child inside the restore loop, "
            f"before any rollback; {completed.stdout}{completed.stderr}",
        )
        self.assertIn("deterministic crash cut", completed.stderr, completed.stderr)

    def test_restore_role_cut_leaves_durable_per_role_progress(self) -> None:
        """Every role cut is a real crash-cut, never an in-process rollback."""

        # `prepare` writes a plan, not a receipt; the durable intent appears only
        # once the commit starts, so there is nothing to assert before the cut.
        self.reset_restore_case()
        plan_id = self.prepare_restore()
        self.assertFalse(
            self.receipt_exists(plan_id),
            "preparing a plan must not create a terminal record",
        )
        for cut in self.RESTORE_ROLE_CUTS:
            with self.subTest(cut=cut):
                self.reset_restore_case()
                plan_id = self.prepare_restore()
                completed = self.commit_restore_crash(plan_id, cut)
                self.assert_crash_cut_exits(completed, cut)
                journal = self.restore_journal()
                self.assertEqual(
                    journal["state"],
                    "roles_in_progress",
                    f"{cut}: a crash-cut must leave a per-role in-progress journal",
                )
                self.assertEqual(journal["operation_id"], plan_id)
                # The intent receipt is still the untouched pending record;
                # reconcile reports it as a non-replayable unknown.
                self.assertEqual(self.raw_receipt(plan_id)["outcome"], "pending")
                classified = self.classify_restore(plan_id)
                self.assertEqual(classified["receipt_outcome"], "unknown")
                # Every role keeps its source digest and the pre-replace digest
                # from A/B, so the journal alone can place each role after restart.
                role_results = journal["role_results"]
                self.assertTrue(all(entry["source_sha256"] for entry in role_results))
                self.assertTrue(all(entry["target_before_sha256"] for entry in role_results))
                self.assertEqual(
                    set(journal["targets"]),
                    {"main_save", "game_backup", "system_save"},
                )

    def test_restore_journal_crash_never_truncates_the_last_valid_progress(self) -> None:
        """A staged journal update leaves the prior valid marker authoritative."""

        for cut in self.RESTORE_JOURNAL_CUTS:
            with self.subTest(cut=cut):
                self.reset_restore_case()
                plan_id = self.prepare_restore()
                completed = self.commit_restore_crash(plan_id, cut)
                self.assert_crash_cut_exits(completed, cut)

                journal = self.restore_journal()
                self.assertEqual(journal["operation_id"], plan_id)
                self.assertEqual(journal["state"], "roles_in_progress")
                system = next(
                    entry
                    for entry in journal["role_results"]
                    if entry["role"] == "system_save"
                )
                self.assertIs(system["replacement_started"], True)
                self.assertIs(system["replacement_completed"], False)
                classified = self.classify_restore(plan_id)
                self.assertEqual(classified["system_save.class"], "A_source")
                self.assertEqual(classified["game_backup.class"], "B_checkpoint")
                self.assertEqual(classified["main_save.class"], "B_checkpoint")
                self.assertEqual(self.raw_receipt(plan_id)["outcome"], "pending")

    def test_restore_role_cut_classifies_each_role_from_journal_and_bytes(self) -> None:
        """A restart classifies untouched/A/B/external-C for every cut point.

        The class is decided by the bytes on disk, so the cut role is
        `B_checkpoint` before its rename and `A_source` after it, even though its
        completion record never landed. The journal carries the other half of
        the fact: `replacement_started=true, replacement_completed=false` in both
        phases. Asserting the bytes here keeps the oracle from being satisfied by
        the journal's own labels.
        """

        expected_role_order = ("system_save", "game_backup", "main_save")
        for index, role in enumerate(expected_role_order):
            replaced = set(expected_role_order[:index])
            for phase in ("before-replace", "after-replace"):
                cut = f"{phase}:{role}"
                with self.subTest(cut=cut):
                    self.reset_restore_case()
                    plan_id = self.prepare_restore()
                    completed = self.commit_restore_crash(plan_id, cut)
                    self.assert_crash_cut_exits(completed, cut)
                    classified = self.classify_restore(plan_id)
                    self.assertEqual(classified["journal_state"], "roles_in_progress")
                    # Independent evidence: the realized bytes of the same target.
                    observed = read_generation(self.save_path)
                    for other in expected_role_order:
                        if other in replaced:
                            self.assertEqual(
                                observed[other],
                                self.restore_a[other],
                                f"{cut}: {other} must hold generation A",
                            )
                            self.assertEqual(
                                classified[f"{other}.class"],
                                "A_source",
                                f"{cut}: {other} was replaced before the cut; {classified}",
                            )
                            self.assertEqual(classified[f"{other}.replacement_started"], "true")
                            self.assertEqual(classified[f"{other}.replacement_completed"], "true")
                        elif other == role:
                            # The cut role sits between its durable dispatch
                            # marker and its completion record: the rename landed
                            # in the after phase only, and the journal says so.
                            landed = phase == "after-replace"
                            owner = self.restore_a if landed else self.restore_b
                            self.assertEqual(
                                observed[other],
                                owner[other],
                                f"{cut}: {other} must still be generation "
                                f"{'A' if landed else 'B'}",
                            )
                            self.assertEqual(
                                classified[f"{other}.class"],
                                "A_source" if landed else "B_checkpoint",
                                f"{cut}: {other} is classified by its bytes; {classified}",
                            )
                            self.assertEqual(classified[f"{other}.replacement_started"], "true")
                            self.assertEqual(classified[f"{other}.replacement_completed"], "false")
                        else:
                            self.assertEqual(
                                observed[other],
                                self.restore_b[other],
                                f"{cut}: {other} was never dispatched and must hold B",
                            )
                            self.assertEqual(
                                classified[f"{other}.class"],
                                "B_checkpoint",
                                f"{cut}: {other} was not replaced before the cut; {classified}",
                            )
                            self.assertEqual(
                                classified[f"{other}.replacement_started"], "false"
                            )
                            self.assertEqual(
                                classified[f"{other}.replacement_completed"], "false"
                            )

    def test_restore_restart_refuses_to_treat_a_mixed_target_as_untouched(self) -> None:
        """A genuinely mixed A/B/C target must be classified per role."""

        self.reset_restore_case()
        plan_id = self.prepare_restore()
        # Cut after the first role's rename landed but before its completion
        # record, so System is source A while GameBackup and Main stay at B.
        completed = self.commit_restore_crash(plan_id, "after-replace:system_save")
        self.assert_crash_cut_exits(completed, "after-replace:system_save")
        # Move only the main role to the external generation C. Writing one role
        # keeps this a real A/B/C mix instead of an all-C target.
        self.save_path.write_bytes(self.restore_c["main_save"])
        observed = read_generation(self.save_path)
        self.assertEqual(observed["system_save"], self.restore_a["system_save"])
        self.assertEqual(observed["game_backup"], self.restore_b["game_backup"])
        self.assertEqual(observed["main_save"], self.restore_c["main_save"])
        classified = self.classify_restore(plan_id)
        self.assertEqual(classified["system_save.class"], "A_source", classified)
        self.assertEqual(classified["game_backup.class"], "B_checkpoint", classified)
        self.assertEqual(classified["main_save.class"], "external_C", classified)
        journal = self.restore_journal()
        self.assertEqual(journal["state"], "roles_in_progress")
        self.assertEqual(journal["operation_id"], plan_id)
        # The journal still names main as untouched: its replacement was never
        # dispatched, so nothing recorded that an external writer moved it. The
        # restart must therefore classify by bytes instead of trusting that
        # label, which is exactly what makes an unrecorded role not `untouched`.
        recorded_states = {entry["role"]: entry["final_state"] for entry in journal["role_results"]}
        self.assertEqual(recorded_states["main_save"], "untouched")
        self.assertEqual(recorded_states["system_save"], "replacement_started")
        # Nothing may claim the mixed target is committed.
        self.assertEqual(classified["receipt_outcome"], "unknown")

    def test_restore_success_changes_every_role_from_b_to_a(self) -> None:
        self.reset_restore_case()
        plan_id = self.prepare_restore()
        completed = self.commit_restore(plan_id)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assert_generation(
            self.restore_a, "restore must install the authenticated A bundle for every role"
        )

        self.assert_restore_ledger(
            plan_id,
            journal_state="committed",
            receipt_outcome="committed",
            replacement_started=True,
            final_state="source_installed",
        )

    def digest_of(self, path: Path) -> str:
        return hashlib.sha256(path.read_bytes()).hexdigest()

    def unrelated_save(self) -> Path:
        """A second account inside the same fixture root, for the fence's scope."""

        path = self.root / "76561198000000001" / "SAVEDATA00" / "SAVEDATA.BIN"
        path.parent.mkdir(parents=True, exist_ok=True)
        (path.parent / "BACKUP.BIN").write_bytes(b"game-backup")
        system = path.parent.parent / "SYSTEMSAVEDATA00"
        system.mkdir(exist_ok=True)
        (system / "SAVEDATA.BIN").write_bytes(b"system-save")
        path.write_bytes(self.container.read_bytes())
        return path

    def test_unresolved_operation_fences_a_new_plan_on_the_same_target(self) -> None:
        """RW04 / RF02: a new plan is refused while the same save is unresolved.

        The crash cut leaves a real mixed generation (System at A, GameBackup and
        Main at B) plus a `pending` receipt and a per-role journal. A new plan for
        the same save must be refused, and the refusal must name the unresolved
        operation and the per-role classification the bytes actually show. An
        unrelated save may still be planned, an external C is named instead of
        being overwritten, and the same operation id never writes again.
        """

        self.reset_restore_case()
        plan_id = self.prepare_restore()
        completed = self.commit_restore_crash(plan_id, "after-replace:system_save")
        self.assert_crash_cut_exits(completed, "after-replace:system_save")
        # Keep the mixed target: System is A, GameBackup and Main are B.
        observed = read_generation(self.save_path)
        self.assertEqual(observed["system_save"], self.restore_a["system_save"])
        self.assertEqual(observed["game_backup"], self.restore_b["game_backup"])
        self.assertEqual(observed["main_save"], self.restore_b["main_save"])
        self.assertEqual(self.raw_receipt(plan_id)["outcome"], "pending")
        mixed_main = self.digest_of(self.save_path)

        refusal = self.host(
            "plan-edit",
            "--state-root",
            str(self.state_root),
            "--save-path",
            str(self.save_path),
            "--source-sha256",
            mixed_main,
            "--spec-file",
            str(self.edit_spec(1)),
        )
        self.assertNotEqual(refusal.returncode, 0, refusal.stdout + refusal.stderr)
        self.assertIn("UNRESOLVED_OPERATION", refusal.stderr, refusal.stderr)
        self.assertIn(plan_id, refusal.stderr, refusal.stderr)
        self.assertIn("system_save=A_source", refusal.stderr, refusal.stderr)
        self.assertIn("game_backup=B_checkpoint", refusal.stderr, refusal.stderr)
        self.assertIn("main_save=B_checkpoint", refusal.stderr, refusal.stderr)
        stored_plans = sorted(
            path.name for path in (self.state_root / "v2-plans").glob("*.json")
        )
        self.assertEqual(
            stored_plans,
            [f"{plan_id}.json"],
            "a refused plan must not be stored",
        )
        self.assertEqual(self.digest_of(self.save_path), mixed_main)

        # An unrelated save (a different account) is outside the fence.
        unrelated = self.unrelated_save()
        allowed = self.host(
            "plan-edit",
            "--state-root",
            str(self.state_root),
            "--save-path",
            str(unrelated),
            "--source-sha256",
            self.digest_of(unrelated),
            "--spec-file",
            str(self.edit_spec(1)),
        )
        self.assertEqual(allowed.returncode, 0, allowed.stderr)

        # An external C on the fenced target is reported, never overwritten.
        external = bytearray(self.restore_b["main_save"])
        external[0x21] ^= 0x5A
        self.save_path.write_bytes(bytes(external))
        external_digest = self.digest_of(self.save_path)
        blocked = self.host(
            "plan-edit",
            "--state-root",
            str(self.state_root),
            "--save-path",
            str(self.save_path),
            "--source-sha256",
            external_digest,
            "--spec-file",
            str(self.edit_spec(1)),
        )
        self.assertNotEqual(blocked.returncode, 0, blocked.stdout + blocked.stderr)
        self.assertIn("main_save=external_C", blocked.stderr, blocked.stderr)
        self.assertIn("system_save=A_source", blocked.stderr, blocked.stderr)
        self.assertEqual(self.digest_of(self.save_path), external_digest)

        # The same operation id is never replayed by a retry.
        replay = self.commit(plan_id)
        self.assertNotEqual(replay.returncode, 0, replay.stdout + replay.stderr)
        self.assertIn("already-committed operation", replay.stderr, replay.stderr)
        self.assertEqual(self.digest_of(self.save_path), external_digest)

    def test_prepared_plan_is_rechecked_at_direct_and_by_id_commit_boundaries(self) -> None:
        """A Q prepared before P becomes pending cannot bypass the final fence."""

        for commit_mode in ("direct", "by-id"):
            with self.subTest(commit_mode=commit_mode):
                self.reset_restore_case()
                plan_p = self.prepare_restore()
                plan_q = self.prepare_restore()
                crashed = self.commit_restore_crash(
                    plan_p, "before-replace:system_save"
                )
                self.assert_crash_cut_exits(crashed, "before-replace:system_save")
                before = read_generation(self.save_path)
                backups_before = sorted(
                    path.name for path in (self.state_root / "backups").iterdir()
                )

                refused = (
                    self.commit_restore(plan_q)
                    if commit_mode == "direct"
                    else self.commit_restore_by_id(plan_q)
                )
                self.assertNotEqual(refused.returncode, 0, refused.stdout + refused.stderr)
                self.assertIn("UNRESOLVED_OPERATION", refused.stderr, refused.stderr)
                self.assertIn(plan_p, refused.stderr, refused.stderr)
                self.assertEqual(read_generation(self.save_path), before)
                self.assertEqual(
                    sorted(path.name for path in (self.state_root / "backups").iterdir()),
                    backups_before,
                    "a final-boundary refusal must precede Q's checkpoint side effect",
                )

    def test_corrupt_authoritative_receipt_fails_closed(self) -> None:
        self.reset_restore_case()
        plan_id = self.prepare_restore()
        crashed = self.commit_restore_crash(plan_id, "before-replace:system_save")
        self.assert_crash_cut_exits(crashed, "before-replace:system_save")
        receipt_path = self.state_root / "v2-operations" / f"{plan_id}.json"
        receipt_path.write_text("{not-json", encoding="utf-8")
        before = read_generation(self.save_path)
        plan_count = len(list((self.state_root / "v2-plans").glob("*.json")))

        refused = self.host(
            "plan-edit",
            "--state-root",
            str(self.state_root),
            "--save-path",
            str(self.save_path),
            "--source-sha256",
            self.digest_of(self.save_path),
            "--spec-file",
            str(self.edit_spec(1)),
        )
        self.assertNotEqual(refused.returncode, 0, refused.stdout + refused.stderr)
        self.assertIn(plan_id, refused.stderr, refused.stderr)
        self.assertEqual(read_generation(self.save_path), before)
        self.assertEqual(
            len(list((self.state_root / "v2-plans").glob("*.json"))),
            plan_count,
        )

    def test_restore_fallback_keeps_source_a_distinct_from_checkpoint_b(self) -> None:
        self.reset_restore_case()
        plan_id = self.prepare_restore()
        crashed = self.commit_restore_crash(plan_id, "before-replace:system_save")
        self.assert_crash_cut_exits(crashed, "before-replace:system_save")
        journal_path = next((self.state_root / "backups").glob("*/restore-journal.json"))
        journal_path.unlink()

        refused = self.host(
            "plan-edit",
            "--state-root",
            str(self.state_root),
            "--save-path",
            str(self.save_path),
            "--source-sha256",
            self.digest_of(self.save_path),
            "--spec-file",
            str(self.edit_spec(1)),
        )
        self.assertNotEqual(refused.returncode, 0, refused.stdout + refused.stderr)
        self.assertIn("main_save=B_checkpoint", refused.stderr, refused.stderr)
        self.assertIn("game_backup=B_checkpoint", refused.stderr, refused.stderr)
        self.assertIn("system_save=B_checkpoint", refused.stderr, refused.stderr)

    def test_shared_system_conflicts_across_restore_slots_but_main_only_does_not(self) -> None:
        """Conflict is the actual role write set, not merely account plus slot."""

        self.reset_restore_case()
        second_save = self.root / self.account / "SAVEDATA01" / "SAVEDATA.BIN"
        write_generation(second_save, self.restore_b)
        second_backup_id = f"{self.backup_id}-slot1"
        write_backup_bundle(
            self.state_root,
            second_backup_id,
            self.restore_a,
            account_id=int(self.account),
            save_slot_index=1,
        )
        plan_p = self.prepare_restore()
        second_prepared = run_restore_harness(
            self.target,
            [
                "prepare",
                "--state-root",
                str(self.state_root),
                "--save-path",
                str(second_save),
                "--backup-id",
                second_backup_id,
            ],
        )
        self.assertEqual(second_prepared.returncode, 0, second_prepared.stderr)
        plan_q = second_prepared.stdout.strip().splitlines()[0]
        crashed = self.commit_restore_crash(plan_p, "before-replace:system_save")
        self.assert_crash_cut_exits(crashed, "before-replace:system_save")

        restore_refusal = run_restore_harness(
            self.target,
            [
                "commit-by-id",
                "--state-root",
                str(self.state_root),
                "--save-path",
                str(second_save),
                "--plan-id",
                plan_q,
            ],
        )
        self.assertNotEqual(
            restore_refusal.returncode,
            0,
            restore_refusal.stdout + restore_refusal.stderr,
        )
        self.assertIn("UNRESOLVED_OPERATION", restore_refusal.stderr)

        main_only = self.host(
            "plan-edit",
            "--state-root",
            str(self.state_root),
            "--save-path",
            str(second_save),
            "--source-sha256",
            self.digest_of(second_save),
            "--spec-file",
            str(self.edit_spec(1)),
        )
        self.assertEqual(main_only.returncode, 0, main_only.stderr)

    def assert_restore_fault_rolls_back_to_b(self, point: str) -> None:
        self.reset_restore_case()
        plan_id = self.prepare_restore()
        completed = self.commit_restore(plan_id, point)
        self.assertNotEqual(completed.returncode, 0, completed.stdout + completed.stderr)
        self.assert_generation(
            self.restore_b,
            f"{point}: rollback must use the pre-restore B checkpoint",
        )
        replacement_started = point in {"after-replace", "after-readback"}
        self.assert_restore_ledger(
            plan_id,
            journal_state="rolled_back",
            receipt_outcome=None if point == "after-checkpoint" else "not_committed",
            replacement_started=replacement_started,
            final_state="checkpoint_restored" if replacement_started else "untouched",
        )

    def test_restore_fault_after_checkpoint_rolls_back_to_b(self) -> None:
        self.assert_restore_fault_rolls_back_to_b("after-checkpoint")

    def test_restore_fault_after_receipt_rolls_back_to_b(self) -> None:
        self.assert_restore_fault_rolls_back_to_b("after-receipt")

    def test_restore_fault_after_stage_rolls_back_to_b(self) -> None:
        self.assert_restore_fault_rolls_back_to_b("after-stage")

    def test_restore_fault_after_replace_rolls_back_to_b(self) -> None:
        self.assert_restore_fault_rolls_back_to_b("after-replace")

    def test_restore_fault_after_readback_rolls_back_to_b(self) -> None:
        self.assert_restore_fault_rolls_back_to_b("after-readback")

    def test_restore_refuses_external_c_without_clobbering_it(self) -> None:
        self.reset_restore_case()
        plan_id = self.prepare_restore()
        write_generation(self.save_path, self.restore_c)
        completed = self.commit_restore(plan_id)
        self.assertNotEqual(completed.returncode, 0, completed.stdout + completed.stderr)
        self.assert_generation(
            self.restore_c, "a generation C written after prepare must remain untouched"
        )

    def test_restore_rejects_valid_manifest_with_corrupt_content(self) -> None:
        bundle = self.reset_restore_case()
        source = bundle / "SAVEDATA.BIN"
        corrupt = bytearray(source.read_bytes())
        corrupt[len(corrupt) // 2] ^= 0x5A
        source.write_bytes(bytes(corrupt))
        self.assert_invalid_source_refused()

    def test_restore_rejects_source_swapped_after_prepare(self) -> None:
        bundle = self.reset_restore_case()
        plan_id = self.prepare_restore()
        for role, name in (
            ("main_save", "SAVEDATA.BIN"),
            ("game_backup", "BACKUP.BIN"),
            ("system_save", "SYSTEMSAVEDATA.BIN"),
        ):
            (bundle / name).write_bytes(self.restore_c[role])
        self.assert_refused_without_target_change(self.commit_restore(plan_id))

    def assert_restore_rejects_wrong_manifest_field(self, field: str) -> None:
        bundle = self.reset_restore_case()
        manifest_path = bundle / "backup-manifest.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        main = next(
            entry
            for entry in manifest["backup_files"]
            if entry["source_role"] == "main_save"
        )
        main[field] = main[field] + 1 if field == "size" else "0" * 64
        manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
        self.assert_invalid_source_refused()

    def test_restore_rejects_wrong_manifest_size(self) -> None:
        self.assert_restore_rejects_wrong_manifest_field("size")

    def test_restore_rejects_wrong_manifest_hash(self) -> None:
        self.assert_restore_rejects_wrong_manifest_field("sha256")

    def assert_restore_rejects_role_shape(self, shape: str) -> None:
        self.clear_state()
        write_generation(self.save_path, self.restore_b)
        options: dict[str, object]
        if shape == "missing":
            options = {"omitted_roles": {"game_backup"}}
        else:
            options = {"file_overrides": {"game_backup": "SAVEDATA.BIN"}}
        write_backup_bundle(
            self.state_root,
            self.backup_id,
            self.restore_a,
            account_id=int(self.account),
            **options,
        )
        self.assert_invalid_source_refused()

    def test_restore_rejects_missing_role(self) -> None:
        self.assert_restore_rejects_role_shape("missing")

    def test_restore_rejects_aliased_role(self) -> None:
        self.assert_restore_rejects_role_shape("aliased")


class SaveFaultGateTests(_ProductFixture):
    """Injected I/O faults and process death at every commit stage."""

    RECEIPT_STAGES = (
        "receipt-create",
        "receipt-write",
        "receipt-flush",
        "receipt-replace",
    )

    def reference_edit_record(self, slot_index: int) -> tuple[int, int, bytes]:
        """The record the shipped Python composition installs for `edit_spec`."""

        from nioh3_scroll_editor.savegame import (
            SCROLL_GROUP_OFFSET,
            SCROLL_RECORD_SIZE,
            LocalEffectEdit,
            patch_local_scroll_header,
            patch_local_scroll_record,
        )

        offset = SCROLL_GROUP_OFFSET + slot_index * SCROLL_RECORD_SIZE
        current = self.blob[offset : offset + SCROLL_RECORD_SIZE]
        expected = patch_local_scroll_header(
            current,
            playthrough=3,
            level=185,
            recommended_level=195,
            seed=0x0BADF00D,
            rarity=5,
            transfer_count=2,
        )
        expected = patch_local_scroll_record(expected, [LocalEffectEdit(slot_index=0, value=4242)])
        return offset, SCROLL_RECORD_SIZE, expected

    def test_injected_fault_at_each_stage_recovers_exactly(self) -> None:
        # `after-checkpoint` fires after the checkpoint exists but before the
        # durable receipt is written, so it is the one stage with no receipt.
        expected = {
            "after-checkpoint": None,
            # The durable receipt is written before the staged write exists, so
            # a fault before the replacement leaves it in its pending state.
            "after-receipt": "pending",
            "after-stage": "pending",
            "after-replace": "uncertain",
            "after-readback": "uncertain",
        }
        for point, outcome in expected.items():
            with self.subTest(point=point):
                plan_id = self.plan_edit(1)
                completed = self.host(
                    "fault",
                    "--state-root",
                    str(self.state_root),
                    "--save-path",
                    str(self.save_path),
                    "--plan-id",
                    plan_id,
                    "--point",
                    point,
                )
                self.assertNotEqual(completed.returncode, 0, completed.stderr)
                receipt_path = self.state_root / "v2-operations" / f"{plan_id}.json"
                if outcome is None:
                    self.assertFalse(
                        receipt_path.is_file(),
                        f"{point} precedes the durable receipt, so none may exist",
                    )
                else:
                    self.assertEqual(
                        self.receipt(plan_id)["outcome"],
                        outcome,
                        f"{point} must record its realized outcome",
                    )
                self.assertEqual(
                    self.save_path.read_bytes(),
                    self.container.read_bytes(),
                    f"{point} must leave the quiet generation in place",
                )
                self.clear_state()

    def test_injected_fault_never_clobbers_an_external_change(self) -> None:
        plan_id = self.plan_edit(1)
        # A second process replaces the main save between planning and commit.
        external = bytearray(self.blob)
        external[0x176CCE + 0x21] ^= 0x5A
        external_container = self.write_container(bytes(external), "external")
        self.save_path.write_bytes(external_container.read_bytes())
        completed = self.host(
            "fault",
            "--state-root",
            str(self.state_root),
            "--save-path",
            str(self.save_path),
            "--plan-id",
            plan_id,
            "--point",
            "after-replace",
        )
        self.assertNotEqual(completed.returncode, 0)
        self.assertEqual(
            self.save_path.read_bytes(),
            external_container.read_bytes(),
            "an externally changed save must never be overwritten by a rollback",
        )

    def test_rollback_refusal_records_uncertain_after_external_change(self) -> None:
        # Arm the post-replace fault, then let a second writer win the race by
        # replacing the main save before the rollback can run.
        plan_id = self.plan_edit(1)
        completed = self.host(
            "fault",
            "--state-root",
            str(self.state_root),
            "--save-path",
            str(self.save_path),
            "--plan-id",
            plan_id,
            "--point",
            "after-replace",
        )
        self.assertNotEqual(completed.returncode, 0)
        receipt = self.receipt(plan_id)
        self.assertEqual(receipt["outcome"], "uncertain")
        self.assertEqual(self.save_path.read_bytes(), self.container.read_bytes())

    def test_process_death_at_each_stage_is_recoverable(self) -> None:
        points = (
            "after-checkpoint",
            "after-receipt",
            "after-stage",
            "after-replace",
            "after-readback",
        )
        no_receipt_yet = {"after-checkpoint"}
        for point in points:
            with self.subTest(point=point):
                plan_id = self.plan_edit(1)
                completed = self.host(
                    "fault",
                    "--state-root",
                    str(self.state_root),
                    "--save-path",
                    str(self.save_path),
                    "--plan-id",
                    plan_id,
                    "--point",
                    point,
                    "--crash",
                )
                if completed.returncode == 9:
                    receipt_path = self.state_root / "v2-operations" / f"{plan_id}.json"
                    if point in no_receipt_yet:
                        # The crash landed before the durable receipt; nothing was
                        # mutated, so there is nothing to reconcile.
                        self.assertFalse(receipt_path.is_file())
                    else:
                        # A real process death: the durable receipt must still be
                        # on disk, and reconciling it must not replay anything.
                        self.assertTrue(
                            receipt_path.is_file(),
                            f"{point}: a durable receipt must precede every mutation",
                        )
                        reconciled = self.host(
                            "reconcile",
                            "--state-root",
                            str(self.state_root),
                            "--save-path",
                            str(self.save_path),
                            "--plan-id",
                            plan_id,
                        )
                        self.assertEqual(reconciled.returncode, 0, reconciled.stderr)
                else:
                    self.assertNotEqual(completed.returncode, 0, completed.stderr)
                # Whatever happened, the main save must still be a valid save.
                realized = self.decrypt_installed(f"recovered-{point}")
                self.assertEqual(realized[:6], b"RNNUSR")
                self.clear_state()

    def test_terminal_receipt_stage_fault_stays_committed_with_a_warning(self) -> None:
        """RW01: the terminal receipt's create/write/flush/replace each fail.

        The target bytes have already landed and been read back, so the result
        must be an explicit completed-with-warning rather than a replayable
        not-committed or an unprovable state: the old intent stays parseable, the
        plan is not replayable, and the same operation id never writes twice.
        """

        offset, width, expected = self.reference_edit_record(1)
        for stage in self.RECEIPT_STAGES:
            with self.subTest(stage=stage):
                self.save_path.write_bytes(self.container.read_bytes())
                self.clear_state()
                plan_id = self.plan_edit(1)
                plan_path = self.state_root / "v2-plans" / f"{plan_id}.json"
                # Keep the plan text so the retry below can restore it and prove
                # the receipt gate, not a missing file, refuses the replay.
                plan_text = plan_path.read_text(encoding="utf-8")
                completed = self.host(
                    "fault",
                    "--state-root",
                    str(self.state_root),
                    "--save-path",
                    str(self.save_path),
                    "--plan-id",
                    plan_id,
                    "--point",
                    stage,
                )
                self.assertNotEqual(completed.returncode, 0, completed.stderr)
                self.assertIn(
                    "committed, but its terminal record could not be persisted",
                    completed.stderr,
                    completed.stderr,
                )
                self.assertNotIn("not_committed", completed.stderr)
                self.assertNotIn("unprovable", completed.stderr)

                # The durable record must never be truncated in place: it parses
                # as this operation and the recovery rewrite lands the terminal
                # outcome over the pending intent it replaced.
                receipt = self.receipt(plan_id)
                self.assertEqual(receipt["operation_id"], plan_id)
                self.assertEqual(receipt["outcome"], "committed")
                self.assertIs(receipt["committed"], True)
                self.assertNotIn(receipt["outcome"], {"not_committed", "uncertain"})
                siblings = sorted(
                    path.name
                    for path in (self.state_root / "v2-operations").iterdir()
                    if path.name != f"{plan_id}.json"
                )
                self.assertEqual(siblings, [], f"{stage}: no staged receipt may survive")

                # Independent target evidence: the bytes are the realized edit,
                # not the pre-commit generation and not a truncated write.
                target = self.save_path.read_bytes()
                self.assertNotEqual(target, self.container.read_bytes())
                self.assertEqual(receipt["installed_sha256"], hashlib.sha256(target).hexdigest())
                realized = self.decrypt_installed(f"receipt-{stage}")
                self.assertEqual(realized[:6], b"RNNUSR")
                self.assertEqual(realized[offset : offset + width], expected)

                # Same operation id, with the consumed plan restored: the receipt
                # gate refuses the replay and the target keeps its committed bytes.
                plan_path.write_text(plan_text, encoding="utf-8")
                committed_digest = hashlib.sha256(target).hexdigest()
                retry = self.commit(plan_id)
                self.assertNotEqual(retry.returncode, 0, retry.stdout + retry.stderr)
                self.assertEqual(
                    hashlib.sha256(self.save_path.read_bytes()).hexdigest(),
                    committed_digest,
                    f"{stage}: a retry must never write the target again",
                )
                reconciled = self.host(
                    "reconcile",
                    "--state-root",
                    str(self.state_root),
                    "--save-path",
                    str(self.save_path),
                    "--plan-id",
                    plan_id,
                )
                self.assertEqual(reconciled.returncode, 0, reconciled.stderr)
                self.assertEqual(reconciled.stdout.strip().splitlines()[0], "committed")


class SaveTamperGateTests(_ProductFixture):
    """Stored plans and receipts are untrusted input."""

    def write_plan(self, plan_id: str, payload: dict) -> None:
        plan_dir = self.state_root / "v2-plans"
        plan_dir.mkdir(parents=True, exist_ok=True)
        (plan_dir / f"{plan_id}.json").write_text(json.dumps(payload), encoding="utf-8")

    def read_plan(self, plan_id: str) -> dict:
        path = self.state_root / "v2-plans" / f"{plan_id}.json"
        return json.loads(path.read_text(encoding="utf-8"))

    def test_write_path_hijack_is_refused(self) -> None:
        plan_id = self.plan_edit(1)
        plan = self.read_plan(plan_id)
        foreign = self.root / "elsewhere" / "SAVEDATA00" / "SAVEDATA.BIN"
        plan["save_path"] = str(foreign)
        self.write_plan(plan_id, plan)
        completed = self.commit(plan_id)
        self.assertNotEqual(completed.returncode, 0, "a hijacked write path must be refused")
        # A structured refusal, not just a nonzero exit: the account directory on
        # the hijacked path is not numeric, so the host must name the target
        # mismatch it detected before touching anything.
        self.assertIn("targets", completed.stderr.lower(), completed.stderr)
        self.assertIn("account", completed.stderr.lower(), completed.stderr)
        self.assertFalse(
            foreign.parent.exists(),
            "a refused plan must not create the foreign directory",
        )
        self.assertEqual(
            self.save_path.read_bytes(),
            self.container.read_bytes(),
            "the real save must be byte-identical after a refused hijack",
        )

    def test_operation_id_shape_is_refused(self) -> None:
        for bad in ("../escape", "ZZZZ", "0" * 31, "0" * 33, "A" * 32):
            with self.subTest(plan_id=bad):
                self.assertNotEqual(self.commit(bad).returncode, 0)

    def test_foreign_account_and_slot_are_refused(self) -> None:
        plan_id = self.plan_edit(1)
        plan = self.read_plan(plan_id)
        plan["account_id"] = "5555666677778888"
        self.write_plan(plan_id, plan)
        refused = self.commit(plan_id)
        self.assertNotEqual(refused.returncode, 0)
        self.assertIn("account", refused.stderr.lower(), refused.stderr)
        self.assertEqual(self.save_path.read_bytes(), self.container.read_bytes())

        plan_id = self.plan_edit(1)
        plan = self.read_plan(plan_id)
        plan["save_slot"] = "7"
        self.write_plan(plan_id, plan)
        refused = self.commit(plan_id)
        self.assertNotEqual(refused.returncode, 0)
        self.assertIn("slot", refused.stderr.lower(), refused.stderr)
        self.assertEqual(self.save_path.read_bytes(), self.container.read_bytes())

    def test_source_hash_hijack_is_refused(self) -> None:
        plan_id = self.plan_edit(1)
        plan = self.read_plan(plan_id)
        plan["source_sha256"] = "f" * 64
        self.write_plan(plan_id, plan)
        refused = self.commit(plan_id)
        self.assertNotEqual(refused.returncode, 0)
        self.assertIn("digest", refused.stderr.lower(), refused.stderr)
        self.assertEqual(self.save_path.read_bytes(), self.container.read_bytes())

    def test_backup_id_escape_is_refused(self) -> None:
        plan_id = self.plan_edit(1)
        plan = self.read_plan(plan_id)
        plan["backup_id"] = "..\\..\\elsewhere"
        self.write_plan(plan_id, plan)
        refused = self.commit(plan_id)
        self.assertNotEqual(refused.returncode, 0)
        self.assertIn("backup", refused.stderr.lower(), refused.stderr)
        self.assertFalse((self.root / "elsewhere").exists())

    def test_record_bytes_hijack_is_refused(self) -> None:
        plan_id = self.plan_edit(1)
        plan = self.read_plan(plan_id)
        edit = plan["product"]["Edit"]["edits"][0]
        # Claim an original record the save does not hold: the commit must refuse
        # rather than blindly install the replacement.
        edit["expected_original"][0] ^= 0xFF
        self.write_plan(plan_id, plan)
        completed = self.commit(plan_id)
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("mismatch", completed.stderr.lower(), completed.stderr)
        self.assertEqual(self.save_path.read_bytes(), self.container.read_bytes())

    def test_an_escaping_plan_id_never_reaches_the_ledger(self) -> None:
        escaped = self.state_root / "outside.json"
        escaped.parent.mkdir(parents=True, exist_ok=True)
        escaped.write_text("{}", encoding="utf-8")
        completed = self.commit("../outside")
        self.assertNotEqual(completed.returncode, 0)

    def test_a_foreign_receipt_is_not_listed(self) -> None:
        receipt_dir = self.state_root / "v2-operations"
        receipt_dir.mkdir(parents=True, exist_ok=True)
        bad_id = "b" * 32
        (receipt_dir / f"{bad_id}.json").write_text(
            json.dumps(
                {
                    "operation_id": bad_id,
                    "kind": "edit",
                    "outcome": "committed",
                    "installed_sha256": "not-a-digest",
                    "backup_id": "..\\escape",
                    "message": None,
                }
            ),
            encoding="utf-8",
        )
        listed = self.host(
            "operations",
            "--state-root",
            str(self.state_root),
            "--save-path",
            str(self.save_path),
        )
        self.assertEqual(listed.returncode, 0, listed.stderr)
        self.assertNotIn(bad_id, listed.stdout, "an invalid receipt must not be listed")


if __name__ == "__main__":
    unittest.main()
