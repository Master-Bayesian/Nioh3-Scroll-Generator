"""Predict the player's D object after equipping one item, from the baseline capture plus that item's record.

The baseline attribute source gets the item's affix entries inserted into map_35D0 (count at +0x41D0);
the baseline inventory gets the item record. The retained materializer then recomputes D from the default
D, and the prediction is compared with the after-swap capture's actual D. The after-swap capture is read
only for the item record (inventory bank) and for the final comparison.
argv: <pkg_root> <cache_dir> <baseline_capture> <after_capture> <record_offset_hex> <insert_index>
"""
import json
import struct
import sys
from pathlib import Path

PKG, CACHE = Path(sys.argv[1]), Path(sys.argv[2])
BASE, AFTER, REC, AT = Path(sys.argv[3]), Path(sys.argv[4]), int(sys.argv[5], 16), int(sys.argv[6])
sys.path.insert(0, str(PKG / "project-source/runtime"))
from r12_bind import prepare  # noqa: E402
prepare(CACHE)
import build_forward as B  # noqa: E402
import materializer_loop as L  # noqa: E402
import field_materializer as F  # noqa: E402
import r9_native as N  # noqa: E402

REC_SIZE, AFFIX_AT, AFFIX_STRIDE, AFFIX_SLOTS = 0xA8, 0x34, 0x18, 5
MAP, COUNT = 0x35D0, 0x35D0 + 0xC00


def regions(path):
    raw = json.loads(path.read_bytes())["raw_capture"]
    entry = next(s for s in raw["snapshots"] if s["kind"] == "MAIN_entry_prestate")
    out = {}
    for r in entry["regions"]:
        if r.get("bytes_hex"):
            out.setdefault(r["role"], (bytes.fromhex(r["bytes_hex"]),
                                       bytes.fromhex(r.get("known_mask_hex") or "ff" * (len(r["bytes_hex"]) // 2))))
    return out


def stat_prefix(R):
    owner = R["source.HP_owner"][0]
    return "source.external_STAT" if struct.unpack_from("<Q", owner, 0x2D90)[0] else "source.inline_STAT"


base, after = regions(BASE), regions(AFTER)
prefix = stat_prefix(base)
stat, _ = base[prefix]
attr, attr_known = (bytearray(x) for x in base[prefix + ".attribute_source"])
inventory = bytearray(base["source.two_banks"][0])
record = after["source.two_banks"][0][REC:REC + REC_SIZE]
affixes = []
for slot in range(AFFIX_SLOTS):
    key, sub, value = struct.unpack_from("<III", record, AFFIX_AT + AFFIX_STRIDE * slot)
    if key and value:
        affixes.append((key, sub, value))
inventory[REC:REC + REC_SIZE] = record

count = struct.unpack_from("<I", attr, COUNT)[0]
tail = attr[MAP + 12 * AT:MAP + 12 * count]
new = b"".join(struct.pack("<HHIHBB", key, 0, value, 0, 0, 0) for key, _, value in affixes)
attr[MAP + 12 * AT:MAP + 12 * (count + len(affixes))] = new + tail
attr_known[MAP + 12 * AT:MAP + 12 * (count + len(affixes))] = b"\xff" * (len(new) + len(tail))
struct.pack_into("<I", attr, COUNT, count + len(affixes))

mode = struct.unpack_from("<i", base["source.HP_owner"][0], 0x2C)[0]
dump = json.loads((PKG / "evidence/r2-hits/PAGE_DUMP_pid29088.json").read_bytes())
d0 = bytes.fromhex(dump["default_D"]["value_hex"])


def materialize(attr_bytes, attr_mask, inv):
    source = B.typed_source(bytes(attr_bytes), bytes(attr_mask), mode)
    stat_n = bytearray(stat)
    struct.pack_into("<Q", stat_n, 8, N.OWNER)
    struct.pack_into("<Q", stat_n, 0x7C0, F.DEST)
    struct.pack_into("<Q", stat_n, 0x7D8, N.ATTR)
    result = L.apply(stat_hex=bytes(stat_n).hex(), materializer_hex=d0.hex(), inventory_hex=bytes(inv).hex(),
                     synchronized_i32=list(struct.unpack_from("<7i", attr_bytes, 0x14)), owner_mode_i32=mode, source=source)
    return bytes.fromhex(result["materializer_hex"])


d_base_model = materialize(base[prefix + ".attribute_source"][0], base[prefix + ".attribute_source"][1],
                           base["source.two_banks"][0])
d_pred_model = materialize(attr, attr_known, inventory)
d_base_actual = base[prefix + ".D"][0]
d_after_actual, d_after_known = after[stat_prefix(after) + ".D"]
# Bytes written by other stages are carried over from the baseline: prediction = baseline + materializer delta.
d_pred = bytearray(d_base_actual)
for i in range(len(d_pred)):
    if d_pred_model[i] != d_base_model[i]:
        d_pred[i] = d_pred_model[i]


def fields(blob, offs):
    return {hex(o): [round(struct.unpack_from("<f", blob, o)[0], 6), struct.unpack_from("<i", blob, o)[0]] for o in offs}


changed_pred = sorted({o & ~3 for o in range(len(d_pred)) if d_pred[o] != d_base_actual[o]})
changed_actual = sorted({o & ~3 for o in range(len(d_pred)) if d_after_actual[o] != d_base_actual[o]})
mismatch = [hex(o) for o in range(len(d_pred)) if d_after_known[o] == 0xFF and d_pred[o] != d_after_actual[o]]
print(json.dumps({"affixes": [[hex(k), hex(s), v] for k, s, v in affixes], "insert_index": AT,
                  "predicted_changes": fields(d_pred, changed_pred), "actual_changes": fields(d_after_actual, changed_actual),
                  "mismatching_known_bytes": len(mismatch), "first_mismatches": mismatch[:12]}, indent=1))
out = CACHE.parent / "work" / "swap_predicted_D.bin"
out.write_bytes(bytes(d_pred))
