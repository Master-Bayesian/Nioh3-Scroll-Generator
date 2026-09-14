"""Recovered local v2.01 contracts, NOT a Seed oracle or a live game result.

E39D40: enumerate existing descriptors into a temporary key->u32 index map.
13684C / 4BD5F8: unsigned sorted unique-key task lookup/insertion semantics.
4BDC24: only statically proven initializer bytes, not a fabricated whole task.
"""
from __future__ import annotations
from bisect import bisect_left
from dataclasses import dataclass
import struct
from typing import Mapping, Sequence

U32 = 0xFFFFFFFF


def u32(value: int) -> int:
    if type(value) is not int or not 0 <= value <= U32:
        raise ValueError('expected u32')
    return value


@dataclass(frozen=True)
class Descriptor:
    raw: bytes
    def __post_init__(self):
        if not isinstance(self.raw,bytes) or len(self.raw)!=0x14:
            raise ValueError('descriptor must be 20 immutable bytes')
    @property
    def key(self) -> int: return struct.unpack_from('<I',self.raw,0)[0]
    @property
    def lookup(self) -> int: return struct.unpack_from('<I',self.raw,4)[0]
    @property
    def selector(self) -> int: return self.raw[0x10]


@dataclass(frozen=True)
class IndexEvent:
    wave: int
    position: int
    key: int
    reason: str
    assigned_u32: int | None


@dataclass(frozen=True)
class PrepassResult:
    indices: dict[int,int]
    next_counter: int
    events: tuple[IndexEvent,...]
    descriptor_count_before: int
    descriptor_count_after: int
    rng_draws: int = 0


def prepass_indices(waves: Sequence[Sequence[Descriptor]], selector: int,
                    enemy_rows: Mapping[int,bytes | None], *, counter: int=0,
                    existing: Mapping[int,int] | None=None) -> PrepassResult:
    """E39D40, including three UNW_FLAG_CHAININFO-linked ranges.

    `enemy_rows` represents completed 134430 hash->row lookup. Missing DB row
    SKIPS here; do not import the different missing-row rule from 10283C0.
    Duplicate descriptor keys increment the shared counter, then overwrite the
    temporary mapped index (E2DD90 returns the existing map entry).
    """
    if type(selector) is not int or not 0<=selector<=255:
        raise ValueError('expected u8 selector')
    counter=u32(counter)
    indices={u32(k):u32(v) for k,v in (existing or {}).items()}
    events=[];n=sum(len(w) for w in waves)
    for i,wave in enumerate(waves):
        for j,d in enumerate(wave):
            if not isinstance(d,Descriptor):raise ValueError('expected Descriptor')
            if d.selector!=selector:
                events.append(IndexEvent(i,j,d.key,'class_mismatch',None));continue
            row=enemy_rows.get(d.lookup)
            if row is None:
                events.append(IndexEvent(i,j,d.key,'missing_enemy_row',None));continue
            if not isinstance(row,bytes) or len(row)!=0x398:
                raise ValueError('enemy DB row must have the recovered 0x398 stride')
            if not struct.unpack_from('<I',row,0x74)[0] & 0x4000:
                events.append(IndexEvent(i,j,d.key,'row_74_bit14_clear',None));continue
            value=counter;counter=(counter+1)&U32
            indices[d.key]=value
            events.append(IndexEvent(i,j,d.key,'indexed',value))
    return PrepassResult(indices,counter,tuple(events),n,n)


def two_pass_indices(waves,enemy_rows) -> PrepassResult:
    """Caller 1C245C7/1C245D3: class 0 then 1, one shared counter/map."""
    a=prepass_indices(waves,0,enemy_rows)
    b=prepass_indices(waves,1,enemy_rows,counter=a.next_counter,existing=a.indices)
    return PrepassResult(b.indices,b.next_counter,a.events+b.events,
                         a.descriptor_count_before,b.descriptor_count_after)


def task_byte96(current: int, key: int, indices: Mapping[int,int]) -> int:
    """1C246BD/1C246C1: copy entry+14 LOW BYTE only, if lookup found."""
    if type(current) is not int or not 0<=current<=255:raise ValueError('expected u8')
    key=u32(key)
    return (u32(indices[key]) & 255) if key in indices else current


@dataclass(frozen=True)
class TaskEntry:
    key: int
    pointer: int
    def __post_init__(self):
        u32(self.key)
        if type(self.pointer) is not int or not 0<=self.pointer<1<<64:
            raise ValueError('expected u64 pointer token')


def checked_entries(entries: Sequence[TaskEntry]) -> tuple[TaskEntry,...]:
    entries=tuple(entries)
    if any(not isinstance(x,TaskEntry) for x in entries):raise ValueError('invalid task entry')
    if any(a.key>=b.key for a,b in zip(entries,entries[1:])):
        raise ValueError('task map must be strictly sorted by unsigned u32 key')
    return entries


def parse_task_map_entries(raw: bytes, count: int, capacity: int) -> tuple[TaskEntry,...]:
    """Snapshot decoder: M+0 data pointer, M+8 count, M+10 capacity; stride16.

    The caller supplies exactly the count*16 bytes actually read. This cannot
    establish atomicity of a live snapshot or bind a task to a physical actor.
    """
    if type(count) is not int or type(capacity) is not int or not 0<=count<=capacity:
        raise ValueError('invalid map count/capacity')
    if len(raw)!=count*16:raise ValueError('truncated/extra task map entry bytes')
    return checked_entries([TaskEntry(struct.unpack_from('<I',raw,i*16)[0],
        struct.unpack_from('<Q',raw,i*16+8)[0]) for i in range(count)])


def task_map_lookup(entries: Sequence[TaskEntry], key: int) -> int:
    """13684C: lower_bound + exact equality, not predecessor lookup."""
    items=checked_entries(entries);key=u32(key)
    at=bisect_left([x.key for x in items],key)
    return items[at].pointer if at<len(items) and items[at].key==key else 0


def task_map_insert(entries: Sequence[TaskEntry], incoming: TaskEntry):
    """Logical successful 4BD5F8 path, including duplicate and cold grow branches.

    Allocation failures/exception unwinding are not modeled. Existing key keeps
    its old pointer; insertion never creates a mission descriptor or actor.
    """
    items=list(checked_entries(entries));at=bisect_left([x.key for x in items],incoming.key)
    if at<len(items) and items[at].key==incoming.key:
        return tuple(items),False,items[at]
    items.insert(at,incoming)
    return tuple(items),True,incoming


def tagged_initializer_fields(key: int, mission: int, terrain: int) -> dict[int,bytes]:
    """Proven 4BDC24 direct stores only. Omits helpers/external state/unwritten bytes.

    The caller 4BC874 later sets kind14c to 1 or 2. These are not prefilled
    class-1 enemy occurrence descriptors; lookup28=0 and descriptor selector=0.
    """
    key=u32(key);mission=u32(mission)
    if type(terrain) is not int or not 0<=terrain<=255:raise ValueError('expected terrain u8')
    return {0x20:struct.pack('<I',key),0x24:struct.pack('<I',mission),
            0x28:b'\0'*4,0x80:b'\0'*8,0x88:struct.pack('<Q',6),
            0x90:b'\0',0x94:bytes([terrain]),0x95:b'\0\xff',
            0xE8:struct.pack('<I',1),0x14C:b'\0'*4}


def pre_iteration_contract(span: str | None) -> dict:
    """Interpret an already validated frontier observation without erasing a counterexample.

    Proven native copy/index helpers do not add/reorder source descriptors under
    their valid input/aliasing contract. A real change falsifies that local model
    or its observation assumptions; it does not certify E39D40 as its writer.
    Capture validity, this model check, and game-wide finality are separate.
    """
    statuses = {
        None: 'not_observed',
        'no_descriptor_change_in_pre_iteration_span': 'consistent',
        'descriptor_expansion_in_pre_iteration_span': 'contradicted',
        'descriptor_change_in_pre_iteration_span': 'contradicted',
    }
    if span not in statuses:
        raise ValueError('unknown validated frontier span')
    status = statuses[span]
    return {
        'model': 'pc_v201_source_copy_and_index_no_descriptor_rewrite',
        'status': status,
        'source_descriptor_rewrite_expected': False,
        'known_helper_augmentation_proved': False,
        'requires_other_writer_or_capture_investigation': status == 'contradicted',
        'global_task_count_or_actor_state_proved': False,
    }
