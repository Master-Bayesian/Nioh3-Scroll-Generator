"""Full model pipeline to weapon records and every other STAT word, under own and foreign attributes. argv: capture.json ..."""
import struct, sys
sys.path.insert(0, "F:/Nioh3_r16_build/worktrees/research-damage/research/damage/evaluator/src")
from composed_model import ComposedModel
CACHE = "F:/Nioh3_r16_build/cache"
for c in sys.argv[1:]:
    m = ComposedModel(CACHE, c)
    print(c[-40:], "mode", m.mode, "attrs", m.raw_attrs)
    out = {}
    for label, attrs in (("own", m.raw_attrs), ("old", [25, 150, 150, 40, 50, 20, 50]), ("new", [37, 150, 27, 26, 80, 15, 150])):
        keep = m.raw_attrs, m.sync_attrs
        m.raw_attrs, m.sync_attrs = list(attrs), list(attrs)
        stat = bytearray(m.stat)
        struct.pack_into("<Q", stat, 8, 0); struct.pack_into("<Q", stat, 0x7D8, m.N.ATTR); stat[0x7D0] = 0
        src = m.B.typed_source(m.attr, m.attr_known, m.mode)
        src["thresholds_u32"] = list(attrs)
        full = {k: src[k] for k in ("map_35d0", "map_4728")}
        src["map_35d0"], src["map_4728"] = [], []
        src["side_state"]["banks"] = [[], []]
        src, _ = m._prepared(stat, m.inv, src)
        body = m.BODY.body_stage(stat_hex=bytes(stat[:0x800]).hex(), stat_address_u64=m.N.STAT, banks=m.banks(m.inv), active_bank_i32=m.active,
            attribute_context={"raw_i32": attrs, "synchronized_i32": attrs}, owner=None, source_factors_f32=m.factors,
            globals={"profile_byte45": m.profile, "progression_byte10": m.progression},
            source_state={"attribute_source": src, "owner_core_alias": "distinct"})
        stat[:0x800] = bytes.fromhex(body["stat_hex"])
        src = body["final_source_state"]["attribute_source"]
        if struct.unpack_from("<H", m.inv, m.active * 0xF70 + 15 * 0xE8)[0]:
            src = m._slot15(stat, m.inv, src)
        src.update(full)  # the captured aggregate replaces the rebuilt one
        struct.pack_into("<f", stat, 0x7A0, m._load_factor(stat))
        struct.pack_into("<Q", stat, 8, m.N.OWNER); struct.pack_into("<Q", stat, 0x7C0, m.F.DEST)
        r = m.L.apply(stat_hex=bytes(stat).hex(), materializer_hex=m.d0.hex(), inventory_hex=bytes(m.inv).hex(),
                      synchronized_i32=attrs, owner_mode_i32=m.mode, source=src)
        s = bytes.fromhex(r["stat_hex"]); d = bytes.fromhex(r["materializer_hex"])
        out[label] = (s, d)
        m.raw_attrs, m.sync_attrs = keep
    own, _ = out["own"]
    print(" record0 captured", struct.unpack_from("<4I", m.stat, 0x24), "model", struct.unpack_from("<4I", own, 0x24))
    n = 0
    for at in range(0, 0x7A0, 4):
        a, b = m.stat[at:at + 4], own[at:at + 4]
        if a != b and at not in (8, 12):
            n += 1
            if n < 70:
                print("  stat", hex(at), struct.unpack("<I", a)[0], struct.unpack("<I", b)[0], "%.4f %.4f" % (struct.unpack("<f", a)[0], struct.unpack("<f", b)[0]))
    print(" stat words differing", n)
    a, b = out["old"], out["new"]
    print(" attribute-sensitive STAT words", [(hex(at), struct.unpack("<I", a[0][at:at + 4])[0], struct.unpack("<I", b[0][at:at + 4])[0]) for at in range(0, 0x800, 4) if a[0][at:at + 4] != b[0][at:at + 4]][:50])
    print(" attribute-sensitive D words", [(hex(at), round(struct.unpack("<f", a[1][at:at + 4])[0], 4), round(struct.unpack("<f", b[1][at:at + 4])[0], 4)) for at in range(0, 0xB00, 4) if a[1][at:at + 4] != b[1][at:at + 4]][:50])
