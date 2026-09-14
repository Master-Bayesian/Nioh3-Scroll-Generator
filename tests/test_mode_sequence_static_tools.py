"""Mechanical verifier tests; not a semantic/native acceptance substitute."""
from pathlib import Path
import importlib.util
import json
import pytest

ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('verify_mode_sequence_sites', ROOT/'tools/verify_mode_sequence_sites.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)


def test_literal_instruction_bytes_and_exact_span():
    d=m.parse_asm('00100000: 48 8b 01     mov rax,[rcx]\n00100003: e8 08 00 00 00    call 0x100010')
    assert m.read_instruction_span(d,0x100000,8)==bytes.fromhex('488B01E808000000')
    assert m.direct_call(d[0x100003][0],0x100003)==0x100010


@pytest.mark.parametrize('start,length',[(0x100001,2),(0x100000,2),(0x100000,4)])
def test_inside_instruction_and_gapped_spans_rejected(start,length):
    d=m.parse_asm('00100000: 48 8b 01   mov rax,[rcx]')
    with pytest.raises(ValueError): m.read_instruction_span(d,start,length)


@pytest.mark.parametrize('text',[
    '00100000: 48 8b 01 mov rax,[rcx]\n00100000: c3 ret',
    '00100000: 48 8b 01 mov rax,[rcx]\n00100001: c3 ret',
    'No bytes here',
])
def test_duplicate_overlap_or_no_bytes_rejected(text):
    with pytest.raises(ValueError):m.parse_asm(text)


def test_negative_rel32_and_indirect_call_not_promoted():
    assert m.direct_call(bytes.fromhex('E8FBFFFFFF'),0x1000)==0x1000
    with pytest.raises(ValueError):m.direct_call(bytes.fromhex('FF9050010000'),0x1000)


def test_exactly_four_pinned_sites_no_old_early_context():
    d=json.loads((ROOT/'research/possessed_enemy_capture/mode_upstream_sequence_v201_locators.json').read_text())
    assert len(d['sites'])==4 and 'context_resolved' not in d['sites']
    assert int(d['sites']['mission_generated']['rva'],16)==0x2237978
    for s in d['sites'].values():
        assert len(bytes.fromhex(s['bytes']))==s['length']==len(s['mask'])//2
