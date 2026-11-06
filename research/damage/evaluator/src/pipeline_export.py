"""Write the tables, baseline frame and characters the Rust pipeline loads, as named binary sections."""
import json
import struct
from pathlib import Path

FORMS = ("map_35d0", "map_4728")


def sections(path, items):
    with Path(path).open("wb") as out:
        for name, data in items:
            raw = name.encode()
            out.write(struct.pack("<I", len(raw)) + raw + struct.pack("<I", len(data)) + bytes(data))


def pairs(indices):
    return b"".join(struct.pack("<II", k, v) for k, v in sorted(indices.items()))


def recipe_rows(cat, simple):
    """Group class and recipe list per group; the class names the Rust branch."""
    out = bytearray()
    classes = {"local_no_write_here": 1, "r5_destination_microrecipe": 2, "stateful70": 3, "conditional27": 4}
    for group, row in sorted(cat["index"].items()):
        kind, ops = classes[row["classification"]], []
        if kind == 2:
            for op in cat["recipes"][group]["operations"]:
                at, width, eq = int(op["destination"], 16), op["width"], op["equation"]
                scale = op.get("scale_bits")
                literal = None
                if "literal_bits" in op:
                    code, literal = 0, int(op["literal_bits"], 16)
                elif "literal" in op:
                    code, literal = 0, int(op["literal"])
                elif "D_f32" in eq:
                    code = 2 if "old_D_f32" in eq else 1
                elif "old_D_u32 + MAX_I32" in eq:
                    code = 3
                elif "MAX_I32" in eq:
                    code = 4
                elif "old_D_u32 + V" in eq:
                    code = 5
                else:
                    raise ValueError("unknown recipe equation " + eq)
                if literal is None and width != 4:
                    raise ValueError("a computed recipe store must be four bytes")
                if width > 8:  # a wide literal store goes out as eight-byte pieces
                    raw = literal.to_bytes(width, "little")
                    ops += [(code, at + k, min(8, width - k), None, int.from_bytes(raw[k:k + 8], "little")) for k in range(0, width, 8)]
                else:
                    ops.append((code, at, width, scale, literal or 0))
        elif group in simple:
            kind = 2
            for op in simple[group]["operations"]:
                at, width, eq = int(op["destination"], 16), op["width"], op["equation"]
                code = 6 if "D_f32" in eq else 7 if width == 2 else 8
                ops.append((code, at, width, op["scale_bits"], 0))
        out += struct.pack("<III", group, kind, len(ops))
        for code, at, width, scale, literal in ops:
            bits = int(scale, 16) if scale else 0
            out += struct.pack("<IIIIIQ", code, at, width, 1 if scale else 0, bits, literal)
    return out


def export_tables(p, out):
    import grade_curves
    import paths
    import r9_model as R9
    import side_sources
    import special_weapon
    import stateful_materializer as S
    m = p.base
    M, T, prev = m.M, m.T, m.M.prev
    rows = []

    def keyed(name, raw, indices):
        rows.extend([(name + ".rows", raw), (name + ".keys", pairs(indices))])

    for name, t in (("item", T.ITEMS), ("effect", T.EFFECTS), ("group", T.GROUPS), ("rarity", M.RARITY)):
        keyed(name, t.raw, t.indices)
    side = side_sources.table()
    keyed("side", side.rows, side.indices)
    try:
        special = special_weapon.table()
        keyed("special", special.rows, special.indices)
    except T.Unsupported:  # the sealed cache lacks manager970; both models then refuse flag-40000 weapons
        keyed("special", b"", {})
    for name in ("core300", "core340"):
        keyed(name, paths.CORE[name].raw, paths.CORE[name].indices)
    rows += [("level", (T.TABLES / "level_curve/rows.bin").read_bytes()), ("base70", prev.BASE), ("power90", prev.POWER), ("base90", R9._base90())]
    steps = sorted((r for r in prev.FAM_ROWS if int(r["key"], 16) == 0x3464), key=lambda r: r["x"])
    bonus = sorted((r for r in prev.FAM_ROWS if int(r["key"], 16) == 0x78C4), key=lambda r: r["x"])
    fam = [len(steps), len(bonus)]
    for rarity in range(6):
        fam += [prev.unsigned(prev.cvtt(r["columns"][rarity], 64) * 100, 32) for r in steps]
    for rarity in range(6):
        fam += [prev.unsigned(prev.cvtt(r["columns"][rarity], 64), 32) for r in bonus]
    rows.append(("fam", struct.pack(f"<{len(fam)}I", *fam)))
    readers = json.loads((M.ROOT / "analysis/alternate_weapon_readers.json").read_text())["records"]
    by_key = {int(r["selector"], 16): int(r["field_offset"], 16) for r in readers}
    alt = [by_key[0x3C2E]] + [v for kv in sorted(by_key.items()) for v in kv]
    rows.append(("alternate", struct.pack(f"<{len(alt)}I", *alt)))
    cat = grade_curves.catalog()
    index, table, at = [], bytearray(), 0
    for key, entries in sorted(cat.groups.items()):
        index += [key, at, len(entries)]
        for e in entries:
            table += e["row"]
        at += len(entries)
    rows += [("grade.rows", table), ("grade.index", struct.pack(f"<{len(index)}I", *index))]
    rows.append(("materializer", recipe_rows(m.F.catalog(), S.SIMPLE)))
    rows += [("families", T.rdata_bytes(0x3976748, 8)), ("slots5", T.rdata_bytes(0x3976820, 20)), ("slots8", T.rdata_bytes(0x3978DB0, 32))]
    if T.rdata_bytes(0x3976838, 8) != struct.pack("<2i", 0, 1):
        raise ValueError("weapon core family order differs")
    fbits = lambda v: struct.unpack("<I", struct.pack("<f", v))[0]
    if prev.constant_f32(0x3BD41B8) != T.native_f32(0x3BD41B8):
        raise ValueError("two copies of one constant differ")
    natives = [T.native_f32(a) for a in (0x3BD4890, 0x3BD4688, 0x3BD41E0, 0x3BD41B8, 0x3BD4214, 0x3BD4B18)]
    natives += [prev.constant_f32(0x3BD4894), T.native_f32(0x3BD4080)]
    option = lambda k: prev.mul(prev.integer_f32(T.unpack("i", T.OPTIONAL.row(k), 0x10)), T.unpack("f", T.OPTIONAL.row(k), 0x18))
    values = natives + [prev.mul(prev.optional(0x22AF), prev.optional(0x1F32)), prev.optional(0xE40F), S.optional_float(0x22AF), S.optional_float(0x1F32),
                        S.optional_float(0xD1F), option(0xDC22), option(0xD84F)]
    words = [fbits(v) for v in values] + [M.optional_int(0xAEAC) & 0xFFFFFFFF]
    rows.append(("consts", struct.pack(f"<{len(words)}I", *words)))
    sections(Path(out) / "tables.bin", rows)


def export_frame(p, out):
    import base_prefix as BP
    m = p.base
    if any(x != 1.0 for x in m.factors):
        raise ValueError("the Rust pipeline assumes unit source factors")
    hp = BP.U.hp_seed(struct.unpack_from("<I", m.attr, 0x30)[0], m.sync_attrs[0], m.sync_attrs[2])["value_u32"]
    q = [m.stat_addr, m.d_addr, m.main_addr, m.R["current_player.external_STAT"][0], m.N.OWNER, m.N.ATTR, m.F.DEST, struct.unpack_from("<I", m.main, 8)[0], hp]
    rows = [("stat", p.frame), ("d0", m.d0), ("s_base", p.s_base), ("d_base", p.d_base), ("addresses", struct.pack(f"<{len(q)}Q", *q))]
    sections(Path(out) / "frame.bin", rows)


def character(p, m):
    prev = m.M.prev
    R9, ctx = m._weapon_context()
    owner = ctx["owner"]
    if m.d0 != p.base.d0 or any(x != 1.0 for x in m.factors):
        raise ValueError("every character shares the default D page and unit factors")
    if R9.sync_rewrite_required(p.frame[0x10], owner, ctx["globals"])[0]:
        raise ValueError("the item synchronization rewrite is not modelled")
    core = owner["core_source"]
    cap = prev.profile_plus_cap(m.profile)
    plus_cap = prev.unsigned(prev.cvtt(prev.add(prev.integer_f32(cap), prev.optional(0xAEAC)), 64), 32)
    head = struct.pack("<iIIIiIIiII", m.mode, m.active, cap, plus_cap, ctx["style_i32"], R9.unarmed(owner, ctx["globals"], m.profile)[0],
                       struct.unpack_from("<I", m.attr, 0x30)[0], core["owner_mode_i32"], len(core["map_35d0"]), len(core["map_4728"]))
    body = bytearray(head + bytes(m.inv))
    for name in FORMS:
        for e in core[name]:
            body += struct.pack("<HHI", e["key_u16"], e["duplicate_u16"], e["base_u32"])
    return body


def export_characters(p, models, out):
    sections(Path(out) / "characters.bin", [(f"char{i}", character(p, m)) for i, m in enumerate(models)])
