"""Prepare two encrypted synthetic generations and one restorable backup."""
from pathlib import Path
import json
import struct
import subprocess
import sys
import uuid

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT))
from emaki_exchange import patch_user_checksum
from nioh3_scroll_editor.savegame import (
    SCROLL_GROUP_OFFSET,
    SaveCrypto,
    backup_related_save_files,
    capture_related_save_fingerprints,
    create_backup_directory,
    default_crypto_tool,
    sha256_file,
    write_backup_manifest,
)

root = Path(sys.argv[1]).resolve(strict=True)
target = root / "local" / "KoeiTecmo" / "NIOH3" / "Savedata"
state = root / "profile"
if target.exists() or state.exists():
    raise FileExistsError("Restore fixture requires new save and profile directories")
created = subprocess.run(
    [sys.executable, str(Path(__file__).with_name("create-save.py")), str(target)],
    check=True, capture_output=True, text=True, timeout=30,
)
save_path = Path(json.loads(created.stdout)["path"])
backup = create_backup_directory(state)
files = backup_related_save_files(backup, capture_related_save_fingerprints(save_path))
write_backup_manifest(
    backup, save_path, files, action="v2-local-edit", operation_id=uuid.uuid4().hex,
)

# Change only the isolated generation, so a no-op restore cannot pass acceptance.
plain = target / "synthetic.bin"
data = bytearray(plain.read_bytes())
for offset in (0x10, 0x12):
    struct.pack_into("<H", data, SCROLL_GROUP_OFFSET + offset, 585)
patch_user_checksum(data)
plain.write_bytes(data)
changed = target / "synthetic-changed.bin"
SaveCrypto(default_crypto_tool(ROOT)).encrypt(plain, changed)
changed.replace(save_path)
before_hash = sha256_file(save_path)
backup_hash = sha256_file(backup / "SAVEDATA.BIN")
if before_hash == backup_hash:
    raise AssertionError("Synthetic generations must differ")
print(json.dumps({
    "path": str(save_path), "backup_id": backup.name,
    "backup_path": str(backup / "SAVEDATA.BIN"),
    "before_sha256": before_hash, "backup_sha256": backup_hash,
}))
