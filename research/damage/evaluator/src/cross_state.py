"""Cross-state prediction: from the 10-16 baseline, rebuild a later character state from inputs and predict its hit.

Inputs of the target: attributes, inventory, named layers from another day, guardian attack, soul core attack (measured).
Nothing else of the target capture enters the prediction; its STAT and D are only compared afterwards.
This is the 10-23 case that the delta model missed (predicted 1188, game 884).
argv: layers.json out.json
"""
import json
import struct
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
P = Path("F:/Nioh3_r16_build/deliverables/r16-damage-evaluator-v1-20261021")
sys.path.insert(0, str(P))
from composed_model import ComposedModel, MAPS, load_layers  # noqa: E402
from fast_eval import Evaluator  # noqa: E402

BASE, IN = P / "inputs/baseline-1016", P / "inputs"
CACHE = "F:/Nioh3_r16_build/cache"
W = "F:/Nioh3_r16_build/work/"
layers_path, out = sys.argv[1:3]
layers = load_layers(layers_path)
NAMES = ["title_blessings", "skills", "clan_blessing", "soul_core:sand_hag", "elixir_upgrades", "area_completion", "unnamed_progress", "guardian:kusanagi"]
CORE_ATTACK = 164  # core 0xBB71 in slot 0, from the passive table (soul_core_numeric.py), equal to the measured value
CORE_DEFENSE_CHANGE = 156  # the second 0x7BD9 of the 10-16 state, from the passive table
CORE2 = {"key": 0x3336, "attack": 116, "defense": 111, "groups": {0x8C1: 19, 0xECC1: 15, 0x5E5D: 11, 0x83FD: 5}}  # slot 1 on 11-06, table and item record
PLUS8_WEAPON = 35  # record +8 minus the attack group, constant on three samurai weapons
TARGETS = [("blind-1023/h1", 884), ("blind-1023/base", 793), ("blind-1023/h2", 793), ("live1022-imbue-plain", None)]
ev = Evaluator(CACHE, BASE / "capture.json", BASE / "ENTRY_REQUEST.json", BASE / "RETAINED_STATIC_CELLS.json",
               [IN / "grade_table_cells.json", IN / "default_d_cells.json"], observe_only=True)
base = ev.model
print("baseline replay", ev.evaluate([])["damage"])


def pipeline(m, frame, names=None):
    """Weapon, body, slot 15 and set stages, then the materializer, on a common frame; names=None keeps the captured aggregate."""
    import guardian_stage as G
    stat = bytearray(frame)
    for i in range(5):  # records are rebuilt, not accumulated
        struct.pack_into("<II", stat, 0x24 + 64 * i, 0, 0)
    struct.pack_into("<Q", stat, 8, 0)
    struct.pack_into("<Q", stat, 0x7D8, m.N.ATTR)
    stat[0x7D0] = 0
    source = m.B.typed_source(m.attr, m.attr_known, m.mode)
    source["thresholds_u32"] = [a & 0xFFFFFFFF for a in m.raw_attrs]
    full = {k: source[k] for k in ("map_35d0", "map_4728")}
    source["map_35d0"], source["map_4728"] = [], []
    source["side_state"]["banks"] = [[], []]
    source, _ = m._prepared(stat, m.inv, source)
    body = m.BODY.body_stage(stat_hex=bytes(stat[:0x800]).hex(), stat_address_u64=m.N.STAT, banks=m.banks(m.inv), active_bank_i32=m.active,
                             attribute_context={"raw_i32": m.raw_attrs, "synchronized_i32": m.sync_attrs}, owner=None,
                             source_factors_f32=m.factors, globals={"profile_byte45": m.profile, "progression_byte10": m.progression},
                             source_state={"attribute_source": source, "owner_core_alias": "distinct"})
    stat[:0x800] = bytes.fromhex(body["stat_hex"])
    source = body["final_source_state"]["attribute_source"]
    if struct.unpack_from("<H", m.inv, m.active * 0xF70 + 15 * 0xE8)[0]:
        source = m._slot15(stat, m.inv, source)
    done = m.SET.expand_set_sources(owner_mode_i32_or_null=m.mode, stat_byte7d0=stat[0x7D0], stat_byte7d1=stat[0x7D1], source=source)
    source = done["source"]
    source["thresholds_u32"] = [a & 0xFFFFFFFF for a in m.raw_attrs]
    if names is None:
        source.update(full)
    else:
        from composed_model import add
        for name in names:
            add(source, m.mode, layers[name].get(m.mode, {}))
    struct.pack_into("<f", stat, 0x7A0, m._load_factor(stat))
    struct.pack_into("<Q", stat, 8, m.N.OWNER)
    struct.pack_into("<Q", stat, 0x7C0, m.F.DEST)
    r = m.L.apply(stat_hex=bytes(stat).hex(), materializer_hex=m.d0.hex(), inventory_hex=bytes(m.inv).hex(), synchronized_i32=m.raw_attrs,
                  owner_mode_i32=m.mode, source=source)
    c = m.attr
    guardian = struct.unpack_from("<IHH", c, 0x58 + c[0x70] * 8)
    n = G.numeric(guardian[0], guardian[1])
    return bytes.fromhex(r["stat_hex"]), bytes.fromhex(r["materializer_hex"]), source, n["attack_increment_u32"] + CORE_ATTACK


frame = bytearray(base.stat)
s_base, d_base, _, _ = pipeline(base, frame)
report = []
def predict(path, observed, attrs=None, names=None, gain_delta=0, defense_delta=0, label=None):
    t = ComposedModel(CACHE, W + path + "/capture.json")
    if attrs:
        t.raw_attrs, t.sync_attrs = list(attrs), list(attrs)
    layers["state_1023"] = {0: {0x3E52: 0}}  # zero-value entry of the 10-23 soul core state, absent on 11-04
    s_t, d_t, source, gain = pipeline(t, frame, (names or NAMES) + ["state_1023"])
    gain += gain_delta
    want = {e["key_u16"]: e["base_u32"] + e["duplicate_u16"] for e in t.captured_source[MAPS[0]]}
    got = {e["key_u16"]: e["base_u32"] + e["duplicate_u16"] for e in source[MAPS[0]]}
    wrong_groups = {hex(g): (want.get(g), got.get(g)) for g in set(want) | set(got) if want.get(g) != got.get(g)}
    pred = bytearray(base.stat)
    for at in range(0, 0x7A0, 4):
        if at in (8, 12):
            continue
        a, b = s_base[at:at + 4], s_t[at:at + 4]
        if a == b:
            continue
        ia, ib, ic = (struct.unpack("<I", x)[0] for x in (a, b, pred[at:at + 4]))
        if max(ia, ib, ic) < 1 << 24:
            struct.pack_into("<I", pred, at, (ic + ib - ia) & 0xFFFFFFFF)
        else:
            fa, fb, fc = (struct.unpack("<f", x)[0] for x in (a, b, pred[at:at + 4]))
            struct.pack_into("<f", pred, at, fc + fb - fa)
    atk = got.get(0x6A7C, 0)
    pred[0x7A0:0x7A4] = s_t[0x7A0:0x7A4]
    pred[0x7B8:0x7BC] = s_t[0x7B8:0x7BC]
    for i in range(5):
        v = struct.unpack_from("<I", s_t, 0x24 + 64 * i)[0] + gain
        struct.pack_into("<II", pred, 0x24 + 64 * i, v, v)
        struct.pack_into("<I", pred, 0x2C + 64 * i, atk + (PLUS8_WEAPON if i < 2 else 0))
    struct.pack_into("<I", pred, 0x12C, atk)
    import base_prefix as BP
    seeds = BP.U.stat_seeds(t.raw_attrs, t.sync_attrs, t.mode)
    old_seeds = BP.U.stat_seeds(base.raw_attrs, base.sync_attrs, base.mode)
    for k, v in seeds["u32_fields"].items():
        if k != "0x1c4":
            struct.pack_into("<I", pred, int(k, 16), v)
    struct.pack_into("<I", pred, 0x1C4, seeds["u32_fields"]["0x1c4"] + 11)
    for k, v in seeds["f32_fields"].items():
        struct.pack_into("<f", pred, int(k, 16), v["value"])
    def hp(x):
        return BP.U.hp_seed(struct.unpack_from("<I", x.attr, 0x30)[0], x.sync_attrs[0], x.sync_attrs[2])["value_u32"]
    struct.pack_into("<I", pred, 0x18, struct.unpack_from("<I", pred, 0x18)[0] + hp(t) - hp(base))
    struct.pack_into("<I", pred, 0x1C8, struct.unpack_from("<I", pred, 0x1C8)[0] - CORE_DEFENSE_CHANGE + defense_delta)
    # Comparison only: which STAT and D words still differ from the target capture.
    stat_wrong = [hex(at) for at in range(0, 0x7C0, 4) if at not in (8, 12) and pred[at:at + 4] != t.stat[at:at + 4]]
    effect = [at for at in range(0, 0xB00, 4) if d_t[at:at + 4] != t.d0[at:at + 4] or d_base[at:at + 4] != t.d0[at:at + 4]]
    d_wrong = [hex(at) for at in effect if d_t[at:at + 4] != t.d[at:at + 4]]
    writes = {}
    for at in range(0, 0x7C0, 4):
        if at not in (8, 12) and pred[at:at + 4] != base.stat[at:at + 4]:
            writes[base.stat_addr + at] = bytes(pred[at:at + 4])
    for at in effect:
        writes[base.d_addr + at] = d_t[at:at + 4]
    u = lambda b, o: struct.unpack_from("<I", b, o)[0]
    packet = (u(base.main, 8) + (u(pred, 0x24) + u(pred, 0x2C)) - (u(base.stat, 0x24) + u(base.stat, 0x2C))) & 0xFFFFFFFF
    writes[base.main_addr + 8] = struct.pack("<I", packet)
    writes[base.main_addr + 0xC] = struct.pack("<I", u(pred, 0x30))
    writes[base.main_addr + 0x10] = struct.pack("<I", u(pred, 0x34))
    writes[base.main_addr + 0x14] = struct.pack("<I", u(pred, 0x3C))
    # The formula reads weight from the current-player STAT, a separate object from the source STAT.
    writes[base.R["current_player.external_STAT"][0] + 0x7B8] = bytes(pred[0x7B8:0x7BC])
    # Attribute reflection reads the attributes inline at STAT+0x844 through grades D+0x30.., beyond the materialized range.
    for i, v in enumerate(t.raw_attrs):
        writes[base.stat_addr + 0x844 + 4 * i] = struct.pack("<i", v)
    r = ev.evaluate([], writes)
    row = {"target": label or path, "observed": observed, "predicted": r["damage"], "status": r["status"], "wrong_groups": wrong_groups,
           "stat_words_wrong": stat_wrong, "d_words_wrong": d_wrong, "packet": packet, "target_packet": struct.unpack_from("<3I", t.main, 8)}
    report.append(row)
    print(json.dumps(row, ensure_ascii=False), flush=True)
    return r["damage"]


for path, observed in TARGETS:
    predict(path, observed)
H1 = "blind-1023/h1"
NOW = [37, 150, 27, 26, 80, 15, 150]
layers["core2"] = {0: CORE2["groups"]}
WORN = NAMES + ["core2"]
SWAP = [n for n in WORN if n != "guardian:kusanagi"] + ["guardian:phoenix"]


def moved(src, dst, n):
    a = list(NOW)
    a[src] -= n
    a[dst] += n
    return a


def worn(observed=None, attrs=None, names=None, guardian_delta=0, label=None):
    return predict(H1, observed, attrs, names or WORN, gain_delta=CORE2["attack"] + guardian_delta, defense_delta=CORE2["defense"], label=label)


frozen = {"before the second core (884)": predict(H1, 884, label="same"),
          "second core 0x3336 (1038, found after diagnosis, not blind)": worn(1038, label="core2"),
          "attributes 37,150,27,36,80,15,140": worn(attrs=moved(6, 3, 10), label="a"),
          "attributes 37,150,37,26,80,15,140": worn(attrs=moved(6, 2, 10), label="b"),
          "attributes 37,140,27,26,80,15,160": worn(attrs=moved(1, 6, 10), label="c"),
          "attributes 37,150,27,26,90,15,140": worn(attrs=moved(6, 4, 10), label="d"),
          "attributes 37,150,27,26,70,15,160": worn(attrs=moved(4, 6, 10), label="e"),
          "guardian phoenix on samurai": worn(names=SWAP, guardian_delta=70 - 71, label="phoenix")}
print(json.dumps(frozen, ensure_ascii=False))
report.append({"frozen_predictions": frozen})
Path(out).write_text(json.dumps(report, ensure_ascii=False, indent=1), encoding="utf-8")
