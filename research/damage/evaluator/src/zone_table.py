"""Multiplier-zone table of one hit: the traced MAIN formula, split into its zones, with the effects behind each field.

1. Trace the candidate's MAIN replay (formula_trace.trace) and walk the spine: from the damage at MAIN+0x218 down to the
   action's motion value, following at each operation the operand that depends on the motion value. Every other
   operand is a zone: a multiplicative factor, an additive term, a cap or a rounding step.
2. Inside each zone, list the state fields it reads (leaves) and print the zone as an expression.
3. Name the fields: every one of the 1152 native effect groups is run through the field materializer (819400) once;
   the D/STAT offsets each group writes, and the effects (catalog names) that feed each group, name the leaf.
4. Decisions: comparisons on state that select this path, grouped by native address, plus the conditional effect
   groups (materializer class conditional27) - the candidate trigger inventory.

The table is exact for the traced path only. A field that a zone reads with value 0 is still listed: its effects
belong to that zone. A field never read on this path is not listed; it may matter on another path (see decisions).
argv: <out_dir> [--outfit spec --inventory dir] [--buff 1.1] [--names effect_names_multilingual.json]
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
from formula_trace import f32, trace, _cone  # noqa: E402

SKILLS = Path("F:/Nioh3_r16_build/deliverables/r16-native-buffs-20261019/catalog/e2e-batch-01/skills/SKILL_SOURCE_CATALOG.json")
NAMES = Path("F:/Nioh3_r16_build/worktrees/research-damage/nioh3_scroll_editor/data/effect_names_multilingual.json")


def group_fields(model):
    """{(object, offset): [group]} from executing every native group once through the field materializer."""
    F, N, T = model.F, model.N, model.T
    stat = bytearray(model.stat)
    struct.pack_into("<Q", stat, 8, N.OWNER)
    struct.pack_into("<Q", stat, 0x7C0, F.DEST)
    struct.pack_into("<Q", stat, 0x7D8, N.ATTR)
    catalog = F.catalog()
    fields, classes, failures = defaultdict(list), {}, {}
    for group, row in catalog["index"].items():
        classes[group] = row["classification"]
        try:
            out = F.apply(group_u16=group, value_u32=100, stat_hex=bytes(stat).hex(), materializer_hex=model.d0.hex(),
                          inventory_hex=bytes(model.inv).hex(), synchronized_i32=model.raw_attrs, owner_mode_i32=model.mode)
        except Exception as error:  # an unsupported domain is reported, never guessed
            failures[group] = str(error)[:160]
            continue
        for w in out.get("ordered_writes", []):
            obj = "D" if w.get("object", "D") == "D" else "STAT"
            fields[(obj, int(w["offset"], 16))].append(group)
    return fields, classes, failures


def effect_names(model, path):
    """{group: [zh-CN effect names]}: the materializer index plus the effect table's own group links."""
    catalog = {int(k, 16): v for k, v in json.loads(Path(path).read_bytes())["effects"].items()}
    ids = defaultdict(set)
    for group, row in model.F.catalog()["index"].items():
        ids[group].update(row.get("effect_ids", []))
    for effect_id in model.T.EFFECTS.indices:  # effect table row +2: the group an item effect submits to
        ids[struct.unpack_from("<H", model.T.EFFECTS.row(effect_id), 2)[0]].add(effect_id)
    out = {}
    for group, members in ids.items():
        names = []
        for effect_id in sorted(members):
            entry = catalog.get(effect_id)
            names.append((entry or {}).get("names", {}).get("zh-CN") or f"effect {effect_id:#x}")
        out[group] = sorted(set(names))
    if SKILLS.exists():  # skills submit to groups too (native skill-source catalog)
        def walk(node, found):
            if isinstance(node, dict):
                if node.get("group_u16"):
                    found.add(node["group_u16"])
                for v in node.values():
                    walk(v, found)
            elif isinstance(node, list):
                for v in node:
                    walk(v, found)
        for skill in json.loads(SKILLS.read_bytes())["skills"]:
            found = set()
            walk(skill, found)
            for group in found:
                out.setdefault(group, []).append("skill:" + (skill.get("native_name_zh") or skill["skill_key_hex"]))
    return out


def leaf_field(label):
    m = re.match(r"source\.external_STAT\.D\+(0x[0-9a-f]+)$", label or "")
    if m:
        return ("D", int(m.group(1), 16))
    m = re.match(r"source\.external_STAT\+(0x[0-9a-f]+)$", label or "")
    if m:
        return ("STAT", int(m.group(1), 16))
    return None


def infix(sh, n, depth=0, seen=None):
    x = sh.nodes[n]
    op, args = x["op"], x["args"]
    if op == "in":
        return x["label"]
    if depth > 14:
        return f"n{n}"
    sub = [infix(sh, a, depth + 1) for a in args]
    const = x.get("const")
    c = "" if const is None else (f"{const:.6g}" if isinstance(const, float) else str(const))
    if op in ("cvtdq2ps", "cvtsi2ss", "movsx", "movsxd", "movzx", "cvttss2si", "view", "cdqe"):
        return sub[0] if sub else f"n{n}"
    sym = {"addss": "+", "subss": "-", "mulss": "*", "divss": "/", "add": "+", "sub": "-", "imul": "*"}.get(op)
    if sym and len(sub) == 2:
        return f"({sub[0]} {sym} {sub[1]})"
    if sym and len(sub) == 1 and c:
        return f"(dest {sym} {sub[0]})" if x.get("const_first") else f"({sub[0]} {sym} {c})"
    if op == "lookup":
        return f"table[{', '.join(sub)}]"
    if op.startswith("select:"):
        return f"select({', '.join(sub)})"
    if op.startswith("cmp:"):
        return f"cmp({', '.join(sub)}{', ' + c if c else ''})"
    return f"{op}({', '.join(sub)}{', ' + c if c else ''})"


def _pool_root(sh, n):
    """Skip the trailing '* 1' / 'dest *' wrapper of a 6267D0 boost/cut factor to reach its additive pool."""
    x = sh.nodes[n]
    while x["op"] == "mulss" and len(x["args"]) == 1 and x.get("const") in (1.0, "dest-before"):
        n = x["args"][0]
        x = sh.nodes[n]
    while x["op"] == "mulss" and len(x["args"]) == 2:
        values = [f32(sh.nodes[a]["value_u"]) for a in x["args"]]
        if values[1] == 1.0:
            n = x["args"][0]
        elif values[0] == 1.0:
            n = x["args"][1]
        else:
            break
        x = sh.nodes[n]
    return n


def terms(sh, n, out=None):
    """Additive terms of a pool: flatten addss over two tracked operands; (x - 1) folds stay as one term."""
    out = [] if out is None else out
    x = sh.nodes[n]
    if x["op"] == "addss" and len(x["args"]) == 2:
        for a in x["args"]:
            terms(sh, a, out)
    elif x["op"] == "addss" and len(x["args"]) == 1 and x.get("const") == 0.0:  # + 0.0 from an empty pool slot
        terms(sh, x["args"][0], out)
    else:
        out.append(n)
    return out


def spine(sh, root):
    cone = _cone(sh, root)
    base = next(i for i in sorted(cone) if sh.nodes[i]["op"] == "in" and "action.modifier" in sh.nodes[i]["label"])
    cones = {}

    def has_base(a):
        if a not in cones:
            cones[a] = base in _cone(sh, a)
        return cones[a]
    steps, n = [], root
    while n != base:
        x = sh.nodes[n]
        nxt = [a for a in x["args"] if has_base(a)]
        if not nxt:
            break
        main = nxt[0]
        others = [a for a in x["args"] if a != main]
        steps.append({"node": n, "op": x["op"], "rva": x["rva"], "value": f32(x["value_u"]), "const": x.get("const"),
                      "others": others})
        n = main
    return steps


def main():
    p = argparse.ArgumentParser()
    p.add_argument("out", type=Path)
    p.add_argument("--outfit", default="")
    p.add_argument("--inventory", default=None)
    p.add_argument("--buff", type=float, default=None)
    p.add_argument("--names", type=Path, default=NAMES)
    args = p.parse_args()
    from fast_eval import Evaluator
    from regress_fast import BASE, CACHE, IN, load_inventory, pieces
    ev = Evaluator(CACHE, BASE / "capture.json", BASE / "ENTRY_REQUEST.json", BASE / "RETAINED_STATIC_CELLS.json",
                   [IN / "grade_table_cells.json"], buff=args.buff)
    plist = pieces(args.outfit, load_inventory(args.inventory)) if args.outfit else []
    sh, root, result, facts = trace(ev, plist)
    damage = struct.unpack("<f", bytes.fromhex(result["output_record_not_automatically_success"]["value_hex"]))[0]
    fields, classes, failures = group_fields(ev.model)
    names = effect_names(ev.model, args.names)

    def describe(label):
        field = leaf_field(label)
        if not field:
            return None
        groups = fields.get(field, [])
        return [{"group": f"{g:#06x}", "class": classes.get(g), "effects": names.get(g, [])[:6]} for g in groups]

    zones = []
    for step in spine(sh, root):
        for other in step["others"]:
            leaves = sorted({sh.nodes[i]["label"] for i in _cone(sh, other) if sh.nodes[i]["op"] == "in"})
            zones.append({"rva": step["rva"], "op": step["op"], "spine_value": step["value"],
                          "operand_value": f32(sh.nodes[other]["value_u"]), "expression": infix(sh, other)[:1500],
                          "terms": [{"value": f32(sh.nodes[k]["value_u"]), "expression": infix(sh, k)[:500],
                                     "fields": sorted({sh.nodes[i]["label"] for i in _cone(sh, k) if sh.nodes[i]["op"] == "in"})}
                                    for k in terms(sh, other[0] if False else _pool_root(sh, other))],
                          "fields": {leaf: describe(leaf) for leaf in leaves}})
        if not step["others"]:
            zones.append({"rva": step["rva"], "op": step["op"], "spine_value": step["value"], "const": step["const"]})
    decisions = defaultdict(lambda: {"count": 0})
    for d in sh.decisions:
        inputs = sorted({sh.nodes[i]["label"] for i in _cone(sh, d["compare"]) if sh.nodes[i]["op"] == "in"})
        row = decisions[(d["rva"], tuple(inputs))]
        row.update(rva=d["rva"], cond=d["cond"], taken=d["taken"], inputs=inputs, compare=infix(sh, d["compare"])[:400])
        row["count"] += 1
    conditional = [{"group": f"{g:#06x}", "effects": names.get(g, [])[:6],
                    "fields": [f"{o}+{a:#x}" for (o, a), gs in fields.items() if g in gs]}
                   for g, c in sorted(classes.items()) if c == "conditional27"]
    args.out.mkdir(parents=True, exist_ok=True)
    out = {"damage": damage, "facts": facts, "zones": zones, "decisions": list(decisions.values()),
           "conditional_groups": conditional, "materializer_failures": failures}
    (args.out / "ZONES.json").write_text(json.dumps(out, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    lines = [f"damage {damage}", ""]
    for z in zones:
        if "expression" in z:
            lines.append(f"[{z['rva']}] {z['op']} by {z['operand_value']:.7g}  (spine after: {z['spine_value']:.7g})")
            lines.append("    " + z["expression"][:600])
            if len(z["terms"]) > 1:
                for term in z["terms"]:
                    lines.append(f"      term {term['value']:.7g}: {term['expression'][:300]}")
            for leaf, groups in z["fields"].items():
                if groups:
                    lines.append(f"      {leaf}: " + "; ".join(f"{g['group']}[{g['class']}] " + "/".join(g["effects"][:3]) for g in groups[:8]))
        else:
            lines.append(f"[{z['rva']}] {z['op']} const={z.get('const')}  (spine after: {z['spine_value']:.7g})")
    (args.out / "ZONES.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(json.dumps({"damage": damage, "zones": len(zones), "decisions": len(decisions), "conditional_groups": len(conditional),
                      "materializer_failures": len(failures)}))


if __name__ == "__main__":
    main()
