"""Batch-install parity gate: Rust `install_many` against the shipped Python one.

The single-install path was already byte-compared. This gate covers the batch
contract that the two implementations share byte for byte:

- mixed supported record types (rarity 3 / 4 / 5) in one batch,
- a non-contiguous inventory (a hole before the occupied tail),
- candidate subsets and repeated candidates,
- the contiguous-run capacity boundary and its refusal,
- no partial write on any refusal,
- source preservation outside the appended slots,
- the written inventory keys / generation serials / installed records,
- explicit user-checksum derivation checks on the resulting plaintext.

The Rust side runs the ported Rust codec end to end — the crate contains no
external-process call, so its container cannot be the shipped tool's output. The
shipped `bin/Nioh_Savefile_decrypt.exe` is used only as the oracle: it builds the
fixture container and decrypts results for the record-level assertions. The
Python installer is run entirely inside a short temp directory because the
shipped tool refuses long paths. No user save and no game process is involved.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
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
from tests.migration.test_save_read_parity import (  # noqa: E402
    OWN_ACCOUNT,
    SCROLL_GROUP_OFFSET,
    SCROLL_RECORD_SIZE,
    _pack_record,
    build_fixture_bytes,
    native_transform_short,
)

FIXTURE_ROOT = Path(
    os.environ.get(
        "NIOH3_SAVE_FIXTURE_ROOT",
        r"D:\Nioh3_v080_deliverables\m3-save-acceptance",
    )
)

# One supported native type per rarity tier, from the shipped category table.
RARITY_RECORDS = {
    "r3": 0x1E82,
    "r4": 0xE604,
    "r5": 0xD523,
}


def checksum_of(blob: bytes) -> int:
    """Recompute the folded user checksum from a decrypted save, independently."""

    import struct

    seed = struct.unpack_from("<I", blob, 0x90_0190)[0]
    body = blob[USER_CHECKSUM_BODY_START:USER_CHECKSUM_BODY_END]
    return compute_user_checksum(body, seed)


def stored_checksum(blob: bytes) -> int:
    import struct

    return struct.unpack_from("<I", blob, USER_CHECKSUM_VALUE_OFFSET)[0]


class BatchInstallFixture(unittest.TestCase):
    """Shared helpers; concrete cases live in the subclasses below."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.target = resolved_cargo_target_dir("save-parity")
        cls.host = Path(cls.target) / "debug" / "examples" / "save_transaction.exe"
        build = subprocess.run(
            [
                "cargo",
                "build",
                "--offline",
                "--quiet",
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
        if build.returncode != 0 or not cls.host.is_file():
            raise AssertionError(
                "the save host did not build: "
                + (build.stderr.strip() or build.stdout.strip())
            )
        FIXTURE_ROOT.mkdir(parents=True, exist_ok=True)
        cls.temp = tempfile.TemporaryDirectory(dir=str(FIXTURE_ROOT))
        cls.root = Path(cls.temp.name)
        cls.base_blob = bytes(build_fixture_bytes())
        cls.base_plain = cls.root / "base-plain.bin"
        cls.base_plain.write_bytes(cls.base_blob)
        cls.base_container = cls.root / "base-container.bin"
        native_transform_short(cls.base_plain, cls.base_container)
        cls.base_source_sha256 = hashlib.sha256(cls.base_container.read_bytes()).hexdigest()

    @classmethod
    def tearDownClass(cls) -> None:
        cls.temp.cleanup()

    def short_root(self, label: str) -> Path:
        """A short-path root, because the shipped tool refuses long paths."""

        directory = tempfile.mkdtemp(prefix=f"n3b{label[:2]}-")
        self.addCleanup(shutil.rmtree, directory, ignore_errors=True)
        return Path(directory)

    def candidate_for(self, name: str, seed: int) -> bytes:
        return bytes(
            _pack_record(
                record_type=RARITY_RECORDS[name],
                account_id=OWN_ACCOUNT,
                level=180,
                recommended_level=190,
                seed=seed,
                inventory_key=0,
                serial=0,
                rarity={"r3": 3, "r4": 4, "r5": 5}[name],
                transfer_count=0,
            )
        )

    def filler_for(self, name: str, seed: int, serial: int) -> bytes:
        """An occupied record with its own `+0x28` serial, as a real save has.

        Candidates keep serial 0 because the installer assigns one, but records
        that already occupy the inventory must not share identities or the
        append-only collision gate rightly refuses the batch.
        """

        return bytes(
            _pack_record(
                record_type=RARITY_RECORDS[name],
                account_id=OWN_ACCOUNT,
                level=180,
                recommended_level=190,
                seed=seed,
                inventory_key=serial,
                serial=serial,
                rarity=3,
                transfer_count=0,
            )
        )

    def records_file(self, name: str, records: list[bytes]) -> Path:
        path = self.root / f"{name}.json"
        path.write_text(
            json.dumps({"records": [record.hex() for record in records]}),
            encoding="utf-8",
        )
        return path

    def prepare_rust_save(self, container: Path, label: str) -> Path:
        root = self.short_root(label)
        save = root / str(OWN_ACCOUNT) / "SAVEDATA00" / "SAVEDATA.BIN"
        save.parent.mkdir(parents=True)
        save.write_bytes(container.read_bytes())
        (save.parent / "BACKUP.BIN").write_bytes(b"game-backup")
        system = root / str(OWN_ACCOUNT) / "SYSTEMSAVEDATA00"
        system.mkdir(parents=True)
        (system / "SAVEDATA.BIN").write_bytes(b"system-save")
        return save

    def run_host(self, *arguments: str) -> subprocess.CompletedProcess:
        return subprocess.run(
            [str(self.host), *arguments],
            cwd=str(ROOT),
            capture_output=True,
            text=True,
            check=False,
        )

    def rust_install_many(self, save: Path, state_root: Path, candidates: list[bytes]) -> dict:
        """Plan a batch and commit it, asserting both steps succeed."""

        planned = self.rust_plan_install_many(save, state_root, candidates)
        self.assertEqual(planned.returncode, 0, planned.stderr)
        lines = planned.stdout.strip().splitlines()
        plan_id = lines[0]
        fields: dict[str, list[str]] = {}
        for line in lines[1:]:
            parts = line.split("\t")
            fields[parts[0]] = parts[1:]
        commit = self.run_host(
            "commit",
            "--state-root",
            str(state_root),
            "--save-path",
            str(save),
            "--plan-id",
            plan_id,
        )
        return {"plan": planned, "commit": commit, "fields": fields, "plan_id": plan_id}

    def rust_plan_install_many(
        self, save: Path, state_root: Path, candidates: list[bytes]
    ) -> subprocess.CompletedProcess:
        """Ask the host to plan a batch without requiring it to succeed."""

        source = hashlib.sha256(save.read_bytes()).hexdigest()
        spec = self.records_file("batch-rust", candidates)
        return self.run_host(
            "plan-install",
            "--state-root",
            str(state_root),
            "--save-path",
            str(save),
            "--source-sha256",
            source,
            "--record-file",
            str(spec),
        )

    def python_install_many(self, container: Path, candidates: list[bytes], label: str) -> Path:
        """Run the shipped installer on an identical generation, short paths only."""

        from nioh3_scroll_editor.savegame import SaveCrypto, SaveInstaller, default_crypto_tool

        root = self.short_root(label)
        save = root / str(OWN_ACCOUNT) / "SAVEDATA00" / "SAVEDATA.BIN"
        save.parent.mkdir(parents=True)
        save.write_bytes(container.read_bytes())
        (save.parent / "BACKUP.BIN").write_bytes(b"game-backup")
        system = root / str(OWN_ACCOUNT) / "SYSTEMSAVEDATA00"
        system.mkdir(parents=True)
        (system / "SAVEDATA.BIN").write_bytes(b"system-save")
        installer = SaveInstaller(
            save_path=save,
            crypto=SaveCrypto(default_crypto_tool(ROOT)),
            state_root=root / "state",
        )
        installer.install_many(candidates, action="v2-cart-install")
        return save

    def decrypt(self, container: Path, label: str) -> bytes:
        output = self.root / f"decrypted-{label}.bin"
        native_transform_short(container, output)
        return output.read_bytes()

    def rust_decode(self, container: Path, label: str) -> bytes:
        """Decode with the ported Rust codec, byte for byte and process-free."""

        output = self.root / f"rust-decoded-{label}.bin"
        completed = self.run_host(
            "decrypt-container",
            "--state-root",
            str(self.root / "decode-state"),
            "--save-path",
            str(self.root / "decode-save" / "SAVEDATA00" / "SAVEDATA.BIN"),
            "--container-file",
            str(container),
            "--output-file",
            str(output),
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        return output.read_bytes()

    def installed_region(self, blob: bytes, slots: list[int]) -> bytes:
        return b"".join(
            blob[
                SCROLL_GROUP_OFFSET
                + slot * SCROLL_RECORD_SIZE : SCROLL_GROUP_OFFSET
                + (slot + 1) * SCROLL_RECORD_SIZE
            ]
            for slot in slots
        )


class BatchInstallParityTests(BatchInstallFixture):
    def assert_matches_python(self, candidates: list[bytes], label: str) -> dict:
        rust_save = self.prepare_rust_save(self.base_container, f"r-{label}")
        rust_state = rust_save.parent.parent.parent / "state"
        rust = self.rust_install_many(rust_save, rust_state, candidates)
        self.assertEqual(rust["commit"].returncode, 0, rust["commit"].stderr)

        python_save = self.python_install_many(self.base_container, candidates, f"p-{label}")
        self.assertEqual(
            rust_save.read_bytes(),
            python_save.read_bytes(),
            "the Rust batch install must reproduce the shipped installer bytes exactly",
        )

        slots = [int(value) for value in rust["fields"]["slots"][0].split(",")]
        keys = [int(value) for value in rust["fields"]["keys"][0].split(",")]
        serials = [int(value) for value in rust["fields"]["serials"][0].split(",")]
        installed = rust["fields"]["records"][0].split(",")

        plaintext = self.decrypt(rust_save, label)
        # Installed records are exactly the reported ones, with the reported
        # identity fields written into them.
        import struct

        for index, slot in enumerate(slots):
            offset = SCROLL_GROUP_OFFSET + slot * SCROLL_RECORD_SIZE
            record = plaintext[offset : offset + SCROLL_RECORD_SIZE]
            self.assertEqual(record.hex(), installed[index])
            self.assertEqual(struct.unpack_from("<I", record, 0x1C)[0], keys[index])
            self.assertEqual(struct.unpack_from("<I", record, 0x28)[0], serials[index])
            self.assertNotEqual(struct.unpack_from("<H", record, 0x00)[0], 0)

        # The checksum field is our own derived value, not the tool's leftovers.
        self.assertEqual(
            stored_checksum(plaintext),
            checksum_of(plaintext),
            "the installed save must carry the folded checksum for its own bytes",
        )
        return {"slots": slots, "keys": keys, "serials": serials, "plaintext": plaintext}

    def test_mixed_rarity_batch_matches_python(self) -> None:
        candidates = [
            self.candidate_for("r3", 0x11110001),
            self.candidate_for("r4", 0x22220002),
            self.candidate_for("r5", 0x33330003),
        ]
        result = self.assert_matches_python(candidates, "mixed")
        # The fixture has a hole at slot 2 before the occupied tail, so the batch
        # appends after the tail and never reuses the hole.
        self.assertEqual(result["slots"], [6, 7, 8])
        # Source preservation: every slot outside the appended run is untouched.
        for slot in range(400):
            if slot in result["slots"]:
                continue
            self.assertEqual(
                self.installed_region(result["plaintext"], [slot]),
                self.installed_region(self.base_blob, [slot]),
                f"slot {slot} was rewritten by a batch install",
            )

    def test_subset_and_repeat_candidates_match_python(self) -> None:
        r5 = self.candidate_for("r5", 0x44440004)
        r3 = self.candidate_for("r3", 0x55550005)
        # A subset with a repeated candidate: both implementations address the
        # batch by position, so the outcome must still match byte for byte.
        self.assert_matches_python([r5], "subset-one")
        self.assert_matches_python([r5, r3], "subset-two")
        self.assert_matches_python([r5, r5], "repeat")

    def test_duplicate_serials_refuse_before_writing(self) -> None:
        # One record already shares its +0x28 with another occupied record.
        blob = bytearray(self.base_blob)
        first = SCROLL_GROUP_OFFSET + 1 * SCROLL_RECORD_SIZE
        second = SCROLL_GROUP_OFFSET + 3 * SCROLL_RECORD_SIZE
        blob[second + 0x28 : second + 0x2C] = blob[first + 0x28 : first + 0x2C]
        corrupted = self.root / "dup-plain.bin"
        corrupted.write_bytes(bytes(blob))
        container = self.root / "dup-container.bin"
        native_transform_short(corrupted, container)

        save = self.prepare_rust_save(container, "dup")
        before = save.read_bytes()
        refused = self.rust_plan_install_many(
            save, save.parent.parent.parent / "state", [self.candidate_for("r5", 1)]
        )
        self.assertNotEqual(refused.returncode, 0, "colliding serials must refuse")
        self.assertIn("APPEND_ONLY_REPAIR_REQUIRED", refused.stderr)
        self.assertEqual(save.read_bytes(), before, "a refusal must not write")

        from nioh3_scroll_editor.savegame import SaveCrypto, SaveInstaller, default_crypto_tool

        short = self.short_root("dup-python")
        py_save = short / str(OWN_ACCOUNT) / "SAVEDATA00" / "SAVEDATA.BIN"
        py_save.parent.mkdir(parents=True)
        py_save.write_bytes(container.read_bytes())
        installer = SaveInstaller(
            save_path=py_save,
            crypto=SaveCrypto(default_crypto_tool(ROOT)),
            state_root=short / "state",
        )
        with self.assertRaises(RuntimeError) as raised:
            installer.install_many([self.candidate_for("r5", 1)], action="v2-cart-install")
        self.assertIn("APPEND_ONLY_REPAIR_REQUIRED", str(raised.exception))
        self.assertEqual(py_save.read_bytes(), container.read_bytes())

    def test_capacity_boundary_refuses_without_partial_write(self) -> None:
        # Filling slots 1..397 leaves exactly one free slot (399) after the tail,
        # so a two-record batch crosses the 400-slot boundary and is refused,
        # while a one-record batch fits.
        blob = bytearray(self.base_blob)
        for slot in range(1, 398):
            offset = SCROLL_GROUP_OFFSET + slot * SCROLL_RECORD_SIZE
            blob[offset : offset + SCROLL_RECORD_SIZE] = self.filler_for(
                "r3", 0x1000 + slot, 0x100 + slot
            )
        packed = self.root / "packed-plain.bin"
        packed.write_bytes(bytes(blob))
        container = self.root / "packed-container.bin"
        native_transform_short(packed, container)

        candidates = [self.candidate_for("r4", 0x6000 + index) for index in range(4)]
        save = self.prepare_rust_save(container, "cap")
        before = save.read_bytes()
        refused = self.rust_plan_install_many(
            save, save.parent.parent.parent / "state", candidates
        )
        self.assertNotEqual(
            refused.returncode, 0, "a batch past the slot boundary must refuse"
        )
        self.assertIn("contiguous", refused.stderr.lower(), refused.stderr)
        self.assertEqual(save.read_bytes(), before, "a refusal must not partially write")
        self.assertFalse(
            (save.parent.parent.parent / "state" / "v2-operations").exists(),
            "a refused plan must not leave a receipt",
        )

        # The single record that still fits must match the oracle exactly.
        self.assert_matches_python_for_container(container, candidates[:1], "cap-fit")

    def assert_matches_python_for_container(
        self, container: Path, candidates: list[bytes], label: str
    ) -> None:
        save = self.prepare_rust_save(container, f"r-{label}")
        rust = self.rust_install_many(save, save.parent.parent.parent / "state", candidates)
        self.assertEqual(rust["commit"].returncode, 0, rust["commit"].stderr)
        python_save = self.python_install_many(container, candidates, f"p-{label}")
        self.assertEqual(
            save.read_bytes(),
            python_save.read_bytes(),
            "the fitting batch must match the shipped installer",
        )

    def test_the_rust_install_path_never_shells_out(self) -> None:
        """The byte comparison is only meaningful if Rust wrote its own container.

        If the Rust install had invoked `bin/Nioh_Savefile_decrypt.exe`, matching
        the shipped installer's bytes would prove nothing about the Rust codec.
        The crate carries no external-process call at all, so it cannot; this
        pins the property instead of asserting it in prose.
        """

        crate = ROOT / "crates" / "nioh3-save"
        sources = sorted([*crate.glob("src/**/*.rs"), *crate.glob("examples/**/*.rs")])
        self.assertTrue(sources, "the crate sources must be present")
        offenders = [
            f"{path.relative_to(ROOT)}: {needle}"
            for path in sources
            for needle in ("Command::new", "process::Command")
            if needle in path.read_text(encoding="utf-8")
        ]
        self.assertEqual(
            offenders,
            [],
            "the Rust save crate must not spawn an external codec",
        )

    def test_the_rust_container_decodes_to_the_reported_records(self) -> None:
        """Decode the Rust-written batch container with the Rust codec itself.

        The shipped installer's bytes match in `assert_matches_python`; this case
        shows why that comparison says something about Rust. The container is
        decoded byte-preservingly by the same crate (no oracle, no rewrite), the
        reported slots/keys/serials/records come back exactly, untouched slots
        keep their source bytes, and the stored user checksum equals the fold of
        its own body — the value the Rust writer derived, not the oracle's.
        """

        import struct

        candidates = [
            self.candidate_for("r3", 0x99990009),
            self.candidate_for("r5", 0xAAAA000A),
        ]
        save = self.prepare_rust_save(self.base_container, "rust-decode")
        rust = self.rust_install_many(save, save.parent.parent.parent / "state", candidates)
        self.assertEqual(rust["commit"].returncode, 0, rust["commit"].stderr)
        slots = [int(value) for value in rust["fields"]["slots"][0].split(",")]
        keys = [int(value) for value in rust["fields"]["keys"][0].split(",")]
        serials = [int(value) for value in rust["fields"]["serials"][0].split(",")]
        installed = rust["fields"]["records"][0].split(",")

        plaintext = self.rust_decode(save, "rust-decode")
        for index, slot in enumerate(slots):
            offset = SCROLL_GROUP_OFFSET + slot * SCROLL_RECORD_SIZE
            record = plaintext[offset : offset + SCROLL_RECORD_SIZE]
            self.assertEqual(record.hex(), installed[index])
            self.assertEqual(struct.unpack_from("<I", record, 0x1C)[0], keys[index])
            self.assertEqual(struct.unpack_from("<I", record, 0x28)[0], serials[index])
        for slot in range(400):
            if slot in slots:
                continue
            self.assertEqual(
                self.installed_region(plaintext, [slot]),
                self.installed_region(self.base_blob, [slot]),
                f"slot {slot} was rewritten by a batch install",
            )
        self.assertEqual(
            stored_checksum(plaintext),
            checksum_of(plaintext),
            "the Rust container must carry the fold of its own body",
        )

    def test_batch_commit_receipt_matches_the_reference_fields(self) -> None:
        candidates = [
            self.candidate_for("r3", 0x77770007),
            self.candidate_for("r4", 0x88880008),
        ]
        rust_save = self.prepare_rust_save(self.base_container, "receipt")
        state = rust_save.parent.parent.parent / "state"
        rust = self.rust_install_many(rust_save, state, candidates)
        self.assertEqual(rust["commit"].returncode, 0, rust["commit"].stderr)
        receipt = json.loads(
            (state / "v2-operations" / f"{rust['plan_id']}.json").read_text(encoding="utf-8")
        )
        self.assertEqual(receipt["outcome"], "committed")
        self.assertEqual(
            receipt["installed_sha256"], hashlib.sha256(rust_save.read_bytes()).hexdigest()
        )
        backup = state / "backups" / receipt["backup_id"]
        manifest = json.loads((backup / "backup-manifest.json").read_text(encoding="utf-8"))
        self.assertEqual(manifest["steam_account_id"], OWN_ACCOUNT)
        self.assertEqual(manifest["save_slot_index"], 0)
        self.assertEqual(
            [entry["backup_file"] for entry in manifest["backup_files"]],
            ["SAVEDATA.BIN", "BACKUP.BIN", "SYSTEMSAVEDATA.BIN"],
        )
        self.assertEqual(
            (backup / "SAVEDATA.BIN").read_bytes(),
            self.base_container.read_bytes(),
            "the checkpoint must hold the quiet pre-write generation",
        )


if __name__ == "__main__":
    unittest.main()
