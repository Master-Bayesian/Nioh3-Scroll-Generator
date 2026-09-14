"""Exercise actual supplied raw observations and reject forged cross-run joins."""
from pathlib import Path
import copy
import json
import os
import sys
import pytest
CAP=Path(__file__).resolve().parents[1]/'research/possessed_enemy_capture'
sys.path.insert(0,str(CAP))
from entry_transaction_evidence import reconcile,derive_frontier,derive_sequence,load,output_snapshot

EVIDENCE=Path(os.environ.get(
    'NIOH3_FRONTIER_EVIDENCE',
    str(Path(__file__).resolve().parent/'fixtures'/'entry_transaction'),
))
NAMES=['prior-sequence-c/mode-upstream-sequence.json','prior-sequence-c/mode-upstream-sequence.cleanup.json',
       'current-frontier/materialization-frontier.json','current-frontier/materialization-frontier.cleanup.json']

def fixtures():return [load(EVIDENCE/n) for n in NAMES]

def test_rederive_all_ten_new_tasks_from_actual_bytes():
    a=fixtures();o=reconcile(*a);f=o['frontier'];s=o['sequence_c']
    assert (f['before_count'],f['after_count'],f['linked_count'])==(10,10,10)
    assert f['all_new'] and f['all_match'] and f['raw_snapshot_identical'] and f['complete']
    assert len({t['task'] for t in f['links']})==10
    assert [t['event'] for t in f['links']]==list(range(4,23,2))
    assert [t['class'] for t in f['links']]==[0,1,0,1,0,0,1,0,0,1]
    assert [t['spawn'] for t in f['links'] if t['source_flag']]==[0xF3F]
    assert s['q9']==0 and f['queue_q9_context']==1 and s['tail']==f['queue_tail']=='0000'
    assert o['window_start_difference_ms']==13098688 and o['generated_to_materializer_event_difference_ms']==13060375
    assert o['same_process'] and not o['same_invocation'] and o['reused_stack_address']
    assert o['base_class0_equal_excluding_ordinal'] and o['frontier_materialization_fork_closed']
    assert o['sequence_c_later_became_ten']=='not_observed' and o['frontier_literal_one_producer']=='not_captured'
    assert not f['queue_to_generator_bound'] and not f['function_return_observed'] and not f['whole_manager_count_known']


def test_native_mode_and_actor_never_inferred():
    o=reconcile(*fixtures());assert not o['physical_actor_join'] and not o['product_oracle_accepted']
    assert not o['sequence_c']['native_session_mode_decoded'] and not o['frontier']['native_session_mode_decoded']

@pytest.mark.parametrize('key,value',[('process_id',123),('creation_filetime','134338049984156851')])
def test_same_pid_or_address_not_sufficient_when_birth_differs(key,value):
    a=fixtures()
    for obj in [a[2]['bridge_result']['identity'],a[2]['capture_metadata']['target'],a[3]['cleanup_metadata']['target']]:obj[key]=value
    if key=='process_id':a[2]['bridge_result']['pid']=value;a[2]['capture_metadata']['requested_pid']=value
    with pytest.raises(ValueError,match='different births'):reconcile(*a)


def test_same_run_id_is_not_two_transactions():
    a=fixtures();v=a[0]['bridge_result']['run_id']
    a[2]['bridge_result']['run_id']=v;a[2]['capture_metadata']['run_id']=v;a[2]['capture_metadata']['arm_verification']['run_id']=v
    a[3]['cleanup_metadata']['run_id']=v;a[3]['bridge_result']['probe']['run_id']=v
    with pytest.raises(ValueError,match='Distinct|distinct'):reconcile(*a)


def test_pointer_reuse_and_count_equality_cannot_mask_byte_mismatch():
    a=fixtures();e=a[2]['bridge_result']['events'][3]
    b=bytearray.fromhex(e['task']['descriptor_hex']);b[15]^=1;e['task']['descriptor_hex']=b.hex()
    with pytest.raises(ValueError,match='forged match'):reconcile(*a)
    e['descriptor_matches_source']=False
    o=reconcile(*a)
    assert not o['frontier']['all_match'] and not o['frontier_materialization_fork_closed']


def test_explicit_reuse_counterexample_is_retained():
    a=fixtures();ev=a[2]['bridge_result']['events'];ev[2]['lookup_result']=ev[3]['task']['address']
    ev[2]['branch']=ev[3]['branch']='reuse_path'
    o=reconcile(*a);assert not o['frontier']['all_new'] and not o['frontier_materialization_fork_closed']


def test_partial_links_are_not_complete():
    a=fixtures();p=a[2]['bridge_result'];p['events']=p['events'][:-1];p['event_sequence']-=1;p['linked_count']-=1
    o=reconcile(*a);assert o['frontier']['unlinked_ordinals']==[10] and not o['frontier_materialization_fork_closed']

@pytest.mark.parametrize('which',[0,1])
def test_missing_independent_cleanup_is_not_pass(which):
    a=fixtures();a[1 if which==0 else 3]['cleanup_metadata']['verified']=False
    with pytest.raises(ValueError,match='cleanup'):reconcile(*a)


def test_reconnect_cleanup_supersedes_initial_pending_without_editing_capture():
    a=fixtures();assert a[2]['bridge_result']['cleanup_pending'] is True
    assert a[3]['cleanup_metadata']['reconnect_cycles']==4
    assert reconcile(*a)['frontier_materialization_fork_closed']
    assert a[2]['bridge_result']['cleanup_pending'] is True


def test_unequal_request_data_is_not_merged():
    a=fixtures();a[0]['bridge_result']['events'][2]['request_hex']='A8912D05B700010301010000'
    with pytest.raises(ValueError,match='request changed'):reconcile(*a)


def test_empty_window_is_not_mechanism_failure_or_success():
    a=fixtures();a[2]['bridge_result']['events']=[];a[2]['bridge_result']['event_sequence']=0
    with pytest.raises(ValueError,match='empty'):reconcile(*a)

@pytest.mark.parametrize('field,value',[('descriptor_count',11),('class0_count',0),('terrain',0)])
def test_aggregate_text_cannot_override_raw_bytes(field,value):
    a=fixtures();a[2]['bridge_result']['events'][0]['output'][field]=value
    with pytest.raises(ValueError):reconcile(*a)
