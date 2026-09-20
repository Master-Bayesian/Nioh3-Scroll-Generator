"""Create an isolated, encrypted test container; never a playable user save.

Two mapped records are written so the read/edit/save round trip can be checked
for preservation:

- slot 0: a legacy over-bound record (raw recommended level 1400, display 700)
  with three effect slots populated;
- slot 1: a six-effect record (`0xE604`, rarity 5) whose slots 0..5 are all
  populated and whose seventh slot keeps the empty sentinel.

Neither record claims natural generation legitimacy; both are synthetic
containers for preservation coverage only.
"""
from pathlib import Path
import json
import struct
import sys

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT))
from emaki_exchange import USER_SAVE_SIZE, patch_user_checksum, write_account_id
from nioh3_scroll_editor.savegame import SaveCrypto, default_crypto_tool, SCROLL_GROUP_OFFSET

target = Path(sys.argv[1]).resolve()
if target.exists():
    raise FileExistsError('Fixture output must be new')
account = 0x1111222233334444

EFFECT_SLOT_BASE = 0x34
EFFECT_SLOT_STRIDE = 0x18
EMPTY_EFFECT_ID = 0xFFFFFFFF


def blank_record() -> bytearray:
    record = bytearray(0xE8)
    struct.pack_into('<H', record, 0, 0xE604)
    for offset, value in [(6, 180), (8, 180), (0x10, 183), (0x12, 183)]:
        struct.pack_into('<H', record, offset, value)
    struct.pack_into('<I', record, 0x20, 36526331)
    record[0x30] = record[0x31] = 4
    record[0x33] = 3
    write_account_id(record, account)
    for index in range(7):
        struct.pack_into('<6I', record, EFFECT_SLOT_BASE + index * EFFECT_SLOT_STRIDE,
                         0, EMPTY_EFFECT_ID, 0, 0, 0, 0)
    return record


def record_with_effects(effect_ids, *, recommended_level, seed, rarity=4) -> bytearray:
    record = blank_record()
    struct.pack_into('<I', record, 0x20, seed)
    struct.pack_into('<H', record, 0x10, recommended_level)
    struct.pack_into('<H', record, 0x12, recommended_level)
    record[0x30] = record[0x31] = rarity
    for index, effect_id in enumerate(effect_ids):
        struct.pack_into('<6I', record, EFFECT_SLOT_BASE + index * EFFECT_SLOT_STRIDE,
                         index, effect_id, index + 1, index + 2, index + 3, index + 4)
    return record


# Slot 0: legacy over-bound record, three effect slots.
legacy = record_with_effects([18386, 10449, 12028], recommended_level=1400, seed=180443387)
# Slot 1: six populated effect slots; the seventh keeps the empty sentinel.
six_effect = record_with_effects(
    [18386, 10449, 12028, 27875, 16193, 61335],
    recommended_level=600,
    seed=180443388,
    rarity=5,
)

data = bytearray(USER_SAVE_SIZE)
data[:6] = b'RNNUSR'
data[SCROLL_GROUP_OFFSET:SCROLL_GROUP_OFFSET + len(legacy)] = legacy
second = SCROLL_GROUP_OFFSET + 0xE8
data[second:second + len(six_effect)] = six_effect
patch_user_checksum(data)
folder = target / str(account) / 'SAVEDATA00'
folder.mkdir(parents=True)
plain, encrypted = target / 'synthetic.bin', folder / 'SAVEDATA.BIN'
plain.write_bytes(data)
SaveCrypto(default_crypto_tool(ROOT)).encrypt(plain, encrypted)
print(json.dumps({'path': str(encrypted)}))
