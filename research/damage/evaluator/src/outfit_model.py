"""Native outfit model: baseline capture + armor records -> predicted STAT and D (no after-swap values).

Runs the retained native lifts in rebuild order on the captured baseline:
  body_stage (2FB2BC: defense/toughness/weight/u16/body floats, then 55764C/5557DC source collection
  with side-bank set counting) -> expand_set_sources (3FAA98 set rewards) -> load factor (819204)
  -> materializer loop (81920E..819251 -> 819400, D and STAT writes).
Both the baseline banks and the candidate banks go through the same model; the predicted object is the
captured baseline plus (candidate model - baseline model), so stages the model does not cover cancel
instead of being guessed. Anything the model cannot decide raises Unknown.
Not covered: slot15 stage (81744C), refresh pass when set expansion requests one (stat+7D1), context
deltas (8175F4), guardian/passive stages (374F58, 554268) beyond their baseline values.
"""
import copy
import json
import struct
import sys
from pathlib import Path

PKG = Path("F:/Nioh3_r16_build/deliverables/pro-r2-return/nioh3-damage-formula-r2-pro-return-pc202-20261018")
GRADE_ROOT = Path("F:/Nioh3_r16_build/deliverables/r16-equipment-execution-20261020/candidate-production/static-binding")


class Unknown(Exception):
    pass


def roles(path):
    raw = json.loads(Path(path).read_bytes())["raw_capture"]
    entry = next(s for s in raw["snapshots"] if s["kind"] == "MAIN_entry_prestate")
    out = {}
    for r in entry["regions"]:
        if r.get("bytes_hex"):
            out.setdefault(r["role"], (int(r["address_hex"], 16), bytes.fromhex(r["bytes_hex"]),
                                       bytes.fromhex(r.get("known_mask_hex") or "ff" * (len(r["bytes_hex"]) // 2))))
    return out


class OutfitModel:
    def __init__(self, cache, baseline_capture, observe_only=False, armored_baseline=False):
        sys.path.insert(0, str(PKG / "project-source/runtime"))
        from r12_bind import prepare
        prepare(Path(cache))
        import build_forward as B
        import materializer_loop as L
        import field_materializer as F
        import r9_native as N
        import body_stage as BODY
        import set_expansion as SET
        import actor_local_stages as LOCAL
        import grade_curves
        from paths import M, T
        # The sealed r10 cache lacks the native718 grade-curve resource; Catalog() verifies this copy.
        grade_curves.ROOT = GRADE_ROOT
        self.B, self.L, self.F, self.N, self.BODY, self.SET, self.M, self.T = B, L, F, N, BODY, SET, M, T
        R = roles(baseline_capture)
        self.R = R
        self.main_addr = int(json.loads(Path(baseline_capture).read_bytes())["raw_capture"]["frames"][0]["hit"], 16)
        main_region = next((a, b) for a, b, _ in R.values() if a <= self.main_addr and self.main_addr + 0x20 <= a + len(b))
        self.main = main_region[1][self.main_addr - main_region[0]:][:0x400]
        owner = R["source.HP_owner"][1]
        self.prefix = "source.external_STAT" if struct.unpack_from("<Q", owner, 0x2D90)[0] else "source.inline_STAT"
        self.stat_addr, self.stat, self.stat_known = R[self.prefix]
        self.d_addr, self.d, self.d_known = R[self.prefix + ".D"]
        _, self.attr, self.attr_known = R[self.prefix + ".attribute_source"]
        _, self.inv, _ = R["source.two_banks"]
        self.mode = struct.unpack_from("<i", owner, 0x2C)[0]
        self.active = struct.unpack_from("<I", self.inv, 0x1EE0)[0]
        self.profile = R["profile.object"][1][0x45]
        self.progression = R["profile.object"][1][0x10]  # r9_native: progression_byte10 = PROFILE+0x10
        self.raw_attrs = list(struct.unpack_from("<7i", self.attr, 0x14))
        self.sync_attrs = list(struct.unpack_from("<7i", self.attr, 0x34))
        self.factors = LOCAL.caller_factors(caller_override_hex=None)["values_f32"]
        self.d0 = bytes.fromhex(json.loads((PKG / "evidence/r2-hits/PAGE_DUMP_pid29088.json").read_bytes())["default_D"]["value_hex"])
        self.base_model = None  # observe-only: the capture is replayed as worn, no candidate prediction
        if observe_only:
            return
        # An armored baseline relies on the delta model removing the worn armor's native stages as well.
        if not armored_baseline and any(struct.unpack_from("<H", self.inv, b * 0xF70 + s * 0xE8)[0]
                                        for b in (0, 1) for s in range(4, 13)):
            raise Unknown("baseline must wear no armor in either bank")
        self.base_model = self._model(self.inv)

    def banks(self, inv):
        return [[inv[b * 0xF70 + s * 0xE8:b * 0xF70 + (s + 1) * 0xE8].hex() for s in range(17)] for b in (0, 1)]

    def _load_factor(self, stat):
        M, T = self.M, self.T
        capacity = M.prev.field90(self.raw_attrs[2], 8 if self.mode == 0 else 10)
        params = {k: M.prev.mul(M.prev.integer_f32(T.unpack("i", T.OPTIONAL.row(k), 0x10)), T.unpack("f", T.OPTIONAL.row(k), 0x18))
                  for k in (0xDC22, 0xD84F)}
        weight = struct.unpack_from("<I", stat, 0x7B8)[0]
        ratio = M.prev.div(M.prev.integer_f32(weight), M.prev.integer_f32(capacity))
        return M.prev.add(params[0xD84F], M.prev.mul(ratio, params[0xDC22]))

    def _weapon_context(self):
        """Inputs of the prepared weapon stage (818248) read from the capture; CF8 is an explicit assumption."""
        import r9_model as R9
        import source_collectors as SC
        R, q = self.R, lambda b, o: struct.unpack_from("<Q", b, o)[0]
        owner = R["source.HP_owner"][1]
        entity = R["source.owner_entity.prefix"][1]
        ab84 = struct.unpack("<I", R["source.owner_entity.AB84"][1])[0]
        _, core, core_known = R["source.core_source"]
        style = 0
        if entity[0] & 15 == 0 and q(entity, 0x3B0):  # 817434: query entity u32 +85C
            style = struct.unpack_from("<i", R["source.query.entity.prefix"][1], 0x85C)[0]
        self.weapon_assumptions = {"config_byte_cf8": "0 assumed (no item-sync rewrite); armor predictions matched S1 under it",
                                   "unarmed_record5": "identical in baseline and candidate, excluded from deltas"}
        return R9, {
            "owner": {"u32_24": struct.unpack_from("<I", owner, 0x24)[0], "entity": {"byte0": entity[0], "u32_ab84": ab84},
                      "core_source": SC.core_projection(self.B.typed_source(core, core_known, self.mode))},
            "style_i32": style, "bank_byte_f68": [self.inv[0xF68], self.inv[0x1ED8]],
            "globals": {"profile_byte45": self.profile, "progression_byte10": self.progression,
                        "network_mode_u32_or_null": struct.unpack_from("<I", R["network"][1], 0x34)[0],
                        "config_byte_cf8": 0, "unarmed_level_u32": 0, "unarmed_plus_u32": 0}}

    def _prepared(self, stat, inv, source):
        R9, ctx = self._weapon_context()
        result = R9.prepared_stage(initial_records_hex=[bytes(stat[0x24 + 64 * i:0x64 + 64 * i]).hex() for i in range(6)],
                                   banks=self.banks(inv), active_bank_i32=self.active, bank_byte_f68=ctx["bank_byte_f68"],
                                   attribute_context={"raw_i32": self.raw_attrs, "synchronized_i32": self.sync_attrs},
                                   owner=ctx["owner"], stat_byte10=stat[0x10], stat_byte7d0=0, style_i32=ctx["style_i32"],
                                   refresh_u8=0, source_factors_f32=self.factors, globals=ctx["globals"],
                                   source_state={"attribute_source": source, "owner_core_alias": "distinct"})
        for i, record in enumerate(result["records_hex"] if "records_hex" in result else result["post_records_hex"]):
            if i < 5:
                stat[0x24 + 64 * i:0x64 + 64 * i] = bytes.fromhex(record)
        return result["final_source_state"]["attribute_source"], result

    def _slot15(self, stat, inv, source):
        """81744C slot-15 stage (the equipped scroll): flat attack on every weapon record, defense, scroll effects.

        The lifted stage refuses stat byte10 != 0 (item-sync rewrite guard); like the weapon stage this runs under
        the same no-rewrite assumption (config byte CF8 = 0) the armor and weapon predictions were validated with.
        """
        import slot15_stage as SLOT
        _, ctx = self._weapon_context()
        x = {"stat_byte10": 0, "stat_byte7d0": 0, "banks": self.banks(inv), "active_bank_i32": self.active,
             "initial_records_hex": [bytes(stat[0x24 + 64 * i:0x64 + 64 * i]).hex() for i in range(6)],
             "attribute_context": {"raw_i32": self.raw_attrs, "synchronized_i32": self.sync_attrs},
             "owner": ctx["owner"], "globals": ctx["globals"],
             "source_state": {"attribute_source": source, "owner_core_alias": "distinct"}}
        result = SLOT.stage(x, struct.unpack_from("<I", stat, 0x1C8)[0], self.mode)
        for i, record in enumerate(result["records_hex"]):
            stat[0x24 + 64 * i:0x64 + 64 * i] = bytes.fromhex(record)
        struct.pack_into("<I", stat, 0x1C8, result["defense_u32"])
        return result["final_source_state"]["attribute_source"]

    def _model(self, inv, weapons=False):
        stat = bytearray(self.stat)
        struct.pack_into("<Q", stat, 8, 0)  # owner-null body caller domain (no item sync rewrite)
        struct.pack_into("<Q", stat, 0x7D8, self.N.ATTR)
        stat[0x7D0] = 0  # 64A9BC clears the source gate before the rebuild (build_forward: stat_byte7d0 = 0)
        source = self.B.typed_source(self.attr, self.attr_known, self.mode)
        prepared = None
        if weapons:
            source, prepared = self._prepared(stat, inv, source)
        body = self.BODY.body_stage(stat_hex=bytes(stat[:0x800]).hex(), stat_address_u64=self.N.STAT,
                                    banks=self.banks(inv), active_bank_i32=self.active,
                                    attribute_context={"raw_i32": self.raw_attrs, "synchronized_i32": self.sync_attrs},
                                    owner=None, source_factors_f32=self.factors,
                                    globals={"profile_byte45": self.profile, "progression_byte10": self.progression},
                                    source_state={"attribute_source": source, "owner_core_alias": "distinct"})
        stat[:0x800] = bytes.fromhex(body["stat_hex"])
        source = body["final_source_state"]["attribute_source"]
        if struct.unpack_from("<H", inv, self.active * 0xF70 + 15 * 0xE8)[0]:
            source = self._slot15(stat, inv, source)
        expanded = self.SET.expand_set_sources(owner_mode_i32_or_null=self.mode, stat_byte7d0=stat[0x7D0],
                                               stat_byte7d1=stat[0x7D1], source=source)
        if expanded["stat_byte7d1"] != stat[0x7D1]:
            raise Unknown("set expansion requests a refresh pass (stat+7D1), not modelled")
        source = expanded["source"]
        struct.pack_into("<f", stat, 0x7A0, self._load_factor(stat))
        struct.pack_into("<Q", stat, 8, self.N.OWNER)
        struct.pack_into("<Q", stat, 0x7C0, self.F.DEST)
        result = self.L.apply(stat_hex=bytes(stat).hex(), materializer_hex=self.d0.hex(), inventory_hex=bytes(inv).hex(),
                              synchronized_i32=self.raw_attrs, owner_mode_i32=self.mode, source=source)
        rewards = [e for e in expanded["events"] if any(r.get("decision") == "submitted" for r in e["rewards"])]
        return {"stat": bytes.fromhex(result["stat_hex"]), "d": bytes.fromhex(result["materializer_hex"]),
                "materializer_writes": result["ordered_writes"], "materializer_calls": result["materialization_calls"],
                "source": source, "set_events": rewards, "body": body, "prepared": prepared}

    def predict(self, pieces):
        """pieces: [(record_E8_bytes, physical_slot)] -> dict with predicted STAT/D and change lists."""
        inv = bytearray(self.inv)
        for record, slot in pieces:
            off = self.active * 0xF70 + slot * 0xE8
            inv[off:off + 0xE8] = record
        weapons = any(slot in (0, 1, 2, 3, 13) for _, slot in pieces)
        model = self._model(bytes(inv), weapons)
        if weapons and not hasattr(self, "base_model_weapons"):
            self.base_model_weapons = self._model(self.inv, True)
        base = self.base_model_weapons if weapons else self.base_model
        out = {"approximate_words": []}
        pointers = {8, 12, 0x7C0, 0x7C4, 0x7D8, 0x7DC}
        for name, actual, size in (("stat", self.stat, 0x900), ("d", self.d, 0xB00)):
            pred = bytearray(actual)
            for at in range(0, size, 4):
                if name == "stat" and at in pointers:
                    continue
                cand, old, now = model[name][at:at + 4], base[name][at:at + 4], actual[at:at + 4]
                if cand == old:
                    continue
                if old == now:  # the model reproduces the baseline word: take the candidate value as is
                    pred[at:at + 4] = cand
                    continue
                # The captured word also carries stages the model replays again (or does not cover):
                # carry the captured value forward by the model delta.
                c, o, n = (struct.unpack("<I", x)[0] for x in (cand, old, now))
                if max(c, o, n) < 1 << 24:
                    pred[at:at + 4] = struct.pack("<I", (n + c - o) & 0xFFFFFFFF)
                else:
                    c, o, n = (struct.unpack("<f", x)[0] for x in (cand, old, now))
                    pred[at:at + 4] = struct.pack("<f", n + (c - o))
                    out["approximate_words"].append(f"{name}+{at:#x}")
            out[name] = bytes(pred)
            out[name + "_changed_words"] = sorted(i for i in range(0, size, 4) if pred[i:i + 4] != actual[i:i + 4])
        uncollected = [ev for ev in model["body"]["ordered_source_events"] if ev["result"]["status"] != "completed"]
        if uncollected:
            raise Unknown("source collector stopped: " + json.dumps([ev["result"].get("frontier") for ev in uncollected])[:300])
        out["set_rewards"] = [{"side_key": hex(e["side_key_u32"]), "rewards": [r for r in e["rewards"] if r.get("decision") == "submitted"]}
                              for e in model["set_events"] if e not in base["set_events"]]
        out["requirement_gates"] = [{"slot": r["slot"], "qualifies": r.get("qualifies_source_gate")}
                                    for r in model["body"]["record_events"] if r.get("decision") == "processed" and not r["reserve"]]
        return out
