"""Confirm statically found gates: set the gate state, trace with the effect probe, check the expected fields enter.

Each case names a state (MAIN hit-context offset or HP-owner offset), the value written and the fields expected to
become formula leaves. Result per case: damage, fields switched on and off against the probed baseline, and whether
every expected field switched on.
argv: <out_dir> [--outfit spec --inventory dir]
"""
import argparse
import json
import struct
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
from formula_trace import f32, trace  # noqa: E402
from gate_scan import formula_fields  # noqa: E402
from zone_table import NAMES, effect_names, group_fields  # noqa: E402

# (name, object, offset, bytes, expected fields)
CASES = [
    ("back attack", "MAIN", 0x282, b"\x01", ["D+0x1f4"]),
    ("strong attack", "MAIN", 0x13C, struct.pack("<I", 2), ["D+0x124"]),
    ("low stance", "MAIN", 0x148, struct.pack("<I", 2), ["D+0x114"]),
    ("desperate (绝境) action bit", "MAIN", 0x13C, struct.pack("<I", 0x8000000), ["D+0x19c"]),
    ("pursuit / close thrust", "MAIN", 0x13C, struct.pack("<I", 16), ["D+0x1e8"]),
    ("martial art (武技)", "MAIN", 0x136, b"\x01", ["D+0x550", "D+0x558"]),
    ("unarmed action", "MAIN", 0x144, struct.pack("<I", 0x3C2E), ["D+0x22c"]),
    ("tsukumo (九十九化身)", "OWNER", 0x188, b"\x02", ["D+0x540"]),
    ("element water", "MAIN", 0x14, struct.pack("<i", 1), ["D+0x288", "D+0x3c4"]),
    ("element thunder", "MAIN", 0x14, struct.pack("<i", 2), ["D+0x28c", "D+0x3c8"]),
    ("guardian skill", "MAIN", 0x296, b"\x01", ["D+0x684"]),
    ("target ki exhausted", "TARGET_ENTITY", 0xAD, b"\x06", ["D+0x35c"]),
    ("target status 0x3C", "STATUS", 0x3C, None, ["D+0x304"]),
    ("target status 0x168", "STATUS", 0x168, None, ["D+0x300"]),
    ("target status 0x39", "STATUS", 0x39, None, ["D+0x308"]),
    ("target status 0x226", "STATUS", 0x226, None, ["D+0x310"]),
    ("target status 0xF9", "STATUS", 0xF9, None, ["D+0x314"]),
]


SYNTHETIC = 0x0000100000000000  # far from every captured address
STATUS_MAP_HEAD = 0x1C628757AF0  # target HP_owner+0x2DC0, +0x348: the captured, empty std::map head


def status_override(m, ev, writes, key, head=STATUS_MAP_HEAD):
    """Target has status `key` active, as a one-node std::map (MSVC _Tree_node) in a synthetic region.

    136AC4 starts at head->left, compares u16 node+0x20, returns node+0x28; 78CDF8 tests object+0x14 bit 0.
    The node's left, parent and right are the head (isnil at +0x19 = 1), so a lookup of any other key ends at the
    head exactly as in the captured empty map."""
    if not any(r["name"] == "synthetic_status_node" for r in ev.template["scene"]["memory_regions"]):
        ev.add_region("synthetic_status_node", SYNTHETIC, 0x100,
                      {"stands_for": "one active target status (map node + status object)", "map_head": hex(head)})
    node, obj, head_address = SYNTHETIC, SYNTHETIC + 0x80, head
    head = struct.pack("<Q", head_address)
    return {head_address: struct.pack("<Q", node), node: head, node + 8: head, node + 0x10: head,
            node + 0x18: b"\x00\x00", node + 0x20: struct.pack("<H", key), node + 0x28: struct.pack("<Q", obj),
            obj + 0x14: b"\x01"}


def main():
    p = argparse.ArgumentParser()
    p.add_argument("out", type=Path)
    p.add_argument("--outfit", default="")
    p.add_argument("--inventory", default=None)
    args = p.parse_args()
    from fast_eval import Evaluator
    from regress_fast import BASE, CACHE, IN, load_inventory, pieces
    ev = Evaluator(CACHE, BASE / "capture.json", BASE / "ENTRY_REQUEST.json", BASE / "RETAINED_STATIC_CELLS.json",
                   [IN / "grade_table_cells.json", IN / "default_d_cells.json"])  # target-side reads use the static default D
    m = ev.model
    plist = pieces(args.outfit, load_inventory(args.inventory)) if args.outfit else []
    fields, _, _ = group_fields(m)
    names = effect_names(m, NAMES)
    by_field = {f"{o}+{off:#x}": sorted({n for g in gs for n in names.get(g, [])})[:4] for (o, off), gs in fields.items()}
    writes, _ = ev.writes(plist)
    probe = {}
    for row in m.F.catalog()["recipes"].values():
        for op in row.get("operations", []):
            if str(op.get("equation", "")).startswith("D_f32") and op.get("width") == 4:
                a = m.d_addr + int(op["destination"], 16)
                if all(writes.get(a + i, (ev._template_byte(a + i) or (0, 0))[0]) == 0 for i in range(4)):
                    probe[a] = struct.pack("<f", 0.01)
    # 0x104, 0x530, 0x714 leave the captured state when probed (gate_scan bisection). 0x7CC calls a virtual method of
    # every target status object (vtable +0x78), which a synthetic status object does not have.
    for bad in (0x104, 0x530, 0x714, 0x7CC):
        probe.pop(m.d_addr + bad, None)
    sh, root, _, _ = trace(ev, plist, probe)
    base = formula_fields(sh, root)
    rows = []
    objects = {"MAIN": m.main_addr, "OWNER": m.R["source.HP_owner"][0],
               "TARGET_ENTITY": m.R["target.query.entity.prefix"][0]}
    for name, obj, off, data, expected in CASES:
        override = status_override(m, ev, writes, off) if obj == "STATUS" else {objects[obj] + off: data}
        sh2, root2, _, _ = trace(ev, plist, {**probe, **override})
        got = formula_fields(sh2, root2)
        row = {"case": name, "state": f"{obj}+{off:#x}", "value": data.hex() if data else "active", "expected": expected,
               "damage": f32(sh2.nodes[root2]["value_u"]) if root2 is not None else None}
        if got is None:
            row["status"] = "no damage store"
        else:
            on, off_ = sorted(set(got) - set(base)), sorted(set(base) - set(got))
            row.update(on={f: by_field.get(f, []) for f in on}, off={f: by_field.get(f, []) for f in off_},
                       confirmed=all(e in got for e in expected))
        rows.append(row)
        print(json.dumps(row, ensure_ascii=False), flush=True)
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "VERIFY_GATES.json").write_text(json.dumps(rows, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
