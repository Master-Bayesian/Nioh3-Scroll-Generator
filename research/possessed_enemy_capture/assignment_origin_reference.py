"""Executable pseudocode for *prepared* PC v2.01 descriptors, not a seed oracle.

The caller must supply the state at 0x10283C0 and its actual table lookups.
No game access, shipping-product integration, inference of missing rows, or
claim of live parity. See the accompanying report for every input boundary.
"""
from __future__ import annotations
from dataclasses import dataclass
import math
import struct
from typing import Iterable

A = 0x10DCD
MASK32 = 0xFFFFFFFF


def f32(x: float | int) -> float:
    try:
        return struct.unpack('<f', struct.pack('<f', x))[0]
    except OverflowError:
        return math.copysign(math.inf, x)


def i32(x: int) -> int:
    return (x + 0x80000000) % (1 << 32) - 0x80000000


def cvttss_i32(x: float) -> int:
    """SSE CVTTSS2SI: indefinite INT_MIN for invalid/overflow, not Python int."""
    if not math.isfinite(x):
        return -0x80000000
    n = math.trunc(x)
    return n if -0x80000000 <= n <= 0x7FFFFFFF else -0x80000000


def threshold_4543(row: bytes | None) -> int:
    # 0x10284B9..0x10284F1. A missing config is an actual native branch.
    if row is None:
        return 0
    if len(row) < 0x1C:
        raise ValueError('truncated 0x4543 configuration row')
    integer = struct.unpack_from('<i', row, 0x10)[0]
    multiplier = struct.unpack_from('<f', row, 0x18)[0]
    if multiplier == 1.0:
        return integer
    return cvttss_i32(f32(f32(integer) * multiplier))


def draw_10000(state: int) -> tuple[int, int]:
    state = (state * A + 1) & MASK32
    uniform = f32(f32(state >> 16) * f32(1.0 / 65536.0))
    ticket = cvttss_i32(f32(uniform * f32(10000.0)))
    return state, min(ticket, 9999)


@dataclass(frozen=True)
class PreparedDescriptor:
    raw: bytes  # exactly 0x14 bytes as produced before assignment
    enemy_row_present: bool
    subtype_row_present: bool
    subtype_flags14: int
    threshold: int

    def __post_init__(self) -> None:
        if len(self.raw) != 0x14:
            raise ValueError('descriptor must contain exactly 0x14 bytes')
        if not 0 <= self.subtype_flags14 <= 0xFF:
            raise ValueError('invalid subtype flag byte')
        if not -0x80000000 <= self.threshold <= 0x7FFFFFFF:
            raise ValueError('threshold is a signed int32')

    @property
    def spawn(self) -> int:
        return struct.unpack_from('<I', self.raw)[0]

    @property
    def lookup(self) -> int:
        return struct.unpack_from('<I', self.raw, 4)[0]

    @property
    def selector_class(self) -> int:
        return self.raw[0x10]

    def reason_skipped(self, selector: int) -> str | None:
        if self.selector_class != selector:
            return 'selector_class'
        if self.enemy_row_present and not self.subtype_row_present:
            return 'subtype_missing'
        if self.enemy_row_present and self.subtype_flags14 & 1:
            return 'subtype_flag14_bit0'
        # A missing enemy row falls through; it does not imply missing subtype skip.
        return None


def assign_prepared(
    waves: Iterable[Iterable[PreparedDescriptor]],
    state_at_first_pass: int,
    allow_class1: bool,
) -> dict:
    """Translate 0x10283C0 + 0x102C889..0x102C8AF.

    Ordered Bernoulli trials, stopping on the first <= threshold success.
    No MT, rejection loop, normalized weights, or removal in this helper.
    Does not clear pre-existing descriptor flags (neither does the helper).
    """
    waves = [list(wave) for wave in waves]
    outputs = [[bytearray(d.raw) for d in wave] for wave in waves]
    state = state_at_first_pass & MASK32
    events: list[dict] = []
    for selector in ([0, 1] if allow_class1 else [0]):
        for wave_index, wave in enumerate(waves):
            for position, d in enumerate(wave):
                event = dict(selector=selector, wave_index=wave_index, position=position,
                             spawn=d.spawn, lookup=d.lookup)
                skip = d.reason_skipped(selector)
                if skip:
                    events.append({**event, 'kind': 'skip', 'reason': skip})
                    continue
                before = state
                state, ticket = draw_10000(state)
                accepted = ticket <= d.threshold  # signed <=, not <
                events.append({**event, 'kind': 'trial', 'state_before': before,
                               'state_after': state, 'ticket': ticket,
                               'threshold': d.threshold, 'accepted': accepted})
                if accepted:
                    outputs[wave_index][position][0x0F] = 1
                    return dict(selected_spawn=d.spawn, state=state, events=events,
                                output_descriptors=[[bytes(x).hex() for x in w] for w in outputs])
    return dict(selected_spawn=None, state=state, events=events,
                output_descriptors=[[bytes(x).hex() for x in w] for w in outputs])


def e9_null_source_direct(record: bytes, selector: int, mission_key: int,
                          progression: int, enemy_weight: int | None) -> bool:
    """Only the captured null+0x18 direct branch, not a full E9 oracle.

    None means a lookup actually returned null, NOT that the input is unknown.
    Ordinary weighted E9 assignments remain possible when this returns False.
    """
    if len(record) < 0xF0:
        raise ValueError('truncated task record')
    if any(record[0x18:0x20]):
        raise ValueError('non-null source+0x18 requires external eligibility helpers')
    return (struct.unpack_from('<I', record, 0x24)[0] == mission_key
            and struct.unpack_from('<I', record, 0x80)[0] != 0
            and record[0x90] == selector and record[0x8F] != 0
            and progression >= 3 and (enemy_weight is None or enemy_weight != 0))


def e9_count_prepared(n: int, probability: float, already: int, extra_3b37: bool,
                      bounded_uniform) -> int:
    """0xE3A900 on its ordinary finite p in (0,1] domain.

    bounded_uniform(k) must implement the native inclusive integer draw [0,k].
    Exotic reciprocal overflow, negative probability and giant counts are
    deliberately outside this explanatory model, not silently approximated.
    """
    p = f32(probability)
    if not 0 <= n <= 0x7FFFFFFF or not 0 <= already <= 8:
        raise ValueError('outside ordinary count domain')
    if not math.isfinite(p) or not 0.0 <= p <= 1.0:
        raise ValueError('outside ordinary probability domain')
    count = cvttss_i32(f32(f32(n) * p))
    if p > 0:
        q = math.trunc(f32(1.0 / p))
        if not 1 <= q <= 0x7FFFFFFF:
            raise ValueError('reciprocal out of modeled range')
        remainder = (n - count * q) & 0xFFFFFFFFFFFFFFFF
        u = bounded_uniform(q)  # INCLUSIVE upper bound q, not q-1
        if not 0 <= u <= q:
            raise ValueError('invalid bounded draw')
        if remainder > u:
            count += 1
    return min(count + int(extra_3b37), 8 - already)
