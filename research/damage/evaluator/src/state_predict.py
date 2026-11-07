"""Predict a hit from character inputs, on the 10-16 baseline frame.

Inputs: outfit (a capture's inventory), attributes, guardian, soul cores and the fixed progress layers. Nothing else of the
target capture enters; its STAT and D are only compared afterwards. Soul cores come from their item record and the passive
table, guardians from their table, so neither is measured. The record +8 field (PLUS8_WEAPON) still is.
"""
import json
import struct
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
P = Path("F:/Nioh3_r16_build/deliverables/r16-damage-evaluator-v1-20261021")
sys.path.insert(0, str(P))
from composed_model import ComposedModel, MAPS, add, load_layers  # noqa: E402
from fast_eval import Evaluator  # noqa: E402

BASE, IN = P / "inputs/baseline-1016", P / "inputs"
CACHE = "F:/Nioh3_r16_build/cache"
W = "F:/Nioh3_r16_build/work/"
PASSIVE_ROWS = W + "live-1105/passive118_rows.bin"
RECEIPT = CACHE + "/original/nioh3-r10-pc202-20261016/project-source/model/evidence/actor-tables/capture/CAPTURE_RECEIPT.json"
FIXED = ["title_blessings", "skills", "clan_blessing", "elixir_upgrades", "area_completion", "unnamed_progress"]
PLUS8_WEAPON = 35  # record +8 minus the attack group, constant on three samurai weapons
BASE_DEFENSE_SOURCES = 312 + 66  # two 0x7BD9 cores and 草薙 in the 10-16 baseline
ITEM = 0xE8
CORE_SLOTS = 2  # the attribute and record effects of passive items come from slots 0 and 1


def item_effects(item):
    """Effect groups of a passive item record; a zero value still makes an entry."""
    rows = {}
    for i in range(7):
        row = item[0x30 + 24 * i:0x30 + 24 * (i + 1)]
        group, value = struct.unpack_from("<H", row, 4)[0], struct.unpack_from("<I", row, 12)[0]
        if group:
            rows[group] = rows.get(group, 0) + value
    return rows


class Predictor:
    def __init__(self, layers_path):
        self.layers = load_layers(layers_path)
        self.ev = Evaluator(CACHE, BASE / "capture.json", BASE / "ENTRY_REQUEST.json", BASE / "RETAINED_STATIC_CELLS.json",
                            [IN / "grade_table_cells.json", IN / "default_d_cells.json", HERE.parents[1] / "data/MANAGER130_CELLS.json"], observe_only=True)
        self.base = self.ev.model
        self.frame = bytearray(self.base.stat)
        self.base_replay = self.ev.evaluate([])["damage"]
        self.s_base, self.d_base, _ = self._pipeline(self.base, None)
        self.models = {}
        import passive118_stage as PS
        self.PS = PS
        self.rows = Path(PASSIVE_ROWS).read_bytes()
        receipt = json.loads(Path(RECEIPT).read_bytes())
        table = next(r for r in receipt["tables"] if r["name"] == "passive118")
        self.row_index = {int(k): v for k, v in table["key_to_index"].items()}

    def core(self, item):
        """Attack, defense and effects of one soul core item record."""
        key = struct.unpack_from("<H", item)[0]
        if not key:
            return None
        PS, T = self.PS, self.PS.T
        param = T.ITEMS.row(key)
        passive = T.unpack("H", param, 0x180)
        at = self.row_index[passive]
        row = self.rows[8 + at * 0x54:8 + (at + 1) * 0x54]
        PS._TABLE = type("Row", (), {"lookup": lambda self, k: row, "indices": {passive: at}})()
        n = PS.numeric(PS.Masked(bytes(item).hex(), "ff" * ITEM, ITEM, "item"), self.base.profile, param, row)
        return {"key": key, "attack": n["attack_increment_u16"], "defense": n["defense_increment_u32"], "groups": item_effects(item)}

    def guardian(self, key, level=50, field72=146):
        import guardian_stage as G
        n = G.numeric(key, level)
        groups = {}
        for i in range(6):
            d = G.descriptor(key, i)
            if d["key_u16"] and d["required_u16"] <= field72:
                groups[d["key_u16"]] = groups.get(d["key_u16"], 0) + G.skill_value(key, i, level)[0]
        return {"key": key, "attack": n["attack_increment_u32"], "defense": n["defense_increment_u32"], "groups": groups}

    def _pipeline(self, m, groups):
        """Weapon, body, slot 15 and set stages, then the materializer, on a common frame. groups=None keeps the captured aggregate."""
        stat = bytearray(self.frame)
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
        if groups is None:
            source.update(full)
        else:
            for totals in groups:
                add(source, m.mode, totals)
        struct.pack_into("<f", stat, 0x7A0, m._load_factor(stat))
        struct.pack_into("<Q", stat, 8, m.N.OWNER)
        struct.pack_into("<Q", stat, 0x7C0, m.F.DEST)
        r = m.L.apply(stat_hex=bytes(stat).hex(), materializer_hex=m.d0.hex(), inventory_hex=bytes(m.inv).hex(), synchronized_i32=m.raw_attrs,
                      owner_mode_i32=m.mode, source=source)
        return bytes.fromhex(r["stat_hex"]), bytes.fromhex(r["materializer_hex"]), source

    def model(self, capture):
        if capture not in self.models:
            self.models[capture] = ComposedModel(CACHE, W + capture + "/capture.json")
        return self.models[capture]

    def predict(self, capture, attrs=None, guardian=None, cores=None):
        """capture supplies the outfit; guardian (a dict) and cores (dicts) default to what that capture wore."""
        t, base = self.model(capture), self.base
        original = t.raw_attrs, t.sync_attrs
        if attrs:
            t.raw_attrs, t.sync_attrs = list(attrs), list(attrs)
        try:
            c = t.attr
            if guardian is None:
                guardian = self.guardian(*struct.unpack_from("<IH", c, 0x58 + c[0x70] * 8))
            if cores is None:
                cores = [self.core(c[0x80 + s * ITEM:0x80 + (s + 1) * ITEM]) for s in range(CORE_SLOTS)]
            cores = [x for x in cores if x]
            groups = [self.layers[n].get(t.mode, {}) for n in FIXED] + [guardian["groups"]] + [x["groups"] for x in cores]
            s_t, d_t, source = self._pipeline(t, groups)
            return self._evaluate(t, s_t, d_t, source, guardian, cores)
        finally:
            t.raw_attrs, t.sync_attrs = original

    def _evaluate(self, t, s_t, d_t, source, guardian, cores):
        base, s_base, d_base = self.base, self.s_base, self.d_base
        gain = guardian["attack"] + sum(x["attack"] for x in cores)
        defense = guardian["defense"] + sum(x["defense"] for x in cores)
        got = {e["key_u16"]: e["base_u32"] + e["duplicate_u16"] for e in source[MAPS[t.mode]]}
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
        for k, v in seeds["u32_fields"].items():
            if k != "0x1c4":
                struct.pack_into("<I", pred, int(k, 16), v)
        struct.pack_into("<I", pred, 0x1C4, seeds["u32_fields"]["0x1c4"] + 11)
        for k, v in seeds["f32_fields"].items():
            struct.pack_into("<f", pred, int(k, 16), v["value"])

        def hp(x):
            return BP.U.hp_seed(struct.unpack_from("<I", x.attr, 0x30)[0], x.sync_attrs[0], x.sync_attrs[2])["value_u32"]
        struct.pack_into("<I", pred, 0x18, struct.unpack_from("<I", pred, 0x18)[0] + hp(t) - hp(base))
        struct.pack_into("<I", pred, 0x1C8, struct.unpack_from("<I", pred, 0x1C8)[0] - BASE_DEFENSE_SOURCES + defense)
        effect = [at for at in range(0, 0xB00, 4) if d_t[at:at + 4] != t.d0[at:at + 4] or d_base[at:at + 4] != t.d0[at:at + 4]]
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
        # Attribute reflection reads the attributes inline at STAT+0x844 through grades in D, beyond the materialized range.
        for i, v in enumerate(t.raw_attrs):
            writes[base.stat_addr + 0x844 + 4 * i] = struct.pack("<i", v)
        r = self.ev.evaluate([], writes)
        return {"damage": r["damage"], "status": r["status"], "packet": packet, "defense": u(pred, 0x1C8), "hp": u(pred, 0x18), "groups": len(got), "aggregate": got}
