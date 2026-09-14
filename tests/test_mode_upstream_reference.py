"""New interpreters and supplied-capture comparisons; no generated native evidence."""
from pathlib import Path
import json,struct,sys
import pytest
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'research/possessed_enemy_capture'))
from mode_upstream_reference import (Request,configured_extras,PlacementRow,exact_placement,
                                     descriptor_identity,classify_request_source)
from assignment_origin_reference import draw_10000
D=json.loads((ROOT/'tests/fixtures/mode_upstream_existing_capture.json').read_text())

def test_all_65536_opaque_tails_preserve_projection_not_native_parity():
    prefix=bytes.fromhex('A8912D05B70001030101')
    expected=Request(prefix+b'\0\0').generator_projection()
    for tail in range(65536):
        assert Request(prefix+tail.to_bytes(2,'little')).generator_projection()==expected

@pytest.mark.parametrize('n',[0,10,11,13,20])
def test_request_length(n):
    with pytest.raises(ValueError):Request(bytes(n))

def test_real_context_equal_does_not_make_extra_input_equal():
    row=bytes.fromhex(D['run_d']['context']['raw_hex'])
    expedition=Request.from_hex(D['run_d']['request_hex'])
    solo=Request.from_hex('A8912D05B700010301000000')
    assert configured_extras(expedition,row,5)==(1,1,1,1,0)
    assert configured_extras(solo,row,5)==(0,0,0,0,0)
    assert solo.generator_projection()[:-1]==expedition.generator_projection()[:-1]

@pytest.mark.parametrize('row,waves',[(bytes(47),4),(bytes(49),4),(bytes(48),6),(bytes(48),-1)])
def test_context_bounds(row,waves):
    with pytest.raises(ValueError):configured_extras(Request(bytes(12)),row,waves)

@pytest.mark.parametrize('caller,flag',[(0xF1E4F1,0),(0x21D9E24,1),(0x21DC438,1),(0x2233DE8,3)])
def test_routes_do_not_assign_party_size(caller,flag):
    b=bytearray(12);b[9]=flag;b[10]=255
    assert classify_request_source(caller,bytes(b))

def test_unknown_route_and_literal_contradiction_rejected():
    with pytest.raises(ValueError):classify_request_source(123,bytes(12))
    with pytest.raises(ValueError):classify_request_source(0x21DC438,bytes(12))

def position(terrain=0xD4,slot=1,x=1.0):
    b=bytearray(24);struct.pack_into('<4f',b,0,x,2,3,4);b[18]=terrain;b[19]=slot
    return PlacementRow(bytes(b))

def test_exact_position_requires_full_terrain_slot_identity():
    row=position();assert exact_placement([row,position(0xD3)],0xD4,1)==row
    with pytest.raises(ValueError):exact_placement([row],0xD4,7)
    with pytest.raises(ValueError):exact_placement([row,row],0xD4,1)
    with pytest.raises(ValueError):exact_placement([position(x=float('nan'))],0xD4,1)

def test_stable_occurrence_does_not_use_ordinal_spawn():
    a=bytearray(20);b=bytearray(20);a[:4]=(0xF3D).to_bytes(4,'little');b[:4]=(0xF3E).to_bytes(4,'little')
    a[14]=b[14]=7
    assert descriptor_identity(a,0xD4,1)==descriptor_identity(b,0xD4,1)
    assert descriptor_identity(a,0xD4,1)!=descriptor_identity(b,0xD4,2)

def test_packaged_solo_six_match_expedition_class0_projection():
    solo,expa,expb=[x['tasks'] for x in D['controls']]
    assert [len(solo),len(expa),len(expb)]==[6,10,10]
    # Spawn ordinals change as extras are interleaved, but all remaining
    # descriptor bytes of the six class-zero occurrences match in these captures.
    def fields(ts):return [bytes.fromhex(t['descriptor_hex'])[4:] for t in ts if t['selector_class']==0]
    assert fields(solo)==fields(expa)==fields(expb)
    assert all(t['selector_class']==0 and t['source_flag']==0 for t in solo)
    assert [t['spawn'] for t in expa if t['source_flag']]==[0xF3F]
    assert [t['spawn'] for t in expb if t['source_flag']]==[0xF3F]

def test_expedition_point1_absent_from_solo_without_claiming_actor_join():
    solo,exp,*_= [x['tasks'] for x in D['controls']]
    assert not any(t['position_key']==1 and t['terrain']==0xD4 for t in solo)
    target=[t for t in exp if t['source_flag']]
    assert len(target)==1 and target[0]['lookup']==0xDCB98 and target[0]['position_key']==1
    assert D['physical_actor_join'] is False

def test_existing_run_d_has_35_parent_draw_prefix_and_7_real_trials():
    d=D['run_d'];x=D['seed']
    for _ in range(35):x=(x*69069+1)&0xFFFFFFFF
    assert x==d['parent_rng_entry']==2281170847
    assert len(d['trials'])==7
    for e in d['trials']:
        x,ticket=draw_10000(x)
        assert (x,ticket)==(e['state'],e['ticket'])
        assert e['success']==(ticket<=e['threshold'])
    assert x==d['parent_rng_final']==115239214
    assert [e['spawn'] for e in d['trials'] if e['success']]==[0xF3F]

def test_raw_captures_not_relabelled_as_new_validation():
    assert D['source'].startswith('Provided captures')
    assert len(D['run_d']['tasks'])==10
    assert [sum(t['wave_index']==w and t['selector_class']==1 for t in D['run_d']['tasks']) for w in range(4)]==[1,1,1,1]
