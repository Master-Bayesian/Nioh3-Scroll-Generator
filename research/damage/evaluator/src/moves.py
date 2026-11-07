"""A move as evaluator input: which state words each of its hits sets on the captured hit.

A hit is one damaging row of a move. Its damage comes from the same formula as the captured hit, with these words replaced:
modifier row +0x10 physical motion, +0x12 elemental motion; MAIN+0x13C attack class, MAIN+0x144 weapon type, MAIN+0x148 stance.
"""
import json
import struct
from dataclasses import dataclass
from pathlib import Path

DATA = Path(__file__).resolve().parents[2] / "data/MOVES.json"
MODIFIER_ROLE = "source.query.entity.action.modifier0"
CLASS_MASK = 0xFF7F  # the 0x80 bit of MAIN+0x13C does not come from the move


@dataclass(frozen=True)
class Hit:
    physical: int
    elemental: int
    attack_class: int
    weapon_type: int
    stance: int


class MoveTable:
    def __init__(self, path=DATA):
        self.weapons = {w["type"]: w for w in json.loads(Path(path).read_text(encoding="utf-8"))}

    def hits(self, weapon_type, move_id, stance):
        """Hits of one move in landing order; a row that fires several times counts once per event."""
        moves = {m["id"]: m for m in self.weapons[weapon_type]["moves"]}
        move = moves[move_id]
        flags = move["attack_class"] | move["flag6"] << 8
        events = sorted((t if t is not None else 0, i) for i, row in enumerate(move["hits"]) for t in row["times"])
        return [Hit(move["hits"][i]["physical"], move["hits"][i]["elemental"], flags, weapon_type, stance) for _, i in events]


def hit_writes(model, hit):
    """State words of a hit, {address: bytes}, for overrides of a replay."""
    main = model.main_addr
    row = model.R[MODIFIER_ROLE][0]
    flags = struct.unpack_from("<I", model.main, 0x13C)[0] & ~CLASS_MASK | hit.attack_class
    return {row + 0x10: struct.pack("<hh", hit.physical, hit.elemental), main + 0x13C: struct.pack("<I", flags),
            main + 0x144: struct.pack("<I", hit.weapon_type), main + 0x148: struct.pack("<I", hit.stance)}
