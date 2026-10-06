#!/usr/bin/env python3
"""Read-only extractor for Nioh 3 PC v2.02 item.bin static armor fields.

Outputs:
- equipment_static_data_v202.json
- equipment_static_data_v202.csv          (current armor rows, type_class 24..38)
- item_table_static_fields_v202_full.csv  (all item.bin rows)

No game/save writes are performed.
"""
from __future__ import annotations

import argparse
import collections
import csv
import hashlib
import json
from pathlib import Path

HEADER_SIZE = 8
ROW_SIZE = 0x1A0
ITEM_ID_OFF = 0x152
TYPE_CLASS_OFF = 0x15C
WEIGHT_OFF = 0x98
REQ_OFFSETS = {
    "body": 0x18C,
    "heart": 0x18E,
    "stamina": 0x190,
    "strength": 0x192,
    "skill": 0x194,
}
CURRENT_ARMOR_CLASS_MIN = 24
CURRENT_ARMOR_CLASS_MAX = 38
CATALOG_REPO_PATH = "apps/workshop/item-names.json"

def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()

def u16(row: bytes, off: int) -> int:
    return int.from_bytes(row[off:off+2], "little")

def u32(row: bytes, off: int) -> int:
    return int.from_bytes(row[off:off+4], "little")

def load_catalog(catalog_json: Path) -> tuple[dict[int, list[str]], dict]:
    doc = json.loads(catalog_json.read_text(encoding="utf-8"))
    items = {int(k): v for k, v in doc["items"].items()}
    return items, doc

def catalog_fields(item_id: int, catalog: dict[int, list[str]]) -> dict:
    entry = catalog.get(item_id)
    if not entry:
        return {
            "catalog_known": False,
            "name": "",
            "catalog_major": "",
            "catalog_minor": "",
            "catalog_group": "",
        }
    values = list(entry) + [""] * (4 - len(entry))
    return {
        "catalog_known": True,
        "name": values[0] or "",
        "catalog_major": values[1] or "",
        "catalog_minor": values[2] or "",
        "catalog_group": values[3] or "",
    }

def parse(item_bin: Path, catalog_json: Path) -> tuple[list[dict], dict]:
    data = item_bin.read_bytes()
    if len(data) < HEADER_SIZE or (len(data) - HEADER_SIZE) % ROW_SIZE:
        raise SystemExit(
            f"invalid item.bin shape: size={len(data)}, header={HEADER_SIZE}, row={ROW_SIZE}"
        )
    catalog, catalog_doc = load_catalog(catalog_json)
    count = (len(data) - HEADER_SIZE) // ROW_SIZE
    rows: list[dict] = []
    for index in range(count):
        row = data[HEADER_SIZE + index*ROW_SIZE: HEADER_SIZE + (index+1)*ROW_SIZE]
        item_id = u16(row, ITEM_ID_OFF)
        type_class = u32(row, TYPE_CLASS_OFF)
        record = {
            "row_index": index,
            "item_id": item_id,
            "item_id_hex": f"0x{item_id:04X}",
            "type_class": type_class,
            "weight_raw": u32(row, WEIGHT_OFF),
            "weight": u32(row, WEIGHT_OFF) / 10.0,
            "is_current_armor": CURRENT_ARMOR_CLASS_MIN <= type_class <= CURRENT_ARMOR_CLASS_MAX,
        }
        record.update(catalog_fields(item_id, catalog))
        record["planner_candidate"] = bool(record["is_current_armor"] and record["name"])
        for key, off in REQ_OFFSETS.items():
            word = u16(row, off)
            # Evidence on PC v2.02 current armor: the low byte is the standard
            # displayed requirement. The high byte is retained as unknown aux.
            record[f"req_{key}"] = row[off]
            record[f"req_{key}_aux"] = row[off + 1]
            record[f"req_{key}_raw_u16"] = word
        rows.append(record)
    meta = {
        "format": "nioh3-equipment-static-v202-v1",
        "game_version": "PC v2.02",
        "item_bin": {
            "path": "item.bin",
            "bytes": len(data),
            "sha256": sha256(data),
            "header_size": HEADER_SIZE,
            "row_size": ROW_SIZE,
            "row_count": count,
        },
        "catalog": {
            "source_path_in_repository": CATALOG_REPO_PATH,
            "declared_game_version": catalog_doc.get("game_version"),
            "locale": catalog_doc.get("locale"),
            "provenance_note": (
                "Names come from the checked-in repository catalog; upstream "
                "catalog source is not included in this research package."
            ),
            "catalog_rows": len(catalog),
        },
        "field_contract": {
            "item_id": "row + 0x152, u16 little-endian",
            "type_class": "row + 0x15C, u32 little-endian",
            "weight": "row + 0x98, u32 little-endian; displayed armor weight = raw / 10",
            "requirements": (
                "row + 0x18C/0x18E/0x190/0x192/0x194; "
                "LOW BYTE is the PC v2.02 standard requirement for body/heart/stamina/strength/skill; "
                "HIGH BYTE retained as unknown auxiliary data"
            ),
            "current_armor_filter": "24 <= type_class <= 38 (matches repository armor classification)",
        },
    }
    return rows, meta

def write_csv(path: Path, rows: list[dict]) -> None:
    if not rows:
        return
    fields = list(rows[0].keys())
    with path.open("w", encoding="utf-8-sig", newline="") as f:
        w = csv.DictWriter(f, fieldnames=fields, lineterminator="\n")
        w.writeheader()
        w.writerows(rows)

def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--item-bin", required=True, type=Path)
    repository_root = Path(__file__).resolve().parents[2]
    ap.add_argument(
        "--catalog-json", type=Path,
        default=repository_root / CATALOG_REPO_PATH,
    )
    ap.add_argument("--out", required=True, type=Path)
    args = ap.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)

    rows, meta = parse(args.item_bin, args.catalog_json)
    current = [r for r in rows if r["is_current_armor"]]

    ids = [r["item_id"] for r in rows]
    counts = collections.Counter(ids)
    req_keys = ["body", "heart", "stamina", "strength", "skill"]
    named_current = [r for r in current if r["name"]]
    stats = {
        "all_rows": len(rows),
        "unique_item_ids": len(set(ids)),
        "duplicate_item_ids": sorted(f"0x{k:04X}" for k,v in counts.items() if v > 1),
        "catalog_known_rows": sum(bool(r["catalog_known"]) for r in rows),
        "current_armor_rows": len(current),
        "current_armor_named_rows": len(named_current),
        "current_armor_blank_name_rows": len(current) - len(named_current),
        "current_armor_weight_zero_rows": sum(r["weight_raw"] == 0 for r in current),
        "current_armor_weight_min": min(r["weight"] for r in current),
        "current_armor_weight_max": max(r["weight"] for r in current),
        "named_current_armor_exactly_two_requirements": sum(
            sum(r[f"req_{k}"] != 0 for k in req_keys) == 2 for r in named_current
        ),
    }
    payload = {
        "source": meta,
        "statistics": stats,
        "items": current,
    }
    with (args.out / "equipment_static_data_v202.json").open(
        "w", encoding="utf-8", newline="\n"
    ) as f:
        f.write(json.dumps(payload, ensure_ascii=False, indent=2) + "\n")
    write_csv(args.out / "equipment_static_data_v202.csv", current)
    write_csv(args.out / "item_table_static_fields_v202_full.csv", rows)

if __name__ == "__main__":
    main()
