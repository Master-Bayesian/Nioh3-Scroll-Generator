"""Offline PC v2.02 armor getter model and bounded batch exporter.

Consumes approved, read-only captures. Does not attach to a game or infer
obtainability. Native returns validate individual input configurations only.
"""

from __future__ import annotations

import argparse
import csv
from dataclasses import dataclass
import hashlib
import json
import math
from pathlib import Path
import struct
from typing import Any


EXE_SHA = "E22C4A635E4EC1E27A177B76E27D7F6A637F426C0ED3928B60F5693BC52AE130"
ITEM_SHA = "F680966B0A21AB64EADF31A26E80F83E2EEEAAC1446E305BB87B9E64617B53CE"
ARMOR_TYPES = {0x0DF9: "head", 0x2B2F: "chest", 0x07B7: "arms", 0x403B: "legs", 0x09A9: "feet"}
STAGE_CAPS = {1: 5, 2: 15, 3: 30, 4: 45, 5: 120}
MODES = {"0/0": "unmodified", "1/0": "strengthened", "2/0": "thickened",
         "1/1": "extreme_strengthened", "2/2": "extreme_thickened", "1/2": "strengthened_plus_thickened"}
REQUIREMENTS = ("body", "heart", "stamina", "strength", "skill", "index5", "index6")
LIMITATIONS = [
    "Computed values describe the recovered getters, not obtainable or legal equipment.",
    "The input flags are a scenario; upstream remodel changes to flags remain unresolved.",
    "Rarity-5 and exceptional toughness branches have native control-flow/table evidence but no live return validation.",
    "Names retain their PC v2.01 localization provenance; item parameters are PC v2.02.",
    "Internal alternate targets are excluded from the default inventory-candidate export.",
]
PROJECT_ROOT = Path(__file__).resolve().parents[1]
PUBLIC_EVIDENCE = PROJECT_ROOT / "research/armor_remodel_v202"


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest().upper()


def load_json(path: Path, maximum: int = 2_500_000) -> dict[str, Any]:
    if path.stat().st_size > maximum:
        raise ValueError(f"Input exceeds {maximum} bytes: {path}")
    value = json.loads(path.read_text(encoding="utf-8-sig"))
    if not isinstance(value, dict):
        raise ValueError(f"Expected JSON object: {path}")
    return value


def capture(path: Path) -> dict[str, Any]:
    value = load_json(path)
    if value.get("type") == "result":
        if value.get("ok") is not True or not isinstance(value.get("result"), dict):
            raise ValueError(f"Failed capture: {path}")
        return value["result"]
    return value  # Reviewed phase07/08 cleanup files are unwrapped result objects.


def uint(value: Any, bits: int, label: str) -> int:
    if type(value) is not int or not 0 <= value < 1 << bits:
        raise ValueError(f"Invalid {label}: {value!r}")
    return value


def f32(value: float) -> float:
    return struct.unpack("<f", struct.pack("<f", value))[0]


def map_hash(key: int) -> int:
    """Replay +0x1BBA2BC with the sign-extended byte and uint32 wrapping."""
    word = key & 0xFFFFFFFF
    value = ((9 * word) & 0xFFFFFFFF) & 0x7FFFFFFF
    mixed = (value >> 11) ^ value
    return (((mixed << 15) + mixed + (word >> 16)) & 0xFFFFFFFF) & 0x7FFFFFFF


@dataclass(frozen=True)
class Item:
    index: int
    item_id: int
    type_code: int
    alternate_id: int
    weight: int
    toughness: int
    cold_key: int
    requirements: tuple[int, ...]

    @classmethod
    def parse(cls, index: int, row: bytes) -> "Item":
        u16 = lambda offset: struct.unpack_from("<H", row, offset)[0]
        u32 = lambda offset: struct.unpack_from("<I", row, offset)[0]
        return cls(index, u16(0x152), u16(0x60), u16(0x160), u32(0x98), u32(0xFC),
                   struct.unpack_from("<b", row, 0x183)[0],
                   tuple(u16(0x18C + 2 * i) for i in range(7)))


class ArmorModel:
    def __init__(self, evidence: Path = PUBLIC_EVIDENCE, names: Path = PROJECT_ROOT / "apps/workshop/item-names.json"):
        self.evidence = evidence
        manifest_path = evidence / "public-evidence.json"
        self.public_manifest = load_json(manifest_path) if manifest_path.is_file() else None
        if self.public_manifest is not None:
            if (self.public_manifest.get("schema") != "nioh3-armor-public-evidence-v202/v1"
                    or self.public_manifest.get("executable_sha256") != EXE_SHA
                    or self.public_manifest.get("item_sha256") != ITEM_SHA):
                raise ValueError("Unsupported public evidence manifest")
            self.check_public_hashes()
        root = evidence / "batch-dependencies"
        item_path = (PROJECT_ROOT / "nioh3_scroll_editor/data/r4_finalizer/pc_v2_02/resource_v1/tables/item.bin"
                     if self.public_manifest is not None else root / "item-live.bin")
        if item_path.stat().st_size != 1_398_600 or digest(item_path) != ITEM_SHA:
            raise ValueError("Unsupported or corrupted PC v2.02 item table")
        data = item_path.read_bytes()
        if struct.unpack_from("<I", data, 4)[0] != 3362:
            raise ValueError("Unexpected item row count")
        self.items: dict[int, Item] = {}
        for index in range(3362):
            item = Item.parse(index, data[8 + index * 416:8 + (index + 1) * 416])
            if item.item_id in self.items:
                raise ValueError(f"Duplicate item id: {item.item_id:#x}")
            self.items[item.item_id] = item
        self.armor = {key: item for key, item in self.items.items() if item.type_code in ARMOR_TYPES}
        self.internal = {item.alternate_id for item in self.armor.values() if item.alternate_id}
        for item in self.armor.values():
            if item.alternate_id and item.alternate_id not in self.armor:
                raise ValueError(f"Unresolved armor alternate: {item.item_id:#x}")
        progression_path = root / "progression-full.json"
        progression = self.table_capture(progression_path, "nioh3-armor-remodel-progression-fields-v202/v1")
        if progression["table_count"] != 502 or progression["stride"] != 0x94 or progression["header_size"] != 8:
            raise ValueError("Unsupported progression table shape")
        if len(progression["rows"]) != 502:
            raise ValueError("Incomplete progression table")
        self.curve = []
        for index, row in enumerate(progression["rows"]):
            if row["plus_index"] != index:
                raise ValueError("Progression indices are not consecutive")
            self.curve.append((uint(row["requirement_word"], 16, "requirement coefficient"),
                               uint(row["weight_word"], 16, "weight coefficient")))
        self.captured_stage = uint(progression["stage"], 8, "captured stage")
        cold_path = root / "toughness-cold-table.json"
        cold = self.table_capture(cold_path, "nioh3-armor-toughness-cold-table-v202/v1")
        if cold["table_stride"] != 20 or cold["table_count"] != len(cold["rows"]) or not 0 < cold["table_count"] <= 128:
            raise ValueError("Unsupported cold table shape")
        self.cold_rows = []
        for index, row in enumerate(cold["rows"]):
            raw = self.raw_bytes(row["bytes"], 20)
            if row["index"] != index:
                raise ValueError("Cold table indices are not consecutive")
            slope, offset = struct.unpack_from("<f", raw, 8)[0], struct.unpack_from("<f", raw, 16)[0]
            if not math.isfinite(slope) or not math.isfinite(offset):
                raise ValueError("Non-finite cold-table parameter")
            self.cold_rows.append((slope, offset))
        self.empty_key = uint(cold["empty_key_byte"], 8, "map sentinel")
        if cold["map_slot_count"] != len(cold["map_entries"]) or not 0 < cold["map_slot_count"] <= 512:
            raise ValueError("Unsupported cold lookup map")
        self.map_entries = []
        for slot, entry in enumerate(cold["map_entries"]):
            raw = self.raw_bytes(entry["bytes"], 8)
            if entry["slot"] != slot:
                raise ValueError("Cold map slots are not consecutive")
            self.map_entries.append((raw[0], struct.unpack_from("<I", raw, 4)[0]))
        name_data = load_json(names)
        if name_data.get("schema") != "nioh3-item-names/v1" or name_data.get("game_version") != "2.01":
            raise ValueError("Unsupported name catalog provenance")
        self.names = name_data["items"]
        self.provenance = {"item_sha256": ITEM_SHA, "executable_sha256": EXE_SHA,
                           "progression_capture_sha256": digest(progression_path),
                           "cold_table_capture_sha256": digest(cold_path), "name_catalog_sha256": digest(names),
                           "name_game_version": name_data["game_version"], "name_locale": name_data["locale"]}
        self.live_fields: set[tuple[Any, ...]] = set()

    def check_public_hashes(self) -> None:
        expected_files = {"batch-dependencies/progression-full.json", "batch-dependencies/toughness-cold-table.json", "native-returns.json"}
        if set(self.public_manifest["files"]) != expected_files:
            raise ValueError("Unexpected public evidence members")
        for name, expected in self.public_manifest["files"].items():
            if digest(self.evidence / name) != expected:
                raise ValueError(f"Public evidence hash mismatch: {name}")

    @staticmethod
    def raw_bytes(values: list[int], size: int) -> bytes:
        if len(values) != size:
            raise ValueError("Unexpected native row byte count")
        return bytes(uint(value, 8, "native byte") for value in values)

    @staticmethod
    def table_capture(path: Path, schema: str) -> dict[str, Any]:
        value = capture(path)
        if (value.get("schema") != schema or value.get("read_only") is not True
                or value.get("executable_sha256") != EXE_SHA
                or value.get("actual_breakpoints") != [] or value.get("context_type") != "nil"):
            raise ValueError(f"Unapproved table capture: {path}")
        return value

    def cold_index(self, key: int) -> int | None:
        slot = map_hash(key) % len(self.map_entries)
        for _ in self.map_entries:
            candidate, index = self.map_entries[slot]
            if candidate == key & 0xFF:
                return index if index < len(self.cold_rows) else None
            if candidate == self.empty_key:
                return None
            slot = (slot + 1) % len(self.map_entries)
        return None

    @staticmethod
    def field_key(result: dict[str, Any], field: str) -> tuple[Any, ...]:
        flags = result["flags"] & 0x240000 if field == "toughness" and result["flags"] & 0x40000 else 0
        return (result["item_id"], result["rarity"], result["level"], result["plus"], result["stage"], result["mode"], flags, field)

    def calculate(self, item_id: int, *, rarity: int = 4, level: int = 180,
                  plus: int = 20, stage: int = 3, flags: int = 0, mode: str = "0/0") -> dict[str, Any]:
        uint(item_id, 16, "item id"); uint(level, 16, "level"); uint(plus, 16, "plus"); uint(flags, 32, "flags")
        if type(rarity) is not int or rarity not in range(6) or type(stage) is not int or stage not in STAGE_CAPS:
            raise ValueError("Rarity must be 0..5 and stage must be 1..5")
        if mode not in MODES:
            raise ValueError(f"Unsupported remodel mode: {mode}")
        if item_id not in self.armor:
            raise ValueError(f"ID is not in the five armor parameter groups: {item_id:#06x}")
        original = self.armor[item_id]
        selected = self.items[original.alternate_id] if rarity >= 4 and original.alternate_id else original
        a, b = map(int, mode.split("/"))
        thick = (a == 2) + (b == 2) if rarity >= 3 else 0
        strong = (a == 1) + (b == 1) if rarity >= 3 else 0
        effective_plus = min(plus, STAGE_CAPS[stage])
        req_coefficient, weight_coefficient = self.curve[effective_plus]
        weight_delta = weight_coefficient * thick // 2
        req_delta = req_coefficient * strong // 2
        high = rarity >= 5 and original.type_code != 0x2D32
        bases = [(word >> 8) if high else (word & 255) for word in original.requirements]
        requirements = [base + req_delta if base else 0 for base in bases]
        toughness = selected.toughness
        cold_index = None
        candidate = None
        if flags & 0x40000:
            cold_index = self.cold_index(original.cold_key)
            if cold_index is not None:
                slope, offset = self.cold_rows[cold_index]
                factor = 0 if flags & 0x200000 else min(level, 180)
                candidate_float = f32(f32(float(factor) * f32(slope * f32(0.01))) + offset)
                if not math.isfinite(candidate_float) or not -2147483648 <= candidate_float < 2147483648:
                    raise ValueError("Exceptional toughness conversion outside modeled signed-int range")
                candidate = math.trunc(candidate_float)
                toughness = min(toughness, candidate)
        name_entry = self.names.get(str(item_id), ["", "", "", ""])
        result = {
            "item_id": item_id, "item_id_hex": f"0x{item_id:04X}", "name": name_entry[0],
            "name_category": name_entry[1], "name_slot": name_entry[2], "name_group": name_entry[3],
            "slot": ARMOR_TYPES[original.type_code], "row_index": original.index,
            "graph_role": "internal_alternate_target" if item_id in self.internal else "alternate_source" if original.alternate_id else "standalone",
            "selected_row_id": selected.item_id, "selected_row_id_hex": f"0x{selected.item_id:04X}",
            "selected_slot": ARMOR_TYPES[selected.type_code], "cross_slot_alternate": selected.type_code != original.type_code,
            "rarity": rarity, "level": level, "plus": plus, "stage": stage, "flags": flags,
            "flags_hex": f"0x{flags:08X}", "stage_cap": STAGE_CAPS[stage], "effective_plus": effective_plus,
            "mode": mode, "mode_name": MODES[mode], "weight_coefficient": weight_coefficient,
            "requirement_coefficient": req_coefficient, "selected_base_weight_raw": selected.weight,
            "weight_delta_raw": weight_delta, "weight_raw": selected.weight + weight_delta,
            "weight": (selected.weight + weight_delta) / 10,
            "requirement_base_source_id": original.item_id, "requirement_base_byte": "high" if high else "low",
            "requirement_bases": bases, "requirement_delta_nonzero": req_delta, "requirements": requirements,
            "selected_base_toughness": selected.toughness, "toughness": toughness,
            "cold_lookup_key": original.cold_key, "cold_table_index": cold_index, "cold_candidate": candidate,
            "unverified_branches": (["rarity5_requirement_selector"] if rarity == 5 else [])
                                   + (["exceptional_toughness"] if flags & 0x40000 else []),
        }
        fields = ["weight", "toughness"] + [f"requirement_{i}" for i in range(7)]
        result["live_matched_fields"] = [field for field in fields if self.field_key(result, field) in self.live_fields]
        result["verification"] = "live-matched-configuration" if len(result["live_matched_fields"]) == 9 else "computed-only"
        result["verification_method"] = ";".join(sorted({getattr(self, "live_methods", {}).get(self.field_key(result, field), "native-return")
                                                        for field in result["live_matched_fields"]})) or "computed-only"
        return result

    def validate_live(self) -> dict[str, Any]:
        # A failed revalidation must also revoke an earlier successful grade.
        self.live_fields = set()
        self.live_methods = {}
        if self.public_manifest is not None:
            return self.validate_public_returns()
        phases = [(self.evidence / f"stop{phase:02}.json", self.evidence / f"clean{phase:02}.json")
                  for phase in (3, 4, 5, 7, 8, 9)]
        extended = self.evidence / "extended-armor"
        phases += [(extended / f"stop-{part}-{kind}.json", extended / f"clean-{part}-{kind}.json")
                   for part in ("chest", "arms", "legs", "feet") for kind in ("combined", "requirements")]
        matched = 0
        configurations = set()
        by_kind = {"weight": 0, "requirements": 0, "toughness": 0}
        inputs = []
        verified: set[tuple[Any, ...]] = set()
        for stop_path, clean_path in phases:
            run, clean = capture(stop_path), capture(clean_path)
            probe, final = run["probe"], clean["probe"]
            if (probe.get("error") or probe.get("stale_pairs") != 0 or probe.get("ignored_hits") != 0
                    or probe.get("pid") != final.get("pid") or probe.get("active") is not False
                    or probe["total_hits"] != 2 * len(run["events"])):
                raise ValueError(f"Incomplete native pairing: {stop_path}")
            if (final.get("cleanup_verified") is not True or final.get("cleanup_pending") is not False
                    or final.get("active") is not False or final.get("owned_breakpoints") != []
                    or clean.get("actual_breakpoints") != [] or clean.get("context_type") != "nil"
                    or clean.get("cleanup_errors", []) or final.get("process_hook_restored") is not True
                    or final["timer_cleanup"].get("budget_timer_destroyed") is not True
                    or final["timer_cleanup"].get("cleanup_timer_destroyed") is not True):
                raise ValueError(f"Unverified native capture cleanup: {clean_path}")
            for sequence, event in enumerate(run["events"], 1):
                if event["sequence"] != sequence or event["stack_relation"] != "exit_rsp_is_entry_rsp_minus_0x28":
                    raise ValueError(f"Invalid native event sequence: {stop_path}")
                result = self.calculate(event["item_id"], rarity=event["rarity"], level=event["level"],
                                        plus=event["plus"], stage=self.captured_stage,
                                        flags=event.get("flags_0x18", 0), mode=event["mode"])
                kind = event.get("call_kind", event.get("observer_kind", "weight"))
                if kind == "weight":
                    field, predicted = "weight", result["weight_raw"]
                    actual = event["effective_weight_raw"]
                    if (event["selected_static_row_id"] != result["selected_row_id"]
                            or event["selected_base_weight_raw"] != result["selected_base_weight_raw"]):
                        raise ValueError(f"Native selected-row mismatch: {stop_path}")
                elif kind == "requirements":
                    index = uint(event["stat_index"], 8, "native stat index")
                    if index > 6:
                        raise ValueError("Native requirement index outside 0..6")
                    field, predicted, actual = f"requirement_{index}", result["requirements"][index], event["requirement_raw"]
                elif kind == "toughness":
                    field, predicted, actual = "toughness", result["toughness"], event["toughness_raw"]
                else:
                    raise ValueError(f"Unexpected native getter kind: {kind}")
                if uint(actual, 32, "native return") != predicted:
                    raise ValueError(f"Native/model mismatch in {stop_path.name}, event {sequence}: {actual} != {predicted}")
                verified.add(self.field_key(result, field))
                configurations.add((event["item_id"], event["mode"]))
                matched += 1
                by_kind[kind] += 1
            inputs.append({"capture": stop_path.name, "capture_sha256": digest(stop_path),
                           "cleanup_sha256": digest(clean_path), "matched_returns": len(run["events"])})
        if matched != 1190 or len(configurations) != 34 or len(verified) != 306:
            raise ValueError(f"Approved native corpus incomplete: {matched} returns, {len(configurations)} configurations, {len(verified)} fields")
        self.live_fields = verified
        self.live_methods = {key: "natural-ui-getter-observation" for key in verified}
        return {"paired_native_returns": matched, "callbacks": 2 * matched,
                "distinct_item_mode_configurations": len(configurations), "distinct_verified_fields": len(verified),
                "by_kind": by_kind, "mismatches": 0, "all_cleanup_verified": True,
                "scope": {"rarity": 4, "level": 180, "plus": 20, "stage": 3, "special_toughness_flag": False},
                "inputs": inputs}

    def validate_public_returns(self) -> dict[str, Any]:
        self.check_public_hashes()
        corpus = load_json(self.evidence / "native-returns.json")
        if corpus.get("schema") != "nioh3-armor-native-returns-v202/v1" or corpus.get("executable_sha256") != EXE_SHA:
            raise ValueError("Unsupported public native-return corpus")
        runs = {run["run_id"]: run for run in corpus["runs"]}
        if len(runs) != 15 or len(runs) != len(corpus["runs"]):
            raise ValueError("Incomplete or repeated native capture runs")
        verified, configurations, field_methods = set(), set(), {}
        by_kind = {"weight": 0, "requirements": 0, "toughness": 0}
        counts = {key: 0 for key in runs}
        methods = {"natural-ui-getter-observation": 0, "native-query-on-temporary-record": 0}
        for event in corpus["events"]:
            run = runs[event["run_id"]]
            if run.get("cleanup_verified") is not True or run["method"] != event["method"]:
                raise ValueError("Unverified public run cleanup or method")
            if event["sequence"] != counts[event["run_id"]] + 1:
                raise ValueError("Invalid public event sequence")
            counts[event["run_id"]] += 1
            result = self.calculate(event["item_id"], rarity=event["rarity"], level=event["level"], plus=event["plus"],
                                    stage=event["stage"], flags=event["flags"], mode=event["mode"])
            kind = event["kind"]
            if kind == "weight":
                field, predicted = "weight", result["weight_raw"]
                if "selected_row_id" in event and (event["selected_row_id"] != result["selected_row_id"]
                        or event["selected_base_weight_raw"] != result["selected_base_weight_raw"]):
                    raise ValueError("Public native selected-row mismatch")
            elif kind == "toughness":
                field, predicted = "toughness", result["toughness"]
            elif kind == "requirements":
                index = uint(event["stat_index"], 8, "native stat index")
                if index > 6:
                    raise ValueError("Native requirement index outside 0..6")
                field, predicted = f"requirement_{index}", result["requirements"][index]
            else:
                raise ValueError(f"Unexpected getter kind: {kind}")
            if uint(event["actual"], 32, "native return") != predicted:
                raise ValueError(f"Native/model mismatch: {event['run_id']} event {event['sequence']}")
            verified.add(self.field_key(result, field))
            field_methods[self.field_key(result, field)] = event["method"]
            configurations.add(tuple(result[key] for key in ("item_id", "rarity", "level", "plus", "stage", "mode")))
            by_kind[kind] += 1
            methods[event["method"]] += 1
        for key, run in runs.items():
            if counts[key] != run["return_count"]:
                raise ValueError("Incomplete public run")
            if run["method"] == "natural-ui-getter-observation" and run["callbacks"] != 2 * counts[key]:
                raise ValueError("Incomplete natural native pairing")
            if run["method"] == "native-query-on-temporary-record" and (run.get("temporary_allocation_released") is not True
                    or run.get("independent_cleanup_verified") is not True):
                raise ValueError("Unverified native query release")
        if (len(corpus["events"]), len(configurations), len(verified)) != (1352, 52, 468) or list(methods.values()) != [1190, 162]:
            raise ValueError("Approved public native corpus incomplete")
        self.live_fields = verified
        self.live_methods = field_methods
        return {"native_returns": 1352, "paired_native_returns": 1190, "direct_native_queries": 162, "callbacks": 2380,
                "distinct_item_mode_configurations": 52, "distinct_verified_fields": 468, "by_kind": by_kind,
                "mismatches": 0, "all_cleanup_verified": True, "scope": self.public_manifest["scope"],
                "evidence_methods": methods, "corpus_sha256": digest(self.evidence / "native-returns.json")}


def export(model: ArmorModel, output: Path, *, rarity: int, level: int, plus_values: list[int],
           stage: int, flags: int, include_internal: bool = False, item_ids: list[int] | None = None) -> dict[str, Any]:
    validation = model.validate_live()
    ids = sorted(model.armor if item_ids is None else set(item_ids))
    if any(item_id not in model.armor for item_id in ids):
        raise ValueError("Explicit item selection contains non-armor IDs")
    if not include_internal:
        ids = [item_id for item_id in ids if item_id not in model.internal]
    plus_values = sorted(set(plus_values))
    if not ids or not plus_values or len(ids) * len(plus_values) * len(MODES) > 250_000:
        raise ValueError("Empty selection or batch exceeds 250,000 rows")
    # Calculate and validate every result before opening any output file.
    results = [model.calculate(item_id, rarity=rarity, level=level, plus=plus, stage=stage, flags=flags, mode=mode)
               for item_id in ids for plus in plus_values for mode in MODES]
    paths = [output / filename for filename in ("armor-remodel.json", "armor-remodel.csv", "validation.json")]
    if any(path.exists() for path in paths):
        raise FileExistsError("Output files already exist; choose a new output directory")
    output.mkdir(parents=True, exist_ok=True)
    summary = {"schema": "nioh3-armor-remodel-batch-v202/v1", "game_version": "2.0.2.0", "offline_only": True,
               "scope": {"rarity": rarity, "level": level, "plus_values": plus_values, "stage": stage,
                         "flags": flags, "includes_internal_rows": include_internal},
               "armor_parameter_rows": len(model.armor), "internal_alternate_rows": len(model.internal),
               "exported_item_ids": len(ids), "exported_configurations": len(results),
               "live_matched_configurations": sum(row["verification"] == "live-matched-configuration" for row in results),
               "computed_only_configurations": sum(row["verification"] == "computed-only" for row in results),
               "unnamed_item_ids": sum(not model.names.get(str(item_id), [""])[0] for item_id in ids),
               "cross_slot_configurations": sum(row["cross_slot_alternate"] for row in results),
               "provenance": model.provenance, "limitations": LIMITATIONS, "validation": validation}
    paths[0].write_text(json.dumps({**summary, "rows": results}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    scalar_fields = [key for key, value in results[0].items() if not isinstance(value, list)]
    csv_fields = scalar_fields + [f"requirement_{name}" for name in REQUIREMENTS] + ["live_matched_fields", "unverified_branches"]
    with paths[1].open("w", encoding="utf-8-sig", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=csv_fields)
        writer.writeheader()
        for row in results:
            flat = {key: row[key] for key in scalar_fields}
            flat.update({f"requirement_{name}": row["requirements"][i] for i, name in enumerate(REQUIREMENTS)})
            flat["live_matched_fields"] = ";".join(row["live_matched_fields"])
            flat["unverified_branches"] = ";".join(row["unverified_branches"])
            # Name-catalog text remains inert when opened in a spreadsheet.
            for key in ("name", "name_category", "name_slot", "name_group"):
                if str(flat[key]).startswith(("=", "+", "-", "@")):
                    flat[key] = "'" + str(flat[key])
            writer.writerow(flat)
    paths[2].write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return summary


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence-root", type=Path, default=PUBLIC_EVIDENCE)
    parser.add_argument("--names", type=Path, default=Path(__file__).resolve().parents[1] / "apps/workshop/item-names.json")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--rarity", type=int, choices=range(6), default=4)
    parser.add_argument("--level", type=int, default=180)
    parser.add_argument("--plus", type=int, action="append")
    parser.add_argument("--stage", type=int, choices=range(1, 6), default=3)
    parser.add_argument("--flags", type=lambda value: int(value, 0), default=0)
    parser.add_argument("--item-id", type=lambda value: int(value, 0), action="append")
    parser.add_argument("--include-internal", action="store_true")
    args = parser.parse_args()
    model = ArmorModel(args.evidence_root, args.names)
    summary = export(model, args.output, rarity=args.rarity, level=args.level, plus_values=args.plus or [20],
                     stage=args.stage, flags=args.flags, include_internal=args.include_internal, item_ids=args.item_id)
    print(json.dumps({key: summary[key] for key in ("exported_item_ids", "exported_configurations",
                                                  "live_matched_configurations", "computed_only_configurations")}, indent=2))
    print(f"Output: {args.output.resolve()}")


if __name__ == "__main__":
    main()
