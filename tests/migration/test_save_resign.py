"""Save re-sign (存档改签) through the Rust protected save worker.

A synthetic save signed to one account replaces a registered save slot of
another account. The plan commits through the ordinary backup/commit path; the
committed save names the slot's account in its header and in the scrolls the
old account owned, keeps a scroll another player shared, and leaves a backup of
the replaced save.
"""

from __future__ import annotations

import os
from pathlib import Path
import struct
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from tests.migration.cargo_target import built_cargo_binary, resolved_cargo_target_dir  # noqa: E402
from tests.migration.test_protected_worker_parity import SCHEMA_DIR, FramedWorker  # noqa: E402
from tests.migration.test_save_read_parity import (  # noqa: E402
    FOREIGN_ACCOUNT,
    OWN_ACCOUNT,
    SCROLL_GROUP_OFFSET,
    SCROLL_RECORD_SIZE,
    build_fixture_bytes,
    native_transform_short,
)

TARGET_ACCOUNT = 76561198000000000
HEADER_ACCOUNT_OFFSET = 0x10


def record_owner(plain: bytes, slot: int) -> int:
    offset = SCROLL_GROUP_OFFSET + slot * SCROLL_RECORD_SIZE
    high, middle = struct.unpack_from("<HH", plain, offset + 0x02)
    (low,) = struct.unpack_from("<I", plain, offset + 0x14)
    return (high << 48) | (middle << 32) | low


class SaveResignTests(unittest.TestCase):
    def test_resign_rebinds_header_and_own_scrolls_and_backs_up_the_slot(self) -> None:
        target_dir = resolved_cargo_target_dir()
        worker_binary = built_cargo_binary(
            ROOT / "crates" / "nioh3-protected" / "Cargo.toml", target_dir, binary="nioh3-protected-worker"
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            plain = build_fixture_bytes()
            struct.pack_into("<Q", plain, HEADER_ACCOUNT_OFFSET, OWN_ACCOUNT)
            owners = {slot: record_owner(plain, slot) for slot in range(8)}
            own_slots = [slot for slot, owner in owners.items() if owner == OWN_ACCOUNT]
            foreign_slots = [slot for slot, owner in owners.items() if owner == FOREIGN_ACCOUNT]
            self.assertTrue(own_slots and foreign_slots, "fixture must carry own and foreign scrolls")
            (root / "plain.bin").write_bytes(bytes(plain))
            source = root / "source" / "SAVEDATA.BIN"
            native_transform_short(root / "plain.bin", source)
            target = root / str(TARGET_ACCOUNT) / "SAVEDATA00" / "SAVEDATA.BIN"
            native_transform_short(root / "plain.bin", target)
            state = root / "state"
            state.mkdir()
            worker = FramedWorker(
                [
                    str(worker_binary), "--role", "save", "--dev-protected-only", "--legacy-test-context",
                    "--state-root", str(state), "--data-root", str(ROOT / "nioh3_scroll_editor" / "data"),
                    "--contract-dir", str(SCHEMA_DIR),
                ],
                cwd=ROOT,
                env={**os.environ, "NIOH3_STATE_ROOT": str(state)},
                name="resign worker",
            )
            try:
                self.assertTrue(worker.call("handshake")["ok"])
                registered = self.drive(worker, "save.register", {"path": str(target)})
                inventory = self.drive(worker, "save.inventory", {"save_id": registered["save_id"]})
                plan = self.drive(worker, "save.prepare_resign", {
                    "save_id": registered["save_id"],
                    "snapshot_id": inventory["snapshot_id"],
                    "source_path": str(source),
                })
                self.assertEqual(plan["kind"], "resign")
                self.assertEqual(plan["preview"]["from_account"], str(OWN_ACCOUNT))
                self.assertEqual(plan["preview"]["to_account"], str(TARGET_ACCOUNT))
                self.assertEqual(plan["preview"]["rebound_scrolls"], len(own_slots))
                receipt = self.drive(worker, "save.commit", {"plan_id": plan["plan_id"]})
                self.assertTrue(receipt["commit_status"].startswith("committed"), receipt)
                backups = self.drive(worker, "save.backups", {"save_id": registered["save_id"]})
                self.assertEqual(len(backups["backups"]), 1, "the replaced save is backed up")
            finally:
                worker.terminate()
            after = root / "after.bin"
            native_transform_short(target, after)
            committed = after.read_bytes()
            self.assertEqual(struct.unpack_from("<Q", committed, HEADER_ACCOUNT_OFFSET)[0], TARGET_ACCOUNT)
            for slot in own_slots:
                self.assertEqual(record_owner(committed, slot), TARGET_ACCOUNT)
            for slot in foreign_slots:
                self.assertEqual(record_owner(committed, slot), FOREIGN_ACCOUNT)

    def drive(self, worker: FramedWorker, method: str, params: dict) -> dict:
        started = worker.call(method, params)
        self.assertTrue(started["ok"], started)
        job = started["result"]
        while job["state"] in ("running", "cancel_requested"):
            snapshot = worker.call("job.snapshot", {"job_id": job["job_id"]})
            self.assertTrue(snapshot["ok"], snapshot)
            job = snapshot["result"]
        self.assertNotEqual(job["state"], "failed", job.get("error"))
        return job["result"]


if __name__ == "__main__":
    unittest.main()
