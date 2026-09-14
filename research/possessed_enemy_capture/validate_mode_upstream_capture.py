"""Validate a single upstream observation; never turn it into product acceptance."""
from __future__ import annotations
import argparse
import json
from pathlib import Path
from mode_upstream_reference import Request, PlacementRow, SITES, EXE_SHA256, classify_request_source


def number(v):
    if isinstance(v, bool):
        raise ValueError('bool is not an address')
    return int(v, 0) if isinstance(v, str) else int(v)


def unwrap(d):
    if 'bridge_result' in d:
        d = d['bridge_result']
    return d.get('result', d.get('value', d))


def require(condition, message):
    if not condition:
        raise ValueError(message)


def address_set(value):
    require(isinstance(value, (list, dict)), 'breakpoint inventory is not explicit')
    items = value.values() if isinstance(value, dict) else value
    return {number(item) for item in items}


def same_process_identity(attested, observed, module_base):
    if not isinstance(attested, dict) or not isinstance(observed, dict):
        return False
    attested = dict(attested)
    observed = dict(observed)
    attested_base = attested.pop('module_base', None)
    observed_base = observed.pop('module_base', None)
    if attested != observed:
        return False
    for candidate in (attested_base, observed_base):
        if candidate is not None and number(candidate) != module_base:
            return False
    return True


def validate(data: dict, cleanup: dict) -> dict:
    p = unwrap(data)
    require(p.get('schema') == 'nioh3.mode-upstream.v1', 'wrong schema')
    require(p.get('active') is False and not p.get('error'), 'capture active or errored')
    require(p.get('stop_reason') == 'upstream_request_bound', 'capture did not reach its final site')
    identity = p.get('identity', {})
    require(identity.get('executable_sha256') == EXE_SHA256 and identity.get('image_size') == 77814240,
            'wrong executable')
    require(identity.get('process_id') == p.get('pid') and bool(identity.get('creation_filetime')), 'bad process birth identity')
    base = number(p['module_base'])
    metadata = data.get('capture_metadata')
    require(isinstance(metadata, dict), 'complete runner capture metadata required')
    require(metadata.get('phase') == 'mode-upstream'
            and metadata.get('run_id') == p.get('run_id')
            and metadata.get('requested_pid') == identity.get('process_id')
            and metadata.get('target_seed') == p.get('target_seed')
            and same_process_identity(metadata.get('target'), identity, base),
            'outer capture identity mismatch')
    require(metadata.get('read_only') is True and metadata.get('writes_game_memory') is False,
            'outer capture is not read-only')
    source = metadata.get('source', {})
    require(source.get('phase_file') == 'mode_upstream_ce.lua'
            and isinstance(source.get('phase_sha256'), str) and len(source['phase_sha256']) == 64
            and isinstance(source.get('lifecycle_sha256'), str) and len(source['lifecycle_sha256']) == 64,
            'capture source identity missing')
    fresh = unwrap(metadata.get('fresh_phase_initialization', {}))
    require(fresh.get('initialized') is True and fresh.get('debugger_broken') is False
            and fresh.get('breakpoints') in ([], {}), 'fresh phase initialization not proven')
    arm = unwrap(metadata.get('arm_verification', {}))
    expected_sites = {base + rva for rva in SITES.values()}
    require(arm.get('active') is True and arm.get('run_id') == p.get('run_id')
            and arm.get('schema') == p.get('schema') and arm.get('debugger_broken') is False,
            'observer arm state not proven')
    require(address_set(arm.get('owned_breakpoints')) == expected_sites
            and address_set(arm.get('breakpoints')) == expected_sites,
            'observer arm breakpoint ownership mismatch')
    c = unwrap(cleanup)
    cp = c.get('probe', {})
    require(cp.get('active') is False and cp.get('cleanup_pending') is False
            and cp.get('owned_breakpoints') in ([], {}) and c.get('breakpoints') in ([], {})
            and cp.get('debugger_broken') is False, 'cleanup not explicitly verified')
    require(cp.get('run_id') == p.get('run_id') and cp.get('schema') == p.get('schema'), 'wrong cleanup owner')
    cm = cleanup.get('cleanup_metadata', {})
    require(cm.get('verified') is True and cm.get('run_id') == p.get('run_id')
            and same_process_identity(cm.get('target'), identity, base),
            'cleanup process/run attestation missing')
    events = p.get('events')
    require(isinstance(events, list) and len(events) == 4, 'exactly four causal events required')
    for i, ((site, rva), e) in enumerate(zip(SITES.items(), events), 1):
        require(e.get('site') == site and number(e.get('rva')) == rva and e.get('sequence') == i, 'event order/site mismatch')
        require(number(e.get('thread_id', 0)) > 0, 'missing thread')
        require(e.get('thread_id_source', '').startswith(('debug_event_api:', 'callback_api_unverified:')), 'missing thread provenance')
    enq, queued, consumed, context = events
    req = Request.from_hex(enq['request_hex'])
    require(req.seed == p['target_seed'], 'seed mismatch')
    require(all(e['request_hex'].upper() == req.raw.hex().upper() for e in events), 'request changed in transit')
    route = classify_request_source(number(enq['return_rva']), req.raw)
    require(enq['route'] == route, 'route mislabelled')
    request_address = number(enq['request_address'])
    caller_rsp = number(enq['caller_rsp'])
    caller_rbp = number(enq['caller_rbp'])
    expected_request = {
        'owned_scroll_branch': caller_rbp - 0x50,
        'session_view_branch': caller_rbp - 0x50,
        'parameterized_session_branch': caller_rsp + 0x68,
        'current_session_requeue': caller_rbp - 0x29,
    }[route]
    require(request_address == expected_request, 'producer-local request identity mismatch')
    require(queued['thread_id'] == enq['thread_id'], 'producer thread mismatch')
    qi = number(queued['queue_index'])
    require(0 <= qi < 3 and number(queued['queue_node']) == base+0x45B83E0+qi*0x58,
            'not a valid queued native node')
    require(number(queued['request_address']) == number(queued['queue_node'])+0x20
            and number(queued['source_address']) == number(enq['request_address'])
            and number(queued['queue_rsp']) + 0x658 == caller_rsp
            and number(queued['mission_type']) == 0xCC96, 'queue input/output identity mismatch')
    require(number(consumed['request_address']) == base+0x45B8400
            and number(consumed['return_rva']) == 0x2237978, 'not the mission-entry consumer')
    require(context['thread_id'] == consumed['thread_id'], 'consumer thread mismatch')
    frame = number(context['parent_frame'])
    require(frame+0x1778 == number(consumed['consumer_rsp']), 'wrong generator parent frame')
    require([number(v) for v in context['ancestry_rvas']] == [0x20E1943,0x20E19C2,0x2237978], 'wrong ancestry')
    require(context['seed'] == req.seed and context['playthrough'] == req.raw[7]
            and context['extra_generation'] == req.raw[9], 'generator input projection mismatch')
    row = context['context']
    raw = bytes.fromhex(row['raw_hex'])
    require(len(raw) == 0x30 and raw[0x28] == context['context_key']
            and raw[0x29] == row['path'] and list(raw[0x2A:0x2F]) == row['counts'], 'context bytes/interpretation differ')
    require(0 <= row['row_index'] < row['row_count'] <= 256
            and number(row['address']) == number(row['store'])+8+row['row_index']*0x30, 'context row geometry')
    if route == 'session_view_branch':
        v = enq['session']
        require(number(v['view']) == number(v['root'])+0x1010 and v['mission_type'] == 0xCC96,
                'session owner/view mismatch')
        require((v['scroll_seed'],v['metadata'],v['byte6'],v['playthrough'],int(v['byte8_source']!=0))
                == (req.seed,int.from_bytes(req.raw[4:6],'little'),req.raw[6],req.raw[7],req.raw[8]),
                'session field projection mismatch')
    placements = context['placements']
    require(placements['stride'] == 0x18 and 0 < placements['row_count'] <= 2048
            and number(placements['finish'])-number(placements['begin']) == placements['row_count']*0x18, 'placement vector geometry')
    rows = placements.get('rows_for_terrain')
    require(isinstance(rows,list) and 1 <= len(rows) <= 128, 'no placement rows')
    keys=[]
    for item in rows:
        r=PlacementRow(bytes.fromhex(item['raw_hex']));r.checked_position()
        require(r.key == (context['terrain'],item['slot']), 'placement row key mismatch')
        require(0 <= item['row_index'] < placements['row_count']
                and number(item['address']) == number(placements['begin'])+item['row_index']*0x18, 'placement address mismatch')
        keys.append(r.key)
    # Duplicate physical keys are evidence needing more analysis, not a clean join.
    require(len(set(keys)) == len(keys), 'ambiguous placement key')
    return {'upstream_link_validated':True, 'producer_route':route,
            'literal_writer_linked':route != 'current_session_requeue',
            'request_extra':req.raw[9], 'padding_observed':req.padding_observed.hex(),
            'context_key':context['context_key'], 'path':row['path'], 'placement_rows':len(rows),
            'physical_actor_join_validated':False, 'product_oracle_accepted':False,
            'target_thread_id_from_debug_api':all(e['thread_id_source'].startswith('debug_event_api:') for e in events),
            'live_status':'real live evidence only if the input files were captured in CE, not synthetic fixtures'}


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('capture',type=Path);ap.add_argument('cleanup',type=Path)
    args=ap.parse_args()
    print(json.dumps(validate(json.loads(args.capture.read_text()),json.loads(args.cleanup.read_text())),indent=2))

if __name__=='__main__':main()
