from __future__ import annotations

import struct
from dataclasses import dataclass

from emaki_exchange import (
    EFFECT_START,
    EFFECT_STRIDE,
    SCROLL_RECORD_SIZE,
    account_id_from_record,
    incomplete_from_record,
)

from .savegame import (
    SCROLL_GROUP_OFFSET,
    SCROLL_SLOT_COUNT,
    SaveInventory,
    next_generation_serial,
)


CONTEXTUAL_TEST_EFFECT_ID = 0xBABD
CONTEXTUAL_TEST_SLOTS = (4, 5, 6)
CONTEXTUAL_TEST_RECORD_TYPE = 0xE604
CONTEXTUAL_TEST_RARITY = 5


@dataclass(frozen=True, slots=True)
class ContextualSlotExperimentRecord:
    target_slot: int
    original_effect_id: int
    generation_serial: int
    transfer_count: int
    record: bytes


@dataclass(frozen=True, slots=True)
class ContextualSlotExperiment:
    template_slot: int
    template_record: bytes
    records: tuple[ContextualSlotExperimentRecord, ...]

    def metadata(self) -> dict[str, object]:
        return {
            "format": "nioh3-contextual-babd-slot-experiment-v1",
            "game_version": "2.00.02",
            "locale_capture_status": "pending",
            "hypothesis": {
                "slot_4": "月读的恩宠",
                "slot_5": "月读的恩宠",
                "slot_6": "月读的恩宠",
            },
            "retracted_claim": "0xBABD is not 技之深奥 in slot 4 or slot 5",
            "raw_effect_id": f"0x{CONTEXTUAL_TEST_EFFECT_ID:08X}",
            "template_slot": self.template_slot,
            "template_record_hex": self.template_record.hex(),
            "records": [
                {
                    "target_slot": item.target_slot,
                    "original_effect_id": f"0x{item.original_effect_id:08X}",
                    "generation_serial": item.generation_serial,
                    "transfer_count": item.transfer_count,
                    "record_hex": item.record.hex(),
                    "target_slot_hex": _slot_bytes(item.record, item.target_slot).hex(),
                }
                for item in self.records
            ],
            "acceptance": "Requires one in-game detail capture for every record",
            "propagation_claim": False,
        }


@dataclass(frozen=True, slots=True)
class ExistingContextualSlotExperimentEdit:
    inventory_slot: int
    target_slot: int
    original_effect_id: int
    original_record: bytes
    replacement_record: bytes


@dataclass(frozen=True, slots=True)
class ExistingContextualSlotExperiment:
    edits: tuple[ExistingContextualSlotExperimentEdit, ...]

    def metadata(self) -> dict[str, object]:
        return {
            "format": "nioh3-contextual-babd-existing-slot-experiment-v1",
            "game_version": "2.00.02",
            "locale_capture_status": "pending",
            "hypothesis": {
                "slot_4": "月读的恩宠",
                "slot_5": "月读的恩宠",
                "slot_6": "月读的恩宠",
            },
            "control_scope": (
                "exploratory only; the three source records are not identical"
            ),
            "retracted_claim": "0xBABD is not 技之深奥 in slot 4 or slot 5",
            "raw_effect_id": f"0x{CONTEXTUAL_TEST_EFFECT_ID:08X}",
            "edits": [
                {
                    "inventory_slot": item.inventory_slot,
                    "target_slot": item.target_slot,
                    "original_effect_id": f"0x{item.original_effect_id:08X}",
                    "seed": struct.unpack_from("<I", item.original_record, 0x20)[0],
                    "generation_serial": struct.unpack_from(
                        "<I", item.original_record, 0x28
                    )[0],
                    "transfer_count": struct.unpack_from(
                        "<I", item.original_record, 0xDC
                    )[0],
                    "original_record_hex": item.original_record.hex(),
                    "replacement_record_hex": item.replacement_record.hex(),
                    "original_target_slot_hex": _slot_bytes(
                        item.original_record, item.target_slot
                    ).hex(),
                    "replacement_target_slot_hex": _slot_bytes(
                        item.replacement_record, item.target_slot
                    ).hex(),
                }
                for item in self.edits
            ],
            "acceptance": "Requires one in-game detail capture for every edited record",
            "propagation_claim": False,
        }


def _effect_id_offset(slot: int) -> int:
    if not 1 <= slot <= 7:
        raise ValueError("effect slot must be between 1 and 7")
    return EFFECT_START + (slot - 1) * EFFECT_STRIDE + 4


def _effect_id(record: bytes, slot: int) -> int:
    return struct.unpack_from("<I", record, _effect_id_offset(slot))[0]


def _slot_bytes(record: bytes, slot: int) -> bytes:
    start = EFFECT_START + (slot - 1) * EFFECT_STRIDE
    return record[start:start + EFFECT_STRIDE]


def _rarity_mirrors_match(record: bytes, rarity: int) -> bool:
    return record[0x30] == rarity and record[0x31] == rarity


def _occupied_records(inventory: SaveInventory):
    for slot in range(SCROLL_SLOT_COUNT):
        start = SCROLL_GROUP_OFFSET + slot * SCROLL_RECORD_SIZE
        record = inventory.decrypted[start:start + SCROLL_RECORD_SIZE]
        if any(record):
            yield slot, record


def select_contextual_experiment_template(inventory: SaveInventory) -> tuple[int, bytes]:
    candidates: list[tuple[bool, int, bytes]] = []
    for slot, record in _occupied_records(inventory):
        if struct.unpack_from("<H", record, 0)[0] != CONTEXTUAL_TEST_RECORD_TYPE:
            continue
        if not _rarity_mirrors_match(record, CONTEXTUAL_TEST_RARITY):
            continue
        if incomplete_from_record(record):
            continue
        visible_ids = tuple(_effect_id(record, index) for index in range(1, 7))
        if CONTEXTUAL_TEST_EFFECT_ID in visible_ids:
            continue
        if 0xFFFFFFFF in visible_ids:
            continue
        own_origin = account_id_from_record(record) == inventory.account_id
        candidates.append((own_origin, slot, record))
    if not candidates:
        raise RuntimeError(
            "存档中没有不含 0xBABD 的完整 E604 稀有度 5 绘卷，无法建立受控实验"
        )
    candidates.sort(key=lambda item: (not item[0], item[1]))
    _, slot, record = candidates[0]
    return slot, record


def build_contextual_babd_experiment(inventory: SaveInventory) -> ContextualSlotExperiment:
    template_slot, template = select_contextual_experiment_template(inventory)
    serial_start = next_generation_serial(inventory)
    records: list[ContextualSlotExperimentRecord] = []
    for index, target_slot in enumerate(CONTEXTUAL_TEST_SLOTS):
        record = bytearray(template)
        original_effect_id = _effect_id(record, target_slot)
        generation_serial = serial_start + index
        transfer_count = target_slot
        struct.pack_into("<I", record, _effect_id_offset(target_slot), CONTEXTUAL_TEST_EFFECT_ID)
        struct.pack_into("<I", record, 0x28, generation_serial)
        struct.pack_into("<I", record, 0xDC, transfer_count)
        records.append(
            ContextualSlotExperimentRecord(
                target_slot=target_slot,
                original_effect_id=original_effect_id,
                generation_serial=generation_serial,
                transfer_count=transfer_count,
                record=bytes(record),
            )
        )
    return ContextualSlotExperiment(
        template_slot=template_slot,
        template_record=template,
        records=tuple(records),
    )


def select_existing_contextual_experiment_records(
    inventory: SaveInventory,
) -> tuple[tuple[int, bytes], ...]:
    candidates: list[tuple[int, bytes]] = []
    for inventory_slot, record in _occupied_records(inventory):
        if struct.unpack_from("<H", record, 0)[0] != CONTEXTUAL_TEST_RECORD_TYPE:
            continue
        if not _rarity_mirrors_match(record, CONTEXTUAL_TEST_RARITY):
            continue
        if incomplete_from_record(record):
            continue
        effect_ids = tuple(_effect_id(record, index) for index in range(1, 7))
        if CONTEXTUAL_TEST_EFFECT_ID in effect_ids:
            continue
        if any(effect_id in (0, 0xFFFFFFFF) for effect_id in effect_ids):
            continue
        candidates.append((inventory_slot, record))
    if len(candidates) < len(CONTEXTUAL_TEST_SLOTS):
        raise RuntimeError(
            "存档中没有三张已揭秘且不含 0xBABD 的完整 E604 稀有度 5 绘卷"
        )
    candidates.sort(key=lambda item: item[0])
    return tuple(candidates[: len(CONTEXTUAL_TEST_SLOTS)])


def build_existing_contextual_babd_experiment(
    inventory: SaveInventory,
) -> ExistingContextualSlotExperiment:
    selected = select_existing_contextual_experiment_records(inventory)
    edits: list[ExistingContextualSlotExperimentEdit] = []
    for (inventory_slot, original), target_slot in zip(
        selected, CONTEXTUAL_TEST_SLOTS, strict=True
    ):
        replacement = bytearray(original)
        original_effect_id = _effect_id(original, target_slot)
        struct.pack_into(
            "<I",
            replacement,
            _effect_id_offset(target_slot),
            CONTEXTUAL_TEST_EFFECT_ID,
        )
        edits.append(
            ExistingContextualSlotExperimentEdit(
                inventory_slot=inventory_slot,
                target_slot=target_slot,
                original_effect_id=original_effect_id,
                original_record=original,
                replacement_record=bytes(replacement),
            )
        )
    return ExistingContextualSlotExperiment(edits=tuple(edits))
