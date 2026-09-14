"""Validate the four-site causal capture. Never substitutes five late snapshots
for an independently captured producer trace. Unknown or incomplete == rejected.
This validator is offline and reads only the two explicitly provided JSON files.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
from assignment_origin_reference import PreparedDescriptor, assign_prepared, threshold_4543

EXPECTED_SHA = '4047ECB623F3AE033F7D6F3637CD1C5408C72A89A038463268A7C9AC5C394159'
SITES = {'origin_entry': 0x10283C0, 'trial_decision': 0x1028570,
         'task_copy': 0x1BF2D54, 'task_linked': 0x1C24662}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def unwrap(x):
    if isinstance(x, dict) and 'bridge_result' in x:
        x = x['bridge_result']
    if isinstance(x, dict) and 'result' in x:
        x = x['result']
    if isinstance(x, dict) and 'value' in x:
        x = x['value']
    require(isinstance(x, dict), 'invalid bridge result')
    return x


def raw_descriptor(d):
    raw = bytes.fromhex(d['raw_hex'])
    require(len(raw) == 20, 'invalid descriptor length')
    require(int.from_bytes(raw[:4], 'little') == d['spawn'], 'spawn/raw mismatch')
    require(int.from_bytes(raw[4:8], 'little') == d['lookup'], 'lookup/raw mismatch')
    require(raw[15] == d['flag'] and raw[16] == d['selector_class'], 'flags/raw mismatch')
    return raw


def prepared(origin):
    config = origin['tables']['config_4543']
    require(type(config.get('present')) is bool, 'config presence unknown')
    c = bytes.fromhex(config['raw_hex']) if config['present'] else None
    require(c is None or len(c) == 32, 'truncated config row')
    threshold = threshold_4543(c)
    out = []
    coords = set()
    for d in origin['descriptors']:
        raw = raw_descriptor(d)
        require(raw[15] == 0, 'source already assigned before observed origin')
        g = d['eligibility']
        require(type(g.get('enemy_row_present')) is bool, 'enemy row presence unknown')
        sr, flags = False, 0
        if g['enemy_row_present']:
            require(type(g.get('subtype_row_present')) is bool, 'subtype presence unknown')
            sr = g['subtype_row_present']
            if sr:
                s = bytes.fromhex(g['subtype_raw_hex'])
                require(len(s) == 0x54 and s[0x14] == g['flags14'], 'subtype row mismatch')
                flags = g['flags14']
        coordinate = (d['wave_index'], d['position'])
        require(coordinate not in coords, 'duplicate descriptor coordinate')
        coords.add(coordinate)
        out.append((coordinate, PreparedDescriptor(raw, g['enemy_row_present'], sr, flags, threshold)))
    require(out and len(out) <= 96, 'empty/oversized source vector')
    require([c for c, d in out] == sorted(c for c, d in out), 'source vector order lost')
    waves = []
    for (w, p), d in out:
        while len(waves) <= w:
            waves.append([])
        require(len(waves[w]) == p, 'descriptor positions not contiguous')
        waves[w].append(d)
    return waves


def validate(capture: dict, cleanup: dict) -> dict:
    p = unwrap(capture)
    require(p.get('schema') == 'nioh3.assignment-origin.v1', 'wrong observer schema')
    require(p.get('read_only') is True and p.get('writes_game_memory') is False, 'wrong observer mode')
    require(p.get('active') is False and not p.get('error'), 'observer active or failed')
    require(p.get('stop_reason') == 'all_generated_tasks_linked', 'incomplete capture stop')
    ident = p['identity']
    require(ident.get('executable_sha256') == EXPECTED_SHA and ident.get('image_size') == 77814240,
            'wrong executable identity')
    require(isinstance(ident.get('creation_filetime'), str) and ident['creation_filetime'].isdigit(),
            'process creation identity missing')
    q = cleanup.get('cleanup_metadata', {})
    require(q.get('verified') is True and q.get('run_id') == p['run_id'], 'cleanup unverified/mismatched run')
    require(q.get('target') == ident, 'cleanup target differs')
    cp = unwrap(cleanup)
    require(cp['probe'].get('active') is False and cp['probe'].get('cleanup_pending') is False,
            'cleanup still active/pending')
    require(cp['probe'].get('owned_breakpoints') in ([], {}), 'owned breakpoint inventory is unknown or nonempty')
    require(cp.get('breakpoints') in ([], {}), 'breakpoint inventory is unknown or nonempty')
    require(cp['probe'].get('run_id') == p['run_id'], 'cleanup probe owner differs')
    require(cp['probe'].get('schema') == p['schema'], 'cleanup belongs to a different observer schema')
    es = p.get('events', [])
    require(4 <= len(es) <= 256, 'empty/truncated/oversized event vector')
    require([e['sequence'] for e in es] == list(range(1, len(es) + 1)), 'noncontiguous event order')
    require(all(e['site'] in SITES and int(e['rva'],16) == SITES[e['site']] for e in es), 'wrong event site')
    require(len({e['thread_id'] for e in es}) == 1 and es[0]['thread_id'] > 0, 'mixed/unknown debug-event threads')
    origins = [e for e in es if e['site'] == 'origin_entry']
    require(1 <= len(origins) <= 2 and [e['selector'] for e in origins] in ([0], [0,1]), 'invalid selector pass order')
    a = origins[0]
    require(a is es[0] and a['seed'] == p['target_seed'], 'missing first origin or seed mismatch')
    require(a['parent_returns'] == ['0x20E1943','0x20E19C2','0x2237978'], 'origin ancestry unproven')
    context_row = bytes.fromhex(a['generator_context_row']['raw_hex'])
    require(len(context_row) == 0x30, 'invalid generator context row')
    require(a['generator_context_row']['path'] == context_row[0x29], 'generator context path mismatch')
    require(a['generator_context_row']['configured_counts'] == list(context_row[0x2A:0x2F]),
            'generator configured counts mismatch')
    context_table = a['tables']['generator_context']
    require(context_table['key'] == a['context_key'], 'generator context table key mismatch')
    require(context_table['address'] == a['generator_context_row']['address'],
            'generator context table row mismatch')
    require(a['rng']['route'] in ('primary_scoped','secondary_scoped','fallback_global'), 'unexpected RNG route')
    parent_rng_address = int(a['parent_frame'],16) + 0xC0
    require(int(a['parent_rng']['address'],16) == parent_rng_address, 'parent RNG address mismatch')
    require(type(a['parent_rng']['state']) is int, 'parent RNG state missing')
    require(a['rng_scope_matches_parent'] is (int(a['rng']['address'],16) == parent_rng_address),
            'RNG scope comparison mismatch')
    require(a['rng']['current_thread'] == a['thread_id'], 'RNG routing thread differs from event thread')
    waves = prepared(a)
    # The live v2.01 capture proves the generator-local state at frame+0xC0:
    # every recorded post-draw state and ticket follows it exactly. The
    # thread-owner/global route diagnostic remained constant and is not the
    # assignment helper's consumed stream.
    simulation = assign_prepared(waves, a['parent_rng']['state'], a['allow_class1'] != 0)
    expected_trials = [e for e in simulation['events'] if e['kind'] == 'trial']
    trials = [e for e in es if e['site'] == 'trial_decision']
    require(len(trials) == len(expected_trials), 'missed/extra native LCG trial')
    reached_selectors = {e['selector'] for e in simulation['events']}
    require([e['selector'] for e in origins] == sorted(reached_selectors), 'missing/extra origin pass')
    if len(origins) == 2:
        b = origins[1]
        require(b['descriptors'] == a['descriptors'] and b['tables'] == a['tables'], 'pool/config changed between passes')
        require(b['parent_frame'] == a['parent_frame'] and b['seed'] == a['seed'], 'different generator invocation')
        before_class1 = a['parent_rng']['state']
        for t in expected_trials:
            if t['selector'] == 0:
                before_class1 = t['state_after']
        require(b['parent_rng']['state'] == before_class1, 'RNG reset/extra draw between passes')
    initial = {d['address']: d for d in a['descriptors']}
    require(len(initial) == len(a['descriptors']), 'duplicate source pointers')
    require(len({d['spawn'] for d in initial.values()}) == len(initial), 'duplicate spawn identifiers')
    for t, x in zip(trials, expected_trials):
        d = t['descriptor']; raw_descriptor(d)
        require(d['address'] in initial and d['raw_hex'] == initial[d['address']]['raw_hex'], 'wrong source during trial')
        require((t['selector'],d['spawn'],d['wave_index'],d['position']) ==
                (x['selector'],x['spawn'],x['wave_index'],x['position']), 'trial order/eligibility mismatch')
        require(t['rng']['address'] == a['rng']['address'] and t['rng']['route'] == a['rng']['route'], 'RNG owner changed')
        require(t['rng']['current_thread'] == t['thread_id'], 'trial RNG routing thread mismatch')
        require(int(t['parent_rng']['address'],16) == parent_rng_address, 'trial parent RNG address mismatch')
        require((t['parent_rng']['state'],t['ticket'],t['threshold'],t['branch_will_set']) ==
                (x['state_after'],x['ticket'],x['threshold'],x['accepted']), 'LCG/threshold/branch mismatch')
        require(t['source_flag_before'] == 0, 'flag already set before branch')
        oe = next(e for e in origins if e['selector'] == t['selector'])
        require(t['sequence'] > oe['sequence'], 'trial precedes its entry')
        if t['selector'] == 0 and len(origins)==2:
            require(t['sequence'] < origins[1]['sequence'], 'class0 trial after class1 entry')
    copies = [e for e in es if e['site']=='task_copy']
    links = [e for e in es if e['site']=='task_linked']
    require(len(copies)==len(initial)==len(links), 'missing/duplicate object-copy/link events')
    copy_by = {e['source']['address']:e for e in copies}
    link_by = {e['source']['address']:e for e in links}
    require(set(initial)==set(copy_by)==set(link_by), 'copy/link source set mismatch')
    final_by = {}
    for d in initial.values():
        expected = simulation['output_descriptors'][d['wave_index']][d['position']]
        co, li = copy_by[d['address']], link_by[d['address']]
        require(co['source']['raw_hex'].lower()==expected and li['source']['raw_hex'].lower()==expected,
                'upstream flag/descriptor does not match assignment')
        raw_descriptor(co['source']);raw_descriptor(li['source'])
        require(co['return_rva']=='0x1C24635', 'constructor caller differs')
        require(co['sequence']>max(e['sequence'] for e in origins+trials) and li['sequence']>co['sequence'],
                'copy temporal provenance broken')
        require(li['manager_membership'] is True and li['copy_seen'] is True, 'persistent task not owned/fresh')
        require(li['source0_null'] is True and li['source18_null'] is True, 'task base shape differs')
        require(li['descriptor_hex'].lower()==expected, 'typed copy parity mismatch')
        require(li['flag8f']==bytes.fromhex(expected)[15], 'task flag mismatch')
        identity_bytes=bytes.fromhex(li['task_identity_hex'])
        require(len(identity_bytes)==12 and int.from_bytes(identity_bytes[:4],'little')==d['spawn'] and
                int.from_bytes(identity_bytes[4:8],'little')==0xCC96 and
                int.from_bytes(identity_bytes[8:12],'little')==d['lookup'],'persistent task identity/raw mismatch')
        require(0 < li['vector_count'] <=1024,'bad owner vector count')
        final_by[str(d['spawn'])]=li['flag8f']
    return {'validated':True,'scope':'one native prepared-descriptor assignment and typed-copy path only',
            'run_id':p['run_id'],'seed':a['seed'],'trials':len(trials),'tasks':len(links),
            'selected_spawn':simulation['selected_spawn'],'state_at_entry':a['parent_rng']['state'],
            'state_after':simulation['state'],'flags8f':final_by,
            'confirmed_rng_source':'generator parent frame +0xC0',
            'rng_route_diagnostic':a['rng']['route'],
            'rng_route_diagnostic_matches_parent':a['rng_scope_matches_parent'],
            'falsified_claims':[],
            'does_not_validate':['upstream seed-to-prepared-state replay','visual actor join',
                                 'trainer-free behavior','product forward/inverse oracle','E9 MT ordering']}


def main():
    pa=argparse.ArgumentParser(description=__doc__)
    pa.add_argument('capture',type=Path);pa.add_argument('--cleanup',type=Path,required=True)
    args=pa.parse_args()
    try:
        r=validate(json.loads(args.capture.read_text(encoding='utf-8')),
                   json.loads(args.cleanup.read_text(encoding='utf-8')))
    except (ValueError,KeyError,TypeError,IndexError) as exc:
        print(json.dumps({'validated':False,'error':str(exc)}));raise SystemExit(1)
    print(json.dumps(r,indent=2))

if __name__=='__main__':main()
