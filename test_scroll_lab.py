import tempfile
import unittest
import json
from pathlib import Path
from unittest.mock import patch

from scroll_lab import (
    ByteRange,
    SCROLL_RECORD_SIZE,
    USER_CHECKSUM_BODY_END,
    analyze_experiment,
    capture_experiment_stage,
    changed_ranges,
    create_experiment,
    compute_user_checksum,
    diff_report,
    file_kind,
    merge_ranges,
    paired_u16_runs,
    parse_scroll_record,
    prepare_decrypted_save_for_encryption,
    transplant_effect_slot,
)


class ScrollLabTests(unittest.TestCase):
    def test_diff_and_merge(self) -> None:
        before = bytes([0, 0, 0, 0, 0, 0, 0])
        after = bytes([0, 1, 2, 0, 0, 3, 0])
        exact = changed_ranges(before, after)
        self.assertEqual(exact, [ByteRange(1, 3), ByteRange(5, 6)])
        self.assertEqual(merge_ranges(exact, 2), [ByteRange(1, 6)])

    def test_size_mismatch_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            before = root / "before.bin"
            after = root / "after.bin"
            before.write_bytes(b"a")
            after.write_bytes(b"ab")
            with self.assertRaises(ValueError):
                diff_report(before, after, 0)

    def test_record_run_scanner(self) -> None:
        data = bytearray(48)
        for offset, value in ((4, 0x1234), (12, 0x5678), (20, 0x9ABC)):
            data[offset : offset + 4] = value.to_bytes(2, "little") * 2
        runs = paired_u16_runs(bytes(data), record_size=8, start=0, end=len(data))
        self.assertEqual(runs[0]["start"], 4)
        self.assertEqual(runs[0]["count"], 3)

    def test_file_kind_recognizes_decrypted_nioh3_data(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "save.bin"
            path.write_bytes(b"RNNUSR" + bytes(10))
            self.assertEqual(file_kind(path), "decrypted_nioh3_user")

    def test_scroll_record_parser_uses_effect_id_at_slot_plus_four(self) -> None:
        record = bytearray(SCROLL_RECORD_SIZE)
        record[6:8] = (180).to_bytes(2, "little")
        record[8:10] = (180).to_bytes(2, "little")
        record[0x20:0x24] = (1).to_bytes(4, "little")
        record[0x34:0x38] = (0x56CE).to_bytes(4, "little")
        record[0x38:0x3C] = (0x47BC).to_bytes(4, "little")
        record[0x3C:0x40] = (68).to_bytes(4, "little")
        record[0xDC:0xE0] = (4).to_bytes(4, "little")

        parsed = parse_scroll_record(bytes(record))

        self.assertEqual(parsed["scroll_id"], 1)
        self.assertEqual(parsed["transfer_count"], 4)
        self.assertEqual(parsed["effects"][0]["prefix"], 0x56CE)
        self.assertEqual(parsed["effects"][0]["effect_id"], 0x47BC)
        self.assertEqual(parsed["effects"][0]["value"], 68)

    def test_scroll_record_parser_rejects_wrong_size(self) -> None:
        with self.assertRaisesRegex(ValueError, "exactly 0xe8"):
            parse_scroll_record(bytes(SCROLL_RECORD_SIZE - 1))

    def test_zero_checksum_body_with_even_block_count_is_zero(self) -> None:
        self.assertEqual(compute_user_checksum(bytes(0x900000), 0x12345678), 0)

    def test_effect_transplant_copies_whole_slot_and_patches_checksum(self) -> None:
        data = bytearray(USER_CHECKSUM_BODY_END + 8)
        data[:6] = b"RNNUSR"
        destination_record = 0x200
        donor_record = 0x400
        destination_slot = destination_record + 0x34
        donor_slot = donor_record + 0x34
        data[destination_slot : destination_slot + 0x18] = bytes([0x11]) * 0x18
        data[donor_slot : donor_slot + 0x18] = bytes([0x22]) * 0x18

        edited, report = transplant_effect_slot(
            bytes(data), destination_record, donor_record, 1, 1
        )

        self.assertEqual(edited[destination_slot : destination_slot + 0x18], bytes([0x22]) * 0x18)
        self.assertEqual(report["changed_effect_bytes"], 0x18)

    def test_encryption_preparation_only_clears_body_crypto_material(self) -> None:
        data = bytearray([0xAA]) * 0x100
        data[:6] = b"RNNUSR"
        prepared = prepare_decrypted_save_for_encryption(bytes(data))
        self.assertEqual(prepared[0x49:0x89], bytes(0x40))
        self.assertEqual(prepared[:0x49], bytes(data[:0x49]))
        self.assertEqual(prepared[0x89:], bytes(data[0x89:]))

    @patch("scroll_lab.nioh3_is_running", return_value=False)
    def test_experiment_capture_and_analysis(self, _running) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            experiment = root / "experiment"
            create_experiment(experiment, "2.00.02")

            before = root / "before.bin"
            after = root / "after.bin"
            before.write_bytes(b"RNNUSR" + bytes(10))
            changed = bytearray(before.read_bytes())
            changed[8] = 1
            after.write_bytes(changed)

            capture_experiment_stage(
                before, experiment, "source_before_obtain", "source", "clean baseline"
            )
            capture_experiment_stage(
                after, experiment, "source_after_obtain", "source", "first scroll"
            )
            manifest_path = experiment / "experiment.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["stages"]["source_before_obtain"]["sha256"] = manifest["stages"][
                "source_before_obtain"
            ]["sha256"].upper()
            manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
            report = analyze_experiment(experiment, 0)
            self.assertFalse(report["complete"])
            self.assertEqual(report["comparisons"][0]["changed_bytes"], 1)
            self.assertTrue(report["comparisons"][0]["same_account"])

    @patch("scroll_lab.nioh3_is_running", return_value=False)
    def test_experiment_rejects_opaque_input_without_partial_capture(self, _running) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            experiment = root / "experiment"
            create_experiment(experiment, "2.00.02")
            opaque = root / "SAVEDATA.BIN"
            opaque.write_bytes(bytes(16))

            with self.assertRaisesRegex(ValueError, "requires a decrypted Nioh 3 user save"):
                capture_experiment_stage(
                    opaque, experiment, "source_before_obtain", "source", ""
                )
            self.assertFalse((experiment / "captures").exists())


if __name__ == "__main__":
    unittest.main()
