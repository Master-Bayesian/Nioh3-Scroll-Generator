"""Validate only the materializer frontier, never global absence or actor state."""
from __future__ import annotations
import argparse
import hashlib
import json
import re
from pathlib import Path
from validate_mode_upstream_capture import address_set, require, same_process_identity, unwrap
from validate_mode_upstream_sequence import number, array, blob, u
from mode_upstream_reference import EXE_SHA256
from augmentation_semantics import pre_iteration_contract
SITES={'materialize_enter':0x1C244E4,'after_prepass':0x1C245D8,'task_lookup':0x1C24614,'task_link':0x1C24662}
SCHEMA='nioh3.materialization-frontier.v1'

def check_envelope(data, cleanup):
    p = unwrap(data)
    require(p.get('schema') == SCHEMA, 'wrong frontier schema')
    require(p.get('active') is False and not p.get('error'), 'capture active or errored')
    require(p.get('stop_reason') in ('observation_window_elapsed', 'entry_observation_complete'),
            'capture interrupted; retain evidence, no completed-window claim')
    require(p.get('stop_on_first_complete_chain') is False, 'first-chain censoring still enabled')
    require(p.get('read_only') is True and p.get('writes_game_memory') is False
            and p.get('calls_game_functions') is False, 'not explicitly read-only')
    identity = p.get('identity', {})
    require(identity.get('executable_sha256') == EXE_SHA256 and identity.get('image_size') == 77814240,
            'wrong executable')
    require(identity.get('process_id') == p.get('pid') and bool(identity.get('creation_filetime')), 'bad birth identity')
    base = number(p['module_base'])
    m = data.get('capture_metadata', {})
    require(m.get('phase') == 'materialization-frontier' and m.get('run_id') == p.get('run_id')
            and m.get('requested_pid') == p['pid'] and m.get('target_seed') == p['target_seed']
            and same_process_identity(m.get('target'), identity, base), 'outer capture identity mismatch')
    require(m.get('read_only') is True and m.get('writes_game_memory') is False
            and not m.get('stop_on_first_final'), 'outer scope mismatch')
    source = m.get('source', {})
    require(source.get('phase_file') == 'materialization_frontier_ce.lua'
            and all(re.fullmatch(r'[0-9A-Fa-f]{64}', source.get(k, '') or '') for k in ('phase_sha256', 'lifecycle_sha256')),
            'source provenance missing')
    cap = Path(__file__).resolve().parent
    expected_source = hashlib.sha256((cap/'materialization_frontier_ce.lua').read_bytes()).hexdigest()
    expected_lifecycle = hashlib.sha256((cap.parent/'owned_breakpoint_lifecycle_ce.lua').read_bytes()).hexdigest()
    require(source['phase_sha256'].lower() == expected_source and
            source['lifecycle_sha256'].lower() == expected_lifecycle, 'collector/lifecycle source hash mismatch')
    fresh = unwrap(m.get('fresh_phase_initialization', {}))
    require(fresh.get('initialized') is True and fresh.get('debugger_broken') is False
            and fresh.get('breakpoints') in ([], {}), 'fresh phase not proven')
    arm = unwrap(m.get('arm_verification', {}))
    expected = {base+r for r in SITES.values()}
    require(arm.get('active') is True and arm.get('schema') == SCHEMA and arm.get('run_id') == p['run_id']
            and arm.get('debugger_broken') is False and address_set(arm.get('owned_breakpoints')) == expected
            and address_set(arm.get('breakpoints')) == expected, 'arm/ownership mismatch')
    c = unwrap(cleanup)
    cp = c.get('probe', {})
    require(cp.get('active') is False and cp.get('cleanup_pending') is False
            and cp.get('owned_breakpoints') in ([], {}) and c.get('breakpoints') in ([], {})
            and cp.get('debugger_broken') is False and cp.get('run_id') == p['run_id']
            and cp.get('schema') == SCHEMA, 'cleanup not verified')
    cm = cleanup.get('cleanup_metadata', {})
    require(cm.get('verified') is True and cm.get('run_id') == p['run_id']
            and same_process_identity(cm.get('target'), identity, base), 'cleanup identity mismatch')
    return p, base


def validate_output(out, address):
    require(number(out['address']) == address, 'wrong generation output address')
    h = blob(out['header_hex'], 0x34)
    b, e, c = u(h, 0, 8), u(h, 8, 8), u(h, 16, 8)
    require(0 <= b <= e <= c and (e-b) % 0x28 == 0 and (c-b) % 0x28 == 0
            and (b != 0 or c == 0), 'wave geometry')
    waves = array(out['waves'])
    require((e-b)//0x28 == len(waves) == out['wave_count'] <= 8, 'wave count')
    require((out['context_key'], out['terrain'], out['control24'], out['metadata'], out['playthrough'])
            == (h[0x1E], h[0x1F], h[0x24], u(h, 0x28, 4), h[0x30]), 'output interpretation mismatch')
    require(out.get('stage') == 'at_event_boundary' and out.get('persistent_task_join') is False
            and out.get('physical_actor_join') is False, 'descriptor snapshot overclaimed')
    all_desc = []
    for wi, wave in enumerate(waves):
        require(wave['wave_index'] == wi and number(wave['address']) == b+wi*0x28, 'wave address/order mismatch')
        w = blob(wave['raw_hex'], 0x28)
        db, de, dc = u(w, 0, 8), u(w, 8, 8), u(w, 16, 8)
        require(0 <= db <= de <= dc and (de-db) % 0x14 == 0 and (dc-db) % 0x14 == 0
                and (db != 0 or dc == 0), 'descriptor geometry')
        ds = array(wave['descriptors'])
        require((de-db)//0x14 == len(ds) <= 96 and len(all_desc)+len(ds) <= 96, 'descriptor count')
        for i, item in enumerate(ds):
            d = blob(item['raw_hex'], 0x14)
            require(item['wave_index'] == wi and item['position'] == i
                    and number(item['address']) == db+i*0x14, 'descriptor identity/order mismatch')
            require((item['spawn'], item['lookup'], item['point_key'], item['source_flag'], item['selector_class'])
                    == (u(d, 0, 4), u(d, 4, 4), d[0xE], d[0xF], d[0x10]), 'descriptor interpretation mismatch')
            all_desc.append(item)
    require(out['descriptor_count'] == len(all_desc)
            and out['class0_count'] == sum(d['selector_class'] == 0 for d in all_desc)
            and out['class1_count'] == sum(d['selector_class'] == 1 for d in all_desc), 'aggregate counts mismatch')
    return all_desc



def validate_task(t):
    identity=blob(t['identity_hex'],12);d=blob(t['descriptor_hex'],20)
    require((t['spawn'],t['mission'],t['lookup'])==(u(identity,0,4),u(identity,4,4),u(identity,8,4)), 'task identity interpretation')
    require(number(t['address'])>0 and t.get('physical_actor_join') is False
            and t.get('whole_manager_enumeration') is False, 'task pointer/scope')
    blob(t['flags_e5_f5_hex'],17)
    require(all(type(t[k]) is int and 0<=t[k]<=255 for k in ('terrain','index95','value96')), 'task byte range')
    require(type(t['kind14c']) is int and 0<=t['kind14c']<=0xFFFFFFFF,'task type range')
    return identity,d


def validate(data, cleanup):
    p,base=check_envelope(data,cleanup)
    require(p.get('max_events')==256 and p.get('max_invocations')==16 and p.get('max_hits')==4096
            and p.get('max_seconds')==120,'bounds changed')
    events=array(p.get('events'));require(0<len(events)<=256,'empty/oversized events are not mechanism evidence')
    require(p['event_sequence']==len(events) and len(events)<=p['total_hits']<=4096,'event/hit count')
    require(0<=p['read_bytes']<=4*1024*1024,'read budget')
    elapsed=p.get('stopped_elapsed_ms',p.get('elapsed_ms',-1))
    require(type(elapsed) in (int,float) and elapsed>=0,'missing elapsed')
    if p['stop_reason']=='observation_window_elapsed':require(elapsed>=120000,'false completed window')
    inv={};lasttime=lastread=0;linked_total=0
    for seq,e in enumerate(events,1):
        site=e.get('site');require(site in SITES and number(e['rva'])==SITES[site] and e['sequence']==seq,'site/order')
        require(lasttime<=e['elapsed_ms']<120000 and lastread<=e['read_bytes_total']<=p['read_bytes'],'event chronology/budget')
        lasttime=e['elapsed_ms'];lastread=e['read_bytes_total']
        require(e.get('thread_id_source')=='unavailable' or
                e.get('thread_id_source','').startswith(('debug_event_api:','callback_api_unverified:')),'thread provenance')
        iid=e['invocation_id']
        if site=='materialize_enter':
            require(iid==len(inv)+1 and iid<=16,'invocation id reused')
            require(type(e['mission_parameter']) is int and 0<=e['mission_parameter']<=0xFFFFFFFF,'mission parameter range')
            rsp=number(e['entry_rsp']);out=number(e['output_address'])
            ret=number(e['return_address']);rr=e.get('return_rva')
            if rr is not None:require(ret==base+number(rr),'return VA/RVA mismatch')
            known=ret==base+0x2237994
            require(e['caller_grade']==('known_mission_materializer_call' if known else 'unbound_caller'),'caller classification')
            if known:require(out==rsp+0x70,'known caller ABI')
            require(e.get('seed_binding')=='queue snapshot only; no same-run generator-request join','seed overclaim')
            blob(e['queue_context_hex'],12)
            before=validate_output(e['output'],out)
            inv[iid]={'entry':e,'before':before,'post':None,'lookups':{},'links':[], 'span':None}
        else:
            require(iid in inv,'event without entry');g=inv[iid]
            require(e['entry_rsp']==g['entry']['entry_rsp'],'stack-frame association')
            if site=='after_prepass':
                require(g['post'] is None and not g['lookups'],'duplicate or late prepass')
                g['post']=validate_output(e['output'],number(g['entry']['output_address']))
                g['post_event']=e
                require(e['attribution']=='span only, not proof that E39D40 alone is the writer','false single-helper attribution')
                before=[(d['wave_index'],d['position'],d['raw_hex'].upper()) for d in g['before']]
                after=[(d['wave_index'],d['position'],d['raw_hex'].upper()) for d in g['post']]
                g['span']='descriptor_expansion_in_pre_iteration_span' if len(after)>len(before) else (
                    'descriptor_change_in_pre_iteration_span' if after!=before else 'no_descriptor_change_in_pre_iteration_span')
            elif site=='task_lookup':
                require(g['post'] is not None,'lookup before prepass')
                ix=e['lookup_ordinal'];require(ix==len(g['lookups'])+1 and ix<=len(g['post']),'lookup order/count')
                require(e['source']==g['post'][ix-1] and blob(e['source_current_hex'],20)==blob(e['source']['raw_hex'],20),'lookup source identity')
                require(e['manager']==g['entry']['manager'],'lookup manager mismatch')
                ptr=number(e['lookup_result'])
                require(ptr>=0 and e['branch']==('reuse_path' if ptr else 'new_path'),'lookup branch interpretation')
                if ptr:
                    validate_task(e['existing_task']);require(number(e['existing_task']['address'])==ptr,'lookup task pointer')
                else:require('existing_task' not in e,'null lookup has alleged existing task')
                g['lookups'][ix]=e
            else:
                ix=e['lookup_ordinal'];require(ix in g['lookups'] and ix not in [x['lookup_ordinal'] for x in g['links']],'missing/duplicate link')
                le=g['lookups'][ix]
                require(ix==max(g['lookups']),'link belongs to a previous loop visit')
                require(e['source']==le['source'] and e['source_current_hex'].upper()==le['source_current_hex'].upper(),'link source mismatch')
                require(e['branch']==le['branch'] and e['manager']==le['manager'],'link branch/manager mismatch')
                identity,desc=validate_task(e['task']);source=blob(e['source']['raw_hex'],20)
                if le['branch']=='reuse_path':require(e['task']['address']==le['existing_task']['address'],'reuse pointer mismatch')
                require(e['identity_matches_source']==(u(identity,0,4)==u(source,0,4) and u(identity,4,4)==g['entry']['mission_parameter'] and u(identity,8,4)==u(source,4,4)),'identity compare result wrong')
                require(e['descriptor_matches_source']==(desc==source),'raw descriptor compare wrong')
                require(e['terrain_matches_source']==(e['task']['terrain']==g['post_event']['output']['terrain']),'terrain compare wrong')
                require(e['task_stage']=='before common F4/E5 writes; task pointer observed, not physical actor or entire manager','link overclaim')
                g['links'].append(e);linked_total+=1
    require(len(inv)==p['invocation_count'] and linked_total==p['linked_count'],'aggregate count mismatch')
    summaries=[]
    for iid,g in inv.items():
        summaries.append({'invocation_id':iid,'caller_grade':g['entry']['caller_grade'],
            'return_rva':g['entry'].get('return_rva'),'mission_parameter':g['entry']['mission_parameter'],'queue_context_seed':u(blob(g['entry']['queue_context_hex'],12),0,4),
            'queue_context_is_not_seed_proof':True,'before_descriptor_count':len(g['before']),
            'after_descriptor_count':len(g['post']) if g['post'] is not None else None,
            'helper_span_result':g['span'],
            'static_pre_iteration_contract':pre_iteration_contract(g['span']),
            'control24_before':g['entry']['output']['control24'],
            'control24_after':g['post_event']['output']['control24'] if g['post'] is not None else None,
            'reuse_mismatches':[e['lookup_ordinal'] for e in g['links'] if e['branch']=='reuse_path'
                                and not(e['identity_matches_source'] and e['descriptor_matches_source'] and e['terrain_matches_source'])],
            'new_path_copy_mismatches':[e['lookup_ordinal'] for e in g['links'] if e['branch']=='new_path'
                                       and not(e['identity_matches_source'] and e['descriptor_matches_source'] and e['terrain_matches_source'])],
            'unlinked_lookup_ordinals':[i for i in g['lookups'] if i not in [e['lookup_ordinal'] for e in g['links']]],
            'linked_records':len(g['links']),
            'all_post_descriptors_linked':g['post'] is not None and len(g['links'])==len(g['post']),
            'function_return_observed':False})
    return {'observation_validated':True,'invocations':summaries,
        'window_ms':elapsed,'all_writer_coverage':False,'physical_actor_join':False,
        'whole_manager_state_known':False,'unobserved_generator_excluded':False,
        'native_session_mode_decoded':False,'product_oracle_accepted':False,
        'boundary':'four points within 1C244E4; no global absence or causality across processes'}


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('capture',type=Path);p.add_argument('cleanup',type=Path);p.add_argument('--output',type=Path)
    a=p.parse_args();result=validate(json.loads(a.capture.read_text()),json.loads(a.cleanup.read_text()))
    text=json.dumps(result,indent=2)
    if a.output:
        with a.output.open('x',encoding='utf-8') as f:f.write(text+'\n')
    print(text)
if __name__=='__main__':main()
