"""Validate a bounded sequence without equating an invocation to session finality.

All results are observations at four named PCs. Identical pending requests are
ambiguous, unobserved enqueues remain unbound, and no absence outside the recorded
window is inferred. The old v1 four-event capture has a separate validator.
"""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

from mode_upstream_reference import CALLER_ROUTES, EXE_SHA256, Request
from validate_mode_upstream_capture import (
    address_set, require, same_process_identity, unwrap,
)

SITES = {
    'request_enqueue': 0x10D9180,
    'request_queued': 0x10D9368,
    'mission_consume': 0x20E198C,
    'mission_generated': 0x2237978,
}
SCHEMA = 'nioh3.mode-upstream-sequence.v1'


def number(v):
    if isinstance(v, bool) or not isinstance(v, (str, int)):
        raise ValueError('integer/address expected')
    return int(v, 0) if isinstance(v, str) else v


def array(v, message='array expected'):
    # CE serializes an empty Lua table as {} on some bridge versions.
    if v == {}:
        return []
    require(isinstance(v, list), message)
    return v


def blob(text, length):
    require(isinstance(text, str) and re.fullmatch(r'[0-9a-fA-F]+', text) is not None,
            'raw hex required')
    data = bytes.fromhex(text)
    require(len(data) == length, 'raw byte count mismatch')
    return data


def u(data, offset, size):
    return int.from_bytes(data[offset:offset+size], 'little')


def check_envelope(data, cleanup):
    p = unwrap(data)
    require(p.get('schema') == SCHEMA, 'wrong sequence schema')
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
    require(m.get('phase') == 'mode-upstream-sequence' and m.get('run_id') == p.get('run_id')
            and m.get('requested_pid') == p['pid'] and m.get('target_seed') == p['target_seed']
            and same_process_identity(m.get('target'), identity, base), 'outer capture identity mismatch')
    require(m.get('read_only') is True and m.get('writes_game_memory') is False
            and not m.get('stop_on_first_final'), 'outer scope mismatch')
    source = m.get('source', {})
    require(source.get('phase_file') == 'mode_upstream_sequence_ce.lua'
            and all(re.fullmatch(r'[0-9A-Fa-f]{64}', source.get(k, '') or '') for k in ('phase_sha256', 'lifecycle_sha256')),
            'source provenance missing')
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


def validate_output(out, request, address):
    require(number(out['address']) == address, 'wrong generation output address')
    h = blob(out['header_hex'], 0x34)
    b, e, c = u(h, 0, 8), u(h, 8, 8), u(h, 16, 8)
    require(0 <= b <= e <= c and (e-b) % 0x28 == 0 and (c-b) % 0x28 == 0
            and (b != 0 or c == 0), 'wave geometry')
    waves = array(out['waves'])
    require((e-b)//0x28 == len(waves) == out['wave_count'] <= 8, 'wave count')
    require((out['context_key'], out['terrain'], out['byte8_copy'], out['metadata'], out['playthrough'])
            == (h[0x1E], h[0x1F], h[0x24], u(h, 0x28, 4), h[0x30]), 'output interpretation mismatch')
    require((h[0x24], u(h, 0x28, 4), h[0x30]) == (request.raw[8], u(request.raw, 4, 2), request.raw[7]),
            'request metadata projection mismatch')
    require(out.get('stage') == 'post_20E198C_pre_1C244E4' and out.get('persistent_task_join') is False
            and out.get('physical_actor_join') is False, 'generation output overclaimed as task/actor')
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


def check_producer(e, req):
    route = CALLER_ROUTES.get(number(e['return_rva'])) if e.get('return_rva') else None
    require(e['route'] == (route or 'unrecovered'), 'route mislabelled')
    if route:
        from mode_upstream_reference import classify_request_source
        classify_request_source(number(e['return_rva']), req.raw)
        anchor = number(e['caller_rsp']) if route == 'parameterized_session_branch' else number(e['caller_rbp'])
        offset = {'owned_scroll_branch':-0x50, 'session_view_branch':-0x50,
                  'parameterized_session_branch':0x68, 'current_session_requeue':-0x29}[route]
        require(number(e['request_address']) == anchor+offset, 'producer-local request mismatch')
    require(e['padding_observed'].lower() == req.padding_observed.hex(), 'tail interpretation mismatch')
    if route == 'session_view_branch':
        v = e['session_view']
        require(v.get('available') is True and number(v['view']) == number(v['root'])+0x1010
                and v['mission_type'] == 0xCC96, 'session-view source unavailable')
        require((v['scroll_seed'], v['metadata'], v['byte6'], v['playthrough'], int(v['byte8_source'] != 0))
                == (req.seed, u(req.raw,4,2), req.raw[6], req.raw[7], req.raw[8]), 'session-view projection mismatch')
    if route == 'current_session_requeue':
        v = e['current_session']
        require(v.get('available') is True and v['mission_type'] == 0xCC96
                and number(v['request_address']) == number(v['object'])+0x38
                and blob(v['request_hex'],12) == req.raw, 'requeue is not the captured session copy')
    return route


def validate(data, cleanup):
    p, base = check_envelope(data, cleanup)
    events = array(p.get('events'))
    require(p.get('max_events') == 128 and p.get('max_requests') == 16
            and p.get('max_invocations') == 16 and p.get('max_hits') == 4096
            and p.get('max_seconds') == 120, 'collector limits changed')
    require(0 <= p.get('read_bytes', -1) <= 4*1024*1024
            and 0 <= p.get('total_hits', -1) <= 4096, 'collector budget exceeded')
    require(len(events) <= 128 and p['event_sequence'] == len(events), 'event count mismatch')
    requests, invocations, completed = {}, {}, []
    last_time = 0
    last_read = 0
    for seq, e in enumerate(events, 1):
        site = e.get('site')
        require(site in SITES and number(e.get('rva')) == SITES[site] and e['sequence'] == seq, 'site/order mismatch')
        t = number(e['elapsed_ms'])
        require(last_time <= t < 120000, 'event clock invalid')
        last_time = t
        event_read = e.get('read_bytes_total', -1)
        require(last_read <= event_read <= p['read_bytes'], 'read accounting mismatch')
        last_read = event_read
        require(number(e['thread_id']) > 0 and e.get('thread_id_source','').startswith(('debug_event_api:', 'callback_api_unverified:')),
                'thread token provenance missing')
        req = Request(blob(e['request_hex'],12))
        require(req.seed == p['target_seed'], 'event target mismatch')
        rid = e.get('request_id')
        if site == 'request_enqueue':
            route = check_producer(e, req)
            require(rid == len(requests)+1 and rid <= 16, 'request id reuse/gap/bound')
            requests[rid] = {'phase':'enqueued', 'request':req.raw, 'enqueue':e, 'route':route}
        elif site == 'request_queued':
            qi = number(e['queue_index'])
            require(0 <= qi < 3 and number(e['queue_node']) == base+0x45B83E0+qi*0x58
                    and number(e['request_address']) == number(e['queue_node'])+0x20
                    and e['mission_type'] == 0xCC96 and blob(e['source_hex'],12) == req.raw, 'queue geometry/content mismatch')
            matches = [i for i,r in requests.items() if r['phase']=='enqueued'
                       and number(r['enqueue']['request_address']) == number(e['source_address'])
                       and number(r['enqueue']['caller_rsp']) == number(e['queue_rsp'])+0x658]
            if matches:
                require(matches == [rid] and e['enqueue_link'] == 'frame_and_source', 'ambiguous/wrong enqueue-copy link')
                r = requests[rid]
                require(r['request'] == req.raw and r['enqueue']['thread_id'] == e['thread_id'], 'enqueue-copy content/thread mismatch')
                r['phase'] = 'queued'; r['queued'] = e
            else:
                require(rid == len(requests)+1 and rid <= 16 and e['enqueue_link'] == 'unobserved_enqueue', 'unobserved queue link mislabelled')
                requests[rid] = {'phase':'queued','request':req.raw,'queued':e,'route':None}
        elif site == 'mission_consume':
            iid = e['invocation_id']
            require(iid == len(invocations)+1 and iid <= 16, 'invocation id reuse/gap/bound')
            require(number(e['request_address']) == base+0x45B8400 and number(e['return_rva']) == 0x2237978
                    and number(e['output_address']) == number(e['consumer_rsp'])+0x70, 'mission caller/output mismatch')
            matches = [i for i,r in requests.items() if r['phase']=='queued' and r['request']==req.raw]
            require(array(e['matching_request_ids']) == matches, 'candidate pending requests mismatch')
            grade = 'unique_pending_payload' if len(matches)==1 else ('unobserved_or_changed_request' if not matches else 'ambiguous_pending_payload')
            require(e['link_grade'] == grade, 'consumer linkage overclaimed')
            if len(matches)==1:
                require(rid == matches[0], 'wrong consumer request id');requests[rid]['phase']='consumed'
            else:
                require(rid is None, 'must not bind by seed/FIFO when identity is ambiguous')
                for i in matches:requests[i]['phase']='ambiguous_consumption'
            require(not any(g.get('done') is False and g['consume']['consumer_rsp']==e['consumer_rsp'] for g in invocations.values()),
                    'overlapping generator frame reuse')
            invocations[iid] = {'consume':e,'request':req,'request_id':rid,'done':False}
        else:
            iid = e['invocation_id']
            require(iid in invocations and not invocations[iid]['done'], 'orphan/duplicate generation return')
            g = invocations[iid];s=g['consume']
            require(rid == g['request_id'] and req.raw == g['request'].raw and e['thread_id'] == s['thread_id']
                    and number(e['consumer_rsp']) == number(s['consumer_rsp'])
                    and number(e['return_rsp']) == number(s['consumer_rsp'])+8
                    and number(e['output_address']) == number(s['output_address']), 'return invocation/frame identity mismatch')
            require(e['extra_from_request'] == req.raw[9]
                    and e['extra_evidence'] == 'request_projection_static; not a new F+1648 observation', 'extra evidence overclaimed')
            validate_output(e['output'], req, number(s['output_address']))
            g['done']=True
            completed.append({'invocation_id':iid,'request_id':rid,'producer_route':requests[rid]['route'] if rid else None,
                              'link_grade':s['link_grade'],'start_sequence':s['sequence'],'return_sequence':seq,
                              'request_hex':e['request_hex'],'request_extra':req.raw[9],
                              'tail_hex':req.padding_observed.hex(),'context_key':e['output']['context_key'],
                              'terrain':e['output']['terrain'],'descriptor_count':e['output']['descriptor_count'],
                              'class0_count':e['output']['class0_count'],'class1_count':e['output']['class1_count']})
    require(p['request_count']==len(requests) and p['invocation_count']==len(invocations)
            and p['completed_invocations']==len(completed)
            and p['unbound_invocations']==sum(g['request_id'] is None for g in invocations.values()), 'collector counter mismatch')
    stopped = number(p['stopped_elapsed_ms'])
    require(last_time <= stopped and (p['stop_reason'] != 'observation_window_elapsed' or stopped >= 120000), 'window closed before claimed deadline')
    pending_requests = [i for i,r in requests.items() if r['phase'] in ('enqueued','queued','ambiguous_consumption')]
    pending_generations = [i for i,g in invocations.items() if not g['done']]
    zero_one = [(a['invocation_id'],b['invocation_id']) for a in completed for b in completed
                if a['return_sequence'] < b['start_sequence'] and a['request_extra']==0 and b['request_extra']!=0]
    return {'observation_validated': bool(completed),
            'status': 'no_target_generation_return' if not completed else ('complete_window_with_unresolved_links' if pending_requests or pending_generations or p['unbound_invocations'] else 'observed_generation_returns'),
            'completed_invocations':completed,'pending_request_ids':pending_requests,'pending_invocation_ids':pending_generations,
            'strictly_sequential_zero_then_nonzero':zero_one,
            'target_thread_id_from_debug_api': bool(events) and all(e['thread_id_source'].startswith('debug_event_api:') for e in events),
            'no_later_request_proven':False,'native_session_mode_decoded':False,
            'persistent_task_join_validated':False,'physical_actor_join_validated':False,'product_oracle_accepted':False,
            'boundary':'Only these instrumented sites and this time window. A return is not session finality; raw generator output precedes task materialization.'}


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('capture',type=Path);ap.add_argument('cleanup',type=Path)
    args=ap.parse_args()
    print(json.dumps(validate(json.loads(args.capture.read_text(encoding='utf-8')),
                              json.loads(args.cleanup.read_text(encoding='utf-8'))),indent=2))


if __name__=='__main__':
    main()
