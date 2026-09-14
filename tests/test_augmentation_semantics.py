from pathlib import Path
import random
import struct
import sys
import pytest
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'research/possessed_enemy_capture'))
from augmentation_semantics import *


def d(key,lookup,cls):
    raw=bytearray(20);struct.pack_into('<II',raw,0,key,lookup);raw[0x10]=cls
    return Descriptor(bytes(raw))


def row(flags):
    b=bytearray(0x398);struct.pack_into('<I',b,0x74,flags);return bytes(b)


def test_prepass_orders_class0_then_class1_and_never_adds_descriptors():
    waves=((d(3,1,1),d(4,1,0)),(d(5,1,0),d(6,1,1)))
    saved=repr(waves)
    r=two_pass_indices(waves,{1:row(0x4000)})
    assert r.indices=={4:0,5:1,3:2,6:3}
    assert r.descriptor_count_before==r.descriptor_count_after==4
    assert r.rng_draws==0 and repr(waves)==saved


def test_prepass_eligibility_is_not_possession_eligibility():
    waves=((d(1,1,0),d(2,2,0),d(3,3,0),d(4,1,1)),)
    r=prepass_indices(waves,0,{1:row(0x4000),2:row(0),3:None})
    assert r.indices=={1:0}
    assert [x.reason for x in r.events]==['indexed','row_74_bit14_clear','missing_enemy_row','class_mismatch']


def test_duplicate_key_updates_index_but_does_not_deduplicate_source():
    waves=((d(9,1,0),d(9,1,1)),)
    r=two_pass_indices(waves,{1:row(0x4000)})
    assert r.indices=={9:1} and r.next_counter==2 and r.descriptor_count_after==2


def test_counter_wrap_and_low_byte_task_projection():
    r=prepass_indices(((d(1,1,0),d(2,1,0),d(3,1,0)),),0,{1:row(0x4000)},counter=0xfffffffe)
    assert r.indices=={1:0xfffffffe,2:0xffffffff,3:0} and r.next_counter==1
    assert [task_byte96(123,k,r.indices) for k in (1,2,3,4)]==[254,255,0,123]


def test_selected_class_only_does_not_require_other_class_db_bytes():
    r=prepass_indices(((d(1,2,1),),),0,{2:b'bad'})
    assert not r.indices and r.next_counter==0
    with pytest.raises(ValueError,match='stride'):prepass_indices(((d(1,2,1),),),1,{2:b'bad'})


@pytest.mark.parametrize('cls',[-1,256,True])
def test_invalid_selector_rejected(cls):
    with pytest.raises(ValueError):prepass_indices((),cls,{})


def test_map_lookup_is_unsigned_equality_not_nearest_or_mission_aware():
    entries=(TaskEntry(3,0x111),TaskEntry(0x80000000,0x222),TaskEntry(0xffffffff,0x333))
    assert task_map_lookup(entries,2)==0 and task_map_lookup(entries,4)==0
    assert task_map_lookup(entries,0x80000000)==0x222
    assert task_map_lookup(entries,0xffffffff)==0x333


def test_insert_does_not_overwrite_existing_object_same_key():
    a=(TaskEntry(3,0x111),)
    b,inserted,got=task_map_insert(a,TaskEntry(3,0x999))
    assert b==a and not inserted and got.pointer==0x111
    c,inserted,_=task_map_insert(b,TaskEntry(2,0x222))
    assert inserted and c==(TaskEntry(2,0x222),TaskEntry(3,0x111))


def test_map_contract_randomized_against_dictionary():
    rng=random.Random(130913);entries=();expected={}
    for _ in range(200):
        key=rng.getrandbits(8);pointer=rng.getrandbits(48)
        expected.setdefault(key,pointer)
        entries,_,_=task_map_insert(entries,TaskEntry(key,pointer))
    for key in range(300):assert task_map_lookup(entries,key)==expected.get(key,0)
    assert [x.key for x in entries]==sorted(expected)


def test_raw_map_stride_is16_with_u32_key_and_u64_pointer():
    raw=struct.pack('<IIQ',3,0xdeadbeef,0x123456789abcd)+struct.pack('<IIQ',7,0xface,0x22222222222)
    out=parse_task_map_entries(raw,2,20)
    assert out==(TaskEntry(3,0x123456789abcd),TaskEntry(7,0x22222222222))
    with pytest.raises(ValueError):parse_task_map_entries(raw[:-1],2,20)
    with pytest.raises(ValueError):parse_task_map_entries(raw,2,1)
    with pytest.raises(ValueError):parse_task_map_entries(raw[16:]+raw[:16],2,2)


def test_tag_constructor_does_not_copy_base_descriptor_or_make_class1():
    f=tagged_initializer_fields(0x10000f3c,0xcc96,0xd4)
    assert f[0x28]==b'\0'*4
    assert f[0x80]==b'\0'*8 and f[0x88]==b'\x06'+b'\0'*7
    assert f[0x90]==b'\0' and f[0xe8]==b'\x01\0\0\0'
    assert f[0x95]==b'\0\xff' and f[0x14c]==b'\0'*4
    assert set(f)!={0} # not pretending this is a complete task record


def test_prepass_expansion_counterexample_is_preserved_but_not_promoted_to_generator_proof():
    from augmentation_semantics import pre_iteration_contract
    d=pre_iteration_contract('descriptor_expansion_in_pre_iteration_span')
    assert d['status']=='contradicted'
    assert d['requires_other_writer_or_capture_investigation']
    assert not d['known_helper_augmentation_proved']
    assert not d['global_task_count_or_actor_state_proved']


def test_prepass_contract_distinguishes_missing_boundary_from_unchanged_evidence():
    from augmentation_semantics import pre_iteration_contract
    assert pre_iteration_contract(None)['status']=='not_observed'
    assert pre_iteration_contract('no_descriptor_change_in_pre_iteration_span')['status']=='consistent'
    assert pre_iteration_contract('descriptor_change_in_pre_iteration_span')['status']=='contradicted'
    with pytest.raises(ValueError):pre_iteration_contract('PASS')
