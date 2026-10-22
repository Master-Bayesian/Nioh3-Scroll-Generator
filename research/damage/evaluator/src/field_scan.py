"""Which effect fields act on one hit, and which state switches the others on.

Field scan: every D/STAT field that any of the 1152 native effect groups writes (zone_table.group_fields) is checked
against the read set of the candidate's replay. A field the replay never read cannot change this hit (the replay is
deterministic in the bytes it reads), so it is "not read" without a replay. Every read field is perturbed once
(f32 +0.1 for float-like words, +10 for small integers) and replayed: its damage delta shows whether it acts.

Trigger scan: every actor/target state value the traced MAIN compares (formula_trace decisions) is set to a few
other values, one at a time. A flip that makes the replay read effect fields it did not read before is a gate for
those fields: the state that switches those effects on. Damage after each flip is reported as well.
argv: <out_dir> [--outfit spec --inventory dir] [--buff 1.1]
"""
import argparse
import json
import struct
import sys
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
from formula_trace import _cone, trace  # noqa: E402
from zone_table import NAMES, effect_names, group_fields  # noqa: E402

STATIC = ("parameter", "heap:")


def perturbed(word):
    u = struct.unpack("<I", word)[0]
    if u and u < 0x10000:
        return struct.pack("<I", u + 10), "int+10"
    return struct.pack("<f", struct.unpack("<f", word)[0] + 0.1), "f32+0.1"


def main():
    p = argparse.ArgumentParser()
    p.add_argument("out", type=Path)
    p.add_argument("--outfit", default="")
    p.add_argument("--inventory", default=None)
    p.add_argument("--buff", type=float, default=None)
    args = p.parse_args()
    from fast_eval import Evaluator
    from regress_fast import BASE, CACHE, IN, load_inventory, pieces
    ev = Evaluator(CACHE, BASE / "capture.json", BASE / "ENTRY_REQUEST.json", BASE / "RETAINED_STATIC_CELLS.json",
                   [IN / "grade_table_cells.json"], buff=args.buff)
    m = ev.model
    plist = pieces(args.outfit, load_inventory(args.inventory)) if args.outfit else []
    base = ev.evaluate(plist)
    reads = base["reads"]
    fields, classes, _ = group_fields(m)
    names = effect_names(m, NAMES)
    writes, _ = ev.writes(plist)

    def current(address, size=4):
        return bytes(writes.get(address + i, (ev._template_byte(address + i) or (0, 0))[0]) for i in range(size))

    objects = {"D": m.d_addr, "STAT": m.stat_addr}
    rows = []
    for (obj, off), groups in sorted(fields.items()):
        address = objects[obj] + off
        row = {"field": f"{obj}+{off:#x}", "groups": [f"{g:#06x}" for g in groups],
               "classes": sorted({classes[g] for g in groups}),
               "effects": sorted({n for g in groups for n in names.get(g, [])})[:12]}
        if not any(a in reads for a in range(address, address + 4)):
            row["status"] = "not_read"
        else:
            word = current(address)
            new, how = perturbed(word)
            r = ev.evaluate(plist, {address: new})
            row.update(status="acts" if r["damage"] != base["damage"] else "read_no_effect", perturbation=how,
                       value_hex=word.hex(), damage=r["damage"], delta=None if r["damage"] is None else r["damage"] - base["damage"])
        rows.append(row)
        print(json.dumps({k: row.get(k) for k in ("field", "status", "delta")}), flush=True)
    field_at = {objects[o] + off: f"{o}+{off:#x}" for (o, off) in fields}

    def fields_read(read_set):
        return {name for a, name in field_at.items() if a in read_set}

    base_fields = fields_read(reads)
    sh, root, _, _ = trace(ev, plist)
    leaf_of = {}
    for (address, size), node in sh.leaves.items():
        leaf_of[node] = (address, size, sh.nodes[node]["label"], sh.nodes[node]["value_u"])
    gates = defaultdict(set)
    for d in sh.decisions:
        for n in _cone(sh, d["compare"]):
            if n in leaf_of and not leaf_of[n][2].startswith(STATIC) and "external_STAT" not in leaf_of[n][2]:
                gates[leaf_of[n][:3]].add(d["rva"])
    triggers = []
    for (address, size, label), rvas in sorted(gates.items(), key=lambda kv: kv[0][2]):
        width = min(size, 4)
        now = int.from_bytes(current(address, width), "little")
        is_float = width == 4 and (now >> 23) & 0xFF not in (0, 0xFF) and now > 0x00800000
        if is_float:
            value = struct.unpack("<f", struct.pack("<I", now))[0]
            options = [struct.pack("<f", v) for v in (0.0, 0.5, 2.0) if v != value]
        else:
            options = [v.to_bytes(width, "little") for v in (0, 1, 2, 4, 0xFF) if v != now and v < 1 << (8 * width)][:4]
        for new in options:
            r = ev.evaluate(plist, {address: new})
            opened = sorted(fields_read(r["reads"] or set()) - base_fields)
            closed = sorted(base_fields - fields_read(r["reads"] or set()))
            row = {"state": label, "address": hex(address), "width": width, "from_hex": now.to_bytes(width, "little").hex(),
                   "to_hex": new.hex(), "compared_at": sorted(rvas), "damage": r["damage"],
                   "delta": None if r["damage"] is None else r["damage"] - base["damage"],
                   "opens_fields": opened, "closes_fields": closed}
            if opened or closed or row["delta"]:
                triggers.append(row)
            print(json.dumps({k: row[k] for k in ("state", "to_hex", "delta")} | {"opens": len(opened)}), flush=True)
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "FIELDS.json").write_text(json.dumps({"damage": base["damage"], "fields": rows}, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    (args.out / "TRIGGERS.json").write_text(json.dumps({"damage": base["damage"], "triggers": triggers}, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    counts = defaultdict(int)
    for r in rows:
        counts[r["status"]] += 1
    print(json.dumps({"damage": base["damage"], "fields": dict(counts), "trigger_rows": len(triggers), "stats": ev.stats}))


if __name__ == "__main__":
    main()
