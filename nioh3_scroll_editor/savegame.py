from __future__ import annotations

import hashlib
import ctypes
import json
import os
import re
import shutil
import struct
import subprocess
import tempfile
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Sequence

from emaki_exchange import (
    CATEGORY_TO_TYPE,
    SCROLL_RECORD_SIZE,
    TYPE_TO_CATEGORY,
    account_id_from_record,
    describe_record_for_report,
    insert_scroll_record,
    patch_user_checksum,
    require_decrypted_user_save,
    write_account_id,
)

from .native import build_source_record
from .effect_sequence import materialize_ng3_certified_install_record
from .models import CandidateRecordStage, ScrollCandidate


SCROLL_GROUP_OFFSET = 0x176CCE
# The array starts at 0x176CCE and the following save structure begins exactly
# at 0x18D74E: (0x18D74E - 0x176CCE) / 0xE8 == 400 records.  Never scan beyond
# that boundary; doing so would treat unrelated save data as scroll slots.
SCROLL_SLOT_COUNT = 400
SCROLL_INVENTORY_KEY_OFFSET = 0x1C
SCROLL_INVENTORY_KEY_MAX = 0xFFFF
SCROLL_GENERATION_SERIAL_OFFSET = 0x28
SCROLL_GENERATION_SERIAL_MAX = 0xFFFFFFFC


def scroll_slot_is_empty(record: bytes) -> bool:
    """Return the native free-slot state for one fixed inventory record.

    Deletion clears the record type at +0x00 but can leave stale bytes in the
    remainder of the 0xE8-byte slot. Requiring every byte to be zero therefore
    undercounts capacity after normal in-game deletion.
    """

    if len(record) != SCROLL_RECORD_SIZE:
        raise ValueError("scroll slot record must be exactly 0xE8 bytes")
    return struct.unpack_from("<H", record, 0)[0] == 0


def _clear_native_free_scroll_slot(data: bytes, record_offset: int) -> bytes:
    """Normalize one native free slot before the strict insert helper runs.

    The standalone insert helper intentionally accepts only an all-zero
    destination.  Normal game deletion can leave stale payload bytes behind a
    zero record type, so the save transaction clears exactly the already
    verified free slot before replacing the complete 0xE8-byte record.
    """

    end = record_offset + SCROLL_RECORD_SIZE
    if record_offset < 0 or end > len(data):
        raise ValueError("scroll slot lies outside the save")
    existing = data[record_offset:end]
    if not scroll_slot_is_empty(existing):
        raise RuntimeError("目标绘卷栏位已占用，已拒绝写入")
    if not any(existing):
        return data
    cleared = bytearray(data)
    cleared[record_offset:end] = bytes(SCROLL_RECORD_SIZE)
    return bytes(cleared)


def allocate_scroll_inventory_keys(
    decrypted: bytes,
    count: int,
) -> tuple[int, ...]:
    """Allocate conservative globally unused values for scroll ``+0x1C``.

    The exact semantics of this field remain unproven.  New records keep the
    existing conservative allocation policy, but existing records are never
    rewritten solely because the same scalar appears elsewhere in the save.
    Live FB-016 testing specifically disproved ``+0x1C`` as the cause of the
    equipment-rendering defect.
    """

    if count < 0:
        raise ValueError("inventory-key count cannot be negative")
    if count == 0:
        return ()
    require_decrypted_user_save(decrypted)
    scroll_keys: set[int] = set()
    for slot_index in range(SCROLL_SLOT_COUNT):
        record_offset = SCROLL_GROUP_OFFSET + slot_index * SCROLL_RECORD_SIZE
        record = decrypted[record_offset:record_offset + SCROLL_RECORD_SIZE]
        if scroll_slot_is_empty(record):
            continue
        value = struct.unpack_from("<I", record, SCROLL_INVENTORY_KEY_OFFSET)[0]
        if 1 <= value <= SCROLL_INVENTORY_KEY_MAX:
            scroll_keys.add(value)
    if count > SCROLL_INVENTORY_KEY_MAX - len(scroll_keys):
        raise RuntimeError("绘卷实例键空间不足，无法安全添加新记录")

    start = (max(scroll_keys) + 1) if scroll_keys else 1
    allocated: list[int] = []
    for step in range(SCROLL_INVENTORY_KEY_MAX):
        value = ((start - 1 + step) % SCROLL_INVENTORY_KEY_MAX) + 1
        if value in scroll_keys or value in allocated:
            continue
        if struct.pack("<I", value) in decrypted:
            continue
        allocated.append(value)
        if len(allocated) == count:
            return tuple(allocated)
    raise RuntimeError("无法为新增绘卷分配唯一实例键")


def write_scroll_inventory_key(record: bytes, inventory_key: int) -> bytes:
    """Return one complete record with a validated native inventory key."""

    if len(record) != SCROLL_RECORD_SIZE:
        raise ValueError("候选绘卷记录必须为 0xE8 字节")
    if not 1 <= inventory_key <= SCROLL_INVENTORY_KEY_MAX:
        raise ValueError("绘卷实例键必须是非零 uint16")
    output = bytearray(record)
    struct.pack_into("<I", output, SCROLL_INVENTORY_KEY_OFFSET, inventory_key)
    return bytes(output)


def allocate_scroll_generation_serials(
    decrypted: bytes,
    count: int,
) -> tuple[int, ...]:
    """Allocate save-wide unique values for scroll record ``+0x28``.

    Live FB-016 testing proved that ``+0x28`` shares one identity namespace
    with non-scroll inventory records.  The previous allocator considered only
    scroll records, so ``max(scroll serial) + 1`` could reuse an equipment
    serial and make the new scroll render as that equipment item.

    Start after the largest scroll serial to preserve the existing behavior,
    then reject values already used by a scroll or by a captured native
    equipment record.  The equipment predicate is intentionally strict so an
    unrelated scalar does not perturb the native serial sequence.
    """

    if count < 0:
        raise ValueError("generation-serial count cannot be negative")
    if count == 0:
        return ()
    require_decrypted_user_save(decrypted)
    serials: list[int] = []
    for slot_index in range(SCROLL_SLOT_COUNT):
        record_offset = SCROLL_GROUP_OFFSET + slot_index * SCROLL_RECORD_SIZE
        record = decrypted[record_offset:record_offset + SCROLL_RECORD_SIZE]
        if scroll_slot_is_empty(record):
            continue
        record_type = struct.unpack_from("<H", record, 0)[0]
        if TYPE_TO_CATEGORY.get(record_type, 0) > 0:
            serials.append(
                struct.unpack_from("<I", record, SCROLL_GENERATION_SERIAL_OFFSET)[0]
            )

    occupied = set(serials)
    occupied.update(_non_scroll_item_generation_serials(decrypted))
    start = (max(serials) + 1) if serials else 1
    if start > SCROLL_GENERATION_SERIAL_MAX:
        raise RuntimeError("绘卷内部序号接近 uint32 上限，无法安全分配新记录")
    allocated: list[int] = []
    for value in range(start, SCROLL_GENERATION_SERIAL_MAX + 1):
        if value in occupied or value in allocated:
            continue
        allocated.append(value)
        if len(allocated) == count:
            return tuple(allocated)
    raise RuntimeError("无法为新增绘卷分配全局唯一内部序号")


def write_scroll_generation_serial(record: bytes, generation_serial: int) -> bytes:
    """Return one complete record with a validated generation serial."""

    if len(record) != SCROLL_RECORD_SIZE:
        raise ValueError("候选绘卷记录必须为 0xE8 字节")
    if not 1 <= generation_serial <= SCROLL_GENERATION_SERIAL_MAX:
        raise ValueError("绘卷内部序号必须是受支持的非零 uint32")
    output = bytearray(record)
    struct.pack_into(
        "<I",
        output,
        SCROLL_GENERATION_SERIAL_OFFSET,
        generation_serial,
    )
    return bytes(output)


def _looks_like_non_scroll_item_record(
    decrypted: bytes,
    record_offset: int,
) -> bool:
    """Return whether ``record_offset`` has the captured equipment header."""

    if record_offset < 0 or record_offset + 0x2C > len(decrypted):
        return False
    record_type = struct.unpack_from("<H", decrypted, record_offset)[0]
    mirrored_type = struct.unpack_from("<H", decrypted, record_offset + 0x02)[0]
    item_count = struct.unpack_from("<H", decrypted, record_offset + 0x04)[0]
    level = struct.unpack_from("<H", decrypted, record_offset + 0x06)[0]
    mirrored_level = struct.unpack_from("<H", decrypted, record_offset + 0x08)[0]
    return (
        record_type != 0
        and record_type == mirrored_type
        and TYPE_TO_CATEGORY.get(record_type, 0) == 0
        and item_count == 1
        and level == mirrored_level
    )


def _non_scroll_item_generation_serials(decrypted: bytes) -> tuple[int, ...]:
    """Return captured non-scroll item serials stored at record ``+0x28``."""

    serials: list[int] = []
    search_offset = 4
    item_count_marker = b"\x01\x00"
    while True:
        count_offset = decrypted.find(item_count_marker, search_offset)
        if count_offset < 0:
            break
        record_offset = count_offset - 4
        if _looks_like_non_scroll_item_record(decrypted, record_offset):
            value = struct.unpack_from(
                "<I",
                decrypted,
                record_offset + SCROLL_GENERATION_SERIAL_OFFSET,
            )[0]
            if 1 <= value <= SCROLL_GENERATION_SERIAL_MAX:
                serials.append(value)
        search_offset = count_offset + 1
    return tuple(serials)


def repair_duplicate_scroll_generation_serials(
    decrypted: bytes,
) -> tuple[bytes, tuple[dict[str, int | str], ...]]:
    """Repair scroll ``+0x28`` collisions proven to alias equipment records.

    Scroll-only duplicates keep their first occurrence.  If the same serial is
    present at ``+0x28`` of a captured non-scroll item header, every affected
    scroll is assigned a new save-wide unique serial.  This is the exact state
    transition validated against the FB-016 save in game.
    """

    require_decrypted_user_save(decrypted)
    serial_slots: dict[int, list[tuple[int, int]]] = {}
    for slot_index in range(SCROLL_SLOT_COUNT):
        record_offset = SCROLL_GROUP_OFFSET + slot_index * SCROLL_RECORD_SIZE
        record = decrypted[record_offset:record_offset + SCROLL_RECORD_SIZE]
        if scroll_slot_is_empty(record):
            continue
        record_type = struct.unpack_from("<H", record, 0)[0]
        is_scroll = TYPE_TO_CATEGORY.get(record_type, 0) > 0
        value = struct.unpack_from("<I", record, SCROLL_GENERATION_SERIAL_OFFSET)[0]
        if not 1 <= value <= SCROLL_GENERATION_SERIAL_MAX:
            continue
        if is_scroll:
            serial_offset = record_offset + SCROLL_GENERATION_SERIAL_OFFSET
            serial_slots.setdefault(value, []).append((slot_index, serial_offset))

    repair_slots: list[tuple[int, int, str]] = []
    for value, slots in serial_slots.items():
        encoded = struct.pack("<I", value)
        own_offsets = {serial_offset for _, serial_offset in slots}
        search_offset = 0
        has_external_collision = False
        while True:
            occurrence = decrypted.find(encoded, search_offset)
            if occurrence < 0:
                break
            if (
                occurrence not in own_offsets
                and _looks_like_non_scroll_item_record(
                    decrypted,
                    occurrence - SCROLL_GENERATION_SERIAL_OFFSET,
                )
            ):
                has_external_collision = True
                break
            search_offset = occurrence + 1
        if has_external_collision:
            repair_slots.extend(
                (slot_index, value, "non_scroll_item_generation_serial_collision")
                for slot_index, _ in slots
            )
        else:
            repair_slots.extend(
                (slot_index, value, "duplicate_scroll_generation_serial")
                for slot_index, _ in slots[1:]
            )
    if not repair_slots:
        return decrypted, ()

    replacements = allocate_scroll_generation_serials(decrypted, len(repair_slots))
    output = bytearray(decrypted)
    repairs: list[dict[str, int | str]] = []
    for (slot_index, old_value, reason), new_value in zip(
        repair_slots,
        replacements,
        strict=True,
    ):
        record_offset = SCROLL_GROUP_OFFSET + slot_index * SCROLL_RECORD_SIZE
        struct.pack_into(
            "<I",
            output,
            record_offset + SCROLL_GENERATION_SERIAL_OFFSET,
            new_value,
        )
        repairs.append(
            {
                "slot_index": slot_index,
                "old_generation_serial": old_value,
                "new_generation_serial": new_value,
                "reason": reason,
            }
        )
    return bytes(output), tuple(repairs)


@dataclass(frozen=True, slots=True)
class ScrollInventoryEntry:
    """One occupied record in the fixed 400-slot scroll inventory."""

    slot_index: int
    record_offset: int
    record: bytes
    record_type: int
    playthrough: int | None
    seed: int
    rarity: int
    transfer_count: int
    candidate: ScrollCandidate

    @property
    def is_mapped_scroll(self) -> bool:
        return self.playthrough is not None


@dataclass(frozen=True, slots=True)
class LocalScrollHeaderFields:
    """Editable canonical/header fields of one local scroll record."""

    playthrough: int
    level: int
    recommended_level: int
    seed: int
    rarity: int
    transfer_count: int


@dataclass(frozen=True, slots=True)
class LocalEffectEdit:
    """Local-only replacement fields for one 0x18-byte effect slot."""

    slot_index: int
    effect_id: int | None = None
    value: int | None = None
    prefix: int | None = None
    metadata: int | None = None
    tail_0: int | None = None
    tail_1: int | None = None

    def __post_init__(self) -> None:
        if not 0 <= self.slot_index < 7:
            raise ValueError("effect slot index must be in 0..6")
        for name in ("effect_id", "value", "prefix", "metadata", "tail_0", "tail_1"):
            field_value = getattr(self, name)
            if field_value is not None and not 0 <= field_value <= 0xFFFFFFFF:
                raise ValueError(f"{name} must fit in uint32")


@dataclass(frozen=True, slots=True)
class LocalEffectSlotFields:
    """Editable raw fields for one local-only effect slot."""

    slot_index: int
    effect_id: int
    value: int
    prefix: int
    metadata: int
    tail_0: int
    tail_1: int

    def __post_init__(self) -> None:
        if not 0 <= self.slot_index < 7:
            raise ValueError("effect slot index must be in 0..6")
        for name in (
            "effect_id",
            "value",
            "prefix",
            "metadata",
            "tail_0",
            "tail_1",
        ):
            if not 0 <= getattr(self, name) <= 0xFFFFFFFF:
                raise ValueError(f"{name} must fit in uint32")

    def as_edit(self) -> LocalEffectEdit:
        return LocalEffectEdit(
            slot_index=self.slot_index,
            effect_id=self.effect_id,
            value=self.value,
            prefix=self.prefix,
            metadata=self.metadata,
            tail_0=self.tail_0,
            tail_1=self.tail_1,
        )


def read_local_effect_slots(record: bytes) -> tuple[LocalEffectSlotFields, ...]:
    """Parse all seven editable slots without applying generation semantics."""

    if len(record) != SCROLL_RECORD_SIZE:
        raise ValueError("record must be exactly 0xE8 bytes")
    result: list[LocalEffectSlotFields] = []
    for slot_index in range(7):
        base = 0x34 + slot_index * 0x18
        prefix, effect_id, value, metadata, tail_0, tail_1 = struct.unpack_from(
            "<6I",
            record,
            base,
        )
        result.append(
            LocalEffectSlotFields(
                slot_index=slot_index,
                effect_id=effect_id,
                value=value,
                prefix=prefix,
                metadata=metadata,
                tail_0=tail_0,
                tail_1=tail_1,
            )
        )
    return tuple(result)


def retarget_local_effect_identity(
    fields: LocalEffectSlotFields,
    *,
    effect_id: int,
    group_key: int,
    category_key: int,
) -> LocalEffectSlotFields:
    """Retarget identity fields while preserving value, roll, role, and tails.

    This helper does not validate slot roles, duplicate groups, conflicts,
    rarity, or Seed consistency.  It only keeps the chosen native effect ID,
    group prefix, and category byte internally aligned.  Users may still
    override every resulting raw field before saving.
    """

    for name, value in (
        ("effect_id", effect_id),
        ("group_key", group_key),
        ("category_key", category_key),
    ):
        if not 0 <= value <= 0xFFFFFFFF:
            raise ValueError(f"{name} must fit in uint32")
    if category_key > 0x3F:
        raise ValueError("category_key must fit in the native six-bit field")
    category_and_role = ((fields.metadata >> 8) & 0xC0) | category_key
    metadata = (fields.metadata & 0xFFFF00FF) | (category_and_role << 8)
    return LocalEffectSlotFields(
        slot_index=fields.slot_index,
        effect_id=effect_id,
        value=fields.value,
        prefix=group_key,
        metadata=metadata,
        tail_0=fields.tail_0,
        tail_1=fields.tail_1,
    )


def patch_local_scroll_record(
    record: bytes,
    edits: Sequence[LocalEffectEdit],
) -> bytes:
    """Patch local effect-slot fields without canonical regeneration.

    These edits intentionally do not alter the canonical Seed/rarity tuple.
    They can change local display and gameplay behavior, but propagated copies
    are rebuilt by the recipient from canonical fields and will not retain the
    edited slots.
    """

    if len(record) != SCROLL_RECORD_SIZE:
        raise ValueError("record must be exactly 0xE8 bytes")
    normalized = tuple(edits)
    if not normalized:
        raise ValueError("at least one local effect edit is required")
    slot_indexes = tuple(edit.slot_index for edit in normalized)
    if len(set(slot_indexes)) != len(slot_indexes):
        raise ValueError("the same effect slot cannot be edited twice")

    output = bytearray(record)
    field_offsets = {
        "prefix": 0x00,
        "effect_id": 0x04,
        "value": 0x08,
        "metadata": 0x0C,
        "tail_0": 0x10,
        "tail_1": 0x14,
    }
    for edit in normalized:
        base = 0x34 + edit.slot_index * 0x18
        for name, relative_offset in field_offsets.items():
            field_value = getattr(edit, name)
            if field_value is not None:
                struct.pack_into("<I", output, base + relative_offset, field_value)
    return bytes(output)


def patch_local_scroll_seed(record: bytes, seed: int) -> bytes:
    """Replace the canonical displayed Seed in one local scroll record.

    Enemies, terrain, and special rules are not serialized as independent
    fields in the 0xE8 record. The game derives them from this Seed and the
    scroll playthrough. Local effect-slot bytes remain untouched so callers
    may deliberately combine a Seed-derived auxiliary layout with arbitrary
    local-only effect edits.
    """

    if len(record) != SCROLL_RECORD_SIZE:
        raise ValueError("record must be exactly 0xE8 bytes")
    if not 0 <= seed <= 0xFFFFFFFF:
        raise ValueError("seed must fit in uint32")
    output = bytearray(record)
    struct.pack_into("<I", output, 0x20, seed)
    return bytes(output)


def read_local_scroll_header(record: bytes) -> LocalScrollHeaderFields:
    """Parse the editable canonical/header fields of one mapped scroll."""

    if len(record) != SCROLL_RECORD_SIZE:
        raise ValueError("record must be exactly 0xE8 bytes")
    record_type = struct.unpack_from("<H", record, 0x00)[0]
    playthrough = TYPE_TO_CATEGORY.get(record_type)
    if playthrough not in (1, 2, 3, 4, 5):
        raise ValueError(f"record type 0x{record_type:04X} is not a mapped scroll")
    return LocalScrollHeaderFields(
        playthrough=playthrough,
        level=struct.unpack_from("<H", record, 0x06)[0],
        recommended_level=struct.unpack_from("<H", record, 0x10)[0],
        seed=struct.unpack_from("<I", record, 0x20)[0],
        rarity=record[0x30],
        transfer_count=struct.unpack_from("<I", record, 0xDC)[0],
    )


def patch_local_scroll_header(
    record: bytes,
    *,
    playthrough: int,
    level: int,
    recommended_level: int,
    seed: int,
    rarity: int,
    transfer_count: int,
) -> bytes:
    """Patch mapped scroll header fields without regenerating effect slots."""

    if len(record) != SCROLL_RECORD_SIZE:
        raise ValueError("record must be exactly 0xE8 bytes")
    if playthrough not in (1, 2, 3, 4, 5):
        raise ValueError("playthrough must be in 1..5")
    for name, value, maximum in (
        ("level", level, 0xFFFF),
        ("recommended_level", recommended_level, 0xFFFF),
        ("seed", seed, 0xFFFFFFFF),
        ("transfer_count", transfer_count, 0xFFFFFFFF),
    ):
        if not 0 <= value <= maximum:
            raise ValueError(f"{name} must be in 0..0x{maximum:X}")
    if rarity not in (3, 4, 5):
        raise ValueError("rarity must be 3, 4, or 5")

    output = bytearray(record)
    struct.pack_into("<H", output, 0x00, CATEGORY_TO_TYPE[playthrough])
    struct.pack_into("<H", output, 0x06, level)
    struct.pack_into("<H", output, 0x08, level)
    struct.pack_into("<H", output, 0x10, recommended_level)
    struct.pack_into("<H", output, 0x12, recommended_level)
    struct.pack_into("<I", output, 0x20, seed)
    output[0x30] = rarity
    output[0x31] = rarity
    struct.pack_into("<I", output, 0xDC, transfer_count)
    return bytes(output)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest().upper()


def utc_timestamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S")


def create_backup_directory(state_root: Path) -> Path:
    """Create a unique application-owned backup directory.

    Save transactions can legitimately happen more than once in one second,
    so a timestamp alone is not a sufficient unique key.
    """

    backups_root = state_root.resolve() / "backups"
    backups_root.mkdir(parents=True, exist_ok=True)
    base_name = utc_timestamp()
    for suffix in range(1000):
        name = base_name if suffix == 0 else f"{base_name}-{suffix:03d}"
        candidate = backups_root / name
        try:
            candidate.mkdir()
        except FileExistsError:
            continue
        return candidate
    raise RuntimeError("同一秒内创建的备份过多，无法分配安全目录")


SAVE_SLOT_DIRECTORY_PATTERN = re.compile(r"^SAVEDATA(?P<index>\d{2})$")


def save_slot_index_from_path(path: Path) -> int:
    """Return the zero-based in-game character-save slot for one save path."""

    match = SAVE_SLOT_DIRECTORY_PATTERN.fullmatch(path.parent.name)
    if match is None:
        raise ValueError("无法从存档路径识别游戏存档栏位")
    return int(match.group("index"), 10)


def discover_save_paths() -> list[Path]:
    local_app_data = Path(os.environ.get("LOCALAPPDATA", ""))
    root = local_app_data / "KoeiTecmo" / "NIOH3" / "Savedata"
    if not root.is_dir():
        return []
    discovered: list[Path] = []
    for candidate in root.glob("*/SAVEDATA??/SAVEDATA.BIN"):
        try:
            save_slot_index_from_path(candidate)
            int(candidate.parents[1].name)
        except (ValueError, IndexError):
            continue
        if candidate.is_file():
            discovered.append(candidate)
    return sorted(
        discovered,
        key=lambda path: (
            int(path.parents[1].name),
            save_slot_index_from_path(path),
        ),
    )


def account_id_from_save_path(path: Path) -> int:
    try:
        return int(path.parents[1].name)
    except (ValueError, IndexError) as error:
        raise ValueError("无法从自动发现的存档中识别 Steam ID") from error


def default_crypto_tool(project_root: Path) -> Path:
    development = (
        project_root
        / ".tools"
        / "nioh3-save-crypt-source"
        / "x64"
        / "Release"
        / "Nioh_Savefile_decrypt.exe"
    )
    packaged = project_root / "bin" / "Nioh_Savefile_decrypt.exe"
    for candidate in (packaged, development):
        if candidate.is_file():
            return candidate
    raise FileNotFoundError("未找到内置的存档加解密组件")


class SaveCrypto:
    def __init__(self, executable: Path) -> None:
        self.executable = executable.resolve()
        if not self.executable.is_file():
            raise FileNotFoundError(self.executable)

    def transform(self, source: Path, output: Path) -> None:
        if source.resolve() == output.resolve():
            raise ValueError("拒绝在原文件上直接执行加解密")
        if output.exists():
            raise FileExistsError(output)
        output.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="nioh3-scroll-crypt-") as directory:
            work = Path(directory)
            staged_source = work / "input.bin"
            staged_output = work / "output.bin"
            shutil.copy2(source, staged_source)
            creation_flags = getattr(subprocess, "CREATE_NO_WINDOW", 0)
            result = subprocess.run(
                [
                    str(self.executable),
                    "-i",
                    str(staged_source),
                    "-o",
                    staged_output.name,
                ],
                cwd=work,
                input="\n",
                text=True,
                capture_output=True,
                timeout=60,
                creationflags=creation_flags,
                check=False,
            )
            if result.returncode != 0 or not staged_output.is_file():
                details = (result.stdout + "\n" + result.stderr).strip()
                raise RuntimeError(f"存档加解密组件运行失败：{details}")
            shutil.copy2(staged_output, output)

    def decrypt(self, source: Path, output: Path) -> None:
        self.transform(source, output)
        data = output.read_bytes()
        require_decrypted_user_save(data)

    def encrypt(self, source: Path, output: Path) -> None:
        require_decrypted_user_save(source.read_bytes())
        self.transform(source, output)
        if output.read_bytes().startswith(b"RNNUSR"):
            raise RuntimeError("加密步骤意外返回了解密文件")


@dataclass(frozen=True, slots=True)
class SaveInventory:
    save_path: Path
    decrypted: bytes
    account_id: int
    template_record: bytes
    template_records: tuple[tuple[int, bytes], ...]
    empty_slots: tuple[int, ...]
    next_slot_index: int | None

    def scroll_entries(
        self,
        *,
        include_unmapped: bool = False,
    ) -> tuple[ScrollInventoryEntry, ...]:
        """Return mapped scroll records without compacting or reordering them.

        The fixed record region also contains non-scroll item types. Product
        inventory and deletion surfaces must not expose those records as
        scrolls. Research callers can opt in when diagnosing malformed data.
        """

        entries: list[ScrollInventoryEntry] = []
        for slot_index in range(SCROLL_SLOT_COUNT):
            record_offset = SCROLL_GROUP_OFFSET + slot_index * SCROLL_RECORD_SIZE
            record = self.decrypted[record_offset:record_offset + SCROLL_RECORD_SIZE]
            if scroll_slot_is_empty(record):
                continue
            record_type = struct.unpack_from("<H", record, 0)[0]
            playthrough = TYPE_TO_CATEGORY.get(record_type)
            if playthrough == 0:
                playthrough = None
            if playthrough is None and not include_unmapped:
                continue
            entries.append(
                ScrollInventoryEntry(
                    slot_index=slot_index,
                    record_offset=record_offset,
                    record=record,
                    record_type=record_type,
                    playthrough=playthrough,
                    seed=struct.unpack_from("<I", record, 0x20)[0],
                    rarity=record[0x30],
                    transfer_count=struct.unpack_from("<I", record, 0xDC)[0],
                    candidate=ScrollCandidate.from_record(
                        record,
                        playthrough=playthrough,
                        record_stage=CandidateRecordStage.FINAL_RECORD,
                    ),
                )
            )
        return tuple(entries)

    def template_record_for_playthrough(self, playthrough: int) -> bytes:
        if playthrough not in (1, 2, 3, 4, 5):
            raise ValueError("周目必须在一至五周目之间")
        record_type = CATEGORY_TO_TYPE[playthrough]
        templates = dict(self.template_records)
        if record_type in templates:
            return templates[record_type]
        if playthrough in (4, 5):
            # v2.00.02 contains native category-4/5 record types, but an NG3
            # save naturally has no authentic examples yet.  Clone the genuine
            # highest available template in memory and change only its type.
            # The live native generator reconstructs the complete canonical
            # record before display or installation.  Online exchange remains
            # experimental because the currently reversed message masks the
            # category to two bits.
            base = templates.get(CATEGORY_TO_TYPE[3], self.template_record)
            synthetic = bytearray(base)
            struct.pack_into("<H", synthetic, 0, record_type)
            return bytes(synthetic)
        raise RuntimeError(
            f"存档中没有周目 {playthrough} 所需的 0x{record_type:04X} 合法模板"
        )

    @classmethod
    def load(cls, save_path: Path, decrypted: bytes) -> "SaveInventory":
        require_decrypted_user_save(decrypted)
        account_id = account_id_from_save_path(save_path)
        own_records: list[bytes] = []
        mapped_records: list[bytes] = []
        empty_slots: list[int] = []
        occupied_slots: list[int] = []
        for index in range(SCROLL_SLOT_COUNT):
            offset = SCROLL_GROUP_OFFSET + index * SCROLL_RECORD_SIZE
            record = decrypted[offset:offset + SCROLL_RECORD_SIZE]
            if len(record) != SCROLL_RECORD_SIZE:
                raise ValueError("该存档版本中的绘卷区域与 v2.00.02 不匹配")
            if scroll_slot_is_empty(record):
                empty_slots.append(index)
                continue
            occupied_slots.append(index)
            record_type = struct.unpack_from("<H", record, 0)[0]
            # The native canonicalizer accepts all mapped Emaki record types.
            # Requiring 0x1E82 incorrectly rejected accounts whose authentic
            # self-origin template is another mapped type (E604 in this save).
            if TYPE_TO_CATEGORY.get(record_type, 0) > 0:
                mapped_records.append(record)
                if account_id_from_record(record) == account_id:
                    own_records.append(record)
        if not mapped_records:
            raise RuntimeError("存档中没有可作为原生模板的有效绘卷")

        # E604 is the verified current-NG3 Grace generator context.  A re-sign
        # changes the save owner but intentionally does not rewrite the origin
        # lineage of every received scroll, so a re-signed save may have no
        # self-origin E604 record.  Origin account ID does not enter the effect
        # generator; rebind one genuine E604 record only in the in-memory source
        # template.  Existing save records remain untouched.
        own_e604 = next(
            (record for record in own_records if struct.unpack_from("<H", record, 0)[0] == 0xE604),
            None,
        )
        any_e604 = next(
            (record for record in mapped_records if struct.unpack_from("<H", record, 0)[0] == 0xE604),
            None,
        )
        template_record = (
            own_e604
            or any_e604
            or (own_records[0] if own_records else mapped_records[0])
        )
        if account_id_from_record(template_record) != account_id:
            rebound = bytearray(template_record)
            write_account_id(rebound, account_id)
            template_record = bytes(rebound)
        template_records: list[tuple[int, bytes]] = []
        for record_type in CATEGORY_TO_TYPE[1:]:
            selected = next(
                (
                    record
                    for record in own_records
                    if struct.unpack_from("<H", record, 0)[0] == record_type
                ),
                None,
            ) or next(
                (
                    record
                    for record in mapped_records
                    if struct.unpack_from("<H", record, 0)[0] == record_type
                ),
                None,
            )
            if selected is None:
                continue
            if account_id_from_record(selected) != account_id:
                rebound = bytearray(selected)
                write_account_id(rebound, account_id)
                selected = bytes(rebound)
            template_records.append((record_type, selected))
        # Prefer the first all-zero slot after the current tail, matching the
        # game's append behavior.  If a deletion left a hole before the tail,
        # or a malformed/non-scroll record occupies the last physical slot,
        # reuse the first all-zero hole instead of incorrectly reporting that
        # the 400-slot inventory is full.
        occupied_tail = max(occupied_slots) if occupied_slots else -1
        next_slot_index = next(
            (index for index in empty_slots if index > occupied_tail),
            empty_slots[0] if empty_slots else None,
        )
        return cls(
            save_path=save_path,
            decrypted=decrypted,
            account_id=account_id,
            template_record=template_record,
            template_records=tuple(template_records),
            empty_slots=tuple(empty_slots),
            next_slot_index=next_slot_index,
        )


def prepare_candidate_for_install(record: bytes, *, transfer_count: int) -> bytes:
    if len(record) != SCROLL_RECORD_SIZE:
        raise ValueError("候选绘卷记录必须为 0xE8 字节")
    if not 0 <= transfer_count <= 0xFFFFFFFF:
        raise ValueError("转手次数必须在 0 到 4294967295 之间")
    edited = bytearray(record)
    struct.pack_into("<I", edited, 0xDC, transfer_count)
    return bytes(edited)


def next_generation_serial(inventory: SaveInventory) -> int:
    """Allocate the next save-wide unique per-record serial."""

    return allocate_scroll_generation_serials(inventory.decrypted, 1)[0]


def materialize_effect_sequence_candidate(
    inventory: SaveInventory,
    candidate: ScrollCandidate,
    *,
    level: int,
    recommended_level: int,
    transfer_count: int,
) -> ScrollCandidate:
    """Bind one certified game-closed preview to the current save template."""

    if not candidate.can_materialize_for_install:
        raise ValueError("只有通过完整记录一致性门禁的三周目稀有度3/4/5预览可以离线物化")
    if candidate.record_stage is not CandidateRecordStage.EFFECT_SEQUENCE_ONLY:
        raise ValueError("候选不是离线词条序列预览")
    template = inventory.template_record_for_playthrough(3)
    record, installed_preview = materialize_ng3_certified_install_record(
        template,
        seed=candidate.seed,
        rarity=candidate.rarity,
        level=level,
        recommended_level=recommended_level,
        transfer_count=transfer_count,
        generation_serial=next_generation_serial(inventory),
    )
    installed_record_stage = (
        CandidateRecordStage.NATIVE_STAGE_ONE
        if candidate.rarity == 4
        else CandidateRecordStage.FINAL_RECORD
    )
    materialized = ScrollCandidate.from_record(
        record,
        playthrough=3,
        record_stage=installed_record_stage,
    )
    preview_slots = tuple(
        (
            effect.effect_id,
            effect.value,
            effect.metadata,
            effect.prefix,
            effect.tail_0,
            effect.tail_1,
        )
        for effect in candidate.effects
    )
    preview_candidate = ScrollCandidate.from_effect_sequence(installed_preview)
    materialized_preview_slots = tuple(
        (
            effect.effect_id,
            effect.value,
            effect.metadata,
            effect.prefix,
            effect.tail_0,
            effect.tail_1,
        )
        for effect in preview_candidate.effects[:len(candidate.effects)]
    )
    if materialized_preview_slots != preview_slots:
        raise RuntimeError("安装记录的揭露后结果与求解器预览不一致，已拒绝写入")
    if any(
        not effect.is_empty
        for effect in preview_candidate.effects[len(candidate.effects):]
    ):
        raise RuntimeError("安装时物化记录出现预览之外的额外词条，已拒绝写入")
    if account_id_from_record(record) != inventory.account_id:
        raise RuntimeError("安装时物化记录没有绑定当前存档来源账号，已拒绝写入")
    return materialized


@dataclass(frozen=True, slots=True)
class InstallResult:
    slot_index: int
    record_offset: int
    backup_directory: Path
    installed_sha256: str
    report_path: Path


@dataclass(frozen=True, slots=True)
class BatchInstallResult:
    slot_indices: tuple[int, ...]
    record_offsets: tuple[int, ...]
    backup_directory: Path
    installed_sha256: str
    report_path: Path


@dataclass(frozen=True, slots=True)
class BatchEditResult:
    slot_indices: tuple[int, ...]
    record_offsets: tuple[int, ...]
    backup_directory: Path
    installed_sha256: str
    report_path: Path


@dataclass(frozen=True, slots=True)
class BackupEntry:
    directory: Path
    timestamp: str
    action: str
    account_id: int | None
    report_path: Path | None
    file_count: int
    main_save_sha256: str | None


@dataclass(frozen=True, slots=True)
class RestoreResult:
    source_backup_directory: Path
    checkpoint_directory: Path
    restored_targets: tuple[Path, ...]
    report_path: Path


def _validated_backup_directory(state_root: Path, directory: Path) -> Path:
    backups_root = (state_root.resolve() / "backups").resolve()
    if directory.is_symlink():
        raise ValueError("拒绝操作指向其他位置的备份目录链接")
    candidate = directory.resolve()
    if candidate.parent != backups_root:
        raise ValueError("备份目录不在本应用管理的 backups 根目录中")
    if not candidate.is_dir():
        raise FileNotFoundError(candidate)
    return candidate


def list_backup_entries(state_root: Path) -> tuple[BackupEntry, ...]:
    """List direct child backup bundles without following external paths."""

    backups_root = state_root.resolve() / "backups"
    if not backups_root.is_dir():
        return ()
    entries: list[BackupEntry] = []
    for directory in sorted(backups_root.iterdir(), reverse=True):
        if directory.is_symlink() or not directory.is_dir():
            continue
        report_path = next(
            (
                candidate
                for candidate in (
                    directory / "restore-report.json",
                    directory / "edit-report.json",
                    directory / "install-report.json",
                )
                if candidate.is_file()
            ),
            None,
        )
        report: dict[str, Any] = {}
        if report_path is not None:
            try:
                loaded = json.loads(report_path.read_text(encoding="utf-8"))
                if isinstance(loaded, dict):
                    report = loaded
            except (OSError, ValueError, TypeError):
                report = {}
        action = str(report.get("action") or "")
        if not action:
            metadata = report.get("metadata")
            if isinstance(metadata, dict):
                action = str(metadata.get("operation") or "")
        if not action:
            action = "scroll-install" if "candidate" in report else "unknown"
        account_id_value = report.get("steam_account_id")
        try:
            account_id = int(account_id_value) if account_id_value is not None else None
        except (TypeError, ValueError):
            account_id = None
        main_save = directory / "SAVEDATA.BIN"
        reported_main_hash: str | None = None
        for collection_name in ("backup_files", "checkpoint_files"):
            collection = report.get(collection_name)
            if not isinstance(collection, list):
                continue
            for item in collection:
                if not isinstance(item, dict):
                    continue
                if item.get("backup_file") != "SAVEDATA.BIN":
                    continue
                digest = item.get("sha256")
                if isinstance(digest, str) and digest:
                    reported_main_hash = digest.upper()
                    break
            if reported_main_hash is not None:
                break
        entries.append(
            BackupEntry(
                directory=directory.resolve(),
                timestamp=directory.name,
                action=action,
                account_id=account_id,
                report_path=report_path.resolve() if report_path is not None else None,
                file_count=sum(1 for item in directory.iterdir() if item.is_file()),
                main_save_sha256=(
                    reported_main_hash
                    if reported_main_hash is not None
                    else sha256_file(main_save)
                    if main_save.is_file()
                    else None
                ),
            )
        )
    return tuple(entries)


def move_backup_to_recycle_bin(state_root: Path, directory: Path) -> None:
    """Move one application-owned backup directory to the Windows recycle bin."""

    target = _validated_backup_directory(state_root, directory)
    if os.name != "nt":
        raise OSError("Windows recycle-bin deletion is only available on Windows")

    class SHFILEOPSTRUCTW(ctypes.Structure):
        _fields_ = (
            ("hwnd", ctypes.c_void_p),
            ("wFunc", ctypes.c_uint),
            ("pFrom", ctypes.c_wchar_p),
            ("pTo", ctypes.c_wchar_p),
            ("fFlags", ctypes.c_ushort),
            ("fAnyOperationsAborted", ctypes.c_int),
            ("hNameMappings", ctypes.c_void_p),
            ("lpszProgressTitle", ctypes.c_wchar_p),
        )

    source_buffer = ctypes.create_unicode_buffer(str(target) + "\0")
    operation = SHFILEOPSTRUCTW()
    operation.wFunc = 0x0003  # FO_DELETE
    operation.pFrom = ctypes.cast(source_buffer, ctypes.c_wchar_p)
    operation.fFlags = 0x0040 | 0x0010 | 0x0004  # ALLOWUNDO | NOCONFIRMATION | SILENT
    result = ctypes.windll.shell32.SHFileOperationW(ctypes.byref(operation))
    if result != 0:
        raise OSError(result, "Windows 拒绝把备份目录移入回收站")
    if operation.fAnyOperationsAborted:
        raise RuntimeError("备份删除操作已被取消")


class SaveInstaller:
    def __init__(self, *, save_path: Path, crypto: SaveCrypto, state_root: Path) -> None:
        self.save_path = save_path.resolve()
        self.crypto = crypto
        self.state_root = state_root.resolve()

    def capture_inventory(self) -> SaveInventory:
        with tempfile.TemporaryDirectory(prefix="nioh3-scroll-read-") as directory:
            decrypted_path = Path(directory) / "decrypted.bin"
            self.crypto.decrypt(self.save_path, decrypted_path)
            return SaveInventory.load(self.save_path, decrypted_path.read_bytes())

    def restore_backup(self, backup_directory: Path) -> RestoreResult:
        """Restore an application backup after checkpointing the current files."""

        source_directory = _validated_backup_directory(
            self.state_root,
            backup_directory,
        )
        source_targets = (
            (
                source_directory / "SYSTEMSAVEDATA.BIN",
                self.save_path.parent.parent / "SYSTEMSAVEDATA00" / "SAVEDATA.BIN",
                "system_save",
            ),
            (
                source_directory / "BACKUP.BIN",
                self.save_path.parent / "BACKUP.BIN",
                "game_backup",
            ),
            (source_directory / "SAVEDATA.BIN", self.save_path, "main_save"),
        )
        available = tuple(
            (source, target, role)
            for source, target, role in source_targets
            if source.is_file()
        )
        if not any(role == "main_save" for _source, _target, role in available):
            raise FileNotFoundError("所选备份不含主 SAVEDATA.BIN")
        if not self.save_path.is_file():
            raise FileNotFoundError(self.save_path)

        current_targets = (
            (self.save_path, "SAVEDATA.BIN"),
            (self.save_path.parent / "BACKUP.BIN", "BACKUP.BIN"),
            (
                self.save_path.parent.parent / "SYSTEMSAVEDATA00" / "SAVEDATA.BIN",
                "SYSTEMSAVEDATA.BIN",
            ),
        )
        source_hashes = {
            path.resolve(): sha256_file(path)
            for path, _backup_name in current_targets
            if path.is_file()
        }
        checkpoint_directory = create_backup_directory(self.state_root)
        checkpoint_files: list[dict[str, object]] = []
        for current_path, backup_name in current_targets:
            if not current_path.is_file():
                continue
            destination = checkpoint_directory / backup_name
            shutil.copy2(current_path, destination)
            checkpoint_files.append(
                {
                    "source_role": backup_name,
                    "backup_file": backup_name,
                    "size": destination.stat().st_size,
                    "sha256": sha256_file(destination),
                }
            )

        staged: list[tuple[Path, Path, str, str]] = []
        try:
            for source, target, role in available:
                target.parent.mkdir(parents=True, exist_ok=True)
                temporary = target.with_name(target.name + ".scroll-generator-restore.tmp")
                if temporary.exists():
                    temporary.unlink()
                shutil.copy2(source, temporary)
                source_digest = sha256_file(source)
                if sha256_file(temporary) != source_digest:
                    raise RuntimeError("恢复文件没有通过临时副本哈希验证")
                staged.append((temporary, target, source_digest, role))
            for path, expected_hash in source_hashes.items():
                if not path.is_file() or sha256_file(path) != expected_hash:
                    raise RuntimeError("准备恢复期间当前存档发生变化，已拒绝覆盖")
            restored_targets: list[Path] = []
            restored_roles: list[str] = []
            for temporary, target, expected_digest, role in staged:
                os.replace(temporary, target)
                if sha256_file(target) != expected_digest:
                    raise RuntimeError("恢复后的文件哈希与所选备份不一致")
                restored_targets.append(target.resolve())
                restored_roles.append(role)
        finally:
            for temporary, _target, _digest, _role in staged:
                if temporary.exists():
                    temporary.unlink()

        report = {
            "action": "restore-backup",
            "restored_at_utc": datetime.now(timezone.utc).isoformat(),
            "steam_account_id": account_id_from_save_path(self.save_path),
            "source_backup_directory": source_directory.name,
            "restored_targets": restored_roles,
            "checkpoint_files": checkpoint_files,
        }
        report_path = checkpoint_directory / "restore-report.json"
        report_path.write_text(
            json.dumps(report, indent=2, ensure_ascii=False),
            encoding="utf-8",
        )
        return RestoreResult(
            source_backup_directory=source_directory,
            checkpoint_directory=checkpoint_directory,
            restored_targets=tuple(restored_targets),
            report_path=report_path,
        )

    def edit_many(
        self,
        edits: Sequence[tuple[int, bytes, bytes]],
        *,
        action: str,
        metadata: dict[str, Any] | None = None,
    ) -> BatchEditResult:
        """Replace existing records under exact-original and source-hash gates."""
        normalized_edits = tuple(edits)
        if not normalized_edits:
            raise ValueError("至少需要一项绘卷编辑")
        slot_indices = tuple(item[0] for item in normalized_edits)
        if len(set(slot_indices)) != len(slot_indices):
            raise ValueError("同一绘卷栏位不能在一次事务中重复编辑")
        for slot_index, expected_original, replacement in normalized_edits:
            if not 0 <= slot_index < SCROLL_SLOT_COUNT:
                raise ValueError("绘卷栏位超出当前存档结构范围")
            if len(expected_original) != SCROLL_RECORD_SIZE:
                raise ValueError("原始绘卷记录必须为 0xE8 字节")
            if len(replacement) != SCROLL_RECORD_SIZE:
                raise ValueError("替换绘卷记录必须为 0xE8 字节")
            if not any(expected_original):
                raise ValueError("现有绘卷编辑不能以全零栏位作为原始记录")
            if any(replacement) and struct.unpack_from("<H", replacement, 0)[0] == 0:
                raise ValueError("非空替换绘卷类型不能为零")
        normalized_action = action.strip()
        if not normalized_action:
            raise ValueError("备份操作名称不能为空")
        if not self.save_path.is_file():
            raise FileNotFoundError(self.save_path)

        source_hash = sha256_file(self.save_path)
        backup_directory = create_backup_directory(self.state_root)

        save_directory = self.save_path.parent
        related = (
            self.save_path,
            save_directory / "BACKUP.BIN",
            save_directory.parent / "SYSTEMSAVEDATA00" / "SAVEDATA.BIN",
        )
        copied: list[dict[str, object]] = []
        for source in related:
            if not source.is_file():
                continue
            destination = backup_directory / source.name
            if source.name == "SAVEDATA.BIN" and source != self.save_path:
                destination = backup_directory / "SYSTEMSAVEDATA.BIN"
            shutil.copy2(source, destination)
            copied.append(
                {
                    "source_role": (
                        "main_save"
                        if source == self.save_path
                        else "system_save"
                        if source.name == "SAVEDATA.BIN"
                        else "game_backup"
                    ),
                    "backup_file": destination.name,
                    "size": destination.stat().st_size,
                    "sha256": sha256_file(destination),
                }
            )

        with tempfile.TemporaryDirectory(prefix="nioh3-scroll-batch-edit-") as directory:
            work = Path(directory)
            decrypted_path = work / "decrypted.bin"
            edited_path = work / "edited.bin"
            encrypted_path = work / "encrypted.bin"
            verification_path = work / "verification.bin"
            self.crypto.decrypt(self.save_path, decrypted_path)
            decrypted = decrypted_path.read_bytes()
            require_decrypted_user_save(decrypted)
            edited = bytearray(decrypted)
            edit_reports: list[dict[str, object]] = []
            record_offsets: list[int] = []
            for slot_index, expected_original, replacement in normalized_edits:
                record_offset = SCROLL_GROUP_OFFSET + slot_index * SCROLL_RECORD_SIZE
                current = decrypted[record_offset:record_offset + SCROLL_RECORD_SIZE]
                if current != expected_original:
                    raise RuntimeError(
                        f"栏位 {slot_index} 已发生变化，未通过原始记录逐字节门禁"
                    )
                changed_offsets = [
                    index
                    for index, (before, after) in enumerate(
                        zip(expected_original, replacement, strict=True)
                    )
                    if before != after
                ]
                edited[record_offset:record_offset + SCROLL_RECORD_SIZE] = replacement
                record_offsets.append(record_offset)
                edit_reports.append(
                    {
                        "slot_index": slot_index,
                        "record_offset": record_offset,
                        "record_offset_hex": hex(record_offset),
                        "changed_record_byte_count": len(changed_offsets),
                        "changed_record_offsets_hex": [hex(value) for value in changed_offsets],
                        "before": describe_record_for_report(expected_original),
                        "after": describe_record_for_report(replacement),
                        "before_record_hex": expected_original.hex(),
                        "after_record_hex": replacement.hex(),
                    }
                )
            old_checksum, new_checksum = patch_user_checksum(edited)
            edited_bytes = bytes(edited)
            edited_path.write_bytes(edited_bytes)
            self.crypto.encrypt(edited_path, encrypted_path)
            self.crypto.decrypt(encrypted_path, verification_path)
            if verification_path.read_bytes() != edited_bytes:
                raise RuntimeError("批量编辑存档未通过加密后精确回读验证")
            if sha256_file(self.save_path) != source_hash:
                raise RuntimeError("准备期间游戏存档发生变化，已拒绝写入")

            installed_temp = self.save_path.with_name("SAVEDATA.BIN.scroll-generator.tmp")
            if installed_temp.exists():
                installed_temp.unlink()
            try:
                shutil.copy2(encrypted_path, installed_temp)
                if sha256_file(installed_temp) != sha256_file(encrypted_path):
                    raise RuntimeError("批量编辑临时写入文件的哈希不一致")
                os.replace(installed_temp, self.save_path)
            finally:
                if installed_temp.exists():
                    installed_temp.unlink()

        report = {
            "installed_at_utc": datetime.now(timezone.utc).isoformat(),
            "action": normalized_action,
            "steam_account_id": account_id_from_save_path(self.save_path),
            "source_sha256": source_hash,
            "installed_sha256": sha256_file(self.save_path),
            "slot_indices": list(slot_indices),
            "record_offsets": record_offsets,
            "record_offsets_hex": [hex(offset) for offset in record_offsets],
            "checksum": {"old": old_checksum, "new": new_checksum},
            "edits": edit_reports,
            "backup_files": copied,
            "metadata": metadata or {},
        }
        report_path = backup_directory / "edit-report.json"
        report_path.write_text(
            json.dumps(report, indent=2, ensure_ascii=False), encoding="utf-8"
        )
        return BatchEditResult(
            slot_indices=slot_indices,
            record_offsets=tuple(record_offsets),
            backup_directory=backup_directory,
            installed_sha256=report["installed_sha256"],
            report_path=report_path,
        )

    def delete_many(
        self,
        entries: Sequence[ScrollInventoryEntry],
    ) -> BatchEditResult:
        """Clear selected records in place under the normal edit transaction."""

        selected = tuple(entries)
        if not selected:
            raise ValueError("至少需要选择一张要删除的绘卷")
        return self.edit_many(
            tuple(
                (entry.slot_index, entry.record, bytes(SCROLL_RECORD_SIZE))
                for entry in selected
            ),
            action="local-scroll-delete",
            metadata={
                "local_only": True,
                "operation": "clear-record-without-compaction",
                "slot_indices": [entry.slot_index for entry in selected],
            },
        )

    def install_many(
        self,
        candidate_records: Sequence[bytes],
        *,
        action: str,
        metadata: dict[str, Any] | None = None,
    ) -> BatchInstallResult:
        """Insert several complete records in one backed-up atomic transaction."""
        records = tuple(candidate_records)
        if not records:
            raise ValueError("至少需要一张候选绘卷")
        for record in records:
            if len(record) != SCROLL_RECORD_SIZE:
                raise ValueError("每张候选绘卷记录必须为 0xE8 字节")
            if struct.unpack_from("<H", record, 0)[0] == 0:
                raise ValueError("候选绘卷类型不能为零")
        normalized_action = action.strip()
        if not normalized_action:
            raise ValueError("备份操作名称不能为空")
        if not self.save_path.is_file():
            raise FileNotFoundError(self.save_path)

        source_hash = sha256_file(self.save_path)
        backup_directory = create_backup_directory(self.state_root)

        save_directory = self.save_path.parent
        related = (
            self.save_path,
            save_directory / "BACKUP.BIN",
            save_directory.parent / "SYSTEMSAVEDATA00" / "SAVEDATA.BIN",
        )
        copied: list[dict[str, object]] = []
        for source in related:
            if not source.is_file():
                continue
            destination = backup_directory / source.name
            if source.name == "SAVEDATA.BIN" and source != self.save_path:
                destination = backup_directory / "SYSTEMSAVEDATA.BIN"
            shutil.copy2(source, destination)
            copied.append(
                {
                    "source_role": (
                        "main_save"
                        if source == self.save_path
                        else "system_save"
                        if source.name == "SAVEDATA.BIN"
                        else "game_backup"
                    ),
                    "backup_file": destination.name,
                    "size": destination.stat().st_size,
                    "sha256": sha256_file(destination),
                }
            )

        with tempfile.TemporaryDirectory(prefix="nioh3-scroll-batch-install-") as directory:
            work = Path(directory)
            decrypted_path = work / "decrypted.bin"
            edited_path = work / "edited.bin"
            encrypted_path = work / "encrypted.bin"
            verification_path = work / "verification.bin"
            self.crypto.decrypt(self.save_path, decrypted_path)
            inventory = SaveInventory.load(self.save_path, decrypted_path.read_bytes())
            normalized_decrypted, generation_serial_repairs = (
                repair_duplicate_scroll_generation_serials(inventory.decrypted)
            )
            if generation_serial_repairs:
                inventory = SaveInventory.load(self.save_path, normalized_decrypted)
            inventory_key_repairs: tuple[dict[str, int | str], ...] = ()
            first_slot = inventory.next_slot_index
            if first_slot is None:
                raise RuntimeError("400 个绘卷栏位均已占用，无法添加实验记录")
            slot_indices = tuple(range(first_slot, first_slot + len(records)))
            if slot_indices[-1] >= SCROLL_SLOT_COUNT:
                raise RuntimeError("没有足够的连续栏位添加全部实验记录")
            if any(index not in inventory.empty_slots for index in slot_indices):
                raise RuntimeError("目标连续栏位中存在已占用记录，已拒绝写入")

            inventory_keys = allocate_scroll_inventory_keys(
                inventory.decrypted,
                len(records),
            )
            generation_serials = allocate_scroll_generation_serials(
                inventory.decrypted,
                len(records),
            )
            installed_records = tuple(
                write_scroll_generation_serial(
                    write_scroll_inventory_key(record, inventory_key),
                    generation_serial,
                )
                for record, inventory_key, generation_serial in zip(
                    records,
                    inventory_keys,
                    generation_serials,
                    strict=True,
                )
            )

            edited = inventory.decrypted
            insert_reports: list[dict[str, object]] = []
            record_offsets: list[int] = []
            for slot_index, record in zip(
                slot_indices,
                installed_records,
                strict=True,
            ):
                record_offset = SCROLL_GROUP_OFFSET + slot_index * SCROLL_RECORD_SIZE
                edited = _clear_native_free_scroll_slot(edited, record_offset)
                edited, insert_report = insert_scroll_record(
                    edited,
                    record_offset=record_offset,
                    record=record,
                )
                record_offsets.append(record_offset)
                insert_reports.append(insert_report)

            edited_path.write_bytes(edited)
            self.crypto.encrypt(edited_path, encrypted_path)
            self.crypto.decrypt(encrypted_path, verification_path)
            if verification_path.read_bytes() != edited:
                raise RuntimeError("批量候选存档未通过加密后精确回读验证")
            if sha256_file(self.save_path) != source_hash:
                raise RuntimeError("准备期间游戏存档发生变化，已拒绝写入")

            installed_temp = self.save_path.with_name("SAVEDATA.BIN.scroll-generator.tmp")
            if installed_temp.exists():
                installed_temp.unlink()
            try:
                shutil.copy2(encrypted_path, installed_temp)
                if sha256_file(installed_temp) != sha256_file(encrypted_path):
                    raise RuntimeError("批量临时写入文件的哈希不一致")
                os.replace(installed_temp, self.save_path)
            finally:
                if installed_temp.exists():
                    installed_temp.unlink()

        report = {
            "installed_at_utc": datetime.now(timezone.utc).isoformat(),
            "action": normalized_action,
            "steam_account_id": account_id_from_save_path(self.save_path),
            "source_sha256": source_hash,
            "installed_sha256": sha256_file(self.save_path),
            "slot_indices": list(slot_indices),
            "record_offsets": record_offsets,
            "record_offsets_hex": [hex(offset) for offset in record_offsets],
            "inventory_keys": list(inventory_keys),
            "inventory_keys_hex": [hex(value) for value in inventory_keys],
            "inventory_key_repairs": list(inventory_key_repairs),
            "generation_serials": list(generation_serials),
            "generation_serials_hex": [hex(value) for value in generation_serials],
            "generation_serial_repairs": list(generation_serial_repairs),
            "records": [
                describe_record_for_report(record) for record in installed_records
            ],
            "inserts": insert_reports,
            "backup_files": copied,
            "metadata": metadata or {},
        }
        report_path = backup_directory / "install-report.json"
        report_path.write_text(
            json.dumps(report, indent=2, ensure_ascii=False), encoding="utf-8"
        )
        return BatchInstallResult(
            slot_indices=slot_indices,
            record_offsets=tuple(record_offsets),
            backup_directory=backup_directory,
            installed_sha256=report["installed_sha256"],
            report_path=report_path,
        )

    def install(
        self,
        candidate_record: bytes,
        *,
        transfer_count: int,
        expected_source_sha256: str | None = None,
        metadata: dict[str, Any] | None = None,
    ) -> InstallResult:
        if not self.save_path.is_file():
            raise FileNotFoundError(self.save_path)
        source_hash = sha256_file(self.save_path)
        if (
            expected_source_sha256 is not None
            and source_hash.upper() != expected_source_sha256.upper()
        ):
            raise RuntimeError("候选绑定后游戏存档发生变化，已拒绝写入")
        backup_directory = create_backup_directory(self.state_root)

        save_directory = self.save_path.parent
        related = (
            self.save_path,
            save_directory / "BACKUP.BIN",
            save_directory.parent / "SYSTEMSAVEDATA00" / "SAVEDATA.BIN",
        )
        copied: list[dict[str, object]] = []
        for source in related:
            if not source.is_file():
                continue
            destination = backup_directory / source.name
            if source.name == "SAVEDATA.BIN" and source != self.save_path:
                destination = backup_directory / "SYSTEMSAVEDATA.BIN"
            shutil.copy2(source, destination)
            copied.append(
                {
                    "source_role": (
                        "main_save"
                        if source == self.save_path
                        else "system_save"
                        if source.name == "SAVEDATA.BIN"
                        else "game_backup"
                    ),
                    "backup_file": destination.name,
                    "size": destination.stat().st_size,
                    "sha256": sha256_file(destination),
                }
            )

        with tempfile.TemporaryDirectory(prefix="nioh3-scroll-install-") as directory:
            work = Path(directory)
            decrypted_path = work / "decrypted.bin"
            edited_path = work / "edited.bin"
            encrypted_path = work / "encrypted.bin"
            verification_path = work / "verification.bin"
            self.crypto.decrypt(self.save_path, decrypted_path)
            inventory = SaveInventory.load(self.save_path, decrypted_path.read_bytes())
            normalized_decrypted, generation_serial_repairs = (
                repair_duplicate_scroll_generation_serials(inventory.decrypted)
            )
            if generation_serial_repairs:
                inventory = SaveInventory.load(self.save_path, normalized_decrypted)
            inventory_key_repairs: tuple[dict[str, int | str], ...] = ()
            slot_index = inventory.next_slot_index
            if slot_index is None:
                raise RuntimeError("400 个绘卷栏位均已占用，无法添加新绘卷")
            if slot_index not in inventory.empty_slots:
                raise RuntimeError("下一个绘卷栏位已占用，已拒绝写入")
            record_offset = SCROLL_GROUP_OFFSET + slot_index * SCROLL_RECORD_SIZE
            record = prepare_candidate_for_install(
                candidate_record, transfer_count=transfer_count
            )
            inventory_key = allocate_scroll_inventory_keys(
                inventory.decrypted,
                1,
            )[0]
            generation_serial = allocate_scroll_generation_serials(
                inventory.decrypted,
                1,
            )[0]
            record = write_scroll_inventory_key(record, inventory_key)
            record = write_scroll_generation_serial(record, generation_serial)
            normalized = _clear_native_free_scroll_slot(
                inventory.decrypted,
                record_offset,
            )
            edited, insert_report = insert_scroll_record(
                normalized,
                record_offset=record_offset,
                record=record,
            )
            edited_path.write_bytes(edited)
            self.crypto.encrypt(edited_path, encrypted_path)
            self.crypto.decrypt(encrypted_path, verification_path)
            if verification_path.read_bytes() != edited:
                raise RuntimeError("候选存档未通过加密后精确回读验证")
            if sha256_file(self.save_path) != source_hash:
                raise RuntimeError("准备期间游戏存档发生变化，已拒绝写入")

            installed_temp = self.save_path.with_name("SAVEDATA.BIN.scroll-generator.tmp")
            if installed_temp.exists():
                installed_temp.unlink()
            try:
                shutil.copy2(encrypted_path, installed_temp)
                if sha256_file(installed_temp) != sha256_file(encrypted_path):
                    raise RuntimeError("临时写入文件的哈希不一致")
                os.replace(installed_temp, self.save_path)
            finally:
                if installed_temp.exists():
                    installed_temp.unlink()

        report = {
            "installed_at_utc": datetime.now(timezone.utc).isoformat(),
            "steam_account_id": account_id_from_save_path(self.save_path),
            "source_sha256": source_hash,
            "installed_sha256": sha256_file(self.save_path),
            "slot_index": slot_index,
            "record_offset": record_offset,
            "record_offset_hex": hex(record_offset),
            "inventory_key": inventory_key,
            "inventory_key_hex": hex(inventory_key),
            "inventory_key_repairs": list(inventory_key_repairs),
            "generation_serial": generation_serial,
            "generation_serial_hex": hex(generation_serial),
            "generation_serial_repairs": list(generation_serial_repairs),
            "candidate": describe_record_for_report(record),
            "insert": insert_report,
            "backup_files": copied,
            "metadata": metadata or {},
        }
        report_path = backup_directory / "install-report.json"
        report_path.write_text(
            json.dumps(report, indent=2, ensure_ascii=False), encoding="utf-8"
        )
        return InstallResult(
            slot_index=slot_index,
            record_offset=record_offset,
            backup_directory=backup_directory,
            installed_sha256=report["installed_sha256"],
            report_path=report_path,
        )

    def install_effect_sequence_candidate(
        self,
        candidate: ScrollCandidate,
        *,
        level: int,
        recommended_level: int,
        transfer_count: int,
    ) -> InstallResult:
        """Materialize and install one certified preview under a source hash gate."""

        source_hash = sha256_file(self.save_path)
        inventory = self.capture_inventory()
        if sha256_file(self.save_path) != source_hash:
            raise RuntimeError("读取候选模板期间游戏存档发生变化，已拒绝写入")
        materialized = materialize_effect_sequence_candidate(
            inventory,
            candidate,
            level=level,
            recommended_level=recommended_level,
            transfer_count=transfer_count,
        )
        generation_serial = struct.unpack_from("<I", materialized.record, 0x28)[0]
        return self.install(
            materialized.record,
            transfer_count=transfer_count,
            expected_source_sha256=source_hash,
            metadata={
                "materializer": f"game-closed-ng3-rarity{candidate.rarity}-v2.00.02",
                "native_full_record_parity_vectors": 10_000,
                "seed": candidate.seed,
                "generation_serial": generation_serial,
                "preview_record_stage": candidate.record_stage.value,
                "installed_record_stage": materialized.record_stage.value,
                "rarity4_reveal_passes_expected": 1 if candidate.rarity == 4 else 0,
            },
        )


def source_template_for_scan(inventory: SaveInventory) -> bytes:
    return build_source_record(
        inventory.template_record,
        seed=1,
        rarity=5,
        level=180,
        recommended_level=183,
        transfer_count=0,
    )
