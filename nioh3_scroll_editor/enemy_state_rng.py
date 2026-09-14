"""Integer and binary32 RNG operations used by the PC v2.01 enemy-state port.

No game process, live-memory library, numpy, or Python random dependency.
The LCG and the MT streams are separate; local/MT draws never advance parent.
"""
from __future__ import annotations
from dataclasses import dataclass, field
import math
import struct

MASK32 = 0xFFFFFFFF
A = 69069
A_INV = 0xA5E2A705


def f32(value: int | float) -> float:
    try:
        return struct.unpack('<f', struct.pack('<f', value))[0]
    except OverflowError:
        return math.copysign(math.inf, value)


def cvtt_i32(value: float) -> int:
    if not math.isfinite(value) or not -2147483648 <= value < 2147483648:
        return -2147483648
    return math.trunc(value)


def lottery_10000(high16: int) -> int:
    if not 0 <= high16 <= 65535:
        raise ValueError('high16 outside uint16')
    return min(cvtt_i32(f32(f32(high16 / 65536.0) * 10000.0)), 9999)


def threshold_from_config(row: bytes | None) -> int:
    if row is None:  # Observed native lookup failure, NOT an absent capture.
        return 0
    if len(row) != 0x20:
        raise ValueError('configuration row must be 0x20 bytes')
    base = struct.unpack_from('<i', row, 0x10)[0]
    mult = struct.unpack_from('<f', row, 0x18)[0]
    return base if mult == 1 else cvtt_i32(f32(f32(base) * mult))


@dataclass
class LcgStream:
    state: int
    name: str = 'parent'
    events: list[dict] = field(default_factory=list)
    draws: int = 0

    def __post_init__(self):
        self.state &= MASK32

    def u16(self, reason: str, **fields) -> int:
        before = self.state
        self.state = (before * A + 1) & MASK32
        self.draws += 1
        h = self.state >> 16
        self.events.append(dict(stream=self.name, draw=self.draws, reason=reason,
                                before=before, after=self.state, high16=h, **fields))
        return h


class MT19937:
    """Standard 32-bit outputs, matching the game's two-bank recurrence.

    This compact state representation does not claim to have the game's 5000
    byte in-memory ABI. Only output sequence and number of draws are exported.
    """
    def __init__(self, seed: int):
        if not 0 <= seed <= MASK32:
            raise ValueError('MT seed outside uint32')
        self.words = [seed]
        for i in range(1, 624):
            x = self.words[-1]
            self.words.append((1812433253 * (x ^ (x >> 30)) + i) & MASK32)
        self.index = 624
        self.draws = 0
        self.rejections = 0

    def u32(self) -> int:
        if self.index == 624:
            # In-place twist deliberately reads the already-updated wraparound
            # half, just as the native second-bank -> first-bank refill does.
            for i in range(624):
                y = (self.words[i] & 0x80000000) | (self.words[(i+1) % 624] & 0x7FFFFFFF)
                self.words[i] = self.words[(i+397) % 624] ^ (y >> 1) ^ (0x9908B0DF if y & 1 else 0)
            self.index = 0
        y = self.words[self.index]
        self.index += 1
        self.draws += 1
        y ^= y >> 11
        y ^= (y << 7) & 0x9D2C5680
        y ^= (y << 15) & 0xEFC60000
        return (y ^ (y >> 18)) & MASK32

    def inclusive(self, upper: int) -> int:
        if not 0 <= upper <= MASK32:
            raise ValueError('unsupported inclusive bound')
        if upper == 0:
            return 0  # The native singleton range consumes no output.
        if upper == MASK32:
            return self.u32()
        size = upper + 1
        limit = (0x100000000 // size) * size
        while True:
            x = self.u32()
            if x < limit:
                return x % size
            self.rejections += 1


def native_shuffle(values: list[int], mt: MT19937) -> None:
    """Forward Fisher-Yates, not Python shuffle's reverse iteration."""
    for i in range(1, len(values)):
        j = mt.inclusive(i)
        if j != i:
            values[i], values[j] = values[j], values[i]


def affine(draw: int) -> tuple[int, int]:
    if type(draw) is not int or draw < 0:
        raise ValueError('draw must be nonnegative integer')
    a, c, ba, bc = 1, 0, A, 1
    while draw:
        if draw & 1:
            a, c = (ba*a) & MASK32, (ba*c+bc) & MASK32
        ba, bc = (ba*ba) & MASK32, (ba*bc+bc) & MASK32
        draw >>= 1
    return a, c


def state_after(seed: int, draw: int) -> int:
    a, c = affine(draw)
    return (a * seed + c) & MASK32
