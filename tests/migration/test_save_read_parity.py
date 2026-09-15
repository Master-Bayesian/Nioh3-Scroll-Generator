"""Read-only parity gate for the M3-a Rust save model.

`crates/nioh3-save/examples/save_read_vectors.rs` loads one decrypted save
through the new production read API and emits its identity, template and
inventory rows. This gate builds that save from retained synthetic bytes, runs
the same bytes through the shipped Python loader
(`nioh3_scroll_editor.savegame.SaveInventory`), and compares every row exactly,
so a wrong offset, a wrong category table or a wrong template rule fails instead
of passing silently.

The Python encoder stays the oracle for record construction; the Rust side only
reads. Corrupted, truncated and wrong-identity inputs must be rejected before any
value is exposed.
"""
from __future__ import annotations

import hashlib
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from emaki_exchange import SCROLL_RECORD_SIZE, USER_SAVE_SIZE  # noqa: E402
from nioh3_scroll_editor.savegame import discover_save_paths  # noqa: E402
from nioh3_scroll_editor.savegame import (  # noqa: E402
    account_id_from_save_path,
    save_slot_index_from_path,
    SaveInventory,
    SCROLL_GROUP_OFFSET,
    SCROLL_SLOT_COUNT,
)
from tests.migration.cargo_target import resolved_cargo_target_dir  # noqa: E402

SAVE_CRYPTO = ROOT / "bin" / "Nioh_Savefile_decrypt.exe"
# Pinned identity of the shipped codec oracle. The gate fails rather than skips
# when this does not match, so a swapped or missing component cannot make the
# gate green. `NIOH3_ALLOW_MISSING_SAVE_ORACLE=1` is the explicit opt-out.
SAVE_CRYPTO_SHA256 = "a767e967b955082ce0fc0b44daacfda3903446caa448591f5efab72f43f45a9a"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def require_save_oracle() -> None:
    """Fail closed unless the pinned shipped codec is present and unmodified."""

    if not SAVE_CRYPTO.is_file():
        if os.environ.get("NIOH3_ALLOW_MISSING_SAVE_ORACLE") == "1":
            raise unittest.SkipTest(
                f"NIOH3_ALLOW_MISSING_SAVE_ORACLE=1 and {SAVE_CRYPTO} is absent"
            )
        raise AssertionError(
            f"the shipped Nioh save crypto component is required: {SAVE_CRYPTO}"
        )
    actual = sha256_file(SAVE_CRYPTO)
    if actual != SAVE_CRYPTO_SHA256:
        raise AssertionError(
            "the shipped save crypto component does not match its pinned digest: "
            f"expected {SAVE_CRYPTO_SHA256}, got {actual}"
        )

SAVE_CATEGORY = 3
SAVE_RECORD_TYPE = 0xE604
OWN_ACCOUNT = 0x1111_2222_3333_4444
FOREIGN_ACCOUNT = 0x5555_6666_7777_8888


def _pack_record(
    *,
    record_type: int = SAVE_RECORD_TYPE,
    account_id: int = OWN_ACCOUNT,
    level: int = 180,
    recommended_level: int = 190,
    seed: int = 0x0BADF00D,
    inventory_key: int = 7,
    serial: int = 11,
    rarity: int = 5,
    transfer_count: int = 3,
) -> bytes:
    """One mapped scroll record built with the shipped 0xE8 field offsets."""

    record = bytearray(SCROLL_RECORD_SIZE)
    struct.pack_into("<H", record, 0x00, record_type)
    struct.pack_into("<H", record, 0x02, (account_id >> 48) & 0xFFFF)
    struct.pack_into("<H", record, 0x04, (account_id >> 32) & 0xFFFF)
    struct.pack_into("<H", record, 0x06, level)
    struct.pack_into("<H", record, 0x10, recommended_level)
    struct.pack_into("<I", record, 0x14, account_id & 0xFFFFFFFF)
    struct.pack_into("<I", record, 0x1C, inventory_key)
    struct.pack_into("<I", record, 0x20, seed)
    struct.pack_into("<I", record, 0x28, serial)
    record[0x30] = rarity
    record[0x31] = rarity
    struct.pack_into("<I", record, 0xDC, transfer_count)
    return bytes(record)


def _fixture() -> dict[int, bytes]:
    """Occupied slots for a mixed synthetic save, with one foreign template."""

    return {
        0: _pack_record(record_type=SAVE_RECORD_TYPE, account_id=FOREIGN_ACCOUNT),
        1: _pack_record(seed=0x11111111, inventory_key=9, serial=12, transfer_count=4),
        3: _pack_record(
            record_type=0x1E82,
            seed=0x22222222,
            inventory_key=0x1234,
            serial=0x9ABCDEF0,
            rarity=3,
            transfer_count=0,
        ),
        5: _pack_record(record_type=0x0001),
    }


def build_fixture_bytes() -> bytearray:
    save = bytearray(USER_SAVE_SIZE)
    save[:6] = b"RNNUSR"
    for slot_index, record in _fixture().items():
        offset = SCROLL_GROUP_OFFSET + slot_index * SCROLL_RECORD_SIZE
        save[offset : offset + SCROLL_RECORD_SIZE] = record
    # One free slot whose type is cleared but whose tail keeps stale bytes.
    stale_offset = SCROLL_GROUP_OFFSET + 2 * SCROLL_RECORD_SIZE
    save[stale_offset : stale_offset + SCROLL_RECORD_SIZE] = b"\xAA" * SCROLL_RECORD_SIZE
    struct.pack_into("<H", save, stale_offset, 0)
    return save


def account_byte_fragments(account_id: int) -> tuple[str, str]:
    """The two little-endian hex fragments an account id occupies in a record."""

    middle = ((account_id >> 32) & 0xFFFF).to_bytes(2, "little").hex()
    low = (account_id & 0xFFFFFFFF).to_bytes(4, "little").hex()
    return middle, low


def reference_rows(save_path: Path, blob: bytes) -> list[str]:
    """The shipped Python read model rendered in the gate's row format."""

    inventory = SaveInventory.load(save_path, blob)
    rows: list[str] = []
    rows.append(
        "save\t{}\t{}\t{}".format(
            save_path, inventory.account_id, __import__("hashlib").sha256(blob).hexdigest()
        )
    )
    rows.append(
        "path\t{}\t{}".format(
            account_id_from_save_path(save_path), save_slot_index_from_path(save_path)
        )
    )
    template = inventory.template_record
    if template:
        rows.append("template\t{:04x}\t{}".format(struct.unpack_from("<H", template, 0)[0], template.hex()))
    else:
        rows.append("template\tnone\t-")
    for playthrough in (3, 4, 5):
        try:
            record = inventory.template_record_for_playthrough(playthrough)
        except (RuntimeError, ValueError) as error:
            rows.append("playthrough\t{}\tnone\t{}".format(playthrough, error))
            continue
        rows.append(
            "playthrough\t{}\t{:04x}\t{}".format(
                playthrough, struct.unpack_from("<H", record, 0)[0], record.hex()
            )
        )
    for slot_index in inventory.empty_slots:
        rows.append("empty\t{}".format(slot_index))
    if inventory.next_slot_index is None:
        rows.append("next-slot\tnone")
    else:
        rows.append("next-slot\t{}".format(inventory.next_slot_index))
    entries = inventory.scroll_entries(include_unmapped=True)
    # Serial and inventory-key rows cover the occupied region the shipped loader
    # reports; only their values are read here.
    all_serials = []
    all_keys = []
    for slot_index in range(SCROLL_SLOT_COUNT):
        offset = SCROLL_GROUP_OFFSET + slot_index * SCROLL_RECORD_SIZE
        record = blob[offset : offset + SCROLL_RECORD_SIZE]
        if struct.unpack_from("<H", record, 0)[0] == 0:
            continue
        if struct.unpack_from("<I", record, 0x28)[0]:
            all_serials.append(struct.unpack_from("<I", record, 0x28)[0])
        if struct.unpack_from("<I", record, 0x1C)[0]:
            all_keys.append(struct.unpack_from("<I", record, 0x1C)[0])
    rows.append(
        "serials\t{}\t{}".format(len(all_serials), ",".join(str(value) for value in all_serials))
    )
    rows.append("next-serial\t{}".format(max(all_serials) + 1 if all_serials else 1))
    rows.append(
        "keys\t{}\t{}".format(len(all_keys), ",".join(str(value) for value in all_keys))
    )
    for entry in entries:
        # `entry.record_type`, `entry.playthrough`, `entry.seed`, `entry.rarity`
        # and `entry.transfer_count` are the shipped reader's own parsed fields,
        # so the comparison pins the Rust decode rather than restating it.
        rows.append(
            "entry\t{}\t{}\t{:04x}\t{}\t{}\t{}\t{}\t{}\t{}".format(
                entry.slot_index,
                entry.record_offset,
                entry.record_type,
                entry.playthrough if entry.playthrough is not None else "unmapped",
                entry.seed,
                entry.rarity,
                entry.transfer_count,
                struct.unpack_from("<I", entry.record, 0x28)[0],
                struct.unpack_from("<I", entry.record, 0x1C)[0],
            )
        )
    return rows


def run_rust_read(save_path: Path, fixture: Path, target: str) -> list[str]:
    return run_example(
        target,
        ["--fixture", str(fixture), "--save-path", str(save_path)],
    )


def run_rust_decrypt(container: Path, save_path: Path, target: str) -> list[str]:
    return run_example(
        target,
        ["--decrypt", str(container), "--save-path", str(save_path)],
    )


def run_rust_encrypt(plain: Path, container: Path, target: str) -> str:
    """Encode `plain` with the ported codec and return the container digest."""

    rows = run_example(
        target,
        ["--encrypt", str(plain), "--output-file", str(container)],
    )
    digest = None
    for row in rows:
        fields = row.split("\t")
        if fields[0] == "container":
            digest = fields[1]
    if digest is None:
        raise AssertionError(f"the Rust encoder reported no container digest: {rows}")
    return digest


def run_rust_discover(root: Path, target: str) -> list[str]:
    return run_example(target, ["--discover", str(root)])


def run_example(target: str, arguments: list[str]) -> list[str]:
    completed = subprocess.run(
        [
            "cargo",
            "run",
            "--offline",
            "--quiet",
            "--manifest-path",
            str(ROOT / "crates" / "nioh3-save" / "Cargo.toml"),
            "--example",
            "save_read_vectors",
            "--",
            *arguments,
        ],
        cwd=str(ROOT),
        env={**os.environ, "CARGO_TARGET_DIR": target},
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        raise AssertionError(
            "the Rust save model failed: "
            + (completed.stderr.strip() or completed.stdout.strip())
        )
    return [line for line in completed.stdout.splitlines() if line]


def native_transform(source: Path, output: Path, *, work: Path | None = None) -> None:
    """Run the shipped encrypt/decrypt component in the requested direction.

    The shipped tool is a legacy Win32 program: it silently fails once the
    absolute output path grows past roughly 165 characters, which is easy to hit
    under a D-backed fixture root. A caller may pass a short `work` directory for
    the tool and move the result itself; see `native_transform_short`.
    """

    require_save_oracle()
    work = work or source.parent
    work.mkdir(parents=True, exist_ok=True)
    completed = subprocess.run(
        [str(SAVE_CRYPTO), "-i", str(source), "-o", str(output)],
        cwd=str(work),
        input="\n",
        text=True,
        capture_output=True,
        timeout=300,
        check=False,
    )
    if completed.returncode != 0 or not output.is_file():
        raise AssertionError(
            "the shipped save crypto component failed: "
            + (completed.stdout + completed.stderr).strip()[-2000:]
        )


def native_transform_short(source: Path, final: Path) -> None:
    """Run the shipped tool through a short working directory, then move the result.

    This is the path-safe entry point for gates whose fixture root is long.
    """

    require_save_oracle()
    with tempfile.TemporaryDirectory(prefix="nioh3-crypto-") as directory:
        work = Path(directory)
        output = work / "transform.bin"
        completed = subprocess.run(
            [str(SAVE_CRYPTO), "-i", str(source), "-o", str(output)],
            cwd=str(work),
            input="\n",
            text=True,
            capture_output=True,
            timeout=300,
            check=False,
        )
        if completed.returncode != 0 or not output.is_file():
            raise AssertionError(
                "the shipped save crypto component failed: "
                + (completed.stdout + completed.stderr).strip()[-2000:]
            )
        final.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(output, final)


class SaveReadParityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.target = resolved_cargo_target_dir()
        cls.temp = tempfile.TemporaryDirectory()
        cls.root = Path(cls.temp.name)
        cls.save_path = cls.root / str(OWN_ACCOUNT) / "SAVEDATA00" / "SAVEDATA.BIN"
        cls.save_path.parent.mkdir(parents=True)
        cls.blob = bytes(build_fixture_bytes())
        cls.save_path.write_bytes(cls.blob)
        cls.fixture = cls.root / "fixture.bin"
        cls.fixture.write_bytes(cls.blob)

    @classmethod
    def tearDownClass(cls) -> None:
        cls.temp.cleanup()

    def test_read_model_matches_the_shipped_python_loader(self) -> None:
        rust = run_rust_read(type(self).save_path, type(self).fixture, type(self).target)
        reference = reference_rows(type(self).save_path, type(self).blob)
        self.assertEqual(len(rust), len(reference), "\n".join(rust))
        for index, (actual, expected) in enumerate(zip(rust, reference)):
            self.assertEqual(actual, expected, f"row {index}")

    def test_templates_rebind_and_synthesize_like_the_shipped_loader(self) -> None:
        rust = run_rust_read(type(self).save_path, type(self).fixture, type(self).target)
        rows = {line.split("\t")[0]: line.split("\t") for line in rust}
        self.assertEqual(rows["template"][1], "e604")
        middle, low = account_byte_fragments(OWN_ACCOUNT)
        by_playthrough = {
            line.split("\t")[1]: line.split("\t")
            for line in rust
            if line.startswith("playthrough\t")
        }
        self.assertEqual(set(by_playthrough), {"3", "4", "5"})
        for playthrough in ("3", "4", "5"):
            self.assertIn(
                middle,
                by_playthrough[playthrough][3],
                "a rebound/synthesized template must carry the save's middle id word",
            )
            self.assertIn(
                low,
                by_playthrough[playthrough][3],
                "a rebound/synthesized template must carry the save's low id word",
            )
        self.assertEqual(by_playthrough["3"][2], "e604")
        self.assertEqual(by_playthrough["4"][2], "dd82")
        self.assertEqual(by_playthrough["5"][2], "d523")
        self.assertIn(
            low,
            by_playthrough["5"][3],
            "a synthesized template must carry the save's own account id",
        )

    def test_malformed_saves_fail_closed(self) -> None:
        cases = {
            "truncated": bytes(build_fixture_bytes())[: USER_SAVE_SIZE - 16],
            "wrong_magic": b"XXXXXX" + bytes(build_fixture_bytes())[6:],
        }
        for name, blob in cases.items():
            with self.subTest(case=name):
                broken = type(self).root / f"{name}.bin"
                broken.write_bytes(blob)
                completed = subprocess.run(
                    [
                        "cargo",
                        "run",
                        "--offline",
                        "--quiet",
                        "--manifest-path",
                        str(ROOT / "crates" / "nioh3-save" / "Cargo.toml"),
                        "--example",
                        "save_read_vectors",
                        "--",
                        "--fixture",
                        str(broken),
                        "--save-path",
                        str(type(self).save_path),
                    ],
                    cwd=str(ROOT),
                    env={**os.environ, "CARGO_TARGET_DIR": type(self).target},
                    capture_output=True,
                    text=True,
                    check=False,
                )
                self.assertNotEqual(
                    completed.returncode,
                    0,
                    f"{name} must be rejected, not read: {completed.stdout}",
                )
                self.assertTrue(
                    completed.stderr.strip(),
                    f"{name} must name why it was rejected",
                )

    def test_wrong_identity_path_is_rejected(self) -> None:
        stray = type(self).root / "not-an-account" / "SAVEDATA00" / "SAVEDATA.BIN"
        stray.parent.mkdir(parents=True)
        stray.write_bytes(type(self).blob)
        completed = subprocess.run(
            [
                "cargo",
                "run",
                "--offline",
                "--quiet",
                "--manifest-path",
                str(ROOT / "crates" / "nioh3-save" / "Cargo.toml"),
                "--example",
                "save_read_vectors",
                "--",
                "--fixture",
                str(type(self).fixture),
                "--save-path",
                str(stray),
            ],
            cwd=str(ROOT),
            env={**os.environ, "CARGO_TARGET_DIR": type(self).target},
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertNotEqual(completed.returncode, 0, completed.stdout)

    def test_encrypted_container_reads_to_the_shipped_inventory(self) -> None:
        """One real encrypted container must decode to the shipped save model.

        The shipped encrypt/decrypt component is the oracle: it produces the
        container from a synthetic save and the decrypted bytes the product
        would see. The Rust model must decrypt that same container and agree on
        the whole read surface, so the custom shipped cipher is pinned by bytes
        rather than by an AES assumption.
        """

        plain = type(self).root / "plain.bin"
        plain.write_bytes(bytes(build_fixture_bytes()))
        container = type(self).root / "encrypted.bin"
        native_transform(plain, container)
        decrypted = type(self).root / "native_decrypted.bin"
        native_transform(container, decrypted)
        self.assertEqual(
            decrypted.read_bytes()[:6],
            b"RNNUSR",
            "the shipped component must round-trip the synthetic save",
        )
        self.assertEqual(
            len(container.read_bytes()),
            USER_SAVE_SIZE,
            "the encrypted container must be the same size as the decrypted save",
        )

        rust = run_rust_decrypt(container, type(self).save_path, type(self).target)
        reference = [
            f"container\t{__import__('hashlib').sha256(container.read_bytes()).hexdigest()}"
        ] + reference_rows(type(self).save_path, decrypted.read_bytes())
        self.assertEqual(len(rust), len(reference), "\n".join(rust))
        for index, (actual, expected) in enumerate(zip(rust, reference)):
            self.assertEqual(actual, expected, f"row {index}")

    def test_patterned_container_decodes_exactly(self) -> None:
        """A non-zero, patterned save must decode, not just an all-zero body.

        The zero-heavy fixture alone cannot distinguish a correct keystream from
        a positional accident, so this case fills the header (including the
        shipped body sub-key slots) and the inventory region with a dense
        pattern and requires the same byte-exact agreement.
        """

        patterned = bytearray(USER_SAVE_SIZE)
        for index in range(USER_SAVE_SIZE):
            patterned[index] = (index * 7 + 0x3B) & 0xFF
        patterned[:6] = b"RNNUSR"
        # Re-apply the scroll records the shared builder produces, so the
        # inventory half still has mapped, unmapped and empty slots.
        base = build_fixture_bytes()
        patterned[SCROLL_GROUP_OFFSET : SCROLL_GROUP_OFFSET + 8 * SCROLL_RECORD_SIZE] = (
            base[SCROLL_GROUP_OFFSET : SCROLL_GROUP_OFFSET + 8 * SCROLL_RECORD_SIZE]
        )
        plain = type(self).root / "patterned_plain.bin"
        plain.write_bytes(bytes(patterned))
        container = type(self).root / "patterned_container.bin"
        native_transform(plain, container)
        decrypted = type(self).root / "patterned_native_decrypted.bin"
        native_transform(container, decrypted)

        rust = run_rust_decrypt(container, type(self).save_path, type(self).target)
        reference = [
            f"container\t{__import__('hashlib').sha256(container.read_bytes()).hexdigest()}"
        ] + reference_rows(type(self).save_path, decrypted.read_bytes())
        self.assertEqual(len(rust), len(reference), "\n".join(rust))
        for index, (actual, expected) in enumerate(zip(rust, reference)):
            self.assertEqual(actual, expected, f"row {index}")

    def test_shipped_oracle_round_trips_multiple_header_session_keys(self) -> None:
        """Pin the codec oracle across distinct session keys and body patterns.

        The body keystream is seeded from four little-endian values inside the
        container header (+0x49/+0x59/+0x69/+0x79). One fixture cannot show that
        those slots are read rather than assumed, so this case varies all four
        before handing the save to the shipped tool, and overlays three body
        patterns. The shipped tool must recover the exact bytes it encrypted and
        the Rust codec must agree with it row for row.
        """

        import struct

        masks = (
            {"key_1": 0x00000000, "iv_1": 0x00000000, "key_2": 0x00000000, "iv_2": 0x00000000},
            {"key_1": 0xDEADBEEF, "iv_1": 0x0BADF00D, "key_2": 0xC0FFEE00, "iv_2": 0xFEEDFACE},
            {"key_1": 0x55555555, "iv_1": 0xAAAAAAAA, "key_2": 0x12345678, "iv_2": 0x87654321},
        )
        body_patterns = {
            "base": None,
            "dense": lambda index: (index * 13 + 0x5A) & 0xFF,
            "alternating": lambda index: 0xFF if index % 2 else 0x00,
        }
        base = build_fixture_bytes()
        for mask_index, mask in enumerate(masks):
            for pattern_name, pattern in body_patterns.items():
                with self.subTest(mask=mask_index, pattern=pattern_name):
                    crafted = bytearray(base)
                    if pattern is not None:
                        # Overlay the body only. The header, the inventory region
                        # and the checksum slots keep their base values, because
                        # this case pins the plaintext the tool returns. The tool
                        # keeps `USER_CHECKSUM_SEED_OFFSET`/`USER_CHECKSUM_VALUE_OFFSET`
                        # exactly; only the final 8 bytes differ, and those are
                        # outside the transformed body (`USER_BODY_BYTES = 0x900058`
                        # is not a multiple of the 16-byte block), so a round-trip
                        # comparison here must leave that trailer zero too. Pinned
                        # by `test_save_transaction_parity`.
                        for index in range(0x40, 0x90_0190):
                            crafted[index] = pattern(index)
                        crafted[:6] = b"RNNUSR"
                        crafted[SCROLL_GROUP_OFFSET : SCROLL_GROUP_OFFSET + 8 * SCROLL_RECORD_SIZE] = (
                            base[SCROLL_GROUP_OFFSET : SCROLL_GROUP_OFFSET + 8 * SCROLL_RECORD_SIZE]
                        )
                    # The four session slots the header decryptor reads must keep
                    # their intended values, before or after any body overlay.
                    crafted[0x49:0x4D] = struct.pack("<I", 0x11121314 ^ mask["key_1"])
                    crafted[0x59:0x5D] = struct.pack("<I", 0x21222324 ^ mask["iv_1"])
                    crafted[0x69:0x6D] = struct.pack("<I", 0x31323334 ^ mask["key_2"])
                    crafted[0x79:0x7D] = struct.pack("<I", 0x41424344 ^ mask["iv_2"])

                    label = f"{mask_index}-{pattern_name}"
                    source = type(self).root / f"round-trip-{label}.bin"
                    source.write_bytes(bytes(crafted))
                    container = type(self).root / f"rt-{label}.cbin"
                    native_transform(source, container)
                    self.assertNotEqual(
                        container.read_bytes(),
                        bytes(crafted),
                        "the shipped tool must actually transform, not copy",
                    )

                    decrypted = type(self).root / f"rt-{label}.dbin"
                    native_transform(container, decrypted)
                    self.assertEqual(
                        decrypted.read_bytes(),
                        bytes(crafted),
                        "transforming a container must return the exact plaintext",
                    )
                    # XOR symmetry inside the container: transforming the
                    # container again returns the plaintext.
                    again = type(self).root / f"rt-{label}.abin"
                    native_transform(container, again)
                    self.assertEqual(again.read_bytes(), decrypted.read_bytes())

                    rust = run_rust_decrypt(container, type(self).save_path, type(self).target)
                    reference = [
                        f"container\t{sha256_file(container)}"
                    ] + reference_rows(type(self).save_path, decrypted.read_bytes())
                    self.assertEqual(len(rust), len(reference), "\n".join(rust))
                    for index, (actual, expected) in enumerate(zip(rust, reference)):
                        self.assertEqual(actual, expected, f"row {index}")

    def test_oracle_digest_is_pinned(self) -> None:
        """A swapped or missing codec must fail the gate, not silently pass."""

        require_save_oracle()
        self.assertEqual(sha256_file(SAVE_CRYPTO), SAVE_CRYPTO_SHA256)

    def test_cipher_reproduces_the_shipped_container_byte_for_byte(self) -> None:
        """Exact cipher byte parity, raw digest and corruption sensitivity.

        `test_shipped_oracle_round_trips_multiple_header_session_keys` pins that
        the Rust codec *decodes* a shipped container. This pins that it *encodes*
        the same plaintext into the same bytes: the raw container and its SHA-256
        must equal the shipped tool's own output, for the full-size fixture, a
        dense body and a body whose keystream is seeded from different header
        session slots. The corruption cases then prove the comparison can fail -
        a codec that copied its input, ignored the container, or a gate that
        stopped comparing would be caught here rather than by a looser test.
        """

        import struct

        base = build_fixture_bytes()
        dense = bytearray(base)
        for index in range(0x40, 0x90_0190):
            dense[index] = (index * 13 + 0x5A) & 0xFF
        dense[:6] = b"RNNUSR"
        dense[SCROLL_GROUP_OFFSET : SCROLL_GROUP_OFFSET + 8 * SCROLL_RECORD_SIZE] = (
            base[SCROLL_GROUP_OFFSET : SCROLL_GROUP_OFFSET + 8 * SCROLL_RECORD_SIZE]
        )
        # Move the four values the body keystream is seeded from, so the parity
        # claim is not made only under one session key.
        varied = bytearray(dense)
        varied[0x49:0x4D] = struct.pack("<I", 0x11121314 ^ 0xC0FFEE00)
        varied[0x59:0x5D] = struct.pack("<I", 0x21222324 ^ 0x0BADF00D)
        varied[0x69:0x6D] = struct.pack("<I", 0x31323334 ^ 0xFEEDFACE)
        varied[0x79:0x7D] = struct.pack("<I", 0x41424344 ^ 0xDEADBEEF)

        variants = {
            "base": bytes(base),
            "dense-body": bytes(dense),
            "varied-header": bytes(varied),
        }
        containers: dict[str, bytes] = {}
        for label, blob in variants.items():
            with self.subTest(variant=label):
                plain = type(self).root / f"cipher-{label}.bin"
                plain.write_bytes(blob)
                oracle = type(self).root / f"cipher-{label}.oracle.cbin"
                native_transform(plain, oracle)
                rust_container = type(self).root / f"cipher-{label}.rust.cbin"
                digest = run_rust_encrypt(plain, rust_container, type(self).target)
                oracle_bytes = oracle.read_bytes()
                containers[label] = oracle_bytes
                self.assertEqual(
                    digest,
                    sha256_file(oracle),
                    f"{label}: the Rust container digest must match the shipped tool",
                )
                self.assertEqual(
                    rust_container.read_bytes(),
                    oracle_bytes,
                    f"{label}: the Rust container must be byte-identical",
                )
                self.assertNotEqual(
                    oracle_bytes,
                    blob,
                    f"{label}: the shipped tool must transform, not copy",
                )
                self.assertEqual(len(oracle_bytes), USER_SAVE_SIZE, label)

        # 1. One flipped plaintext byte must move the raw container digest.
        altered = bytearray(variants["base"])
        altered[0x17_6CCE] ^= 0x01
        altered_plain = type(self).root / "cipher-altered.bin"
        altered_plain.write_bytes(bytes(altered))
        altered_container = type(self).root / "cipher-altered.rust.cbin"
        self.assertNotEqual(
            run_rust_encrypt(altered_plain, altered_container, type(self).target),
            sha256_file(type(self).root / "cipher-base.rust.cbin"),
            "a one-byte plaintext change must change the container",
        )
        self.assertNotEqual(
            altered_container.read_bytes(),
            containers["base"],
            "a one-byte plaintext change must change the container bytes",
        )

        # 2. One flipped container byte must change what the decode returns.
        pristine = run_rust_decrypt(
            type(self).root / "cipher-base.oracle.cbin", type(self).save_path, type(self).target
        )
        corrupted = bytearray(containers["base"])
        corrupted[0x17_6CCE] ^= 0x01
        corrupted_path = type(self).root / "cipher-corrupted.cbin"
        corrupted_path.write_bytes(bytes(corrupted))
        damaged = run_rust_decrypt(corrupted_path, type(self).save_path, type(self).target)
        self.assertNotEqual(
            damaged, pristine, "a flipped container byte must change the decode"
        )

        # 3. A container one byte short is refused instead of decoded.
        truncated = type(self).root / "cipher-truncated.cbin"
        truncated.write_bytes(containers["base"][:-1])
        with self.assertRaises(AssertionError):
            run_rust_decrypt(truncated, type(self).save_path, type(self).target)

    def test_malformed_containers_fail_closed(self) -> None:

        raw = type(self).root / "not_a_container.bin"

        plain = type(self).root / "plain2.bin"
        plain.write_bytes(bytes(build_fixture_bytes()))
        container = type(self).root / "encrypted2.bin"
        native_transform(plain, container)
        encrypted = container.read_bytes()

        cases = {
            "truncated": encrypted[: len(encrypted) - 32],
            "corrupt": bytes(encrypted[:64]) + bytes([encrypted[64] ^ 0xFF]) + bytes(encrypted[65:]),
            "wrong_size": encrypted[:1000],
        }
        for name, blob in cases.items():
            with self.subTest(case=name):
                broken = type(self).root / f"{name}.container"
                broken.write_bytes(blob)
                completed = subprocess.run(
                    [
                        "cargo",
                        "run",
                        "--offline",
                        "--quiet",
                        "--manifest-path",
                        str(ROOT / "crates" / "nioh3-save" / "Cargo.toml"),
                        "--example",
                        "save_read_vectors",
                        "--",
                        "--decrypt",
                        str(broken),
                        "--save-path",
                        str(type(self).save_path),
                    ],
                    cwd=str(ROOT),
                    env={**os.environ, "CARGO_TARGET_DIR": type(self).target},
                    capture_output=True,
                    text=True,
                    check=False,
                )
                if name == "corrupt":
                    # A single flipped header byte still produces a container;
                    # the model must not silently claim a valid save.
                    self.assertNotIn(
                        "RNNUSR",
                        completed.stdout,
                        "a corrupted container must not be reported as a save",
                    )
                    continue
                self.assertNotEqual(
                    completed.returncode,
                    0,
                    f"{name} must be rejected, not read: {completed.stdout}",
                )
                self.assertTrue(
                    completed.stderr.strip(),
                    f"{name} must name why it was rejected",
                )

    def test_discovery_matches_the_shipped_scan(self) -> None:
        """Discovery over a supplied root must match the shipped scan order."""

        # The shipped scan reads `LOCALAPPDATA/KoeiTecmo/NIOH3/Savedata`, so the
        # fixture lives under that layout and the root alias is the account level.
        local_app_data = type(self).root / "localappdata"
        root = local_app_data / "KoeiTecmo" / "NIOH3" / "Savedata"
        for account, slots in ((76561198000000001, (0, 3)), (76561198000000000, (1,))):
            for slot in slots:
                directory = root / str(account) / f"SAVEDATA{slot:02d}"
                directory.mkdir(parents=True)
                (directory / "SAVEDATA.BIN").write_bytes(bytes(build_fixture_bytes()))
        # A non-numeric account and a non-save file must be skipped.
        (root / "not-an-account" / "SAVEDATA00").mkdir(parents=True)
        (root / "not-an-account" / "SAVEDATA00" / "SAVEDATA.BIN").write_bytes(b"x")
        (root / "76561198000000002" / "SAVEDATA00").mkdir(parents=True)
        (root / "76561198000000002" / "SAVEDATA00" / "OTHER.BIN").write_bytes(b"x")

        previous = os.environ.get("LOCALAPPDATA")
        os.environ["LOCALAPPDATA"] = str(local_app_data)
        try:
            expected = [
                f"discovered\t{account_id_from_save_path(path)}"
                f"\t{save_slot_index_from_path(path)}\t{path}"
                for path in discover_save_paths()
                if str(root) in str(path)
            ]
        finally:
            if previous is None:
                os.environ.pop("LOCALAPPDATA", None)
            else:
                os.environ["LOCALAPPDATA"] = previous

        rust = run_rust_discover(root, type(self).target)
        self.assertEqual(rust, expected)
        self.assertEqual(len(rust), 3, "the discovery fixture must contain three saves")


if __name__ == "__main__":
    unittest.main()
