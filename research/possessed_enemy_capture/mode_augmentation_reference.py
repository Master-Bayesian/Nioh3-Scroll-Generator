"""Pure analysis of *supplied* v2.01 records and visible materializer arithmetic.

This is not a generator, native mode decoder, actor join, or production predicate.
All modeled writes are attributed to specific supplied instructions.
"""
from __future__ import annotations
from dataclasses import dataclass
import struct
from typing import Iterable

U32 = 0xFFFFFFFF
LOW28 = 0x0FFFFFFF


@dataclass(frozen=True)
class Descriptor:
    raw: bytes

    def __post_init__(self):
        if len(self.raw) != 0x14:
            raise ValueError('descriptor must be exactly 0x14 bytes')

    @property
    def spawn(self) -> int: return struct.unpack_from('<I', self.raw)[0]
    @property
    def lookup(self) -> int: return struct.unpack_from('<I', self.raw, 4)[0]
    @property
    def selector(self) -> int: return self.raw[0x10]
    @property
    def source_flag(self) -> int: return self.raw[0x0F]
    @property
    def point(self) -> int: return self.raw[0x0E]

    @classmethod
    def from_hex(cls, value: str) -> 'Descriptor':
        return cls(bytes.fromhex(value))


@dataclass(frozen=True)
class TaggedKey:
    ordinal: int
    key: int
    kind14c: int


def tagged_auxiliary_keys(spawn: int, count382: int, count384: int) -> Iterable[TaggedKey]:
    """4BC8CB/4BC967: shared ordinal; SHL r32,28; OR source+20.

    These are attempted lookup/insertion keys, not necessarily new objects:
    existing keys are skipped, allocation can fail, and ordinals >=16 wrap.
    No assumption about enemy row counts or physical enemy semantics is made.
    """
    if type(spawn) is not int or not 0 <= spawn <= U32:
        raise ValueError('spawn must be u32')
    for count in (count382, count384):
        if type(count) is not int or not 0 <= count <= 0xFFFF:
            raise ValueError('counts must be u16')
    ordinal = 1
    for kind, count in ((1, count382), (2, count384)):
        for _ in range(count):
            yield TaggedKey(ordinal, (((ordinal << 28) & U32) | spawn), kind)
            ordinal += 1


def child_key_compatible(source_spawns: Iterable[int], key: int) -> bool:
    """Necessary, not sufficient: factory-generated tag cannot alter low 28 bits."""
    return any((x & LOW28) == (key & LOW28) for x in source_spawns)


def reused_descriptor_comparison(source: Descriptor, task_identity_hex: str,
                                 embedded_hex: str) -> dict:
    """Visible non-null 13684C branch bypasses 1BF2CF8 and 4BC874 entirely.

    It does NOT authorize replacing an existing record. Later helper side effects
    are outside this model. Compare all 20 embedded bytes; no count-based join.
    """
    ident = bytes.fromhex(task_identity_hex)
    if len(ident) != 12:
        raise ValueError('task identity must be 12 bytes (+20..+2B)')
    target = Descriptor.from_hex(embedded_hex)
    spawn, mission, lookup = struct.unpack('<III', ident)
    return {'lookup_key_matches_source': spawn == source.spawn,
            'mission': mission, 'lookup_matches_source': lookup == source.lookup,
            'embedded_descriptor_matches_source': target == source,
            'visible_branch_refreshes_descriptor': False,
            'actor_join': False}


def unwrap_capture(document: dict) -> dict:
    value = document.get('bridge_result', document)
    for _ in range(4):
        if isinstance(value, dict) and 'events' in value:
            return value
        if isinstance(value, dict) and isinstance(value.get('result'), dict):
            value = value['result']
        else:
            break
    raise ValueError('capture has no event payload')


def compare_native_controls(run_c: dict, run_d: dict) -> dict:
    """Recompute facts from raw bytes; do not splice independent processes."""
    c, d = unwrap_capture(run_c), unwrap_capture(run_d)
    generated = [e for e in c['events'] if e.get('site') == 'mission_generated']
    if len(generated) != 1:
        raise ValueError('this comparison is pinned to the one-return accepted Run C')
    origins = [e for e in d['events'] if e.get('site') == 'origin_entry']
    linked = [e for e in d['events'] if e.get('site') == 'task_linked']
    if not origins or not linked:
        raise ValueError('Run D needs both pre-materialization origin and linked tasks')
    c_rows = [(w['wave_index'], Descriptor.from_hex(x['raw_hex']))
              for w in generated[0]['output']['waves'] for x in w['descriptors']]
    d_rows = [(x['wave_index'], Descriptor.from_hex(x['raw_hex'])) for x in origins[0]['descriptors']]
    d_base = [(w, x) for w, x in d_rows if x.selector == 0]
    link_rows = []
    for e in linked:
        source = Descriptor.from_hex(e['source']['raw_hex'])
        target = Descriptor.from_hex(e['descriptor_hex'])
        ident = bytes.fromhex(e['task_identity_hex'])
        if len(ident) != 12:
            raise ValueError('invalid task identity')
        a, m, b = struct.unpack('<III', ident)
        if source != target or a != source.spawn or b != source.lookup or not e['manager_membership']:
            raise ValueError('Run D source-to-task bytes/identity/membership mismatch')
        link_rows.append({'spawn': a, 'mission': m, 'lookup': b, 'selector': target.selector,
                          'descriptor_hex': target.raw.hex().upper(),
                          'manager_vector_count': e['vector_count'], 'copy_seen': e['copy_seen']})
    c_spawns = [x.spawn for _, x in c_rows]
    incompatible = [x.spawn for _, x in d_rows if not child_key_compatible(c_spawns, x.spawn)]
    return {
        'source': 'provided Run C and independent Run D; no new game execution',
        'cross_process_timeline_join': False,
        'c_process': {'pid': c['pid'], 'creation_filetime': c['identity']['creation_filetime']},
        'd_process': {'pid': d['pid'], 'creation_filetime': d['identity']['creation_filetime']},
        'c_descriptor_count': len(c_rows), 'd_pre_materialization_count': len(d_rows),
        'c_wave_counts': [len(w['descriptors']) for w in generated[0]['output']['waves']],
        'd_wave_counts': [sum(w == i for w, _ in d_rows) for i in range(4)],
        'd_origin_parent_returns': origins[0]['parent_returns'],
        'd_origin_is_earlier_than_first_task_copy': origins[0]['sequence'] < min(
            e['sequence'] for e in d['events'] if e.get('site') == 'task_copy'),
        'base_subsequence_identical_except_ordinal': [(w,x.raw[4:]) for w,x in c_rows] == [(w,x.raw[4:]) for w,x in d_base],
        'd_linked_records': link_rows,
        'd_low28_keys_not_in_c_source_domain': incompatible,
        'tagged_factory_alone_cannot_map_c_source_to_d_keys': bool(incompatible),
        'c_generated_to_persistent_task_join': False,
        'c_physical_actor_count_known': False,
        'c_total_hits': c['total_hits'], 'c_ignored_hits': c['ignored_hits'],
    }
