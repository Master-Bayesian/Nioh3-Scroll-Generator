import struct
import tempfile
import unittest
from pathlib import Path

from emaki_exchange import (
    EFFECT_REGION_SIZE,
    SCROLL_RECORD_SIZE,
    USER_CHECKSUM_BODY_END,
    USER_CHECKSUM_BODY_START,
    USER_CHECKSUM_SEED_OFFSET,
    USER_CHECKSUM_VALUE_OFFSET,
    USER_SAVE_SIZE,
    account_id_from_record,
    compute_user_checksum,
    describe_record_for_report,
    insert_scroll_record,
    pack_value,
    patch_canonical_inputs,
    predict_outbound_tuple,
    require_decrypted_user_save,
    require_new_output,
    unpack_value,
)


OWN_ACCOUNT_ID = 0x1111222233334444
DONOR_ACCOUNT_ID = 0x5555666677778888


OWN_HEADER = bytes.fromhex(
    "82 1e 11 11 22 22 a0 00 a0 00 00 00 00 00 00 01 "
    "b5 00 b5 00 44 44 33 33 82 00 c0 06 c6 3c 00 00 "
    "01 00 00 00 00 00 00 00 23 59 0b 00 00 00 00 00 "
    "03 03 00 04"
)

DONOR_HEADER = bytes.fromhex(
    "82 1e 55 55 66 66 b4 00 b4 00 00 00 00 00 00 01 "
    "b7 00 b7 00 88 88 77 77 82 00 80 04 e2 3c 00 00 "
    "01 00 00 00 00 00 00 00 44 6a 0b 00 00 00 00 00 "
    "05 05 00 04"
)


def record_with_header(header: bytes, transfer: int) -> bytes:
    record = bytearray(SCROLL_RECORD_SIZE)
    record[: len(header)] = header
    struct.pack_into("<I", record, 0xDC, transfer)
    return bytes(record)


class EmakiExchangeTests(unittest.TestCase):
    def test_pack_roundtrip(self) -> None:
        value = pack_value(
            category=1,
            effective_level=180,
            recommended_level=183,
            flag_nibble=1,
            rarity=5,
        )
        self.assertEqual(value, 0x510B72D1)
        self.assertEqual(
            unpack_value(value),
            {
                "category": 1,
                "effective_level": 180,
                "recommended_level": 183,
                "flag_nibble": 1,
                "rarity": 5,
            },
        )

    def test_normal_id1_tuple(self) -> None:
        record = record_with_header(OWN_HEADER, 0)
        result = predict_outbound_tuple(record)
        self.assertEqual(result.random_seed, 1)
        self.assertEqual(result.exchange_count, 1)
        self.assertEqual(result.rarity, 3)
        self.assertEqual(result.effective_level, 160)
        self.assertEqual(result.recommended_level, 181)
        self.assertEqual(result.packed_value, 0x310B5281)
        self.assertEqual(result.account_id, OWN_ACCOUNT_ID)

    def test_donor_tier5_id1_tuple(self) -> None:
        record = record_with_header(DONOR_HEADER, 4)
        result = predict_outbound_tuple(record)
        self.assertEqual(result.random_seed, 1)
        self.assertEqual(result.exchange_count, 5)
        self.assertEqual(result.rarity, 5)
        self.assertEqual(result.effective_level, 180)
        self.assertEqual(result.recommended_level, 183)
        self.assertEqual(result.packed_value, 0x510B72D1)
        self.assertEqual(result.account_id, DONOR_ACCOUNT_ID)

    def test_category_four_report_does_not_invent_a_two_bit_wire_value(self) -> None:
        record = bytearray(record_with_header(DONOR_HEADER, 0))
        struct.pack_into("<H", record, 0, 0xDD82)
        report = describe_record_for_report(bytes(record))
        self.assertEqual(report["category"], 4)
        self.assertIsNone(report["packed_value"])
        self.assertFalse(report["known_exchange_encoding"])
        self.assertIn("category & 3", report["exchange_warning"])

    def test_rarity_patch_updates_authoritative_plus_31(self) -> None:
        record = record_with_header(OWN_HEADER, 0)
        edited, report = patch_canonical_inputs(record, record_offset=0, rarity=5)
        self.assertEqual(edited[0x30], 5)
        self.assertEqual(edited[0x31], 5)
        self.assertEqual(report["after"]["rarity"], 5)
        self.assertIsNone(report["checksum"])

    def test_effect_template_is_local_only(self) -> None:
        record = record_with_header(OWN_HEADER, 0)
        template = bytes([0xAA]) * EFFECT_REGION_SIZE
        edited, report = patch_canonical_inputs(
            record, record_offset=0, effect_template=template
        )
        self.assertEqual(edited[0x34:0xDC], template)
        self.assertEqual(report["before"]["packed_value"], report["after"]["packed_value"])

    def test_account_id_layout(self) -> None:
        record = record_with_header(OWN_HEADER, 0)
        self.assertEqual(account_id_from_record(record), OWN_ACCOUNT_ID)

    def test_full_user_save_checksum_is_repaired(self) -> None:
        record_offset = 0x200
        save = bytearray(USER_SAVE_SIZE)
        save[:6] = b"RNNUSR"
        save[record_offset:record_offset + SCROLL_RECORD_SIZE] = record_with_header(
            OWN_HEADER, 0
        )
        struct.pack_into("<I", save, USER_CHECKSUM_SEED_OFFSET, 0x12345678)

        edited, report = patch_canonical_inputs(
            bytes(save), record_offset=record_offset, rarity=5
        )
        stored = struct.unpack_from("<I", edited, USER_CHECKSUM_VALUE_OFFSET)[0]
        calculated = compute_user_checksum(
            edited[USER_CHECKSUM_BODY_START:USER_CHECKSUM_BODY_END],
            struct.unpack_from("<I", edited, USER_CHECKSUM_SEED_OFFSET)[0],
        )
        self.assertEqual(stored, calculated)
        self.assertEqual(report["checksum"]["new"], calculated)

    def test_cli_requires_decrypted_user_save(self) -> None:
        with self.assertRaisesRegex(ValueError, "decrypted RNNUSR"):
            require_decrypted_user_save(bytes(USER_SAVE_SIZE))
        require_decrypted_user_save(b"RNNUSR" + bytes(USER_SAVE_SIZE - 6))

    def test_output_path_cannot_replace_inputs_or_existing_files(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source.bin"
            source.write_bytes(b"source")
            with self.assertRaisesRegex(ValueError, "protected input"):
                require_new_output(source, protected=(source,))

            existing = root / "existing.bin"
            existing.write_bytes(b"existing")
            with self.assertRaises(FileExistsError):
                require_new_output(existing, protected=(source,))

            require_new_output(root / "new.bin", protected=(source,))

    def test_insert_record_requires_a_fully_zeroed_slot(self) -> None:
        record_offset = 0x200
        record = record_with_header(OWN_HEADER, 0)
        save = bytearray(USER_SAVE_SIZE)
        save[:6] = b"RNNUSR"
        struct.pack_into("<I", save, USER_CHECKSUM_SEED_OFFSET, 0x12345678)

        edited, report = insert_scroll_record(
            bytes(save), record_offset=record_offset, record=record
        )
        self.assertEqual(
            edited[record_offset:record_offset + SCROLL_RECORD_SIZE], record
        )
        self.assertEqual(report["inserted"]["random_seed"], 1)

        with self.assertRaisesRegex(ValueError, "not fully zeroed"):
            insert_scroll_record(
                edited,
                record_offset=record_offset,
                record=record,
            )


if __name__ == "__main__":
    unittest.main()
