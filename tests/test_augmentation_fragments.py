"""Run in a subprocess; no game process or live acceptance is implied."""
from pathlib import Path
import hashlib
import json
import platform
import shutil
import subprocess
import pytest
ROOT=Path(__file__).resolve().parents[1]
F=ROOT/'test_fixtures/augmentation_resolved'


def test_isolated_original_prepass_and_lookup_fragments(tmp_path):
    if platform.system()!='Linux' or platform.machine() not in ('x86_64','AMD64'):
        pytest.skip('local native-fragment harness requires Linux x86-64; not a game test')
    cc=shutil.which('gcc')
    if not cc:pytest.fail('gcc required for the explicitly selected native-fragment test')
    expected=json.loads((F/'export_expected.json').read_text())['functions']
    f=next(x for x in expected if x['entry_rva']==0xE39D40)
    raw=b''.join(bytes.fromhex(x['raw_hex']) for x in f['ranges'])
    assert (F/'E39D40_full.bin').read_bytes()==raw and len(raw)==228
    leaf=(F/'13684C_leaf.bin').read_bytes()
    assert hashlib.sha256(leaf).hexdigest().upper()=='627C74A4D65ED70D8F1D6A6D699C6786B23E4A49D5C3EDCF0B81FD134851BF59'
    out=tmp_path/'harness'
    src=ROOT/'research/possessed_enemy_capture/augmentation_fragment_harness.c'
    subprocess.run([cc,'-std=c11','-O2','-Wall','-Wextra',str(src),'-o',str(out)],check=True,capture_output=True,text=True,timeout=20)
    run=subprocess.run([str(out),str(F/'E39D40_full.bin'),str(F/'13684C_leaf.bin')],check=True,capture_output=True,text=True,timeout=10)
    result=json.loads(run.stdout)
    assert result['all_match'] and result['prepass_cases']==1024 and result['lookup_queries']==8224
    assert result['prepass_helper_calls_stubbed']==3 and result['lookup_instruction_patches']==0
    assert result['game_runtime'] is False
    print('\nNATIVE_FRAGMENT_RESULT='+json.dumps(result,sort_keys=True))
