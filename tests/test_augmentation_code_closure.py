from pathlib import Path
import json
import shutil
import struct
import sys
import pytest
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'research/possessed_enemy_capture'))
from export_augmentation_closure import (UnwindIndex,Section,ExportError,RuntimeRange,
    parse_pdata,call_target,check_identity,objdump_range,build_export)
F=ROOT/'test_fixtures/augmentation_resolved'


def require_objdump():
    program=shutil.which('objdump')
    if program is None:
        pytest.skip('optional GNU objdump is not installed in this project environment')
    return program


def fixture_group(name):
    rows=json.loads((F/'actual_unwind_groups.json').read_text())[name]
    rva=min(r['unwind'] for r in rows)
    stop=max(r['unwind']+len(bytes.fromhex(r['xdata_hex'])) for r in rows)
    rd=bytearray(stop-rva)
    for r in rows:
        b=bytes.fromhex(r['xdata_hex']);i=r['unwind']-rva;rd[i:i+len(b)]=b
    pdata=b''.join(struct.pack('<III',r['begin'],r['end'],r['unwind']) for r in rows)
    return rows,UnwindIndex(pdata,Section(rva,bytes(rd)))


def test_actual_prepass_has_body_and_epilogue_missing_from_old_40_byte_export():
    rows,index=fixture_group('00E39D40')
    group=index.group(0xE39D40)
    assert [(x.begin,x.end) for x in group]==[(0xE39D40,0xE39D68),(0xE39D68,0xE39E14),(0xE39E14,0xE39E24)]
    assert sum(x.end-x.begin for x in group)==228
    assert group[0].end-group[0].begin==40
    # The omitted conditional target and body are now real instructions.
    code=b''.join(bytes.fromhex(r['code_hex']) for r in rows)
    text,ins=objdump_range(code,0xE39D40,require_objdump())
    decoded={r['rva']:r for r in ins}
    assert decoded[0xE39DC4]['bytes']=='F7407400400000'
    assert decoded[0xE39DEC]['instruction'].startswith('mov')
    assert decoded[0xE39E23]['instruction']=='ret'


def test_actual_sorted_insert_includes_noncontiguous_grow_and_duplicate_branches():
    rows,index=fixture_group('004BD5F8')
    group=index.group(0x4BD5F8)
    assert [(x.begin,x.end) for x in group]==[(0x4BD5F8,0x4BD6DF),(0x1914D2A,0x1914D5F)]
    assert sum(x.end-x.begin for x in group)==284
    assert all(x.end-x.begin<300 for x in group) # do NOT export the 21 MB gap


def test_multilevel_chained_map_is_one_unwind_family_not_six_functions():
    rows,index=fixture_group('00E2DD90')
    assert len(index.group(0xE2DD90))==6
    assert index.group(rows[2]['begin'])==index.group(0xE2DD90)
    assert len({index.owners[x['begin']] for x in rows})==1


def test_no_address_containment_guess_for_function_entry():
    _,index=fixture_group('00E39D40')
    with pytest.raises(ExportError,match='BEGIN'):index.group(0xE39D41)


def synthetic(parent=None,header=0x21,child=0x3000):
    rva=0x5000;data=bytearray(64)
    data[:4]=bytes([1,0,0,0]);data[16:20]=bytes([header,0,0,0])
    p=parent or (0x1000,0x1010,rva)
    struct.pack_into('<III',data,20,*p)
    pdata=struct.pack('<6I',0x1000,0x1010,rva,child,child+16,rva+16)
    return pdata,Section(rva,bytes(data))


def test_distant_child_follows_chain_not_adjacency():
    p,r=synthetic(child=0x1900000)
    assert len(UnwindIndex(p,r).group(0x1000))==2


def test_wrong_parent_tuple_rejected():
    p,r=synthetic(parent=(0x1000,0x1011,0x5000))
    with pytest.raises(ExportError,match='exact pdata'):UnwindIndex(p,r)


def test_cycle_rejected():
    p,r=synthetic(parent=(0x3000,0x3010,0x5010))
    with pytest.raises(ExportError,match='cyclic'):UnwindIndex(p,r)


@pytest.mark.parametrize('header',[0x29,0x39,0x41,0x23])
def test_invalid_unwind_flags_or_version_rejected(header):
    p,r=synthetic(header=header)
    with pytest.raises(ExportError,match='unwind header'):UnwindIndex(p,r)


def test_odd_unwind_slot_count_uses_even_aligned_trailer():
    p,r=synthetic();rd=bytearray(r.data);rd[18]=1
    rd[20:24]=b'\x01\x50\x00\x00';struct.pack_into('<III',rd,24,0x1000,0x1010,0x5000)
    assert len(UnwindIndex(p,Section(r.rva,bytes(rd))).group(0x1000))==2


def test_missing_xdata_cannot_be_reported_as_complete():
    p,r=synthetic()
    with pytest.raises(ExportError,match='outside'):UnwindIndex(p,Section(r.rva,r.data[:24]))


def test_pdata_padding_not_rows_and_zero_hole_rejected():
    p=struct.pack('<III',0x1000,0x1100,0x5000)
    assert len(parse_pdata(p+b'\0'*20))==1
    with pytest.raises(ExportError):parse_pdata(p+b'\0'*12+p)


@pytest.mark.parametrize('p',[b'',b'\1',struct.pack('<6I',0x1000,0x1100,0x5000,0x1001,0x1010,0x5000)])
def test_bad_pdata_rejected(p):
    with pytest.raises(ExportError):parse_pdata(p)


def test_exact_actual_calls_and_pinned_identity():
    from export_augmentation_closure import REQUESTS
    assert len(REQUESTS)==8
    for site,raw,target in REQUESTS:assert call_target(site,bytes.fromhex(raw))==target
    with pytest.raises(ExportError):call_target(0x1000,b'\x90'*5)
    with pytest.raises(ExportError,match='SHA-256'):check_identity('text',b'wrong build')
    with pytest.raises(ExportError,match='SHA-256'):build_export(b'wrong',b'',b'')


def test_truncated_instruction_is_an_error_not_success():
    with pytest.raises(ExportError):objdump_range(b'\x48\x8b',0x1000,require_objdump())


def test_export_metadata_does_not_claim_game_or_full_transitive_proof():
    d=json.loads((F/'export_expected.json').read_text())
    assert not d['uses_process'] and not d['game_acceptance']
    assert len(d['functions'])==8
    for f in d['functions']:
        assert f['ranges'] and f['decoded_instruction_count']>0
        assert not f['transitive_callees_complete'] and not f['semantic_role_proved_by_export']
        assert f['branch_targets_covered']
