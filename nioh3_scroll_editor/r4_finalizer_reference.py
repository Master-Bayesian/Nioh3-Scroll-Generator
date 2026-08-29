"""Table-driven reference core for Nioh 3 PC v2.00.02 R4 completion.

This module implements the portions recovered exactly from machine code:

* the 0x18-byte effect-slot layout;
* completion-loop eligibility/acceptance;
* the deterministic finalizer seed derivation;
* the 32-bit LCG and all observed integer draws;
* completion-candidate weight calculation (RVA 0x57896C);
* inclusive weighted selection used by category/effect pools;
* the rarity-roll percentile lottery;
* the base resolved-value formula used by RVA 0x571478.

It deliberately does not pretend to be a complete offline finalizer. A complete
engine additionally needs the raw versioned parameter tables and the category,
curve, context-classifier, and normalization providers listed in REPORT.md.
Missing data must be treated fail-closed.
"""

from __future__ import annotations

from dataclasses import dataclass
import struct
from typing import Callable, Iterable, Sequence

RECORD_SIZE = 0xE8
EFFECT_BASE = 0x34
EFFECT_COUNT = 7
EFFECT_STRIDE = 0x18
LCG_MULTIPLIER = 0x00010DCD
LCG_INCREMENT = 1
LCG_MASK = 0xFFFFFFFF
LCG_MULTIPLIER_INVERSE = 0xA5E2A705


def u16(data: bytes | bytearray | memoryview, offset: int) -> int:
    return struct.unpack_from("<H", data, offset)[0]


def u32(data: bytes | bytearray | memoryview, offset: int) -> int:
    return struct.unpack_from("<I", data, offset)[0]


def i16(data: bytes | bytearray | memoryview, offset: int) -> int:
    return struct.unpack_from("<h", data, offset)[0]


def i32(data: bytes | bytearray | memoryview, offset: int) -> int:
    return struct.unpack_from("<i", data, offset)[0]


def f32_from(data: bytes | bytearray | memoryview, offset: int) -> float:
    return struct.unpack_from("<f", data, offset)[0]


def f32(value: float | int) -> float:
    """Round to IEEE-754 binary32 exactly as an SSE scalar operation would."""
    return struct.unpack("<f", struct.pack("<f", float(value)))[0]


def f32_mul(left: float | int, right: float | int) -> float:
    return f32(f32(left) * f32(right))


def f32_add(left: float | int, right: float | int) -> float:
    return f32(f32(left) + f32(right))


def f32_sub(left: float | int, right: float | int) -> float:
    return f32(f32(left) - f32(right))


def f32_div(left: float | int, right: float | int) -> float:
    return f32(f32(left) / f32(right))


@dataclass(frozen=True, slots=True)
class EffectSlot:
    prefix_word: int
    raw_id: int
    value: int
    roll_percent: int
    category_and_flags: int
    effect_flags: int
    byte_0f: int
    tail_0: int
    tail_1: int

    @classmethod
    def parse(cls, record: bytes | bytearray | memoryview, index: int) -> "EffectSlot":
        if not 0 <= index < EFFECT_COUNT:
            raise IndexError(index)
        off = EFFECT_BASE + index * EFFECT_STRIDE
        return cls(
            prefix_word=u32(record, off),
            raw_id=u32(record, off + 0x04),
            value=i32(record, off + 0x08),
            roll_percent=record[off + 0x0C],
            category_and_flags=record[off + 0x0D],
            effect_flags=record[off + 0x0E],
            byte_0f=record[off + 0x0F],
            tail_0=u32(record, off + 0x10),
            tail_1=u32(record, off + 0x14),
        )

    @property
    def prefix_id(self) -> int:
        return self.prefix_word & 0xFFFF

    @property
    def category(self) -> int:
        return self.category_and_flags & 0x3F

    @property
    def is_empty(self) -> bool:
        return self.raw_id == 0xFFFFFFFF

    @property
    def completion_loop_eligible(self) -> bool:
        # RVA 0x10280E0..0x10280F4.
        return (
            self.prefix_id != 0
            and not (self.category_and_flags & 0x40)
            and not (self.effect_flags & 0x04)
        )

    @property
    def wrapper_prior_effect_eligible(self) -> bool:
        # RVA 0x2279A15..0x2279A2A.
        return (
            self.prefix_id != 0
            and not (self.category_and_flags & 0x40)
            and not (self.effect_flags & 0x02)
            and self.value != 1
        )

    @property
    def completion_candidate_is_accepted(self) -> bool:
        # RVA 0x1028106..0x102810C.
        return bool(self.effect_flags & 0x04)


@dataclass(slots=True)
class Lcg32:
    state: int

    def __post_init__(self) -> None:
        self.state &= LCG_MASK

    def next_u16(self) -> int:
        self.state = (self.state * LCG_MULTIPLIER + LCG_INCREMENT) & LCG_MASK
        return self.state >> 16

    def next_float01(self) -> float:
        return f32_mul(self.next_u16(), f32(1.0 / 65536.0))

    def random_int(self, count: int) -> int:
        """RVA 0x56C6A8: integer in [0,count-1], with the game's clamp."""
        if not 1 <= count <= 0xFFFFFFFF:
            raise ValueError("count must be in 1..2^32-1")
        product = f32_mul(self.next_float01(), f32(count))
        result = int(product)  # nonnegative truncation == cvttss2si
        maximum = (count - 1) & LCG_MASK
        return min(result, maximum)

    def random_inclusive(self, low: int, high: int) -> int:
        """RVA 0x56C684: unsigned inclusive integer range."""
        low &= LCG_MASK
        high &= LCG_MASK
        if low >= high:
            return low
        return (low + self.random_int(((high - low) + 1) & LCG_MASK)) & LCG_MASK

    def discard(self, count: int) -> None:
        for _ in range(count):
            self.next_u16()


@dataclass(frozen=True, slots=True)
class EffectRow:
    """Fields of one 0xD8-byte effect-table row used by the finalizer."""

    raw: bytes

    def __post_init__(self) -> None:
        if len(self.raw) != 0xD8:
            raise ValueError("effect row must be exactly 0xD8 bytes")

    @property
    def raw_id(self) -> int:
        return u16(self.raw, 0x00)

    @property
    def prefix_id(self) -> int:
        return u16(self.raw, 0x02)

    @property
    def candidate_flags(self) -> int:
        return u32(self.raw, 0x1C)

    @property
    def normalization_flags(self) -> int:
        return u32(self.raw, 0x20)

    @property
    def rarity1_weight_bits(self) -> int:
        """Raw IEEE-754 bits at +0x2C; the localized text ID is on the group row."""
        return u32(self.raw, 0x2C)

    @property
    def progress_threshold(self) -> int:
        return u16(self.raw, 0x54)

    @property
    def optional_multiplier_selector(self) -> int:
        return u16(self.raw, 0x56)

    def rarity_weight(self, rarity: int) -> float:
        offsets = {0: 0x28, 1: 0x2C, 2: 0x30, 3: 0x34, 4: 0x38, 5: 0x3C}
        if rarity == 0xFF:
            # Exact max-like path at 0x578A13..0x578A4F.
            return max(f32_from(self.raw, off) for off in (0x28, 0x2C, 0x30, 0x34))
        try:
            return f32_from(self.raw, offsets[rarity])
        except KeyError:
            return f32(1.0)

    def type_multiplier(self, type_class: int) -> float:
        if type_class == 3:
            return f32_from(self.raw, 0x48)
        if type_class == 4:
            return f32_from(self.raw, 0x4C)
        if type_class == 5:
            return f32_from(self.raw, 0x50)
        return f32_from(self.raw, 0x44)

    def slot_weight(self, weight_slot: int) -> int:
        if not 0 <= weight_slot < 64:
            return 0
        return u16(self.raw, 0x58 + weight_slot * 2)


@dataclass(frozen=True, slots=True)
class GroupRow:
    """Fields of one 0x70-byte effect-group row used by the finalizer."""

    raw: bytes

    def __post_init__(self) -> None:
        if len(self.raw) != 0x70:
            raise ValueError("group row must be exactly 0x70 bytes")

    @property
    def category(self) -> int:
        return u16(self.raw, 0x24)

    @property
    def conflict_mask_a(self) -> int:
        return u32(self.raw, 0x54)

    @property
    def conflict_mask_b(self) -> int:
        return u32(self.raw, 0x58)


@dataclass(frozen=True, slots=True)
class Candidate:
    effect: EffectRow
    group: GroupRow
    weight: int


def type_class_for_record_type(record_type: int, rarity: int, rarity5_floor: int = 0) -> int:
    mapping = {
        0x1E82: 1,
        0x516D: 2,
        0xDD82: 4,
        0xD523: 5,
    }
    result = mapping.get(record_type & 0xFFFF, 3)
    if rarity == 5:
        result = max(result, rarity5_floor & 0xFF)
    return result


def derive_finalizer_rng_seed(record: bytes | bytearray | memoryview, target_index: int) -> int:
    """Exact low-32 seed installed by RVA 0x1109659 for one effect call."""
    if len(record) < RECORD_SIZE:
        raise ValueError("record must contain 0xE8 bytes")
    if not 0 <= target_index < EFFECT_COUNT:
        raise IndexError(target_index)
    display_seed = u32(record, 0x20)
    salt16 = u16(record, 0x0C)
    rarity = struct.unpack_from("<b", record, 0x30)[0]
    total = display_seed
    total += salt16
    total += rarity * salt16 * (display_seed >> 16)
    total += 7 * (target_index << 16)
    for index in range(EFFECT_COUNT):
        off = EFFECT_BASE + index * EFFECT_STRIDE
        raw_id_signed = i32(record, off + 0x04)
        roll = min(record[off + 0x0C], 100)
        total += raw_id_signed * roll
    return total & LCG_MASK


def finalizer_discard_count(record: bytes | bytearray | memoryview, target_index: int) -> int:
    return (target_index + u16(record, 0x0C)) & 0x1F


def make_finalizer_rng(record: bytes | bytearray | memoryview, target_index: int) -> Lcg32:
    rng = Lcg32(derive_finalizer_rng_seed(record, target_index))
    rng.discard(finalizer_discard_count(record, target_index))
    return rng


def progress_bucket(threshold: int) -> int:
    # RVA 0x578C18.
    if threshold < 7000:
        return 0
    if threshold < 8000:
        return 1
    if threshold < 9000:
        return 2
    return 3


def effect_weight(
    row: EffectRow,
    *,
    weight_slot: int,
    type_class: int,
    rarity: int,
    progress: Sequence[int],
    extra_selector: int = 0,
    optional_multiplier_lookup: Callable[[int], float] | None = None,
) -> int:
    """RVA 0x57896C, including binary32 ordering and integer truncation."""
    if len(progress) != 4:
        raise ValueError("progress must contain exactly four integers")
    gate = row.progress_threshold
    enabled = progress[progress_bucket(gate)] >= gate
    accumulator = f32(1.0 if enabled else 0.0)
    base = row.rarity_weight(rarity)
    optional = f32(1.0)

    if type_class >= 5 and row.optional_multiplier_selector != 0:
        key = 0x0415 if extra_selector == row.optional_multiplier_selector else 0xA6D1
        optional = f32(optional_multiplier_lookup(key) if optional_multiplier_lookup else 0.0)

    accumulator = f32_mul(accumulator, row.type_multiplier(type_class))
    accumulator = f32_mul(accumulator, base)
    if type_class >= 5:
        accumulator = f32_mul(accumulator, optional)

    slot_weight = row.slot_weight(weight_slot)
    result = f32_mul(f32_mul(f32(slot_weight), f32(100.0)), accumulator)
    return int(result)


def group_conflicts(candidate: GroupRow, existing: GroupRow) -> bool:
    return bool(
        (candidate.conflict_mask_a & existing.conflict_mask_a)
        or (candidate.conflict_mask_b & existing.conflict_mask_b)
    )


def weighted_select_inclusive(candidates: Sequence[Candidate], rng: Lcg32) -> Candidate | None:
    """The inclusive 0..sum lottery at 0x110A26F..0x110A349.

    The first row has one extra lattice point because comparison is `r <= w`.
    This bias is intentional and required for parity.
    """
    positive = [candidate for candidate in candidates if candidate.weight != 0]
    if not positive:
        return None
    total = sum(candidate.weight for candidate in positive) & LCG_MASK
    upper_count = (total + 1) & LCG_MASK
    if upper_count == 0:
        raise OverflowError("native total+1 wrapped to zero; unsupported pathological table")
    r = rng.random_int(upper_count)
    r = min(r, total)
    for candidate in positive:
        weight = candidate.weight & LCG_MASK
        if r <= weight:
            return candidate
        r = (r - weight) & LCG_MASK
    return None


def roll_percentile(minimum: int, maximum: int, rng: Lcg32) -> int:
    """RVA 0x110A275..0x110A314."""
    minimum &= LCG_MASK
    maximum &= LCG_MASK
    if minimum >= maximum:
        return minimum & 0xFF
    first = rng.random_int(46)
    second = rng.random_int(46)
    lottery = first + second + (10 if first == second else 0)
    span = (maximum - minimum) & LCG_MASK
    scaled = f32_div(f32_mul(f32(lottery), f32(span)), f32(100.0))
    return int(f32_add(f32(minimum), scaled)) & 0xFF


def curve_scale_from_raw_table(curve_table: bytes, level: int, selector: int) -> int:
    """RVA 0x571570 for the +0xC0 table's 10-byte rows.

    Layout: u32 count at +4; rows start +8, stride 10; each row contains three
    u16 values at offsets 0/2/4. The selector is expected to be 0,1,2.
    """
    count = u32(curve_table, 0x04)
    if not 0 <= level < count:
        return 0
    if selector not in (0, 1, 2):
        raise ValueError("selector must be 0, 1, or 2")
    return u16(curve_table, 0x08 + level * 10 + selector * 2)


def resolved_base_value(
    row: EffectRow,
    roll_percent: int,
    level: int,
    curve_lookup: Callable[[int, int], int],
) -> int:
    """RVA 0x571478, excluding optional additions handled by 0x5712D8."""
    level = min(level, 500)
    base0 = i16(row.raw, 0x08)
    low = i16(row.raw, 0x0A)
    high = i16(row.raw, 0x0C)
    curve_selector = u16(row.raw, 0x06)

    if row.normalization_flags & 0x10:
        if roll_percent < 80:
            return base0
        t = f32_div(f32(roll_percent - 80), f32(20.0))
    else:
        t = f32_mul(f32(roll_percent), f32(0.01))

    interpolated = f32_add(f32(low), f32_mul(f32(high - low), t))
    curve = curve_lookup(level, curve_selector)
    return int(f32_add(f32(base0), f32_mul(f32_mul(f32(curve), f32(0.001)), interpolated)))


def completion_loop(
    source: bytes,
    finalize_effect: Callable[[bytes, int, bool], bytes],
    *,
    reveal: bool = True,
) -> tuple[bytes, int | None]:
    """Reference for RVA 0x10280BD's first-accepted-effect behavior."""
    if len(source) != RECORD_SIZE:
        raise ValueError("source record must be 0xE8 bytes")
    for index in range(EFFECT_COUNT):
        if not EffectSlot.parse(source, index).completion_loop_eligible:
            continue
        candidate = finalize_effect(source, index, reveal)
        if len(candidate) != RECORD_SIZE:
            raise ValueError("finalizer callback returned a non-0xE8 record")
        if EffectSlot.parse(candidate, index).completion_candidate_is_accepted:
            return candidate, index
    return bytes(source), None


def inverse_seed_after_one_state(state1: int) -> int:
    return (LCG_MULTIPLIER_INVERSE * ((state1 - 1) & LCG_MASK)) & LCG_MASK


__all__ = [
    "Candidate",
    "EFFECT_BASE",
    "EFFECT_COUNT",
    "EFFECT_STRIDE",
    "EffectRow",
    "EffectSlot",
    "GroupRow",
    "LCG_MULTIPLIER",
    "LCG_MULTIPLIER_INVERSE",
    "Lcg32",
    "RECORD_SIZE",
    "completion_loop",
    "derive_finalizer_rng_seed",
    "effect_weight",
    "finalizer_discard_count",
    "group_conflicts",
    "make_finalizer_rng",
    "progress_bucket",
    "resolved_base_value",
    "roll_percentile",
    "type_class_for_record_type",
    "weighted_select_inclusive",
]
