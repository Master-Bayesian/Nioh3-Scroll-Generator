"""Which state switches which effect field into the damage formula.

MAIN reads almost every D field regardless of state, so read sets cannot show gates. The formula can: for the
baseline candidate and for each state flip, trace the replay (formula_trace.trace) and collect the effect fields
that are leaves of the damage expression. A field that becomes a leaf only after a flip is switched on by that
state; one that drops out is switched off. Zero-valued fields stay leaves (they still enter their pool), so this
works for effects the candidate does not wear.

States scanned: every non-table value that a traced MAIN comparison reads (as field_scan.py), plus every byte of the
MAIN hit-context block MAIN+0x280..0x2B0. Each is set to 1 (bytes) or a few values (wider fields).
argv: <out_dir> [--outfit spec --inventory dir] [--buff 1.1]
"""
import argparse
import json
import re
import struct
import sys
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
from formula_trace import _cone, f32, trace  # noqa: E402
from zone_table import NAMES, effect_names, group_fields, leaf_field  # noqa: E402

STATIC = ("parameter", "heap:")


def formula_fields(sh, root):
    if root is None:
        return None
    out = {}
    for n in _cone(sh, root):
        x = sh.nodes[n]
        if x["op"] == "in":
            field = leaf_field(x["label"])
            if field:
                out[f"{field[0]}+{field[1]:#x}"] = x["value_u"]
    return out


def main():
    p = argparse.ArgumentParser()
    p.add_argument("out", type=Path)
    p.add_argument("--outfit", default="")
    p.add_argument("--inventory", default=None)
    p.add_argument("--buff", type=float, default=None)
    p.add_argument("--probe", action="store_true",
                   help="set every zero f32 D field of the materializer recipes to 0.01 first, so effects the candidate "
                        "does not wear still enter the formula (MAIN skips some zero-valued fields)")
    args = p.parse_args()
    from fast_eval import Evaluator
    from regress_fast import BASE, CACHE, IN, load_inventory, pieces
    ev = Evaluator(CACHE, BASE / "capture.json", BASE / "ENTRY_REQUEST.json", BASE / "RETAINED_STATIC_CELLS.json",
                   [IN / "grade_table_cells.json"], buff=args.buff)
    m = ev.model
    plist = pieces(args.outfit, load_inventory(args.inventory)) if args.outfit else []
    fields, classes, _ = group_fields(m)
    names = effect_names(m, NAMES)
    by_field = {f"{o}+{off:#x}": sorted({n for g in gs for n in names.get(g, [])})[:8] for (o, off), gs in fields.items()}
    writes, _ = ev.writes(plist)

    def current(address, size):
        return int.from_bytes(bytes(writes.get(address + i, (ev._template_byte(address + i) or (0, 0))[0])
                                    for i in range(size)), "little")

    probe = {}
    if args.probe:
        for row in m.F.catalog()["recipes"].values():
            for op in row.get("operations", []):
                if str(op.get("equation", "")).startswith("D_f32") and op.get("width") == 4:
                    address = m.d_addr + int(op["destination"], 16)
                    if current(address, 4) == 0:
                        probe[address] = struct.pack("<f", 0.01)

    excluded = []

    def admissible(items):
        """Largest probe subset whose replay still completes: bisect out fields whose probe leaves the capture."""
        if not items:
            return items
        if ev.evaluate(plist, dict(items))["damage"] is not None:
            return items
        if len(items) == 1:
            excluded.append(f"D+{items[0][0] - m.d_addr:#x}")
            return []
        half = len(items) // 2
        left = admissible(items[:half])
        right = admissible(items[half:])
        joined = left + right
        if joined and ev.evaluate(plist, dict(joined))["damage"] is None:  # interaction between halves: keep left only
            excluded.extend(f"D+{a - m.d_addr:#x}" for a, _ in right)
            return left
        return joined
    probe = dict(admissible(sorted(probe.items())))
    print(json.dumps({"probe_fields": len(probe), "excluded": excluded}), flush=True)

    def run(extra=None):
        return trace(ev, plist, {**probe, **(extra or {})})
    sh, root, result, _ = run()
    base_damage = f32(sh.nodes[root]["value_u"]) if root is not None else None
    base_fields = formula_fields(sh, root)
    states = {}
    for d in sh.decisions:
        for n in _cone(sh, d["compare"]):
            x = sh.nodes[n]
            if x["op"] == "in" and not x["label"].startswith(STATIC) and "external_STAT" not in x["label"]:
                address = next(a for (a, s), node in sh.leaves.items() if node == n)
                states[address] = (min(x["width"] // 8, 4), x["label"])
    main_addr = m.main_addr
    for off in range(0x280, 0x2B0):
        states.setdefault(main_addr + off, (1, f"MAIN.entry_prestate+{off:#x}"))
    rows = []
    for address, (width, label) in sorted(states.items(), key=lambda kv: kv[1][1]):
        now = current(address, width)
        if width == 1:
            options = [1 if now != 1 else 0]
        elif width == 4 and (now >> 23) & 0xFF not in (0, 0xFF) and now > 0x00800000:
            options = [struct.unpack("<I", struct.pack("<f", v))[0] for v in (0.0, 2.0)]
        else:
            options = [v for v in (0, 1) if v != now]
        for new in options:
            sh2, root2, _, _ = run({address: new.to_bytes(width, "little")})
            got = formula_fields(sh2, root2)
            damage = f32(sh2.nodes[root2]["value_u"]) if root2 is not None else None
            row = {"state": label, "address": hex(address), "from": hex(now), "to": hex(new), "damage": damage}
            if got is None:
                row["status"] = "no damage store (path left the captured state)"
            else:
                on = sorted(set(got) - set(base_fields))
                off_ = sorted(set(base_fields) - set(got))
                row.update(switches_on=[{"field": f, "effects": by_field.get(f, [])} for f in on],
                           switches_off=[{"field": f, "effects": by_field.get(f, [])} for f in off_])
            rows.append(row)
            print(json.dumps({"state": label, "to": hex(new), "damage": damage,
                              "on": len(row.get("switches_on", [])), "off": len(row.get("switches_off", []))}), flush=True)
    args.out.mkdir(parents=True, exist_ok=True)
    out = {"damage": base_damage, "probe_fields": len(probe), "probe_excluded": excluded, "formula_fields": {f: {"value_u": v, "effects": by_field.get(f, [])} for f, v in base_fields.items()},
           "flips": rows}
    (args.out / "GATES.json").write_text(json.dumps(out, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    lines = [f"base damage {base_damage}; formula fields:"]
    lines += [f"  {f} = {v:#x}  {'/'.join(by_field.get(f, [])[:4])}" for f, v in sorted(base_fields.items())]
    for r in rows:
        if r.get("switches_on") or r.get("switches_off"):
            lines.append(f"{r['state']} {r['from']}->{r['to']} damage {r['damage']}")
            for s in r["switches_on"]:
                lines.append(f"    + {s['field']} {'/'.join(s['effects'][:4])}")
            for s in r["switches_off"]:
                lines.append(f"    - {s['field']} {'/'.join(s['effects'][:4])}")
    (args.out / "GATES.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(json.dumps({"damage": base_damage, "flips": len(rows), "with_changes": sum(bool(r.get("switches_on") or r.get("switches_off")) for r in rows)}))


if __name__ == "__main__":
    main()
