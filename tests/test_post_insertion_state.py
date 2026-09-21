"""Focused regression: direct save installation writes the post-insertion state.

The game's own pickup path leaves a newly inserted scroll in the lifecycle word
`0x06800082` at record `+0x18`..`+0x1B`: the builder's `0x02800002` descriptor
state plus the engine insertion bits `0x04000080`. A direct save write must
reproduce that inventory state instead of inheriting the donor template's
reveal/seen/owned flags. These cases pin the installed word, the preserved
generation bytes and the untouched neighbouring records for rarity 3 and 4.
"""
from __future__ import annotations

import struct
import tempfile
import unittest
from pathlib import Path

from emaki_exchange import SCROLL_RECORD_SIZE, USER_SAVE_SIZE
from tests.test_beta_editor import (
    SCROLL_GROUP_OFFSET,
    TEST_ACCOUNT_ID,
    make_record,
)
from nioh3_scroll_editor.effect_sequence import generate_ng3_certified_effect_sequence
from nioh3_scroll_editor.models import ScrollCandidate
from nioh3_scroll_editor.savegame import (
    POST_INSERTION_FLAG_WORD,
    SCROLL_GENERATION_SERIAL_OFFSET,
    SCROLL_INVENTORY_KEY_OFFSET,
    SCROLL_SLOT_COUNT,
    SaveInstaller,
    materialize_effect_sequence_candidate,
    write_post_insertion_state,
)

# Observed lifecycle word of a real save record after its first reveal
# (`live_first_reveal_pc_v201.json`, R3 seed 10030565).
REVEALED_DONOR_WORD = 0x0F800080
# Engine insertion result before the item is viewed (`0x02800002 | 0x04000080`).
NEW_ITEM_BIT = 0x02
INSERTION_BITS = 0x04000080
REVEAL_BITS = 0x0000_0900  # +0x1B bits 0x01|0x08


class FixtureCrypto:
    """Synthetic container codec: `ENC` prefix instead of the game cipher."""

    @staticmethod
    def decrypt(source, output):
        data = source.read_bytes()
        assert data.startswith(b'ENC')
        output.write_bytes(data[3:])

    @staticmethod
    def encrypt(source, output):
        output.write_bytes(b'ENC' + source.read_bytes())


class PostInsertionStateTests(unittest.TestCase):
    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)

    def build_installer(self, donor_word: int) -> tuple[SaveInstaller, bytes]:
        """One synthetic save whose NG3 donor record carries `donor_word`."""

        folder = self.root / str(TEST_ACCOUNT_ID) / 'SAVEDATA00'
        folder.mkdir(parents=True, exist_ok=True)
        data = bytearray(USER_SAVE_SIZE)
        data[:6] = b'RNNUSR'
        donor = bytearray(make_record(seed=241719428, account_id=TEST_ACCOUNT_ID))
        struct.pack_into("<H", donor, 0, 0xE604)
        struct.pack_into("<I", donor, 0x28, 40)
        struct.pack_into("<I", donor, 0x18, donor_word)
        data[
            SCROLL_GROUP_OFFSET : SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE
        ] = donor
        path = folder / 'SAVEDATA.BIN'
        path.write_bytes(b'ENC' + bytes(data))
        installer = SaveInstaller(
            save_path=path,
            crypto=FixtureCrypto(),
            state_root=self.root / 'state',
        )
        return installer, bytes(donor)

    def materialize(self, installer: SaveInstaller, rarity: int, seed: int):
        inventory = installer.capture_inventory()
        preview = ScrollCandidate.from_effect_sequence(
            generate_ng3_certified_effect_sequence(seed, rarity=rarity, level=180)
        )
        self.assertTrue(preview.can_materialize_for_install)
        return materialize_effect_sequence_candidate(
            inventory,
            preview,
            level=180,
            recommended_level=183,
            transfer_count=0,
        )

    def decrypted(self, installer: SaveInstaller) -> bytes:
        output = self.root / 'readback.bin'
        installer.crypto.decrypt(installer.save_path, output)
        return output.read_bytes()

    def records(self, blob: bytes) -> list[bytes]:
        return [
            blob[
                SCROLL_GROUP_OFFSET
                + index * SCROLL_RECORD_SIZE : SCROLL_GROUP_OFFSET
                + (index + 1) * SCROLL_RECORD_SIZE
            ]
            for index in range(SCROLL_SLOT_COUNT)
        ]

    def assert_post_insertion_state(self, installed: bytes) -> None:
        word = struct.unpack_from("<I", installed, 0x18)[0]
        self.assertEqual(word, POST_INSERTION_FLAG_WORD, installed[0x18:0x1C].hex())
        self.assertEqual(installed[0x18] & NEW_ITEM_BIT, NEW_ITEM_BIT)
        self.assertEqual(
            word & INSERTION_BITS, INSERTION_BITS, "engine insertion bits are missing"
        )
        self.assertEqual(word & REVEAL_BITS, 0, "the installed record is already revealed")

    def test_install_replaces_revealed_donor_word_for_rarity_3_and_4(self) -> None:
        for rarity, seed in ((3, 10030609), (4, 43723117)):
            with self.subTest(rarity=rarity):
                installer, donor = self.build_installer(REVEALED_DONOR_WORD)
                materialized = self.materialize(installer, rarity, seed)
                self.assertEqual(
                    struct.unpack_from("<I", materialized.record, 0x18)[0],
                    REVEALED_DONOR_WORD,
                    "the materializer inherits donor state; the install boundary fixes it",
                )

                result = installer.install(materialized.record, transfer_count=0)
                records = self.records(self.decrypted(installer))
                installed = records[result.slot_index]

                self.assert_post_insertion_state(installed)
                self.assertEqual(records[0], donor, "the donor record must not change")
                self.assertEqual(
                    installed[0x34:0xDC],
                    materialized.record[0x34:0xDC],
                    "generated effect bytes must be preserved",
                )
                inventory_key = struct.unpack_from(
                    "<I", installed, SCROLL_INVENTORY_KEY_OFFSET
                )[0]
                generation_serial = struct.unpack_from(
                    "<I", installed, SCROLL_GENERATION_SERIAL_OFFSET
                )[0]
                self.assertNotEqual(inventory_key, 0)
                self.assertNotEqual(generation_serial, 0)
                self.assertEqual(installed[0x30], rarity)
                self.assertEqual(installed[0x31], rarity)
                occupied = [
                    index
                    for index, record in enumerate(records)
                    if record[:2] != b'\x00\x00'
                ]
                self.assertEqual(occupied, [0, result.slot_index])

    def test_install_normalizes_unrevealed_donor_state_too(self) -> None:
        installer, donor = self.build_installer(0x06800080)
        materialized = self.materialize(installer, 4, 36526331)
        result = installer.install(materialized.record, transfer_count=0)
        records = self.records(self.decrypted(installer))
        installed = records[result.slot_index]
        self.assert_post_insertion_state(installed)
        self.assertEqual(installed[0x34:0xDC], materialized.record[0x34:0xDC])
        self.assertEqual(records[0], donor)

    def test_batch_install_writes_the_same_state(self) -> None:
        installer, donor = self.build_installer(REVEALED_DONOR_WORD)
        first = self.materialize(installer, 3, 10030609)
        second = self.materialize(installer, 4, 43723117)
        result = installer.install_many(
            [first.record, second.record],
            action="v2-cart-install",
        )
        records = self.records(self.decrypted(installer))
        self.assertEqual(list(result.slot_indices), [1, 2])
        for slot_index, materialized in zip(result.slot_indices, (first, second)):
            installed = records[slot_index]
            self.assert_post_insertion_state(installed)
            self.assertEqual(installed[0x34:0xDC], materialized.record[0x34:0xDC])
        self.assertEqual(records[0], donor)
        occupied = [
            index for index, record in enumerate(records) if record[:2] != b'\x00\x00'
        ]
        self.assertEqual(occupied, [0, 1, 2])

    def test_write_post_insertion_state_preserves_every_other_byte(self) -> None:
        candidate = bytearray(make_record(seed=7, account_id=TEST_ACCOUNT_ID))
        struct.pack_into("<I", candidate, 0x18, REVEALED_DONOR_WORD)
        written = write_post_insertion_state(bytes(candidate))
        self.assertEqual(
            written[0x18:0x1C], struct.pack("<I", POST_INSERTION_FLAG_WORD)
        )
        self.assertEqual(written[:0x18], bytes(candidate)[:0x18])
        self.assertEqual(written[0x1C:], bytes(candidate)[0x1C:])
        with self.assertRaises(ValueError):
            write_post_insertion_state(bytearray(SCROLL_RECORD_SIZE - 1))


if __name__ == '__main__':
    unittest.main()
