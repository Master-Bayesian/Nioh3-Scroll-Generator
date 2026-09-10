"""Create an isolated, encrypted test container; never a playable user save."""
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
record = bytearray(0xE8)
struct.pack_into('<H', record, 0, 0xE604)
for offset, value in [(6, 180), (8, 180), (0x10, 183), (0x12, 183)]:
    struct.pack_into('<H', record, offset, value)
struct.pack_into('<I', record, 0x20, 36526331)
record[0x30] = record[0x31] = 4
record[0x33] = 3
write_account_id(record, account)
for index in range(7):
    struct.pack_into('<6I', record, 0x34 + index * 0x18, 0, 0xFFFFFFFF, 0, 0, 0, 0)
data = bytearray(USER_SAVE_SIZE)
data[:6] = b'RNNUSR'
data[SCROLL_GROUP_OFFSET:SCROLL_GROUP_OFFSET + len(record)] = record
patch_user_checksum(data)
folder = target / str(account) / 'SAVEDATA00'
folder.mkdir(parents=True)
plain, encrypted = target / 'synthetic.bin', folder / 'SAVEDATA.BIN'
plain.write_bytes(data)
SaveCrypto(default_crypto_tool(ROOT)).encrypt(plain, encrypted)
print(json.dumps({'path': str(encrypted)}))
