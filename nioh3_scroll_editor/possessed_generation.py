"""Seed-derived descriptor +0x0F (Possessed), NEVER the later Curse flag.

The location phase consumes one parent LCG draw per non-singleton role pool.
Coordinates/local MT permutation are not needed to derive the parent state.
Missing *captured* rows are unknown, distinct from a proven native null lookup.
"""
from __future__ import annotations
from dataclasses import dataclass
from functools import lru_cache
import json
from pathlib import Path
import struct
from .enemy_state_rng import LcgStream, lottery_10000, threshold_from_config
from .enemy_variant_generation import EnemyVariantResult

TEXT_SHA256 = 'f8799b5db54a9ca46f52bcd6c037b2ad9b413dc83a26f1d3f0e61251bfb48023'


class MissingEnemyStateData(ValueError):
    pass


@dataclass(frozen=True)
class EnemyStateTables:
    positions_by_terrain: dict[int, tuple[bytes, ...]]
    # Values retain native-null versus missing-capture distinctions.
    eligibility_by_lookup: dict[int, dict]
    config_4543: bytes | None
    text_sha256: str
    source_note: str = ''
    enemy_index_complete: bool = False

    @classmethod
    @lru_cache(maxsize=8)
    def load(cls, path: str | Path | None = None):
        path = Path(path) if path else Path(__file__).parent/'data/enemy_states/pc_v2_01/native_tables.json'
        d = json.loads(path.read_text(encoding='utf-8'))
        if d.get('text_sha256', '').lower() != TEXT_SHA256:
            raise MissingEnemyStateData('unsupported executable/table identity')
        if not d.get('config_4543_lookup_observed', False):
            raise MissingEnemyStateData('config 4543 lookup not observed')
        pos = {}
        for k, t in d['positions_by_terrain'].items():
            if t.get('complete_terrain_scan') is not True:
                continue
            rows = tuple(bytes.fromhex(s) for s in t['rows_hex'])
            if any(len(r) != 0x18 or r[0x12] != int(k, 0) for r in rows):
                raise MissingEnemyStateData('invalid terrain position table slice')
            pos[int(k, 0)] = rows
        cfg = d['config_4543_hex']
        if cfg is not None and len(bytes.fromhex(cfg)) != 0x20:
            raise MissingEnemyStateData('invalid config row')
        return cls(pos, {int(k,0): v for k,v in d['eligibility_by_lookup'].items()},
                   bytes.fromhex(cfg) if cfg is not None else None,
                   d['text_sha256'].lower(), d.get('source_note',''), d.get('enemy_index_complete') is True)

    def eligible(self, lookup: int) -> bool:
        e = self.eligibility_by_lookup.get(lookup)
        if e is None and self.enemy_index_complete:
            return True  # Full index proves the native lookup returns null.
        if e is None:
            raise MissingEnemyStateData(f'enemy/subtype lookup not captured: 0x{lookup:X}')
        if e.get('enemy_row_present') is False:
            return True
        if e.get('enemy_row_present') is not True:
            raise MissingEnemyStateData(f'enemy lookup status unknown: 0x{lookup:X}')
        if e.get('subtype_row_present') is False:
            return False
        if e.get('subtype_row_present') is not True or 'flags14' not in e:
            raise MissingEnemyStateData(f'subtype gate unknown: 0x{lookup:X}')
        return not (int(e['flags14']) & 1)


@dataclass(frozen=True)
class PossessedResult:
    status: str  # exact / unknown
    by_occurrence: dict[tuple[int,int], str]  # yes / no / unknown
    source_entry_state: int | None
    source_entry_draw: int | None
    final_state: int | None
    trace: tuple[dict, ...]
    missing: tuple[str, ...] = ()
    evidence_grade: str = 'static_replay'


def position_parent_stream(roster: EnemyVariantResult, tables: EnemyStateTables) -> LcgStream:
    if tables.text_sha256.lower() != TEXT_SHA256:
        raise MissingEnemyStateData('unsupported executable/table identity')
    rows = tables.positions_by_terrain.get(roster.terrain)
    if rows is None:
        raise MissingEnemyStateData(f'complete manager+A90 slice missing: terrain 0x{roster.terrain:X}')
    stream = LcgStream(roster.state_after_roster, 'parent', list(roster.trace), roster.parent_draws)
    # 0x102BFA0..0x102C3BC builds all six pools for EVERY wave. Role pools
    # unused by the resulting roster still consume a shuffle seed if size>1.
    for wave in range(len(roster.waves)):
        for role in range(6):
            pool = [r for r in rows if r[0x13] >= 1 and
                    (wave != 0 or not r[0x14] & 1) and
                    (struct.unpack_from('<H',r,0x10)[0] >> role) & 1]
            if len(pool) > 1:
                stream.u16('position-pool-local-MT-seed', wave=wave, role=role, size=len(pool))
            else:
                stream.events.append(dict(stream='parent', reason='position-pool-no-parent-draw',
                                          wave=wave, role=role, size=len(pool)))
    return stream


def generate_possessed(roster: EnemyVariantResult, *, tables: EnemyStateTables | None = None) -> PossessedResult:
    """Input is generated roster data, never a caller-supplied captured RNG state.

    Public end-to-end entry is generate_enemy_state_preview(seed, ...), which
    obtains this roster itself. This internal stage preserves partial coverage.
    """
    tables = tables or EnemyStateTables.load()
    keys = {(x.wave_index,x.position): 'unknown' for x in roster.occurrences}
    try:
        stream = position_parent_stream(roster, tables)
        entry, entry_draw = stream.state, stream.draws
        # Preflight all gates. No partial failure can accidentally be represented
        # as a proven 'no'; descriptor order and class identity stay unchanged.
        eligible = {x.lookup_key: tables.eligible(x.lookup_key) for x in roster.occurrences}
        threshold = threshold_from_config(tables.config_4543)
    except MissingEnemyStateData as e:
        return PossessedResult('unknown',keys,None,None,None,(),(str(e),),'unknown')
    answer = dict.fromkeys(keys, 'no')
    for selector in (0,1) if roster.variant == 'expedition' else (0,):
        for x in roster.occurrences:
            if x.selector_class != selector or not eligible[x.lookup_key]:
                stream.events.append(dict(stream='parent',reason='source-skip',selector=selector,
                                          spawn=x.native_spawn_key))
                continue
            h = stream.u16('source-first-success', selector=selector, spawn=x.native_spawn_key,
                           wave=x.wave_index, position=x.position)
            ticket = lottery_10000(h)
            stream.events[-1].update(ticket=ticket, threshold=threshold, accepted=ticket<=threshold)
            if ticket <= threshold:
                answer[(x.wave_index,x.position)] = 'yes'
                return PossessedResult('exact',answer,entry,entry_draw,stream.state,tuple(stream.events))
    return PossessedResult('exact',answer,entry,entry_draw,stream.state,tuple(stream.events))
