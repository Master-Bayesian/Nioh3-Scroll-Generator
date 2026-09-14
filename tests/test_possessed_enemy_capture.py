from __future__ import annotations

import importlib.util
import inspect
import json
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CAPTURE_DIR = ROOT / "research" / "possessed_enemy_capture"


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


RUNNER = load_module("possessed_enemy_runner", CAPTURE_DIR / "run_possessed_enemy_observer.py")
VALIDATOR = load_module("possessed_enemy_validator", ROOT / "tools" / "validate_possessed_enemy_collectors.py")
SUMMARIZER = load_module("possessed_enemy_summarizer", ROOT / "tools" / "summarize_possessed_late_mask.py")
POSTSPAWN = load_module(
    "possessed_enemy_postspawn", CAPTURE_DIR / "capture_postspawn_snapshot.py"
)
COMPARATOR = load_module(
    "possessed_enemy_late_mask_comparator", ROOT / "tools" / "compare_possessed_late_mask.py"
)
FIELD_ACCESS_INVENTORY = load_module(
    "possessed_enemy_field_access_inventory",
    CAPTURE_DIR / "inventory_record_field_accesses.py",
)
CALL_XREFS = load_module("runtime_call_xrefs", ROOT / "research" / "find_runtime_call_xrefs.py")
STRING_XREFS = load_module(
    "runtime_string_xrefs", ROOT / "research" / "find_runtime_string_xrefs.py"
)
POINTER_XREFS = load_module(
    "runtime_pointer_xrefs", ROOT / "research" / "find_runtime_pointer_xrefs.py"
)


class PossessedEnemyCaptureTests(unittest.TestCase):
    def test_runner_maps_every_signature_gated_observer(self) -> None:
        mapped = set(RUNNER.PHASES.values())
        actual = {
            path.name
            for path in CAPTURE_DIR.glob("*_ce.lua")
            if path.name not in VALIDATOR.EXCLUDED_OBSERVERS
        }
        self.assertEqual(actual, mapped)
        self.assertEqual("late_mask_observer_ce.lua", RUNNER.PHASES["late-mask"])

    def test_runner_refuses_ambiguous_session_selection(self) -> None:
        sessions = [{"session_id": "ce-a"}, {"session_id": "ce-b"}]
        with self.assertRaisesRegex(RuntimeError, "Multiple Cheat Engine sessions"):
            RUNNER.select_session(sessions, None)
        self.assertEqual("ce-b", RUNNER.select_session(sessions, "ce-b"))

    def test_runner_refuses_to_overwrite_capture_or_cleanup(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "late-mask.json"
            cleanup = RUNNER.require_unused_output(output)
            self.assertEqual(Path(temporary) / "late-mask.cleanup.json", cleanup)
            output.write_text("evidence", encoding="utf-8")
            with self.assertRaisesRegex(FileExistsError, "Refusing to overwrite"):
                RUNNER.require_unused_output(output)

    def test_cleanup_requires_stopped_probe_and_empty_breakpoints(self) -> None:
        complete = {
            "result": {
                "probe": {
                    "active": False,
                    "cleanup_pending": False,
                    "owned_breakpoints": [],
                    "debugger_broken": False,
                },
                "breakpoints": [],
            }
        }
        self.assertTrue(RUNNER.cleanup_verified(complete))
        complete["result"]["probe"]["cleanup_pending"] = True
        self.assertFalse(RUNNER.cleanup_verified(complete))

    def test_runner_allows_gui_time_for_bounded_cleanup_reconnects(self) -> None:
        parameter = inspect.signature(RUNNER.reconnect_cleanup).parameters["max_cycles"]
        self.assertEqual(5, parameter.default)
        source = (CAPTURE_DIR / "run_possessed_enemy_observer.py").read_text(encoding="utf-8")
        self.assertIn("time.sleep(0.75)", source)

    def test_manifest_separates_observation_prediction_and_native_capture(self) -> None:
        manifest = json.loads((CAPTURE_DIR / "run_manifest_template.json").read_text(encoding="utf-8"))
        self.assertEqual(156062997, manifest["seed"])
        self.assertEqual("natural_drop", manifest["scroll_provenance"]["kind"])
        self.assertEqual("Koroka", manifest["owner_observation"]["possessed_enemy_name"])
        self.assertFalse(manifest["offline_prediction"]["native_possession_proven"])
        self.assertIsNone(manifest["native_capture"]["field_8f_visual_correlation_confirmed"])
        self.assertIn("late_mask", manifest["capture_files"])
        self.assertIn("late_mask_cleanup", manifest["capture_files"])

    def test_all_observers_remain_read_only_and_within_breakpoint_limit(self) -> None:
        for filename in RUNNER.PHASES.values():
            source = (CAPTURE_DIR / filename).read_text(encoding="utf-8")
            sites = VALIDATOR.parse_expected(source)
            self.assertGreater(len(sites), 0, filename)
            self.assertLessEqual(len(sites), 4, filename)
            for forbidden in VALIDATOR.FORBIDDEN:
                self.assertNotIn(forbidden, source, f"{filename}: {forbidden}")

    def test_common_observer_rejects_unknown_inventory_and_enforces_event_budget(self) -> None:
        source = (CAPTURE_DIR / "observer_common_ce.lua").read_text(encoding="utf-8")
        self.assertIn("local debugging = debug_isDebugging()", source)
        self.assertIn("(initial == nil and not debugging)", source)
        self.assertIn("(type(initial) == 'table' and next(initial) == nil)", source)
        self.assertIn("if probe.event_sequence > probe.max_events then", source)
        self.assertIn("probe.stop('event_budget')", source)

    def test_late_mask_records_both_candidate_state_fields(self) -> None:
        source = (CAPTURE_DIR / "late_mask_observer_ce.lua").read_text(encoding="utf-8")
        self.assertIn("field_8f=api.u8(record+0x8F", source)
        self.assertIn("field_ea=api.u8(record+0xEA", source)
        self.assertIn("NIOH3_POSSESSED_STOP_ON_FIRST_FINAL == true", source)
        self.assertIn("p.stop('first_final_captured')", source)
        self.assertIn("nioh3-possessed-late-mask/v2", source)

    def test_selector_uses_neutral_names_for_unproved_state_fields(self) -> None:
        source = (CAPTURE_DIR / "selector_summary_ce.lua").read_text(encoding="utf-8")
        self.assertIn("field_e9=fieldE9", source)
        self.assertIn("field_ea=api.u8(record+0xEA", source)
        self.assertNotIn("possessed=api.u8(record+0xEA", source)
        self.assertIn("nioh3-possessed-selector-summary/v3", source)

    def test_static_field_inventory_distinguishes_reads_and_writes(self) -> None:
        import struct

        text = bytes.fromhex(
            "C6 87 8F 00 00 00 01 "
            "0F B6 87 8F 00 00 00 "
            "C3"
        )
        pdata = struct.pack("<III", 0x1000, 0x1000 + len(text), 0)
        result = FIELD_ACCESS_INVENTORY.inventory(
            text,
            pdata,
            0x1000,
            0x8F,
            ROOT / "audit/runtime_deps",
        )
        self.assertEqual(2, result["raw_literal_occurrence_count"])
        self.assertEqual(1, result["validated_write_count"])
        self.assertEqual(1, result["validated_read_count"])
        self.assertEqual("0x1000", result["entries"][0]["rva"])

    def test_call_xrefs_prefilter_before_bounded_disassembly(self) -> None:
        import struct

        instruction_rva = 0x1000
        target_rva = 0x2000
        displacement = target_rva - (instruction_rva + 5)
        text = b"\xE8" + struct.pack("<i", displacement) + b"\xC3"
        pdata = struct.pack("<III", instruction_rva, instruction_rva + len(text), 0)
        self.assertEqual(
            [instruction_rva],
            list(CALL_XREFS.direct_rel32_candidate_rvas(text, target_rva, instruction_rva)),
        )
        matches, skipped, raw_count = CALL_XREFS.validated_direct_calls(
            text,
            pdata,
            target_rva,
            instruction_rva,
            vendor_path=ROOT / "audit/runtime_deps",
        )
        self.assertEqual(1, raw_count)
        self.assertEqual([], skipped)
        self.assertEqual([instruction_rva], [item["rva"] for item in matches])

    def test_string_xrefs_disassemble_only_bounded_pdata_functions(self) -> None:
        import struct

        instruction_rva = 0x1000
        target_rva = 0x2000
        displacement = target_rva - (instruction_rva + 7)
        text = bytes.fromhex("48 8D 05") + struct.pack("<i", displacement) + b"\xC3"
        pdata = struct.pack("<III", instruction_rva, instruction_rva + len(text), 0)
        matches, statistics = STRING_XREFS._validated_rip_relative_references(
            text,
            pdata,
            {target_rva},
            instruction_rva,
            vendor_path=ROOT / "audit/runtime_deps",
        )
        self.assertEqual([instruction_rva], [item["rva"] for item in matches])
        self.assertEqual(1, statistics["raw_candidates"])
        self.assertEqual(1, statistics["scanned_functions"])
        self.assertEqual(len(text), statistics["scanned_bytes"])

        with self.assertRaisesRegex(RuntimeError, "max-total-function-bytes"):
            STRING_XREFS._validated_rip_relative_references(
                text,
                pdata,
                {target_rva},
                instruction_rva,
                max_total_function_bytes=len(text) - 1,
                vendor_path=ROOT / "audit/runtime_deps",
            )

    def test_runtime_pointer_xrefs_infer_aligned_image_base(self) -> None:
        import struct

        base = 0x7FF600000000
        target = 0x2228FA4
        data_rva = 0x38DE000
        data = b"\x00" * 8 + struct.pack("<Q", base + target) + b"\x00" * 8
        result = POINTER_XREFS.find_pointer_xrefs(
            data,
            [target],
            data_rva=data_rva,
        )
        self.assertEqual(1, len(result["matches"]))
        self.assertEqual("0x8", result["matches"][0]["data_offset"])
        self.assertEqual(f"0x{base:X}", result["matches"][0]["inferred_module_base"])

    def test_summarizer_derives_candidate_fields_from_raw_record(self) -> None:
        raw = bytearray(0xF0)
        raw[0x8E] = 3
        raw[0x8F] = 1
        raw[0xE9] = 1
        ordinary_raw = bytearray(raw)
        ordinary_raw[0x8F] = 0
        ordinary_raw[0xE9] = 0
        document = {
            "bridge_result": {
                "run_id": "sample",
                "schema": "nioh3-possessed-late-mask/v1",
                "events": [{
                    "site": "late_mask_final",
                    "selection_mask_hex": "00" * 12,
                    "records": [{
                        "index": 2,
                        "spawn_id": "0xF40",
                        "mission_key": "0xCC96",
                        "enemy_lookup_key": "0x8BC34",
                        "assigned_index": 0,
                        "candidate": 1,
                        "possessed": 0,
                        "record_raw_hex": bytes(raw).hex(),
                    }, {
                        "index": 3,
                        "spawn_id": "0xF41",
                        "mission_key": "0xCC96",
                        "enemy_lookup_key": "0x8BC34",
                        "assigned_index": 0,
                        "candidate": 0,
                        "field_8f": 0,
                        "field_ea": 0,
                        "record_raw_hex": bytes(ordinary_raw).hex(),
                    }],
                }],
            }
        }
        summary = SUMMARIZER.summarize(document)
        self.assertEqual(1, summary["field_8f_nonzero_count"])
        self.assertEqual("0x8BC34", summary["field_8f_nonzero_records"][0]["enemy_lookup_key"])
        self.assertEqual(0, summary["field_ea_nonzero_count"])
        self.assertEqual(2, summary["mission_record_count"])
        self.assertEqual({"zero": 1, "one": 1, "other": 0}, summary["field_e9_counts"])
        self.assertEqual("0xF41", summary["field_e9_zero_records"][0]["spawn_id"])

    def test_postspawn_validation_requires_stable_identity_and_unique_target(self) -> None:
        records = []
        for index in range(6):
            records.append({
                "index": index,
                "record_address": f"0x{0x1000 + index * 0x100:X}",
                "spawn_id": "0xF40" if index == 4 else f"0x{0xF3C + index:X}",
                "mission_key": "0xCC96",
                "enemy_lookup_key": "0x8BC34" if index == 4 else f"0x{index + 1:X}",
                "candidate": 1,
                "field_8f": 1 if index == 4 else 0,
                "record_raw_hex": "00" * 0xF0,
            })
        sample = {
            "pid": 10,
            "module_base": "0x140000000",
            "owner_address": "0x2000",
            "vector_begin": "0x3000",
            "vector_count": 6,
            "read_only": True,
            "writes_game_memory": False,
            "records": records,
        }
        result = POSTSPAWN.validate_samples([sample, json.loads(json.dumps(sample))], "0x140000000")
        self.assertTrue(result["stable_record_identities"])
        self.assertEqual("0x8BC34", result["target_record"]["enemy_lookup_key"])

        changed = json.loads(json.dumps(sample))
        changed["records"][4]["record_address"] = "0xDEAD"
        with self.assertRaisesRegex(RuntimeError, "identities changed"):
            POSTSPAWN.validate_samples([sample, changed], "0x140000000")

    def test_late_mask_comparator_separates_native_repeat_from_visual_semantics(self) -> None:
        raw = bytearray(0xF0)
        raw[0x8F] = 1
        raw[0xE9] = 1

        def capture(run_id: str, address: str) -> dict:
            return {
                "bridge_result": {
                    "run_id": run_id,
                    "schema": "nioh3-possessed-late-mask/v2",
                    "events": [{
                        "site": "late_mask_final",
                        "selection_mask_hex": "00" * 12,
                        "records": [{
                            "index": 0,
                            "record_address": address,
                            "spawn_id": "0xF40",
                            "mission_key": "0xCC96",
                            "enemy_lookup_key": "0x8BC34",
                            "assigned_index": 0,
                            "candidate": 1,
                            "field_8f": 1,
                            "field_ea": 0,
                            "record_raw_hex": bytes(raw).hex(),
                        }],
                    }],
                }
            }

        result = COMPARATOR.compare(capture("a", "0x1000"), capture("b", "0x2000"))
        self.assertTrue(result["native_state_repeat_supported"])
        self.assertFalse(result["candidate_record_addresses_equal"])
        self.assertFalse(result["first_positive_second_zero_split"])
        self.assertIn("requires separate visual labels", result["interpretation"])

    def test_late_mask_comparator_detects_same_count_different_e9_partition(self) -> None:
        def raw(field_8f: int, field_e9: int) -> str:
            value = bytearray(0xF0)
            value[0x8F] = field_8f
            value[0xE9] = field_e9
            return value.hex()

        def capture(run_id: str, base: int, first_e9: int, third_e9: int) -> dict:
            records = []
            for index, (spawn_id, field_8f, field_e9) in enumerate((
                ("0xF3E", 0, first_e9),
                ("0xF3F", 1, 1),
                ("0xF42", 0, third_e9),
            )):
                records.append({
                    "index": index,
                    "record_address": f"0x{base + index * 0x200:X}",
                    "spawn_id": spawn_id,
                    "mission_key": "0xCC96",
                    "enemy_lookup_key": "0xDCB98" if index < 2 else "0x561AC",
                    "assigned_index": index,
                    "candidate": field_e9,
                    "record_raw_hex": raw(field_8f, field_e9),
                })
            return {
                "bridge_result": {
                    "run_id": run_id,
                    "schema": "nioh3-possessed-late-mask/v2",
                    "events": [{
                        "site": "late_mask_final",
                        "selection_mask_hex": "00" * 12,
                        "records": records,
                    }],
                }
            }

        first = capture("a", 0x1000, 1, 0)
        second = capture("b", 0x5000, 0, 1)
        result = COMPARATOR.compare(first, second)
        self.assertTrue(result["mission_identity_equal"])
        self.assertTrue(result["field_8f_native_partition_repeat_supported"])
        self.assertTrue(result["field_e9_counts_equal"])
        self.assertFalse(result["field_e9_zero_records_equal"])
        self.assertFalse(result["field_e9_partition_repeat_supported"])
        self.assertFalse(result["mission_record_addresses_equal"])


if __name__ == "__main__":
    unittest.main()
