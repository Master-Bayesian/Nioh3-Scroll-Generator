"""Bounded data interpreters for the v2.01 upstream observation, not a seed oracle.

Offsets are justified in RESEARCH_FINDINGS.md. Opaque tail bytes are retained
for byte-copy checks but are NOT treated as mode/count inputs.
"""
from __future__ import annotations
from dataclasses import dataclass
import math
import struct

EXE_SHA256 = "4047ECB623F3AE033F7D6F3637CD1C5408C72A89A038463268A7C9AC5C394159"
CALLER_ROUTES = {
    0xF1E4F1: "owned_scroll_branch",
    0x21D9E24: "session_view_branch",
    0x21DC438: "parameterized_session_branch",
    0x2233DE8: "current_session_requeue",
}
SITES = {"request_enqueue":0x10D9180, "request_queued":0x10D9368,
         "mission_consume":0x20E198C, "context_resolved":0x1029FEE}

@dataclass(frozen=True)
class Request:
    raw: bytes

    def __post_init__(self):
        if len(self.raw) != 12:
            raise ValueError("request must be exactly twelve bytes")

    @classmethod
    def from_hex(cls, text: str) -> "Request":
        return cls(bytes.fromhex(text))

    @property
    def seed(self) -> int:
        return struct.unpack_from("<I", self.raw)[0]

    def generator_projection(self) -> tuple[int, int, int, int, int]:
        """20E198C -> 20E1864 arguments; no +A/+B reads on this route."""
        return self.raw[7], self.raw[6], self.seed, self.raw[8], self.raw[9]

    @property
    def padding_observed(self) -> bytes:
        return self.raw[10:12]

    @property
    def extra_generation(self) -> bool:
        return self.raw[9] != 0


def configured_extras(request: Request, row: bytes, waves: int) -> tuple[int, ...]:
    """Decode an upper-level count INPUT, not the resulting descriptor count.

    The helper can leave counts unfulfilled/zero if its candidate pool has no
    valid expandable row. This function must not be presented as forward parity.
    """
    if len(row) != 0x30 or not 0 <= waves <= 5:
        raise ValueError("special-context row/wave bounds")
    return tuple(row[0x2A+i] if request.extra_generation else 0 for i in range(waves))


@dataclass(frozen=True)
class PlacementRow:
    raw: bytes

    def __post_init__(self):
        if len(self.raw) != 0x18:
            raise ValueError("placement row stride must be 0x18")

    @property
    def key(self) -> tuple[int, int]:
        return self.raw[0x12], self.raw[0x13]

    @property
    def xyz_orientation(self) -> tuple[float, float, float, float]:
        return struct.unpack_from("<4f", self.raw)

    def checked_position(self) -> tuple[float, float, float, float]:
        result = self.xyz_orientation
        if not all(math.isfinite(v) for v in result):
            raise ValueError("non-finite authored position")
        return result


def exact_placement(rows: list[PlacementRow], terrain: int, slot: int) -> PlacementRow:
    """Do not silently use native slot-1 fallback for an identity proof.

    Native 1BB152C has a fallback. Identity joins must prove an exact unique
    key; a missing/ambiguous key cannot establish the owner's physical location.
    """
    matches = [r for r in rows if r.key == (terrain, slot)]
    if len(matches) != 1:
        raise ValueError("physical identity requires one exact (terrain, slot) row")
    matches[0].checked_position()
    return matches[0]


def descriptor_identity(descriptor: bytes, terrain: int, wave_index: int) -> tuple[int, int, int]:
    if len(descriptor) != 0x14 or not 0 <= terrain <= 255 or wave_index < 0:
        raise ValueError("invalid descriptor/terrain/wave")
    # Ordinal spawn ID is deliberately excluded. Same point can be reused in
    # different waves, so a task occurrence join additionally includes the wave.
    return wave_index, terrain, descriptor[0xE]


def classify_request_source(return_rva: int, raw: bytes) -> str:
    req = Request(raw)
    route = CALLER_ROUTES.get(return_rva)
    if route is None:
        raise ValueError("unrecovered producer; preserve capture, do not infer mode")
    if route in ("session_view_branch", "parameterized_session_branch") and req.raw[9] != 1:
        raise ValueError("request contradicts the recovered literal-one writer")
    if route == "owned_scroll_branch" and req.raw[9] != 0:
        raise ValueError("request contradicts the recovered zero writer")
    return route
