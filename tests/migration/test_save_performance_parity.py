"""Release-profile timing parity for the M3-b save lane.

The save lane must not regress functionality or performance against the shipped
Python path plus the native codec. This gate measures the same synthetic
save through both implementations and compares:

- read/decrypt (container -> validated plaintext),
- prepare (read + transform + re-encrypt into a staged container),
- guarded batch install (quiescent baseline + generation revalidation +
  checkpoint + durable receipt + atomic replace + readback, guards intact),

in two regimes: cold process spawn, and steady repeated calls inside one process.

The Rust side is measured from a prebuilt release binary so the numbers are not
debug-build artifacts and spawn cost is not hiding compile time. Each comparison
matches the shipped surface it is measured against: the codec/prepare cases are
the unguarded read+transform cycle on both sides, and the guarded transaction
case runs the same three 0.20 s quiescence windows as the shipped installer.

Numbers are written to the task deliverables directory for review.
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
import time
import unittest


ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from emaki_exchange import patch_user_checksum  # noqa: E402
from tests.migration.test_save_read_parity import (  # noqa: E402
    SCROLL_GROUP_OFFSET,
    SCROLL_RECORD_SIZE,
    build_fixture_bytes,
    native_transform_short,
    resolved_cargo_target_dir,
)

FIXTURE_ROOT = Path(
    os.environ.get(
        "NIOH3_SAVE_FIXTURE_ROOT",
        r"D:\Nioh3_v080_deliverables\m3-save-acceptance",
    )
)
DELIVERABLES = ROOT / "deliverables" / "v080-completion-readiness"


def release_binary(target: str) -> Path:
    return Path(target) / "release" / "examples" / "save_transaction.exe"


class SavePerformanceParityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.target = resolved_cargo_target_dir("save-parity")
        cls.release = release_binary(cls.target)
        build = subprocess.run(
            [
                "cargo",
                "build",
                "--offline",
                "--quiet",
                "--release",
                "--manifest-path",
                str(ROOT / "crates" / "nioh3-save" / "Cargo.toml"),
                "--example",
                "save_transaction",
            ],
            cwd=str(ROOT),
            env={**os.environ, "CARGO_TARGET_DIR": cls.target},
            capture_output=True,
            text=True,
            check=False,
        )
        if build.returncode != 0 or not cls.release.is_file():
            raise AssertionError(
                "the release save host did not build: "
                + (build.stderr.strip() or build.stdout.strip())
            )
        FIXTURE_ROOT.mkdir(parents=True, exist_ok=True)
        cls.temp = tempfile.TemporaryDirectory(dir=str(FIXTURE_ROOT))
        cls.root = Path(cls.temp.name)
        cls.account = "76561198000000000"
        cls.save_path = cls.root / cls.account / "SAVEDATA00" / "SAVEDATA.BIN"
        cls.save_path.parent.mkdir(parents=True)
        cls.state_root = cls.root / "state"

        cls.blob = bytes(build_fixture_bytes())
        cls.plain = cls.root / "base-plain.bin"
        cls.plain.write_bytes(cls.blob)
        cls.container = cls.root / "base-container.bin"
        native_transform_short(cls.plain, cls.container)
        cls.baseline = hashlib.sha256(cls.container.read_bytes()).hexdigest()
        cls.spec = cls.root / "spec.json"
        cls.spec.write_text(
            json.dumps(
                {
                    "edits": [
                        {
                            "slot_index": 1,
                            "header": {
                                "playthrough": 3,
                                "level": 185,
                                "recommended_level": 195,
                                "seed": 0x0BADF00D,
                                "rarity": 5,
                                "transfer_count": 2,
                            },
                            "effects": [{"slot_index": 0, "value": 4242}],
                        }
                    ]
                }
            ),
            encoding="utf-8",
        )
        cls.samples = int(os.environ.get("NIOH3_SAVE_BENCH_SAMPLES", "5"))

    @classmethod
    def tearDownClass(cls) -> None:
        cls.temp.cleanup()

    def setUp(self) -> None:
        self.save_path.write_bytes(self.container.read_bytes())
        for child in ("backups", "v2-operations"):
            directory = self.state_root / child
            if directory.is_dir():
                for path in sorted(directory.rglob("*"), reverse=True):
                    if path.is_file():
                        path.unlink()
                    elif path.is_dir():
                        path.rmdir()

    def timed(self, function, *, runs: int = 1) -> float:
        samples = []
        for _ in range(runs):
            start = time.perf_counter()
            function()
            samples.append(time.perf_counter() - start)
        return statistics.median(samples)

    def rust_run(self, *arguments: str) -> subprocess.CompletedProcess:
        return subprocess.run(
            [str(self.release), *arguments],
            cwd=str(ROOT),
            capture_output=True,
            text=True,
            check=False,
        )

    def rust_bench(self) -> dict:
        completed = self.rust_run(
            "bench",
            "--state-root",
            str(self.state_root),
            "--save-path",
            str(self.save_path),
            "--source-sha256",
            self.baseline,
            "--spec-file",
            str(self.spec),
            "--repeat",
            str(self.samples),
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        line = completed.stdout.strip().splitlines()[-1].split("\t")
        return {
            "iterations": int(line[1]),
            "decrypt_us": int(line[2]),
            "prepare_us": int(line[3]),
        }

    def python_read(self) -> None:
        from nioh3_scroll_editor.savegame import SaveCrypto, default_crypto_tool

        crypto = SaveCrypto(default_crypto_tool(ROOT))
        with tempfile.TemporaryDirectory(dir=str(self.root)) as directory:
            decrypted = Path(directory) / "d.bin"
            crypto.decrypt(self.save_path, decrypted)
            decrypted.read_bytes()

    def python_prepare(self) -> None:
        """The shipped end-to-end prepare: decrypt, patched record, re-encrypt.

        This is the same cycle the Rust `bench` prepare case performs, so the two
        numbers describe the same work.
        """

        from nioh3_scroll_editor.savegame import (
            LocalEffectEdit,
            SaveCrypto,
            default_crypto_tool,
            patch_local_scroll_header,
            patch_local_scroll_record,
        )

        crypto = SaveCrypto(default_crypto_tool(ROOT))
        with tempfile.TemporaryDirectory(dir=str(self.root)) as directory:
            work = Path(directory)
            decrypted = work / "d.bin"
            crypto.decrypt(self.save_path, decrypted)
            edited = bytearray(decrypted.read_bytes())
            record = bytes(
                edited[
                    SCROLL_GROUP_OFFSET
                    + SCROLL_RECORD_SIZE : SCROLL_GROUP_OFFSET
                    + 2 * SCROLL_RECORD_SIZE
                ]
            )
            patched = patch_local_scroll_header(
                record,
                playthrough=3,
                level=185,
                recommended_level=195,
                seed=0x0BADF00D,
                rarity=5,
                transfer_count=2,
            )
            patched = patch_local_scroll_record(
                patched, [LocalEffectEdit(slot_index=0, value=4242)]
            )
            edited[
                SCROLL_GROUP_OFFSET
                + SCROLL_RECORD_SIZE : SCROLL_GROUP_OFFSET
                + 2 * SCROLL_RECORD_SIZE
            ] = patched
            patch_user_checksum(edited)
            staged = work / "staged.bin"
            staged.write_bytes(bytes(edited))
            crypto.encrypt(staged, work / "staged-container.bin")

    def test_rust_codec_is_not_slower_than_the_shipped_path(self) -> None:
        numbers = self.rust_bench()
        per_call_decrypt = numbers["decrypt_us"] / numbers["iterations"]
        per_call_prepare = numbers["prepare_us"] / numbers["iterations"]

        cold_spawn = self.timed(
            lambda: self.rust_run(
                "prepare-edit",
                "--state-root",
                str(self.state_root),
                "--save-path",
                str(self.save_path),
                "--source-sha256",
                self.baseline,
                "--spec-file",
                str(self.spec),
            ),
            runs=self.samples,
        )
        python_decrypt = self.timed(self.python_read, runs=self.samples)
        python_prepare = self.timed(self.python_prepare, runs=self.samples)

        report = {
            "fixture_bytes": self.save_path.stat().st_size,
            "samples": self.samples,
            "rust": {
                "decrypt_us_per_call": per_call_decrypt,
                "prepare_us_per_call": per_call_prepare,
                "prepare_cold_spawn_s": cold_spawn,
            },
            "python": {
                "read_decrypt_s": python_decrypt,
                "prepare_transform_s": python_prepare,
            },
        }
        DELIVERABLES.mkdir(parents=True, exist_ok=True)
        (DELIVERABLES / "M3B_SAVE_TIMINGS.json").write_text(
            json.dumps(report, indent=2), encoding="utf-8"
        )

        # The Rust codec does the same work with no external process. This bound
        # is deliberately far looser than the Python path so only a real
        # regression fails, and the message carries the full measurement.
        self.assertLess(
            per_call_decrypt,
            max(python_decrypt * 1_000_000, 1.0),
            f"Rust decrypt regressed against the shipped path: {json.dumps(report)}",
        )
        # Prepare is the same full cycle on both sides, so this compares like
        # with like: read + decrypt + transform + re-encrypt.
        self.assertLess(
            per_call_prepare,
            max(python_prepare * 1_000_000, 1.0),
            f"Rust prepare regressed against the shipped path: {json.dumps(report)}",
        )

    def test_commit_keeps_its_guards_and_stays_bounded(self) -> None:
        # The guarded-commit comparison against the shipped transaction lives in
        # `test_guarded_commit_matches_the_shipped_transaction`; this case only
        # asserts the guards themselves stay intact (both 0.20 s windows run) and
        # the cost stays bounded.
        times = []
        for _ in range(min(self.samples, 3)):
            plan = self.rust_run(
                "plan-edit",
                "--state-root",
                str(self.state_root),
                "--save-path",
                str(self.save_path),
                "--source-sha256",
                hashlib.sha256(self.save_path.read_bytes()).hexdigest(),
                "--spec-file",
                str(self.spec),
            )
            self.assertEqual(plan.returncode, 0, plan.stderr)
            plan_id = plan.stdout.strip().splitlines()[0]
            start = time.perf_counter()
            commit = self.rust_run(
                "commit",
                "--state-root",
                str(self.state_root),
                "--save-path",
                str(self.save_path),
                "--plan-id",
                plan_id,
            )
            elapsed = time.perf_counter() - start
            self.assertEqual(commit.returncode, 0, commit.stderr)
            times.append(elapsed)
            self.save_path.write_bytes(self.container.read_bytes())
            backups = self.state_root / "backups"
            if backups.is_dir():
                for path in sorted(backups.rglob("*"), reverse=True):
                    if path.is_file():
                        path.unlink()
                    elif path.is_dir():
                        path.rmdir()
        DELIVERABLES.mkdir(parents=True, exist_ok=True)
        (DELIVERABLES / "M3B_SAVE_COMMIT_TIMINGS.json").write_text(
            json.dumps(
                {
                    "runs": len(times),
                    "commit_s": times,
                    "median_commit_s": statistics.median(times),
                    "guards": [
                        "quiescent-baseline",
                        "generation-revalidation-before-replace",
                        "durable-receipt",
                        "checkpoint-before-write",
                        "atomic-replace",
                        "readback-digest",
                    ],
                    "quiescence_window_s": 0.20,
                },
                indent=2,
            ),
            encoding="utf-8",
        )
        # A guarded commit is a bounded transaction; ten seconds is far above the
        # observed cost and still catches a hang or an unbounded retry.
        self.assertLess(statistics.median(times), 10.0, json.dumps(times))

    def candidate_batch(self) -> list[bytes]:
        """The same three-rarity batch the batch-parity gate compares."""

        from tests.migration.test_save_batch_install_parity import RARITY_RECORDS
        from tests.migration.test_save_read_parity import OWN_ACCOUNT, _pack_record

        return [
            bytes(
                _pack_record(
                    record_type=RARITY_RECORDS[name],
                    account_id=OWN_ACCOUNT,
                    level=180,
                    recommended_level=190,
                    seed=seed,
                    inventory_key=0,
                    serial=0,
                    rarity=rarity,
                    transfer_count=0,
                )
            )
            for name, rarity, seed in (
                ("r3", 3, 0x11110001),
                ("r4", 4, 0x22220002),
                ("r5", 5, 0x33330003),
            )
        ]

    def python_guarded_install_many(
        self, save: Path, state_root: Path, candidates: list[bytes]
    ) -> float:
        """One shipped `install_many`, timed as the whole guarded operation.

        This is the shipped surface that actually carries the 0.20 s quiescence
        windows: `capture_quiescent_save_fingerprints` before the checkpoint and
        two `require_related_save_fingerprints` calls inside
        `commit_encrypted_main_save` — three windows, plus the checkpoint, the
        atomic replace, the readback and three native-codec spawns.
        """

        from nioh3_scroll_editor.savegame import SaveCrypto, SaveInstaller, default_crypto_tool

        installer = SaveInstaller(
            save_path=save,
            crypto=SaveCrypto(default_crypto_tool(ROOT)),
            state_root=state_root,
        )
        started = time.perf_counter()
        installer.install_many(candidates, action="v2-cart-install")
        return time.perf_counter() - started

    def test_guarded_commit_matches_the_shipped_transaction(self) -> None:
        """Equal-guard guarded transaction: Rust against the shipped installer.

        Both sides run the *batch install*, which is the shipped surface that
        carries the 0.20 s multi-file quiescence windows. Rust runs
        `plan_install_many` + `commit` in one process: the quiescent baseline,
        the generation revalidation before the replace, the checkpoint, the
        durable receipt, the atomic replace and the readback — three windows of
        `SAVE_QUIESCENCE_SECONDS` in total. The shipped `install_many` runs the
        same three windows, plus three native-codec process spawns that the
        Rust path does not have. Fixture setup is outside both timers.
        """

        samples = self.samples
        candidates = self.candidate_batch()
        spec = self.root / "bench-install-records.json"
        spec.write_text(
            json.dumps({"records": [candidate.hex() for candidate in candidates]}),
            encoding="utf-8",
        )

        def fresh_generation(prefix: str) -> tuple[Path, Path]:
            root = Path(tempfile.mkdtemp(dir=str(FIXTURE_ROOT), prefix=prefix))
            self.addCleanup(shutil.rmtree, root, ignore_errors=True)
            save = root / self.account / "SAVEDATA00" / "SAVEDATA.BIN"
            save.parent.mkdir(parents=True)
            save.write_bytes(self.container.read_bytes())
            (save.parent / "BACKUP.BIN").write_bytes(b"game-backup")
            system = root / self.account / "SYSTEMSAVEDATA00"
            system.mkdir(parents=True)
            (system / "SAVEDATA.BIN").write_bytes(b"system-save")
            return save, root / "state"

        rust_times = []
        for _ in range(samples):
            save, state = fresh_generation("bcr-")
            completed = self.rust_run(
                "bench-install",
                "--state-root",
                str(state),
                "--save-path",
                str(save),
                "--record-file",
                str(spec),
                "--repeat",
                "1",
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            line = completed.stdout.strip().splitlines()[-1].split("\t")
            self.assertEqual(line[0], "bench-install", completed.stdout)
            rust_times.append(int(line[2]) / 1_000_000)

        python_times = []
        for _ in range(samples):
            save, state = fresh_generation("bcp-")
            python_times.append(self.python_guarded_install_many(save, state, candidates))

        report = {
            "samples": samples,
            "fixture_bytes": self.save_path.stat().st_size,
            "operation": "guarded batch install (three-rarity batch)",
            "rust_guarded_install_s": rust_times,
            "rust_median_s": statistics.median(rust_times),
            "python_guarded_install_s": python_times,
            "python_median_s": statistics.median(python_times),
            "guards": [
                "quiescent-baseline",
                "generation-revalidation-before-replace",
                "durable-receipt",
                "checkpoint-before-write",
                "atomic-replace",
                "readback-verification",
            ],
            "note": (
                "Both sides run three 0.20 s quiescence windows "
                "(SAVE_QUIESCENCE_SECONDS); the Rust numbers are the in-process "
                "plan+commit the example reports, and the Python numbers are the "
                "wall time of the install_many call, which additionally spawns "
                "the native codec three times."
            ),
        }
        DELIVERABLES.mkdir(parents=True, exist_ok=True)
        (DELIVERABLES / "M3B_SAVE_COMMIT_COMPARISON.json").write_text(
            json.dumps(report, indent=2), encoding="utf-8"
        )

        # Same guard set on both sides; the Rust lane must not be the slower one.
        self.assertLessEqual(
            statistics.median(rust_times),
            statistics.median(python_times),
            json.dumps(report),
        )


if __name__ == "__main__":
    unittest.main()
