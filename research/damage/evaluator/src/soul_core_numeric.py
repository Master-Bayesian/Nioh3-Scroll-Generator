"""Soul core attack and defense from the passive table (manager+0x118, dumped live, hash equals the 10-16 receipt)."""
import json, struct, sys
from pathlib import Path
sys.path.insert(0, "F:/Nioh3_r16_build/worktrees/research-damage/research/damage/evaluator/src")
from composed_model import ComposedModel
CACHE = "F:/Nioh3_r16_build/cache"
W = "F:/Nioh3_r16_build/work/"
blob = Path(W + "live-1105/passive118_rows.bin").read_bytes()
receipt = json.loads(Path(CACHE + "/original/nioh3-r10-pc202-20261016/project-source/model/evidence/actor-tables/capture/CAPTURE_RECEIPT.json").read_bytes())
table = next(r for r in receipt["tables"] if r["name"] == "passive118")
idx = {int(k): v for k, v in table["key_to_index"].items()}
print("row hash matches 10-16 receipt:", __import__("hashlib").sha256(blob).hexdigest().upper() == table["rows_blob"]["sha256"].upper(), len(blob), table["rows_blob"]["size"])
for p in ("live1021-h3-try4", "live-1104/split/frame01"):
    m = ComposedModel(CACHE, W + p + "/capture.json")
    import passive118_stage as P
    T, Mm = P.T, P.M
    for s in range(8):
        item = m.attr[0x80 + s * 0xE8:0x80 + (s + 1) * 0xE8]
        key = struct.unpack_from("<H", item)[0]
        if not key or key not in T.ITEMS.indices:
            continue
        param = T.ITEMS.row(key)
        pk = T.unpack("H", param, 0x180)
        row = blob[8 + idx[pk] * 0x54:8 + (idx[pk] + 1) * 0x54] if pk in idx else None
        if row is None:
            print(p, "slot", s, hex(key), "no passive row", hex(pk)); continue
        mk = P.Masked(item.hex(), "ff" * 0xE8, 0xE8, "item")
        P._TABLE = type("X", (), {"lookup": lambda self, k: row, "indices": {pk: idx[pk]}})()
        d = P.numeric(mk, m.profile, param, row)
        print(p, "slot", s, hex(key), "passive", hex(pk), "attack", d["attack_increment_u16"], "defense", d["defense_increment_u32"], "level", d["level_index"], "plus", d["plus_index"])
