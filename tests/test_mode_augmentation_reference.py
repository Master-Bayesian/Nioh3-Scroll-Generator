from pathlib import Path
import json
import copy
import struct
import sys
import pytest
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'research/possessed_enemy_capture'))
from mode_augmentation_reference import (Descriptor, tagged_auxiliary_keys, child_key_compatible,
                                        reused_descriptor_comparison, compare_native_controls)
FIX=ROOT/'test_fixtures/mode_augmentation'

def controls():return json.loads((FIX/'run_c.json').read_text()),json.loads((FIX/'run_d.json').read_text())

def test_source_controls_really_have_different_processes_and_no_six_to_ten_timeline():
    r=compare_native_controls(*controls())
    assert r['c_process']!=r['d_process'] and not r['cross_process_timeline_join']
    assert r['c_descriptor_count']==6 and r['d_pre_materialization_count']==10
    assert r['d_origin_parent_returns']==['0x20E1943','0x20E19C2','0x2237978']
    assert r['d_origin_is_earlier_than_first_task_copy']
    assert r['base_subsequence_identical_except_ordinal']
    assert not r['c_generated_to_persistent_task_join'] and not r['c_physical_actor_count_known']


def test_run_d_main_task_keys_not_generic_child_count():
    r=compare_native_controls(*controls())
    assert r['d_low28_keys_not_in_c_source_domain']==[0xF42,0xF43,0xF44,0xF45]
    assert r['tagged_factory_alone_cannot_map_c_source_to_d_keys']
    assert len(r['d_linked_records'])==10
    assert [x['manager_vector_count'] for x in r['d_linked_records']]==list(range(11,21))
    assert [x['selector'] for x in r['d_linked_records']].count(1)==4


def test_raw_mutation_not_accepted_by_control_comparator():
    c,d=controls();e=next(x for x in d['bridge_result']['events'] if x['site']=='task_linked')
    e['descriptor_hex']='00'*20
    with pytest.raises(ValueError,match='mismatch'):compare_native_controls(c,d)


def test_observed_total_hits_includes_no_ignored_other_consumers():
    r=compare_native_controls(*controls());assert r['c_total_hits']==4 and r['c_ignored_hits']==0


def test_shared_ordinal_across_factory_two_count_loops():
    k=list(tagged_auxiliary_keys(0xF3C,2,2))
    assert [(x.key,x.kind14c) for x in k]==[(0x10000F3C,1),(0x20000F3C,1),(0x30000F3C,2),(0x40000F3C,2)]


def test_u32_shift_wrap_does_not_manufacture_new_low28_spawns():
    keys=list(tagged_auxiliary_keys(0xF3C,17,2))
    assert keys[15].key==0xF3C and keys[16].key==0x10000F3C
    assert all(x.key&0x0FFFFFFF==0xF3C for x in keys)
    assert not child_key_compatible([0xF3C],0xF3D)


@pytest.mark.parametrize('base',[0,0xF3C,0xFFFFFFFF,0x10AB1234])
def test_all_u16_count_boundaries_preserve_low28(base):
    assert all(k.key&0x0FFFFFFF==base&0x0FFFFFFF for k in tagged_auxiliary_keys(base,65535,2))


@pytest.mark.parametrize('args',[(-1,0,0),(2**32,0,0),(0,-1,0),(0,65536,0),(0,0,65536),(0,True,0)])
def test_malformed_factory_inputs_rejected(args):
    with pytest.raises(ValueError):list(tagged_auxiliary_keys(*args))


def test_reuse_is_not_a_refresh_or_valid_enemy_identity_join():
    src=Descriptor.from_hex('3D0F000098CB0D00010000000000070000FFFFFF')
    stale=bytes.fromhex('3D0F00006EB80300030000000000030001FFFFFF')
    ident=struct.pack('<III',0xF3D,0xCC96,0x3B86E).hex()
    r=reused_descriptor_comparison(src,ident,stale.hex())
    assert r['lookup_key_matches_source'] and not r['lookup_matches_source']
    assert not r['embedded_descriptor_matches_source'] and not r['visible_branch_refreshes_descriptor']
    assert not r['actor_join']


@pytest.mark.parametrize('raw',[b'',b'\0'*19,b'\0'*21])
def test_descriptor_size_exact(raw):
    with pytest.raises(ValueError):Descriptor(raw)


@pytest.mark.parametrize('name,digest',[
 ('run_c.json','1AC727E8FD1AF8BCBA8D4049F6135B8E812C04B0EAB2F51A2D15015E7B0F86E1'),
 ('run_d.json','3A64F16C89975AB466A7EA38F317A0465E320B1F1FEB6D478C6A4007BD564A77'),
])
def test_native_fixture_bytes_not_normalized_or_modified_in_patch(name,digest):
    import hashlib
    assert hashlib.sha256((FIX/name).read_bytes()).hexdigest().upper()==digest
