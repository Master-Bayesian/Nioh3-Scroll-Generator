from pathlib import Path
import sys, struct, math, json
import pytest
P = Path(__file__).resolve().parents[1]/'research'/'possessed_enemy_capture'
sys.path.insert(0,str(P))
from assignment_origin_reference import *


def desc(spawn, cls=0, enemy=True, subtype=True, flags=0, threshold=-1, preset=0):
    raw=bytearray(20);struct.pack_into('<II',raw,0,spawn,0xDCB98);raw[15]=preset;raw[16]=cls
    return PreparedDescriptor(bytes(raw),enemy,subtype,flags,threshold)


def config(value=100,multiplier=1.0):
    r=bytearray(32);struct.pack_into('<i',r,16,value);struct.pack_into('<f',r,24,multiplier);return bytes(r)

@pytest.mark.parametrize('v,m,want',[(123,1.0,123),(123,0.5,61),(-123,0.5,-61),(100,float('nan'),-2**31),
                                    (100,float('inf'),-2**31),(2**31-1,2.0,-2**31),(-2**31,1.0,-2**31)])
def test_config_native_sse(v,m,want):assert threshold_4543(config(v,m))==want

def test_missing_config_is_zero_not_disabled():assert threshold_4543(None)==0

def test_truncated_config():
    with pytest.raises(ValueError):threshold_4543(b'\0'*20)

def test_all_high16_float32_quantization_has_specific_noninteger_boundaries():
    inv=pow(A,-1,2**32)
    differences=[]
    for hi in range(65536):
        x=(hi<<16)|456
        state,t=draw_10000(((x-1)*inv)&MASK32)
        assert state==x
        if t!=hi*10000//65536:differences.append((hi,t,hi*10000//65536))
    assert differences==[(29039, 4431, 4430), (33135, 5056, 5055), (37231, 5681, 5680), (41327, 6306, 6305), (45423, 6931, 6930), (49519, 7556, 7555), (53615, 8181, 8180), (53982, 8237, 8236), (57711, 8806, 8805), (58078, 8862, 8861), (61807, 9431, 9430), (62174, 9487, 9486)]

def test_inclusive_zero_threshold_accepts_ticket_zero():
    state=((0-1)*pow(A,-1,2**32))&MASK32
    r=assign_prepared([[desc(0xF3C,threshold=0)]],state,False)
    assert r['selected_spawn']==0xF3C and r['events'][0]['ticket']==0

@pytest.mark.parametrize('d,reason',[(desc(0xF3C,cls=1),'selector_class'),
                                   (desc(0xF3C,subtype=False),'subtype_missing'),
                                   (desc(0xF3C,flags=1),'subtype_flag14_bit0')])
def test_skip_does_not_draw(d,reason):
    r=assign_prepared([[d]],123,False);assert r['state']==123 and r['events'][0]['reason']==reason

def test_missing_enemy_row_falls_through():
    r=assign_prepared([[desc(0xF3C,enemy=False,subtype=False,threshold=9999)]],123,False)
    assert r['selected_spawn']==0xF3C and len(r['events'])==1

def test_first_success_not_uniform_pool_sampling():
    r=assign_prepared([[desc(0xF3C,threshold=9999),desc(0xF3D,threshold=9999)]],100,True)
    assert r['selected_spawn']==0xF3C and len(r['events'])==1

def test_class0_prioritized_over_earlier_class1():
    r=assign_prepared([[desc(0xF3C,cls=1,threshold=9999),desc(0xF3D,threshold=9999)]],100,True)
    assert r['selected_spawn']==0xF3D

def test_class1_shares_state_after_all_class0_failures():
    r=assign_prepared([[desc(0xF3C),desc(0xF3D,cls=1,threshold=9999)]],100,True)
    ts=[e for e in r['events'] if e['kind']=='trial']
    assert [x['selector'] for x in ts]==[0,1]
    assert ts[0]['state_after']==ts[1]['state_before'] and r['selected_spawn']==0xF3D

def test_class1_not_reached_when_disabled():
    r=assign_prepared([[desc(0xF3C),desc(0xF3D,cls=1,threshold=9999)]],100,False)
    assert r['selected_spawn'] is None and all(e['selector']==0 for e in r['events'])

def test_no_success_no_flag_mutation():
    r=assign_prepared([[desc(0xF3C)]],100,False)
    assert r['selected_spawn'] is None and bytes.fromhex(r['output_descriptors'][0][0])[15]==0

def test_helper_does_not_clear_preexisting_source_flag():
    r=assign_prepared([[desc(0xF3C,preset=1)]],100,False)
    assert bytes.fromhex(r['output_descriptors'][0][0])[15]==1

def test_late_five_controls_have_typed_descriptor_join_not_replay_claim():
    controls=json.loads((Path(__file__).parent/'fixtures'/'assignment_origin_controls.json').read_text())
    assert len(controls)==5 and sum(len(c['records']) for c in controls)==38
    for c in controls:
        flagged=[]
        for v in c['records']:
            raw=bytes.fromhex(v['descriptor_hex']);assert len(raw)==20
            assert struct.unpack_from('<II',raw)==(v['spawn'],v['lookup'])
            assert raw[15]==v['flag8f'] and v['object0_is_null'] and v['object18_is_null']
            assert v['ea']==0
            if v['flag8f']:flagged.append(v['spawn']);assert v['e9']==1
        expected=[] if len(c['records'])==6 and c['seed']==86872488 else [0xF40 if c['seed']==156062997 else 0xF3F]
        assert flagged==expected
        assert set(bytes.fromhex(c['mask_hex']))=={0}

def test_e9_direct_implies_other_gates_not_raw_equality():
    r=bytearray(240);r[0x8F]=1;r[0x90]=0;struct.pack_into('<I',r,0x80,0xF3C);struct.pack_into('<I',r,0x24,0xCC96)
    assert e9_null_source_direct(r,0,0xCC96,3,1)
    assert e9_null_source_direct(r,0,0xCC96,3,None)
    assert not e9_null_source_direct(r,0,0xCC96,3,0)
    assert not e9_null_source_direct(r,0,0xCC96,2,1)
    assert not e9_null_source_direct(r,1,0xCC96,3,1)
    r[0x8F]=0;assert not e9_null_source_direct(r,0,0xCC96,3,1)

def test_e9_nonnull_source_refused():
    r=bytearray(240);r[0x18]=1
    with pytest.raises(ValueError):e9_null_source_direct(r,0,0xCC96,3,1)

def test_e9_count_inclusive_and_still_draws_for_zero_remainder():
    calls=[]
    assert e9_count_prepared(4,0.5,0,False,lambda upper:(calls.append(upper) or 2))==2
    assert calls==[2]

def test_e9_count_no_draw_probability_zero_and_capacity_cap():
    def bad(_):raise AssertionError('unexpected draw')
    assert e9_count_prepared(10,0,7,True,bad)==1
    assert e9_count_prepared(10,1,7,True,lambda q:0)==1

@pytest.mark.parametrize('p',[float('nan'),float('inf'),-1,2,1e-20])
def test_e9_count_exotic_domain_not_silently_approximated(p):
    with pytest.raises(ValueError):e9_count_prepared(5,p,0,False,lambda q:0)
