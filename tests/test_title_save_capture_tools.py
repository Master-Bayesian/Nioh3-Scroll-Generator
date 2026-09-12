"""Synthetic collector/JSONL regressions, not native save acceptance."""
from __future__ import annotations
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
from types import SimpleNamespace
from concurrent.futures import ThreadPoolExecutor
import pytest

ROOT=Path(__file__).resolve().parents[1]

def load(name):
    spec=importlib.util.spec_from_file_location(name,ROOT/'tools'/f'{name}.py')
    result=importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result

collector=load('capture_title_save_lifecycle')
parser=load('analyze_title_save_observer')


def test_stage_sequence_uses_existing_subdirectories_and_gaps(tmp_path):
    (tmp_path/'001_title').mkdir()
    (tmp_path/'001_title'/'snapshot.json').write_text('{}')
    (tmp_path/'005_failed_capture').mkdir()
    sequence,directory=collector.reserve_capture_directory(tmp_path,'after_exit')
    assert sequence==6 and directory.name=='006_after_exit'
    assert (tmp_path/'001_title'/'snapshot.json').read_text()=='{}'


def test_repeated_same_stage_never_overwrites(tmp_path):
    a=collector.reserve_capture_directory(tmp_path,'title')
    b=collector.reserve_capture_directory(tmp_path,'title')
    assert a[0]==1 and b[0]==2 and a[1]!=b[1]


def test_concurrent_stage_reservation_is_unique(tmp_path):
    with ThreadPoolExecutor(max_workers=6) as pool:
        results=list(pool.map(lambda _: collector.reserve_capture_directory(tmp_path,'parallel'),range(20)))
    assert sorted(n for n,_ in results)==list(range(1,21))
    assert len({p for _,p in results})==20


def test_stage_bound_and_path_escape_rejected(tmp_path):
    (tmp_path/'999_full').mkdir()
    with pytest.raises(RuntimeError,match='limit'):
        collector.reserve_capture_directory(tmp_path,'next')
    with pytest.raises(ValueError):
        collector.reserve_capture_directory(tmp_path,'../escape')


def test_fingerprint_detects_change_during_hash(tmp_path,monkeypatch):
    target=tmp_path/'SAVEDATA.BIN';target.write_bytes(b'old')
    def changed(p):
        p.write_bytes(b'new and longer')
        return hashlib.sha256(b'old').hexdigest()
    monkeypatch.setattr(collector,'sha256_file',changed)
    with pytest.raises(RuntimeError,match='changed'):
        collector.stable_fingerprint('main_save',target)


def test_process_instance_uses_decimal_filetime(monkeypatch):
    monkeypatch.setattr(collector,'os',SimpleNamespace(name='nt'))
    seen=[]
    def run(args,**kw):
        seen.append((args,kw))
        return SimpleNamespace(returncode=0,stderr='',stdout=json.dumps({'pid':42,'creation_filetime':'134336000000000000','executable_path':'C:\\game\\Nioh3.exe'}))
    monkeypatch.setattr(collector.subprocess,'run',run)
    assert collector.process_snapshot()[0]['creation_filetime']=='134336000000000000'
    assert 'ToFileTimeUtc' in seen[0][0][-1]
    assert seen[0][1]['timeout']==15


@pytest.mark.parametrize('stdout',['[]','', '[{"pid":0}]','x'*16385,'null'])
def test_process_inventory_empty_or_malformed(monkeypatch,stdout):
    monkeypatch.setattr(collector,'os',SimpleNamespace(name='nt'))
    monkeypatch.setattr(collector.subprocess,'run',lambda *a,**k:SimpleNamespace(returncode=0,stdout=stdout,stderr=''))
    if stdout in ('[]',''):
        assert collector.process_snapshot()==[]
    else:
        with pytest.raises(RuntimeError):collector.process_snapshot()


def test_process_identity_timeout_is_explicit_not_false_empty(monkeypatch):
    monkeypatch.setattr(collector,'os',SimpleNamespace(name='nt'))
    def timeout(*a,**k):raise subprocess.TimeoutExpired('powershell',15)
    monkeypatch.setattr(collector.subprocess,'run',timeout)
    assert 'TimeoutExpired' in collector.process_snapshot()[0]['capture_error']


def fixture(path,events=None,status_updates=None,header_updates=None):
    identity={'pid':42,'creation_filetime':'134336000000000000'}
    header={'kind':'header','schema':parser.SCHEMA,'run_id':'C0.test','profile':'files','process':identity}
    event={'schema':parser.SCHEMA,'run_id':'C0.test','profile':'files','epoch':0,'sequence':1,'thread_id':99,'site':'writer_exit','return_al':1,'process':dict(identity)}
    status={'kind':'status','active':False,'dropped':0,'cleanup_pending':False,'owned_breakpoints':{},'errors':{},'sequence':1,'epoch':0,'continue_failed':False}
    if header_updates:header.update(header_updates)
    if status_updates:status.update(status_updates)
    events=[event] if events is None else events
    path.write_text(''.join(json.dumps(x)+'\n' for x in [header,*events,status]))
    return path


def test_successful_rename_is_never_product_acceptance(tmp_path):
    out=parser.analyze(fixture(tmp_path/'capture.jsonl'))
    assert out['capture_complete_within_selected_profile'] is True
    assert out['capture_integrity_only'] is True
    assert out['native_path_completion_proven'] is False
    assert out['product_commit_proven'] is False and out['release']=='BLOCK'


def test_empty_capture_cannot_pass_by_vacuity(tmp_path):
    out=parser.analyze(fixture(tmp_path/'capture.jsonl',events=[],status_updates={'sequence':0}))
    assert out['capture_complete_within_selected_profile'] is False


@pytest.mark.parametrize('status',[
    {'active':True},{'dropped':1},{'cleanup_pending':True},
    {'errors':['read failed']},{'owned_breakpoints':[123]}, {'continue_failed':True}])
def test_incomplete_lifecycle_cannot_pass(tmp_path,status):
    out=parser.analyze(fixture(tmp_path/'capture.jsonl',status_updates=status))
    assert out['capture_complete_within_selected_profile'] is False


def test_missing_final_fields_rejected(tmp_path):
    path=fixture(tmp_path/'capture.jsonl')
    rows=[json.loads(x) for x in path.read_text().splitlines()]
    rows[-1]={'kind':'status'}
    path.write_text(''.join(json.dumps(x)+'\n' for x in rows))
    with pytest.raises(ValueError,match='status is incomplete'):parser.analyze(path)


@pytest.mark.parametrize('mutation',['pid','filetime','sequence','thread','profile','epoch'])
def test_mixed_or_invalid_event_identity_rejected(tmp_path,mutation):
    path=fixture(tmp_path/'capture.jsonl')
    rows=[json.loads(x) for x in path.read_text().splitlines()]
    event=rows[1]
    if mutation=='pid':event['process']['pid']=43
    elif mutation=='filetime':event['process']['creation_filetime']='1'
    elif mutation=='sequence':event['sequence']=0
    elif mutation=='thread':event['thread_id']=True
    elif mutation=='profile':event['profile']='other'
    elif mutation=='epoch':event['epoch']=3
    path.write_text(''.join(json.dumps(x)+'\n' for x in rows))
    with pytest.raises(ValueError):parser.analyze(path)


def test_empty_queue_with_inflight_task_reported_not_quiescent(tmp_path):
    path=fixture(tmp_path/'capture.jsonl')
    rows=[json.loads(x) for x in path.read_text().splitlines()]
    rows[1]['globals']={'queue_count':0,'task':{'address':'0x140000000'}}
    path.write_text(''.join(json.dumps(x)+'\n' for x in rows))
    out=parser.analyze(path)
    assert out['empty_queue_with_active_task_events']==[1]
    assert out['product_commit_proven'] is False


def test_analyzer_refuses_partial_line_and_output_overwrite(tmp_path):
    path=fixture(tmp_path/'capture.jsonl')
    output=tmp_path/'result.json';output.write_text('existing')
    result=subprocess.run([sys.executable,str(ROOT/'tools/analyze_title_save_observer.py'),str(path),'--output',str(output)],capture_output=True,text=True)
    assert result.returncode!=0 and output.read_text()=='existing'
    path.write_text(path.read_text().rstrip('\n'))
    with pytest.raises(ValueError,match='incomplete'):parser.analyze(path)


def test_collector_cli_stages_and_three_files_are_read_only(tmp_path):
    account=tmp_path/'not-a-real-account'
    slot=account/'SAVEDATA00';slot.mkdir(parents=True)
    system=account/'SYSTEMSAVEDATA00';system.mkdir()
    paths=[slot/'SAVEDATA.BIN',slot/'BACKUP.BIN',system/'SAVEDATA.BIN']
    for i,p in enumerate(paths):p.write_bytes(b'synthetic'+bytes([i]))
    before={p:p.read_bytes() for p in paths}
    snapshots=[]
    for stage in ('title','after_exit'):
        run=subprocess.run([sys.executable,str(ROOT/'tools/capture_title_save_lifecycle.py'),'--save-path',str(paths[0]),'--output-root',str(tmp_path/'captures'),'--run-id','C0.synthetic','--stage',stage],capture_output=True,text=True)
        assert run.returncode==0,run.stderr
        snapshot=Path(run.stdout.splitlines()[0]);snapshots.append(json.loads(snapshot.read_text()))
    assert [s['sequence'] for s in snapshots]==[1,2]
    assert all(s['native_save_ownership']=='not_acquired' and not s['capture_is_atomic'] for s in snapshots)
    assert {p:p.read_bytes() for p in paths}==before
    assert all(len(s['files'])==3 and s['private_files']==[] for s in snapshots)
