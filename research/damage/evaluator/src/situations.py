"""Named hit situations for the 10-16 baseline hit: each is a set of state writes confirmed by replay.

Every situation below was confirmed on this baseline by verify_gates.py (the conditional effect field enters the
formula) or reproduces an in-game observation (after_deflect: 793 / 1120). A scenario is a "+"-joined list of
situation names, e.g. "back+ki_exhausted+status_poison". At most one target status per scenario.

A situation switches the conditional effects of this hit only: the motion value, attack and target stay those of
the captured light attack. "strong" therefore means "this hit with the strong-attack effect gate", not a strong
attack's own motion. An element hit is not offered: on a weapon without that element it is not a real state.

Situations that need memory this capture does not hold (martial art, water/thunder hits, airborne, imbue) were
confirmed on their own 10-22 captures; they are not available on this baseline.
"""
import struct
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

# name -> (object, offset, bytes, meaning). Objects: MAIN hit context, OWNER (source HP owner), TARGET_ENTITY.
FLAGS = {
    "back": ("MAIN", 0x282, b"\x01", "back attack (背后攻击伤害)"),
    "strong": ("MAIN", 0x13C, struct.pack("<I", 2), "strong attack (强攻击伤害 replaces 速攻击伤害)"),
    "high_stance": ("MAIN", 0x148, struct.pack("<I", 0), "high stance (上段架式攻击伤害)"),
    "low_stance": ("MAIN", 0x148, struct.pack("<I", 2), "low stance (下段架式攻击伤害)"),
    "desperate": ("MAIN", 0x13C, struct.pack("<I", 0x8000000), "desperate action bit (近距离攻击伤害（绝境）)"),
    "pursuit": ("MAIN", 0x13C, struct.pack("<I", 0x10), "pursuit / close thrust (追击和近身刺击伤害)"),
    "tsukumo": ("OWNER", 0x188, b"\x02", "九十九化身 active"),
    "ki_exhausted": ("TARGET_ENTITY", 0xAD, b"\x06", "target ki exhausted (灭气追击之法)"),
    "after_deflect": ("OWNER", 0x2E3C, struct.pack("<f", 1.1), "status aggregate 1.1 after a counter deflect"),
}
# Target statuses (key in the target's status map), see verify_gates.status_override.
STATUSES = {"status_poison": 0x3C, "status_paralysis": 0x168, "status_burn": 0x39, "status_water": 0x22F,
            "status_shock": 0x226, "status_wind": 0xF9}
# Side effects of a status on the target's own stats, as observed in game. The water ailment sets the target's
# damage-taken factor (target inline STAT +0x854, read by the formula) to 1.2: 10-22 orb-applied and 10-23
# 水弹-applied water ailments both show 1.2, and with it the 10-23 blind hit replays exactly (1149).
STATUS_TARGET_STAT = {"status_water": {0x854: struct.pack("<f", 1.2)}}
NAMES = sorted(FLAGS) + sorted(STATUSES) + ["plain"]


def overrides(ev, scenario):
    """State writes {address: bytes} for a scenario on evaluator ev (10-16 baseline capture)."""
    from verify_gates import status_override
    m = ev.model
    objects = {"MAIN": m.main_addr, "OWNER": m.R["source.HP_owner"][0],
               "TARGET_ENTITY": m.R["target.query.entity.prefix"][0]}
    out, flags13c, statuses = {}, 0, []
    for name in [s for s in scenario.split("+") if s and s != "plain"]:
        if name in STATUSES:
            statuses.append(STATUSES[name])
            for off, data in STATUS_TARGET_STAT.get(name, {}).items():
                out[m.R["target.inline_STAT"][0] + off] = data
            continue
        if name not in FLAGS:
            raise KeyError(f"unknown situation {name}; known: {', '.join(NAMES)}")
        obj, off, data, _ = FLAGS[name]
        if obj == "MAIN" and off == 0x13C:  # action flag bits combine with each other, not with the captured quick bit
            flags13c |= struct.unpack("<I", data)[0]
            continue
        out[objects[obj] + off] = data
    if flags13c:
        out[m.main_addr + 0x13C] = struct.pack("<I", flags13c)
    if len(statuses) > 1:
        raise ValueError("at most one target status per scenario")
    if statuses:
        out.update(status_override(m, ev, None, statuses[0], m.R["target.status.tree.head"][0]))
    return out
