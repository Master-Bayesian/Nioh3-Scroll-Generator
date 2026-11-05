"""Compose the effect aggregate from its sources instead of carrying the captured one forward.

aggregate(form) = equipment (native stages on an emptied source) + named layers (SOURCE_LAYERS.json).
D and the materializer's STAT writes are then a pure function of that aggregate (819400), so any loadout of items,
guardian, soul core, clan blessing, skills or blessings needs no delta on a captured baseline.
"""
import copy
import json
import struct
from pathlib import Path

from outfit_model import OutfitModel, Unknown

MAPS = {0: "map_35d0", 1: "map_4728"}


def load_layers(path):
    raw = json.loads(Path(path).read_bytes())
    return {name: {int(f): {int(g, 16): v for g, v in rows.items()} for f, rows in layer.items()} for name, layer in raw.items()}


def add(source, form, totals):
    entries = source[MAPS[form]]
    index = {e["key_u16"]: e for e in entries}
    for group, value in totals.items():
        if group in index:
            index[group]["base_u32"] = (index[group]["base_u32"] + value) & 0xFFFFFFFF
            if value < 0 and not index[group]["base_u32"]:  # a removed source leaves no entry; a zero entry still materializes
                entries.remove(index.pop(group))
        else:
            entries.append({"key_u16": group, "base_u32": value & 0xFFFFFFFF, "duplicate_u16": 0, "anonymous_u8": 0})
            index[group] = entries[-1]
    if len(entries) > 256:
        raise Unknown("aggregate exceeds its native capacity")


class ComposedModel(OutfitModel):
    def __init__(self, cache, capture, form=None):
        super().__init__(cache, capture, observe_only=True)
        self.captured_mode = self.mode
        if form is not None:
            self.mode = self.active = form
        self.captured_source = self.B.typed_source(self.attr, self.attr_known, self.mode)

    def items(self, inv=None):
        """Equipment aggregate of `inv`: weapons, body, slot 15, set rewards; returns the source."""
        inv = self.inv if inv is None else inv
        source = copy.deepcopy(self.captured_source)
        source["map_35d0"], source["map_4728"] = [], []
        source["side_state"]["banks"] = [[], []]
        stat = bytearray(self.stat)
        struct.pack_into("<Q", stat, 8, 0)
        struct.pack_into("<Q", stat, 0x7D8, self.N.ATTR)
        stat[0x7D0] = 0
        source, _ = self._prepared(stat, inv, source)
        body = self.BODY.body_stage(stat_hex=bytes(stat[:0x800]).hex(), stat_address_u64=self.N.STAT, banks=self.banks(inv),
                                    active_bank_i32=self.active,
                                    attribute_context={"raw_i32": self.raw_attrs, "synchronized_i32": self.sync_attrs},
                                    owner=None, source_factors_f32=self.factors,
                                    globals={"profile_byte45": self.profile, "progression_byte10": self.progression},
                                    source_state={"attribute_source": source, "owner_core_alias": "distinct"})
        if any(e["result"]["status"] != "completed" for e in body["ordered_source_events"]):
            raise Unknown("source collector stopped")
        stat[:0x800] = bytes.fromhex(body["stat_hex"])
        source = body["final_source_state"]["attribute_source"]
        if struct.unpack_from("<H", inv, self.active * 0xF70 + 15 * 0xE8)[0]:
            source = self._slot15(stat, inv, source)
        done = self.SET.expand_set_sources(owner_mode_i32_or_null=self.mode, stat_byte7d0=stat[0x7D0], stat_byte7d1=stat[0x7D1], source=source)
        if done["stat_byte7d1"] != stat[0x7D1]:
            raise Unknown("set expansion requests a refresh pass")
        return done["source"]

    def materialize(self, source, inv=None):
        """D and STAT from an aggregate. The default D page and the captured STAT fields outside 819400 are the frame."""
        inv = self.inv if inv is None else inv
        stat = bytearray(self.stat)
        struct.pack_into("<Q", stat, 8, self.N.OWNER)
        struct.pack_into("<Q", stat, 0x7D8, self.N.ATTR)
        struct.pack_into("<Q", stat, 0x7C0, self.F.DEST)
        result = self.L.apply(stat_hex=bytes(stat).hex(), materializer_hex=self.d0.hex(), inventory_hex=bytes(inv).hex(),
                              synchronized_i32=self.raw_attrs, owner_mode_i32=self.mode, source=source)
        return bytes.fromhex(result["stat_hex"]), bytes.fromhex(result["materializer_hex"])

    def compose(self, layers, names, inv=None, delta=None):
        source = self.items(inv)
        for name in names:
            add(source, self.mode, layers[name].get(self.mode, {}))
        if delta:
            add(source, self.mode, delta)
        return source
