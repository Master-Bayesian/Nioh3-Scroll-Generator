from __future__ import annotations

import argparse
import json
import struct
from dataclasses import asdict, dataclass
from pathlib import Path


SCROLL_RECORD_SIZE = 0xE8
EFFECT_START = 0x34
EFFECT_STRIDE = 0x18
EFFECT_COUNT = 7
EFFECT_REGION_SIZE = EFFECT_STRIDE * EFFECT_COUNT

USER_CHECKSUM_BODY_START = 0x190
USER_CHECKSUM_BODY_END = 0x900190
USER_CHECKSUM_SEED_OFFSET = 0x900190
USER_CHECKSUM_VALUE_OFFSET = 0x900194
USER_SAVE_SIZE = 0x9001B0

# Nioh3.exe v2.00.02: table at RVA 0x3BD46E0, used by
# sub_20DD84C / sub_20DD86C. Index 1 is the scroll type 0x1E82.
CATEGORY_TO_TYPE = (0x0000, 0x1E82, 0x516D, 0xE604, 0xDD82, 0xD523)
TYPE_TO_CATEGORY = {record_type: category for category, record_type in enumerate(CATEGORY_TO_TYPE)}


@dataclass(frozen=True)
class EmakiCanonicalTuple:
    """The authoritative fields carried by Online::EmakiItemExchange.

    `exchange_count` is the value in the outbound message. The sender increments
    the source record's +0xDC value before transmission.
    """

    record_type: int
    category: int
    random_seed: int
    exchange_count: int
    is_incomplete: bool
    effective_level: int
    recommended_level: int
    flag_nibble: int
    rarity: int
    packed_value: int
    account_id: int

    def emaki_item_bytes(self) -> bytes:
        return struct.pack(
            "<IIB3xI",
            self.random_seed,
            self.exchange_count,
            int(self.is_incomplete),
            self.packed_value,
        )

    def payload_bytes(self) -> bytes:
        """Return emaki_item_ (16 bytes) followed by account_id_ (8 bytes)."""
        return self.emaki_item_bytes() + struct.pack("<Q", self.account_id)

    def to_dict(self) -> dict:
        payload = asdict(self)
        for key in (
            "record_type",
            "random_seed",
            "packed_value",
            "account_id",
        ):
            payload[f"{key}_hex"] = hex(payload[key])
        payload["emaki_item_hex"] = self.emaki_item_bytes().hex()
        payload["payload_hex"] = self.payload_bytes().hex()
        return payload


def parse_int(value: str) -> int:
    return int(value, 0)


def require_new_output(path: Path, *, protected: tuple[Path, ...]) -> None:
    resolved = path.resolve()
    if any(resolved == item.resolve() for item in protected):
        raise ValueError(f"Refusing to overwrite a protected input: {path}")
    if path.exists():
        raise FileExistsError(path)


def require_decrypted_user_save(data: bytes) -> None:
    if len(data) != USER_SAVE_SIZE:
        raise ValueError(
            f"Expected a {USER_SAVE_SIZE:#x}-byte Nioh 3 user save, got {len(data):#x}"
        )
    if not data.startswith(b"RNNUSR"):
        raise ValueError("patch-canonical requires a decrypted RNNUSR user save")


def require_record(data: bytes, offset: int) -> bytes:
    end = offset + SCROLL_RECORD_SIZE
    if offset < 0 or end > len(data):
        raise ValueError("The requested 0xE8 record lies outside the input")
    record = data[offset:end]
    if struct.unpack_from("<H", record, 0)[0] == 0:
        raise ValueError("The selected record is empty")
    return record


def account_id_from_record(record: bytes) -> int:
    high = struct.unpack_from("<H", record, 0x02)[0]
    middle = struct.unpack_from("<H", record, 0x04)[0]
    low = struct.unpack_from("<I", record, 0x14)[0]
    return (high << 48) | (middle << 32) | low


def write_account_id(record: bytearray, account_id: int) -> None:
    if not 0 <= account_id <= 0xFFFFFFFFFFFFFFFF:
        raise ValueError("account_id must fit in uint64")
    struct.pack_into("<H", record, 0x02, (account_id >> 48) & 0xFFFF)
    struct.pack_into("<H", record, 0x04, (account_id >> 32) & 0xFFFF)
    struct.pack_into("<I", record, 0x14, account_id & 0xFFFFFFFF)


def effective_level_from_record(record: bytes) -> int:
    # sub_3DB834: records with bit 0x200000 at +0x18 serialize level 0;
    # otherwise the u16 at +0x06 is capped at 180.
    flags = struct.unpack_from("<I", record, 0x18)[0]
    if flags & 0x00200000:
        return 0
    return min(struct.unpack_from("<H", record, 0x06)[0], 180)


def incomplete_from_record(record: bytes) -> bool:
    # sub_110BF30 checks the sign bit of byte +0x0E in every 0x18 effect slot.
    return any(
        record[EFFECT_START + index * EFFECT_STRIDE + 0x0E] & 0x80
        for index in range(EFFECT_COUNT)
    )


def pack_value(
    *,
    category: int,
    effective_level: int,
    recommended_level: int,
    flag_nibble: int,
    rarity: int,
) -> int:
    for name, value, maximum in (
        ("category", category, 0x3),
        ("effective_level", effective_level, 0x3FF),
        ("recommended_level", recommended_level, 0xFFF),
        ("flag_nibble", flag_nibble, 0xF),
        ("rarity", rarity, 0xF),
    ):
        if not 0 <= value <= maximum:
            raise ValueError(f"{name}={value} does not fit its network bit field")
    return (
        category
        | (effective_level << 2)
        | (recommended_level << 12)
        | (flag_nibble << 24)
        | (rarity << 28)
    )


def unpack_value(value: int) -> dict[str, int]:
    if not 0 <= value <= 0xFFFFFFFF:
        raise ValueError("value must fit in uint32")
    return {
        "category": value & 0x3,
        "effective_level": (value >> 2) & 0x3FF,
        "recommended_level": (value >> 12) & 0xFFF,
        "flag_nibble": (value >> 24) & 0xF,
        "rarity": (value >> 28) & 0xF,
    }


def predict_outbound_tuple(record: bytes) -> EmakiCanonicalTuple:
    """Predict the tuple emitted by the normal send path in v2.00.02.

    The send path first calls sub_20DD6EC, which canonicalizes the selected
    inventory record and regenerates all effects. That function consumes:
      +0x20 seed/ID, +0x31 rarity (minimum 3), +0x10 recommended level,
      +0x06 effective level, +0x0F flag, +0xDC transfer count, and account ID.

    The exact game constructor clamps the recommended level against runtime
    parameter-table bounds. For the captured scrolls, +0x10 already equals the
    canonical +0x12 value, so this prediction is exact. A caller constructing a
    new out-of-range value must validate the clamp in game.
    """
    if len(record) != SCROLL_RECORD_SIZE:
        raise ValueError("record must be exactly 0xE8 bytes")

    record_type = struct.unpack_from("<H", record, 0x00)[0]
    try:
        category = TYPE_TO_CATEGORY[record_type]
    except KeyError as error:
        raise ValueError(f"Unknown item type for Emaki packing: {record_type:#x}") from error

    random_seed = struct.unpack_from("<I", record, 0x20)[0]
    source_transfer_count = struct.unpack_from("<I", record, 0xDC)[0]
    exchange_count = (source_transfer_count + 1) & 0xFFFFFFFF
    level = effective_level_from_record(record)
    recommended = struct.unpack_from("<H", record, 0x10)[0]
    flag = record[0x0F] & 0x0F
    if flag == 0:
        flag = 1  # sub_2277FE8 substitutes 1 for zero.
    rarity = max(record[0x31], 3) & 0x0F
    incomplete = incomplete_from_record(record)
    account_id = account_id_from_record(record)

    value = pack_value(
        category=category,
        effective_level=level,
        recommended_level=recommended,
        flag_nibble=flag,
        rarity=rarity,
    )
    return EmakiCanonicalTuple(
        record_type=record_type,
        category=category,
        random_seed=random_seed,
        exchange_count=exchange_count,
        is_incomplete=incomplete,
        effective_level=level,
        recommended_level=recommended,
        flag_nibble=flag,
        rarity=rarity,
        packed_value=value,
        account_id=account_id,
    )


def describe_record_for_report(record: bytes) -> dict:
    """Describe a stored record without inventing an unsupported wire tuple."""
    if len(record) != SCROLL_RECORD_SIZE:
        raise ValueError("record must be exactly 0xE8 bytes")
    record_type = struct.unpack_from("<H", record, 0)[0]
    try:
        category = TYPE_TO_CATEGORY[record_type]
    except KeyError as error:
        raise ValueError(f"Unknown item type for Emaki report: {record_type:#x}") from error
    if category <= 3:
        result = predict_outbound_tuple(record).to_dict()
        result["known_exchange_encoding"] = True
        return result

    # Categories 4 and 5 are valid entries in the executable's type table and
    # native generator.  The currently reversed EmakiItemExchange payload has
    # only two category bits, so no lossless packed_value can be reported.
    source_transfer_count = struct.unpack_from("<I", record, 0xDC)[0]
    return {
        "record_type": record_type,
        "record_type_hex": hex(record_type),
        "category": category,
        "random_seed": struct.unpack_from("<I", record, 0x20)[0],
        "random_seed_hex": hex(struct.unpack_from("<I", record, 0x20)[0]),
        "exchange_count": (source_transfer_count + 1) & 0xFFFFFFFF,
        "is_incomplete": incomplete_from_record(record),
        "effective_level": effective_level_from_record(record),
        "recommended_level": struct.unpack_from("<H", record, 0x10)[0],
        "flag_nibble": record[0x0F] & 0x0F or 1,
        "rarity": max(record[0x31], 3) & 0x0F,
        "packed_value": None,
        "account_id": account_id_from_record(record),
        "account_id_hex": hex(account_id_from_record(record)),
        "known_exchange_encoding": False,
        "exchange_warning": (
            "The currently reversed EmakiItemExchange message preserves only "
            "category & 3; alternate higher-playthrough exchange is unverified."
        ),
    }


def compute_user_checksum(body: bytes, seed: int) -> int:
    if len(body) != 0x900000:
        raise ValueError(f"Checksum body must be 0x900000 bytes, got {len(body):#x}")
    total = 0
    for base in range(0, len(body), 0x400):
        block_sum = 0
        for offset in range(base, base + 0x400, 8):
            block_sum += struct.unpack_from("<q", body, offset)[0]
        total = ((total + block_sum) ^ seed) & 0xFFFFFFFFFFFFFFFF
    return ((total // 0xFFFFFFFF) + (total & 0xFFFFFFFF)) & 0xFFFFFFFF


def patch_user_checksum(data: bytearray) -> tuple[int, int]:
    if len(data) < USER_CHECKSUM_VALUE_OFFSET + 4:
        raise ValueError("The input is too small to contain the Nioh 3 user checksum")
    seed = struct.unpack_from("<I", data, USER_CHECKSUM_SEED_OFFSET)[0]
    old = struct.unpack_from("<I", data, USER_CHECKSUM_VALUE_OFFSET)[0]
    new = compute_user_checksum(
        bytes(data[USER_CHECKSUM_BODY_START:USER_CHECKSUM_BODY_END]), seed
    )
    struct.pack_into("<I", data, USER_CHECKSUM_VALUE_OFFSET, new)
    return old, new


def patch_canonical_inputs(
    data: bytes,
    *,
    record_offset: int,
    random_seed: int | None = None,
    rarity: int | None = None,
    level: int | None = None,
    recommended_level: int | None = None,
    transfer_count: int | None = None,
    account_id: int | None = None,
    effect_template: bytes | None = None,
) -> tuple[bytes, dict]:
    """Patch only fields consumed by the canonical outbound regeneration path.

    `effect_template` is optional and affects only the sender's local resolved
    display/state. The normal send path discards these slots and regenerates them
    from the canonical type/rarity/seed tuple before serialization.
    """
    record = bytearray(require_record(data, record_offset))
    before_tuple = predict_outbound_tuple(bytes(record))

    if random_seed is not None:
        if not 0 <= random_seed <= 0xFFFFFFFF:
            raise ValueError("random_seed must fit in uint32")
        struct.pack_into("<I", record, 0x20, random_seed)
    if rarity is not None:
        if not 0 <= rarity <= 0x0F:
            raise ValueError("rarity must fit in 4 bits")
        # +0x31 is the authoritative input read by sub_20DD6EC. +0x30 is kept
        # synchronized for local resolved-state consistency/UI.
        record[0x30] = rarity
        record[0x31] = rarity
    if level is not None:
        if not 0 <= level <= 0xFFFF:
            raise ValueError("level must fit in uint16")
        struct.pack_into("<H", record, 0x06, level)
        struct.pack_into("<H", record, 0x08, level)
    if recommended_level is not None:
        if not 0 <= recommended_level <= 0xFFFF:
            raise ValueError("recommended_level must fit in uint16")
        struct.pack_into("<H", record, 0x10, recommended_level)
        struct.pack_into("<H", record, 0x12, recommended_level)
    if transfer_count is not None:
        if not 0 <= transfer_count <= 0xFFFFFFFF:
            raise ValueError("transfer_count must fit in uint32")
        struct.pack_into("<I", record, 0xDC, transfer_count)
    if account_id is not None:
        write_account_id(record, account_id)
    if effect_template is not None:
        if len(effect_template) != EFFECT_REGION_SIZE:
            raise ValueError(
                f"effect_template must be exactly {EFFECT_REGION_SIZE:#x} bytes"
            )
        record[EFFECT_START:EFFECT_START + EFFECT_REGION_SIZE] = effect_template

    after_tuple = predict_outbound_tuple(bytes(record))
    edited = bytearray(data)
    edited[record_offset:record_offset + SCROLL_RECORD_SIZE] = record

    checksum = None
    if edited.startswith(b"RNNUSR"):
        old_checksum, new_checksum = patch_user_checksum(edited)
        checksum = {"old": old_checksum, "new": new_checksum}

    changed_offsets = [
        index
        for index, (left, right) in enumerate(
            zip(data[record_offset:record_offset + SCROLL_RECORD_SIZE], record)
        )
        if left != right
    ]
    report = {
        "record_offset": record_offset,
        "record_offset_hex": hex(record_offset),
        "changed_record_byte_count": len(changed_offsets),
        "changed_record_offsets_hex": [hex(value) for value in changed_offsets],
        "before": before_tuple.to_dict(),
        "after": after_tuple.to_dict(),
        "checksum": checksum,
        "effect_template_applied": effect_template is not None,
    }
    return bytes(edited), report


def insert_scroll_record(
    data: bytes,
    *,
    record_offset: int,
    record: bytes,
) -> tuple[bytes, dict]:
    """Insert one complete record into a fully zeroed fixed inventory slot."""
    require_decrypted_user_save(data)
    if len(record) != SCROLL_RECORD_SIZE:
        raise ValueError("record must be exactly 0xE8 bytes")
    if struct.unpack_from("<H", record, 0)[0] == 0:
        raise ValueError("record type must be nonzero")
    end = record_offset + SCROLL_RECORD_SIZE
    if record_offset < 0 or end > len(data):
        raise ValueError("The requested destination slot lies outside the input")
    existing = data[record_offset:end]
    if any(existing):
        raise ValueError("The destination slot is not fully zeroed")

    edited = bytearray(data)
    edited[record_offset:end] = record
    old_checksum, new_checksum = patch_user_checksum(edited)
    report = {
        "record_offset": record_offset,
        "record_offset_hex": hex(record_offset),
        "inserted_record_size": len(record),
        "inserted": describe_record_for_report(record),
        "checksum": {"old": old_checksum, "new": new_checksum},
    }
    return bytes(edited), report


def _load_effect_template(path: Path, offset: int) -> bytes:
    data = path.read_bytes()
    record = require_record(data, offset)
    return record[EFFECT_START:EFFECT_START + EFFECT_REGION_SIZE]


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Decode and patch Nioh 3 v2.00.02 Emaki propagation inputs"
    )
    commands = parser.add_subparsers(dest="command", required=True)

    inspect = commands.add_parser("inspect", help="Predict the normal outbound Emaki tuple")
    inspect.add_argument("file", type=Path)
    inspect.add_argument("--offset", type=parse_int, default=0)
    inspect.add_argument("--output", type=Path)

    decode = commands.add_parser("decode-value", help="Decode the packed 32-bit Emaki value")
    decode.add_argument("value", type=parse_int)

    patch = commands.add_parser(
        "patch-canonical",
        help="Patch fields consumed by the canonical send-time regeneration path",
    )
    patch.add_argument("file", type=Path)
    patch.add_argument("output", type=Path)
    patch.add_argument("--offset", type=parse_int, required=True)
    patch.add_argument("--random-seed", type=parse_int)
    patch.add_argument("--rarity", type=parse_int)
    patch.add_argument("--level", type=parse_int)
    patch.add_argument("--recommended-level", type=parse_int)
    patch.add_argument("--transfer-count", type=parse_int)
    patch.add_argument("--account-id", type=parse_int)
    patch.add_argument("--effect-template-file", type=Path)
    patch.add_argument("--effect-template-offset", type=parse_int, default=0)
    patch.add_argument("--report", type=Path)

    insert = commands.add_parser(
        "insert-record",
        help="Insert a complete generated record into a fully zeroed inventory slot",
    )
    insert.add_argument("file", type=Path)
    insert.add_argument("output", type=Path)
    insert.add_argument("--offset", type=parse_int, required=True)
    insert.add_argument("--record-file", type=Path, required=True)
    insert.add_argument("--report", type=Path)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    if args.command == "inspect":
        record = require_record(args.file.read_bytes(), args.offset)
        rendered = json.dumps(predict_outbound_tuple(record).to_dict(), indent=2)
        if args.output:
            require_new_output(args.output, protected=(args.file,))
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(rendered, encoding="utf-8")
        print(rendered)
        return 0

    if args.command == "decode-value":
        print(json.dumps(unpack_value(args.value), indent=2))
        return 0

    if args.command == "patch-canonical":
        protected = [args.file]
        if args.effect_template_file:
            protected.append(args.effect_template_file)
        require_new_output(args.output, protected=tuple(protected))
        if args.report:
            require_new_output(
                args.report,
                protected=tuple(protected) + (args.output,),
            )
        source_data = args.file.read_bytes()
        require_decrypted_user_save(source_data)
        effect_template = None
        if args.effect_template_file:
            effect_template = _load_effect_template(
                args.effect_template_file, args.effect_template_offset
            )
        edited, report = patch_canonical_inputs(
            source_data,
            record_offset=args.offset,
            random_seed=args.random_seed,
            rarity=args.rarity,
            level=args.level,
            recommended_level=args.recommended_level,
            transfer_count=args.transfer_count,
            account_id=args.account_id,
            effect_template=effect_template,
        )
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_bytes(edited)
        rendered = json.dumps(report, indent=2)
        if args.report:
            args.report.parent.mkdir(parents=True, exist_ok=True)
            args.report.write_text(rendered, encoding="utf-8")
        print(rendered)
        return 0

    if args.command == "insert-record":
        protected = (args.file, args.record_file)
        require_new_output(args.output, protected=protected)
        if args.report:
            require_new_output(
                args.report,
                protected=protected + (args.output,),
            )
        source_data = args.file.read_bytes()
        require_decrypted_user_save(source_data)
        record = args.record_file.read_bytes()
        edited, report = insert_scroll_record(
            source_data,
            record_offset=args.offset,
            record=record,
        )
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_bytes(edited)
        rendered = json.dumps(report, indent=2)
        if args.report:
            args.report.parent.mkdir(parents=True, exist_ok=True)
            args.report.write_text(rendered, encoding="utf-8")
        print(rendered)
        return 0

    raise AssertionError(args.command)


if __name__ == "__main__":
    raise SystemExit(main())
