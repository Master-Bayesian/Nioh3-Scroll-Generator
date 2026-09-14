"""Validate a producer-at-copy -> consumer -> result -> materializer INPUT join.

No native UI enum or physical actor claim. A unique observed payload link is
not an all-writers proof. State-dependent producer selection remains outside
this phase even when the selected writer and its session projection are bound.
"""
from __future__ import annotations
import argparse
import hashlib
from pathlib import Path
from entry_transaction_evidence import (array, raw, u, number, require, envelope,
    output_snapshot, load, dump_exclusive)
from mode_upstream_reference import CALLER_ROUTES

SCHEMA='nioh3.mode-transaction-join.v1'
SITES={'request_queued':0x10D9368,'mission_consume':0x20E198C,
       'mission_generated':0x2237978,'materialize_enter':0x1C244E4}


def producer_errors(e):
    q=raw(e['source_hex'],12)
    errors=[]
    def check(ok,label):
        if not ok: errors.append(label)
    check(raw(e['request_hex'],12)==q,'source_to_queue_bytes_changed')
    check(e['mission_type']==0xCC96,'queue_mission_type_mismatch')
    route=e['route'];src=number(e['source_address']);rbp=number(e['caller_rbp']);rsp=number(e['caller_rsp'])
    pc=e['producer_call']; direct_ok=False
    if pc.get('available'):
        b=raw(pc['raw_hex'],5)
        require(number(pc['address'])==number(e['producer_return_address'])-5 and pc['direct_call']==(b[0]==0xE8), 'callsite decode')
        if b[0]==0xE8:
            target=number(e['producer_return_address'])+int.from_bytes(b[1:],'little',signed=True)
            base=number(e['producer_return_address'])-number(e['return_rva'])
            require(number(pc['target_rva'])==target-base,'call target decode')
            direct_ok=(target-base)==0x10D9180
    if route!='unrecovered': check(direct_ok,'producer_direct_call_not_verified')
    if route in ('owned_scroll_branch','session_view_branch'):
        check(src==rbp-0x50,'caller_local_request_address_mismatch')
    elif route=='parameterized_session_branch':
        check(src==rsp+0x68,'parameterized_request_address_mismatch')
    elif route=='current_session_requeue':
        check(src==rbp-0x29,'requeue_local_request_address_mismatch')
    if route=='owned_scroll_branch':
        check(q[9]==0,'literal_zero_writer_contradicted')
    elif route in ('session_view_branch','parameterized_session_branch'):
        check(q[9]==1,'literal_one_writer_contradicted')
    if route=='session_view_branch' and e['session_view'].get('available'):
        v=e['session_view']
        check((v['mission_type'],v['scroll_seed'],v['metadata'],v['byte6'],v['playthrough'],int(v['byte8_source']!=0))==
              (0xCC96,u(q,0,4),u(q,4,2),q[6],q[7],q[8]),'session_view_to_request_projection_mismatch')
    if route=='current_session_requeue' and e['current_session'].get('available'):
        cs=e['current_session']
        check(cs['mission_type']==0xCC96 and cs['request_hex'].upper()==q.hex().upper(),'requeue_source_copy_mismatch')
    return errors


def compare_outputs(generated,materialized):
    ga,ma=output_snapshot(generated),output_snapshot(materialized)
    buffers=([(d['address'],d['raw_hex']) for d in ga]==[(d['address'],d['raw_hex']) for d in ma] and
             [(w['address'],w['raw_hex']) for w in array(generated['waves'])]==
             [(w['address'],w['raw_hex']) for w in array(materialized['waves'])])
    gh,mh=raw(generated['header_hex'],0x34),raw(materialized['header_hex'],0x34)
    changed=[i for i in range(0x34) if gh[i]!=mh[i]]
    return buffers,changed


def validate(data,cleanup):
    p=envelope(data,cleanup,SCHEMA,SITES)
    require(p.get('debugger_interface')==2,'VEH required')
    m=data['capture_metadata']
    require(m['phase']=='mode-transaction-join','wrong phase')
    source=m['source'];cap=Path(__file__).resolve().parent
    for key,path in [('phase_sha256',cap/'mode_transaction_join_ce.lua'),
                     ('lifecycle_sha256',cap.parent/'owned_breakpoint_lifecycle_ce.lua')]:
        require(source.get(key,'').lower()==hashlib.sha256(path.read_bytes()).hexdigest(),'capture source hash mismatch')
    require(source.get('phase_file')=='mode_transaction_join_ce.lua','wrong source')
    requests={}; inv={};materializers=[];unknowns=[]
    base=number(p['module_base'])
    for e in array(p['events']):
        site=e['site']
        if site=='request_queued':
            rid=e['request_id'];require(rid==len(requests)+1,'request ID reused')
            require(number(e['caller_rsp'])==number(e['queue_rsp'])+0x658,'live enqueue frame offset')
            ret=number(e['producer_return_address']);rr=e.get('return_rva')
            if rr is not None:require(ret==base+number(rr),'producer return VA/RVA mismatch')
            route=CALLER_ROUTES.get(ret-base,'unrecovered')
            require(e['route']==route,'producer classification wrong')
            require(number(e['queue_node'])==base+0x45B83E0+number(e['queue_index'])*0x58 and
                    0<=number(e['queue_index'])<3 and number(e['request_address'])==number(e['queue_node'])+0x20,'queue geometry')
            errs=producer_errors(e)
            expected_projection='not_recovered_for_this_route'
            if route=='owned_scroll_branch':expected_projection='literal_zero_route_only'
            elif route=='parameterized_session_branch':expected_projection='literal_one_route_only'
            elif route=='session_view_branch':expected_projection='session_view_fields_observed' if e['session_view'].get('available') else 'session_view_unavailable'
            elif route=='current_session_requeue':expected_projection='current_session_copy_observed' if e['current_session'].get('available') else 'current_session_unavailable'
            require(e['source_projection']==expected_projection,'source projection grade incorrect')
            require(array(e['producer_contract_errors'])==errs,'source contradiction omitted/invented')
            status='contradicted' if errs else ('unbound' if route=='unrecovered' else 'consistent')
            require(e['producer_contract_status']==status,'source status overclaim')
            require(e.get('native_mode_enum_decoded') is False,'UI enum overclaim')
            requests[rid]={'event':e,'pending':True,'errors':errs}
        elif site=='mission_consume':
            iid=e['invocation_id'];require(iid==len(inv)+1,'invocation ID reused')
            q=raw(e['request_hex'],12)
            candidates=[rid for rid,v in requests.items() if v['pending'] and raw(v['event']['request_hex'],12)==q]
            require(array(e['matching_request_ids'])==candidates,'pending payload association wrong')
            grade='unique_observed_queued_payload' if len(candidates)==1 else (
                'unobserved_or_changed_request' if not candidates else 'ambiguous_pending_payload')
            require(e['link_grade']==grade,'payload link overclaim')
            require(e.get('request_id')==(candidates[0] if len(candidates)==1 else None),'wrong request link')
            for rid in candidates:requests[rid]['pending']=False
            require(number(e['return_rva'])==0x2237978 and number(e['request_address'])==base+0x45B8400 and
                    number(e['output_address'])==number(e['consumer_rsp'])+0x70,'consumer ABI')
            for g in inv.values():
                if g['consume']['consumer_rsp']==e['consumer_rsp'] and not g.get('materialized'):
                    g['superseded']=True
            inv[iid]={'consume':e,'generated':None,'materialized':None}
        elif site=='mission_generated':
            iid=e['invocation_id'];require(iid in inv,'orphan generated event')
            g=inv[iid];co=g['consume'];require(g['generated'] is None,'duplicate generator return')
            require(e['consumer_rsp']==co['consumer_rsp'] and number(e['return_rsp'])==number(co['consumer_rsp'])+8 and
                    e['output_address']==co['output_address']==e['output']['address'] and
                    e['request_hex'].upper()==co['request_hex'].upper() and e.get('request_id')==co.get('request_id'),'generator frame/request join')
            q=raw(co['request_hex'],12);h=raw(e['output']['header_hex'],0x34)
            output_snapshot(e['output'])
            require(e['extra_from_request']==q[9] and (h[0x24],u(h,0x28,4),h[0x30])==(q[8],u(q,4,2),q[7]), 'generator projection')
            g['generated']=e
        else:
            require(e.get('materializer_input_only') is True and e.get('persistent_task_join') is False and
                    e.get('physical_actor_join') is False,'materializer input overclaim')
            require(number(e['return_rva'])==0x2237994 and number(e['output_address'])==number(e['entry_rsp'])+0x70==number(e['output']['address']), 'materializer ABI')
            output_snapshot(e['output']);q=raw(e['queue_context_hex'],12)
            candidates=[(iid,g) for iid,g in inv.items() if g['generated'] is not None and not g['materialized'] and
                        not g.get('superseded') and g['consume']['consumer_rsp']==e['entry_rsp'] and g['consume']['output_address']==e['output_address']]
            require(len(candidates)<=1,'ambiguous materializer invocation')
            if not candidates:
                require(e.get('invocation_id') is None and e['join_grade']=='unobserved_generator','stale materializer join')
                unknowns.append(e['sequence']);continue
            iid,g=candidates[0];co=g['consume']
            require(e['invocation_id']==iid and e.get('request_id')==co.get('request_id') and
                    e['consumed_request_hex'].upper()==co['request_hex'].upper() and e['join_grade']=='same_live_generator_frame_and_output', 'materializer invocation join')
            buffers,changes=compare_outputs(g['generated']['output'],e['output'])
            require(e['descriptor_buffers_equal']==buffers and array(e['header_changed_offsets'])==changes and
                    e['queue_unchanged_since_consume']==(q==raw(co['request_hex'],12)),'materializer comparison labels wrong')
            require(e['materializer_contract']==('consistent' if buffers else 'contradicted'),'materializer contradiction hidden')
            g['materialized']=e
            producer=requests.get(co.get('request_id'),{}).get('event')
            source_consistent=producer is not None and producer['producer_contract_status']=='consistent'
            materializers.append({'invocation_id':iid,'request_id':co.get('request_id'),
                'producer_route':producer['route'] if producer else None,
                'producer_status':producer['producer_contract_status'] if producer else 'unbound',
                'source_projection':producer['source_projection'] if producer else None,
                'native_source_projection_bound':source_consistent and producer['source_projection'] in ('session_view_fields_observed','current_session_copy_observed'),
                'q9':raw(co['request_hex'],12)[9], 'generated_count':g['generated']['output']['descriptor_count'],
                'materializer_input_count':e['output']['descriptor_count'], 'descriptor_buffers_equal':buffers,
                'header_changed_offsets':changes,
                'pipeline_consistent':source_consistent and buffers and set(changes)<= {0x24} and e['queue_unchanged_since_consume'],
                'join_grade':co['link_grade'], 'native_mode_enum_decoded':False})
    require(p['request_count']==len(requests) and p['invocation_count']==len(inv) and
            p['completed_invocations']==sum(g['generated'] is not None for g in inv.values()) and
            p['materialized_invocations']==len(materializers) and p['unbound_materializers']==len(unknowns),'aggregate mismatch')
    return {'observation_validated':True,'materializations':materializers,
            'unbound_materializer_events':unknowns,
            'unfinished_invocations':[i for i,g in inv.items() if g['materialized'] is None],
            'writer_counterexamples':[rid for rid,r in requests.items() if r['errors']],
            'complete_consistent_pipeline_count':sum(v['pipeline_consistent'] for v in materializers),
            'join_scope':'unique observed queued payload + same live generator/output frame; not all-writers',
            'native_ui_mode_to_producer_decision_recovered':False,'physical_actor_join':False,
            'persistent_task_join':False,'whole_manager_count_known':False,'product_oracle_accepted':False}


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('capture',type=Path);p.add_argument('cleanup',type=Path);p.add_argument('--output',type=Path,required=True)
    a=p.parse_args();dump_exclusive(a.output,validate(load(a.capture),load(a.cleanup)))
if __name__=='__main__':main()
