"""Byte-derived comparisons of bounded v2.01 observations; never a seed oracle.

Independent of the omitted historical validate_mode_upstream_capture module.
A valid observation and an agreed causal hypothesis are separate outputs.
"""
from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path

EXE_SHA256 = '4047ECB623F3AE033F7D6F3637CD1C5408C72A89A038463268A7C9AC5C394159'


def require(test, message):
    if not test:
        raise ValueError(message)


def number(value):
    require(not isinstance(value, bool) and isinstance(value, (int, str)), 'integer required')
    return int(value, 0) if isinstance(value, str) else value


def array(value):
    if value == {}:
        return []
    require(isinstance(value, list), 'array required')
    return value


def raw(text, size):
    require(isinstance(text, str) and re.fullmatch(r'[0-9a-fA-F]*', text) is not None,
            'raw hex required')
    result = bytes.fromhex(text)
    require(len(result) == size, 'raw size mismatch')
    return result


def u(data, offset, size):
    require(0 <= offset and offset + size <= len(data), 'out-of-bounds field')
    return int.from_bytes(data[offset:offset+size], 'little')


def unwrap(obj):
    for _ in range(8):
        require(isinstance(obj, dict), 'object required')
        if 'bridge_result' in obj:
            obj = obj['bridge_result']
        elif 'result' in obj and isinstance(obj['result'], dict):
            obj = obj['result']
        elif 'value' in obj and isinstance(obj['value'], dict):
            obj = obj['value']
        else:
            return obj
    raise ValueError('excessive wrapper nesting')


def identity_key(identity):
    require(isinstance(identity, dict), 'process identity required')
    require(identity.get('executable_sha256', '').upper() == EXE_SHA256 and
            identity.get('image_size') == 77814240, 'unapproved executable')
    birth = identity.get('creation_filetime')
    require(isinstance(birth, str) and birth.isdigit() and int(birth) > 0, 'missing process birth')
    return number(identity['process_id']), birth, identity['executable_sha256'].upper()


def envelope(data, cleanup, schema, sites):
    """Validate scope/chronology/cleanup without trusting summaries or thread labels."""
    p, c = unwrap(data), unwrap(cleanup)
    require(p.get('schema') == schema, 'wrong schema')
    require(p.get('active') is False and not p.get('error'), 'active or errored capture')
    require(p.get('read_only') is True and p.get('writes_game_memory') is False and
            p.get('calls_game_functions') is False, 'not read-only')
    require(p.get('stop_on_first_complete_chain') is False, 'first-chain censoring')
    require(p.get('stop_reason') in ('observation_window_elapsed', 'entry_observation_complete'),
            'interrupted window')
    elapsed = p.get('stopped_elapsed_ms', p.get('elapsed_ms'))
    require(type(elapsed) in (int, float) and elapsed >= 0, 'invalid duration')
    if p['stop_reason'] == 'observation_window_elapsed':
        require(elapsed >= 120000, 'premature full-window claim')
    key = identity_key(p['identity'])
    require(p['pid'] == key[0], 'PID mismatch')
    m = data['capture_metadata']
    require(identity_key(m['target']) == key and m['run_id'] == p['run_id'] and
            m['requested_pid'] == p['pid'] and m['target_seed'] == p['target_seed'], 'outer identity mismatch')
    require(m.get('read_only') is True and m.get('writes_game_memory') is False and
            m.get('stop_on_first_final') is False, 'outer scope mismatch')
    base = number(p['module_base'])
    arm = unwrap(m['arm_verification'])
    expected = {base + value for value in sites.values()}
    require(arm.get('active') is True and arm.get('debugger_broken') is False and
            arm.get('run_id') == p['run_id'] and arm.get('schema') == schema and
            {number(a) for a in array(arm['owned_breakpoints'])} == expected and
            {number(a) for a in array(arm['breakpoints'])} == expected, 'arm proof mismatch')
    fresh = unwrap(m['fresh_phase_initialization'])
    require(fresh.get('initialized') is True and fresh.get('debugger_broken') is False and
            not array(fresh['breakpoints']), 'fresh owner unproved')
    cm, cp = cleanup['cleanup_metadata'], c['probe']
    require(cm.get('verified') is True and cm['run_id'] == p['run_id'] and
            identity_key(cm['target']) == key and cp['run_id'] == p['run_id'] and
            cp['schema'] == schema and cp.get('active') is False and
            cp.get('cleanup_pending') is False and cp.get('debugger_broken') is False and
            not array(cp['owned_breakpoints']) and not array(c['breakpoints']), 'cleanup not verified')
    ev = array(p['events'])
    require(0 < len(ev) <= p['max_events'] <= 256, 'empty/oversized events')
    require(p['event_sequence'] == len(ev) and len(ev) <= p['total_hits'] <= 4096, 'event count mismatch')
    require(0 <= p['read_bytes'] <= 4*1024*1024, 'read cap exceeded')
    last_tick = last_read = 0
    for seq, e in enumerate(ev, 1):
        require(e['sequence'] == seq and e['site'] in sites and number(e['rva']) == sites[e['site']], 'site/order mismatch')
        require(last_tick <= e['elapsed_ms'] < 120000 and
                last_read <= e['read_bytes_total'] <= p['read_bytes'], 'invalid event chronology')
        last_tick, last_read = e['elapsed_ms'], e['read_bytes_total']
    return p


def output_snapshot(out):
    """Reparse geometry and every descriptor. No count-only comparisons."""
    h = raw(out['header_hex'], 0x34)
    begin, end, cap = (u(h, off, 8) for off in (0, 8, 16))
    require(0 <= begin <= end <= cap and (end-begin) % 0x28 == 0 and
            (cap-begin) % 0x28 == 0 and (begin > 0 or cap == 0), 'wave geometry')
    waves = array(out['waves'])
    require(len(waves) == (end-begin)//0x28 == out['wave_count'] <= 8, 'wave count')
    result = []
    for wi, wave in enumerate(waves):
        require(wave['wave_index'] == wi and number(wave['address']) == begin + wi*0x28, 'wave identity')
        w = raw(wave['raw_hex'], 0x28)
        b, e, c = (u(w, off, 8) for off in (0, 8, 16))
        require(0 <= b <= e <= c and (e-b) % 20 == 0 and (c-b) % 20 == 0 and
                (b > 0 or c == 0), 'descriptor geometry')
        ds = array(wave['descriptors'])
        require(len(ds) == (e-b)//20 and len(result)+len(ds) <= 96, 'descriptor count cap')
        for i, d in enumerate(ds):
            bb = raw(d['raw_hex'], 20)
            require((d['wave_index'], d['position'], number(d['address'])) == (wi, i, b+i*20), 'descriptor identity')
            require((d['spawn'], d['lookup'], d['point_key'], d['source_flag'], d['selector_class']) ==
                    (u(bb, 0, 4), u(bb, 4, 4), bb[14], bb[15], bb[16]), 'descriptor decode')
            result.append({'wave': wi, 'position': i, 'address': d['address'],
                           'spawn': u(bb, 0, 4), 'lookup': u(bb, 4, 4), 'point': bb[14],
                           'source_flag': bb[15], 'class': bb[16], 'raw_hex': bb.hex().upper()})
    require((out['context_key'], out['terrain'], out['metadata'], out['playthrough']) ==
            (h[0x1e], h[0x1f], u(h, 0x28, 4), h[0x30]), 'header decode')
    control = out.get('control24', out.get('byte8_copy'))
    require(control == h[0x24], 'control24 decode')
    require(out['descriptor_count'] == len(result) and
            out['class0_count'] == sum(d['class'] == 0 for d in result) and
            out['class1_count'] == sum(d['class'] == 1 for d in result), 'aggregate decode')
    require(out.get('physical_actor_join') is False and out.get('persistent_task_join') is False, 'output scope')
    return result


def snapshot_bytes(out):
    """Capture-exact identity within one object lifetime; includes row buffers."""
    output_snapshot(out)
    return (number(out['address']), out['header_hex'].upper(),
            tuple((number(w['address']), w['raw_hex'].upper(),
                   tuple((number(d['address']), d['raw_hex'].upper()) for d in array(w['descriptors'])))
                  for w in array(out['waves'])))


def descriptors_equal(a, b):
    aa, bb = output_snapshot(a), output_snapshot(b)
    return [(d['wave'], d['position'], d['raw_hex']) for d in aa] == [
        (d['wave'], d['position'], d['raw_hex']) for d in bb]


def derive_frontier(data, cleanup):
    sites = {'materialize_enter': 0x1C244E4, 'after_prepass': 0x1C245D8,
             'task_lookup': 0x1C24614, 'task_link': 0x1C24662}
    p = envelope(data, cleanup, 'nioh3.materialization-frontier.v1', sites)
    ev = array(p['events'])
    require(len([e for e in ev if e['site'] == 'materialize_enter']) == 1, 'focused comparison requires one invocation')
    en, post = ev[:2]
    require(en['site'] == 'materialize_enter' and post['site'] == 'after_prepass', 'missing prepass')
    require(number(en['return_address']) == number(p['module_base']) + 0x2237994 and
            number(en['return_rva']) == 0x2237994, 'not known materializer caller')
    require(number(en['output_address']) == number(en['entry_rsp']) + 0x70 == number(en['output']['address']), 'materializer ABI')
    before, after = output_snapshot(en['output']), output_snapshot(post['output'])
    require(en.get('seed_binding') == 'queue snapshot only; no same-run generator-request join', 'queue promoted into causality')
    for e in ev:
        require(e['invocation_id'] == en['invocation_id'] and e['entry_rsp'] == en['entry_rsp'], 'mixed invocation/frame')
    lookups, links = {}, {}
    for e in ev[2:]:
        ix = e['lookup_ordinal']
        require(type(ix) is int and 1 <= ix <= len(after), 'invalid ordinal')
        source = after[ix-1]
        require(raw(e['source_current_hex'],20).hex().upper() == source['raw_hex'] and
                raw(e['source']['raw_hex'],20).hex().upper() == source['raw_hex'] and
                number(e['source']['address']) == number(source['address']) and
                e['manager'] == en['manager'], 'source/manager mismatch')
        if e['site'] == 'task_lookup':
            require(ix == len(lookups)+1 and ix not in lookups, 'lookup order')
            ptr = number(e['lookup_result'])
            require(e['branch'] == ('reuse_path' if ptr else 'new_path'), 'branch decode')
            lookups[ix] = e
        elif e['site'] == 'task_link':
            require(ix in lookups and ix not in links and ix == len(lookups), 'missing/duplicate/late link')
            le, task = lookups[ix], e['task']
            identity, desc = raw(task['identity_hex'],12), raw(task['descriptor_hex'],20)
            require(e['branch'] == le['branch'] and number(task['address']) > 0, 'link branch/pointer')
            if number(le['lookup_result']):
                require(number(task['address']) == number(le['lookup_result']), 'reuse pointer changed')
            require((task['spawn'],task['mission'],task['lookup']) ==
                    (u(identity,0,4),u(identity,4,4),u(identity,8,4)), 'task decode')
            identity_ok = (u(identity,0,4),u(identity,4,4),u(identity,8,4)) == (
                source['spawn'],en['mission_parameter'],source['lookup'])
            descriptor_ok, terrain_ok = desc.hex().upper() == source['raw_hex'], task['terrain'] == post['output']['terrain']
            require((e['identity_matches_source'],e['descriptor_matches_source'],e['terrain_matches_source']) ==
                    (identity_ok,descriptor_ok,terrain_ok), 'forged match labels')
            links[ix] = {'ordinal': ix, 'event': e['sequence'], 'task': task['address'], 'branch': e['branch'],
                         'identity_match':identity_ok,'descriptor_match':descriptor_ok,'terrain_match':terrain_ok,
                         **{k:source[k] for k in ('wave','spawn','lookup','point','class','source_flag')}}
        else:
            raise ValueError('unexpected post-prepass event')
    require(p['linked_count'] == len(links) and p['invocation_count'] == 1, 'count mismatch')
    linked = list(links.values())
    require(len({number(t['task']) for t in linked}) == len(linked), 'multiple descriptors alias one task')
    complete = len(links) == len(after) == len(lookups)
    all_new = complete and all(t['branch']=='new_path' for t in linked)
    matches = complete and all(t['identity_match'] and t['descriptor_match'] and t['terrain_match'] for t in linked)
    unchanged = snapshot_bytes(en['output']) == snapshot_bytes(post['output'])
    q = raw(en['queue_context_hex'],12)
    return {'run_id':p['run_id'],'process':identity_key(p['identity']),
            'entry_event':en['sequence'],'prepass_event':post['sequence'],'events':len(ev),
            'started_tick_ms':p['started_tick_ms'],'entry_elapsed_ms':en['elapsed_ms'],
            'queue_hex':q.hex().upper(),'queue_seed_context':u(q,0,4),'queue_q9_context':q[9],
            'queue_tail':q[10:12].hex().upper(),'queue_to_generator_bound':False,
            'mission_parameter':en['mission_parameter'],'output_address':en['output_address'],
            'context':en['output']['context_key'],'terrain':en['output']['terrain'],
            'before_count':len(before),'after_count':len(after),'linked_count':len(links),
            'waves':[len(array(w['descriptors'])) for w in array(en['output']['waves'])],
            'raw_snapshot_identical':unchanged,'complete':complete,'all_new':all_new,'all_match':matches,
            'prepass_augmentation_excluded_for_observed_span':unchanged,
            'reuse_excluded_for_observed_links':all_new,'links':linked,
            'unlinked_ordinals':sorted(set(range(1,len(after)+1))-set(links)),
            'function_return_observed':False,'physical_actor_join':False,'whole_manager_count_known':False,
            'native_session_mode_decoded':False,'all_writer_coverage':False}


def derive_sequence(data, cleanup):
    sites={'request_enqueue':0x10D9180,'request_queued':0x10D9368,
           'mission_consume':0x20E198C,'mission_generated':0x2237978}
    p=envelope(data,cleanup,'nioh3.mode-upstream-sequence.v1',sites)
    ev=array(p['events'])
    require([e['site'] for e in ev]==list(sites),'focused comparison requires exact four-event sequence')
    en,qu,co,ge=ev
    q=raw(en['request_hex'],12)
    require(all(raw(e['request_hex'],12)==q for e in ev),'request changed across sequence')
    require(raw(qu['source_hex'],12)==q and number(qu['source_address'])==number(en['request_address']) and
            number(qu['queue_rsp'])+0x658==number(en['caller_rsp']),'enqueue copy link')
    require(len({e['request_id'] for e in ev})==1 and co['link_grade']=='unique_pending_payload' and
            array(co['matching_request_ids'])==[en['request_id']], 'request link')
    require(co['invocation_id']==ge['invocation_id'] and co['consumer_rsp']==ge['consumer_rsp'] and
            number(ge['return_rsp'])==number(co['consumer_rsp'])+8 and
            number(co['output_address'])==number(co['consumer_rsp'])+0x70==number(ge['output']['address']), 'generator invocation ABI')
    require(number(en['return_rva'])==0xF1E4F1 and q[9]==0 and en['route']=='owned_scroll_branch', 'unexpected Sequence C producer')
    desc=output_snapshot(ge['output'])
    return {'run_id':p['run_id'],'process':identity_key(p['identity']),
            'started_tick_ms':p['started_tick_ms'],'enqueue_elapsed_ms':en['elapsed_ms'],
            'generated_elapsed_ms':ge['elapsed_ms'],'request_hex':q.hex().upper(),'q9':q[9],
            'tail':q[10:12].hex().upper(),'seed':u(q,0,4),'producer_return_rva':en['return_rva'],
            'descriptor_count':len(desc),'waves':[len(array(w['descriptors'])) for w in array(ge['output']['waves'])],
            'context':ge['output']['context_key'],'terrain':ge['output']['terrain'],
            'output_address':ge['output_address'],'materialization_observed':False,
            'session_at_enqueue':en.get('current_session'), 'session_at_consume':co.get('current_session'),
            'session_view_at_enqueue':en.get('session_view'),
            'native_session_mode_decoded':False,'physical_actor_join':False}


def reconcile(sequence, sequence_cleanup, frontier, frontier_cleanup):
    s=derive_sequence(sequence,sequence_cleanup); f=derive_frontier(frontier,frontier_cleanup)
    require(s['process']==f['process'],'cannot assert same-process fork across different births')
    require(s['run_id']!=f['run_id'],'distinct run IDs required')
    require(s['seed']==f['queue_seed_context'] and s['context']==f['context'] and s['terrain']==f['terrain'], 'different context')
    gap=f['started_tick_ms']-s['started_tick_ms']
    require(gap>unwrap(sequence)['elapsed_ms'],'observation windows not separated')
    sa=output_snapshot(unwrap(sequence)['events'][-1]['output'])
    fa=output_snapshot(unwrap(frontier)['events'][0]['output'])
    def base(rows):
        return [(d['wave'],raw(d['raw_hex'],20)[4:].hex().upper()) for d in rows if d['class']==0]
    return {'schema':'nioh3.entry-transaction-reconciliation.v1','sequence_c':s,'frontier':f,
            'same_process':True,'same_invocation':False,'window_start_difference_ms':gap,
            'generated_to_materializer_event_difference_ms':gap+f['entry_elapsed_ms']-s['generated_elapsed_ms'],
            'reused_stack_address':number(s['output_address'])==number(f['output_address']),
            'base_class0_equal_excluding_ordinal':base(sa)==base(fa),
            'frontier_materialization_fork_closed':f['complete'] and f['all_new'] and f['all_match'] and f['raw_snapshot_identical'],
            'sequence_c_later_became_ten':'not_observed',
            'frontier_literal_one_producer':'not_captured',
            'next_unresolved_edge':'native request producer/session selection -> Q[9] within this entry',
            'whole_manager_count_known':False,'physical_actor_join':False,'product_oracle_accepted':False}


def load(path):
    return json.loads(Path(path).read_text(encoding='utf-8-sig'))


def dump_exclusive(path, obj):
    with Path(path).open('x',encoding='utf-8') as f:
        json.dump(obj,f,ensure_ascii=False,indent=2);f.write('\n')
