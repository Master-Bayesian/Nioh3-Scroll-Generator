"""Weapon move table for the evaluator, from the native action catalog. argv: catalog_dir out_json

A move lists its damaging rows in landing order. A row can fire several times, at candidate_event_times.
attack_class is parameter byte +5: 1 quick, 2 strong. MAIN+0x13C carries it into the formula.
"""
import json
import sys
from pathlib import Path

folder, out = Path(sys.argv[1]), Path(sys.argv[2])


def lines(name):
    with (folder / name).open(encoding="utf-8") as f:
        return [json.loads(x) for x in f if x.strip()]


weapons = {}
hits = {}
for h in lines("hits.jsonl"):
    hits.setdefault(h["action_key"], []).append(h)
for a in lines("actions.jsonl"):
    kind = a.get("weapon_type_id")
    if kind is None:
        continue
    rows = []
    for h in sorted(hits.get(a["key"], ()), key=lambda r: (r["start_time"] is None, r["start_time"] or 0, r["row_order"])):
        physical, elemental = h["channel_i16_10"], h["channel_i16_12"]
        if physical > 0 or elemental > 0:
            times = h["candidate_event_times"] or [h["start_time"]]  # None: no timeline, the row fires when an event calls it
            rows.append({"physical": physical, "elemental": elemental, "row": h["row_order"], "times": times})
    if not rows:
        continue
    parameter = bytes.fromhex(a["raw_parameter_hex"])
    weapon = weapons.setdefault(kind, {"type": kind, "label": a["weapon_label"], "moves": []})
    weapon["moves"].append({"id": a["action_id"], "name": a.get("display_name_zh"), "attack_class": parameter[5],
                            "flag6": parameter[6], "hits": rows})
data = sorted(weapons.values(), key=lambda w: w["type"])
out.write_text(json.dumps(data, ensure_ascii=False, separators=(",", ":")), encoding="utf-8", newline="\n")
for w in data:
    print(w["type"], w["label"], len(w["moves"]), "moves", max(sum(len(r["times"]) for r in m["hits"]) for m in w["moves"]), "max events")
