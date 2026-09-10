"""Private stdio worker: uint32-LE byte length followed by UTF-8 JSON.

Only offline search is exposed. This process owns no save writer or game handle.
"""
from __future__ import annotations

import sys

from .worker_transport import read_exact, read_frame, write_frame
from .search_jobs import SearchJobs
from .worker_contracts import CONTRACT_DIGEST, MAX_FRAME_BYTES, RequestError, validate_request








def handshake(jobs):
    from .seed_accelerator import cuda_seed_acceleration_available
    from .effect_preimage_accelerator import d3d11_effect_acceleration_available
    return {
        'protocol': 1, 'contract_digest': CONTRACT_DIGEST, 'role': 'offline_search',
        'context': jobs.service.context.to_payload(),
        'capabilities': {
            'playthroughs': [3], 'rarities': [3, 4, 5],
            'cached_rarity5_playthroughs': [4, 5],
            'cuda_pivot_and_auxiliary': cuda_seed_acceleration_available(),
            'directcompute_effect_filter': d3d11_effect_acceleration_available(),
            'cpu_exact_replay': True, 'bulk_cpu_requires_opt_in': True,
            'save_write': False, 'runtime_calls': False,
        },
    }


def main():
    source, sink = sys.stdin.buffer, sys.stdout.buffer
    # Libraries' Python prints are diagnostics, never protocol bytes.
    sys.stdout = sys.stderr
    jobs = SearchJobs()
    negotiated = False
    try:
        while True:
            payload = read_frame(source)
            if payload is None:
                break
            request_id = payload.get('id') if isinstance(payload, dict) else None
            shutdown = False
            try:
                validate_request(payload)
                method, params = payload['method'], payload['params']
                if method == 'handshake':
                    result = handshake(jobs)
                    negotiated = True
                elif not negotiated:
                    raise RequestError('HANDSHAKE_REQUIRED', 'Negotiate before sending commands')
                elif method == 'search.catalog':
                    from .catalog import searchable_scroll_effect_definitions, native_effect_name, R4_FINAL_GRACE_IDS
                    from .grace_map import load_grace_output_map
                    rarity, locale = params['rarity'], params['locale']
                    ordinary = searchable_scroll_effect_definitions(3, rarity)
                    mapping = load_grace_output_map(rarity=rarity) if rarity in (4, 5) else None
                    grace_ids = sorted({entry.grace_id for entry in mapping.ranges
                                        if rarity != 4 or entry.grace_id in R4_FINAL_GRACE_IDS}) if mapping is not None else []
                    result = {
                        'context_digest': jobs.service.context.context_digest,
                        'ordinary_effects': [{'effect_id': effect.effect_id, 'name': effect.name if locale == 'zh-CN' else native_effect_name(effect.effect_id, locale) or effect.name} for effect in ordinary],
                        'grace_effects': [{'effect_id': effect_id, 'name': native_effect_name(effect_id, locale) or f'0x{effect_id:04X}'} for effect_id in grace_ids],
                    }
                    from .catalog_application import auxiliary_catalog, recommended_level_metadata
                    result.update(auxiliary_catalog(3, locale))
                    result['recommended_level'] = recommended_level_metadata()
                elif method == 'recommended_level.resolve':
                    from .catalog_application import resolve_recommended_level_payload
                    # JSON Schema integers include integral JSON numbers such as 350.0.
                    result = resolve_recommended_level_payload(int(params['displayed_level']))
                elif method == 'search.start':
                    result = jobs.start(params)
                elif method == 'cache.register':
                    result = jobs.register_cache(params['cache_json'])
                elif method == 'candidate.preview':
                    from dataclasses import replace
                    from .models import ScrollCandidate
                    from .effect_sequence import generate_ng3_certified_effect_sequence
                    from .auxiliary_generation import generate_complete_auxiliary
                    from .candidate_transfer import export_candidate
                    from .worker_contracts import candidate_payload
                    from .search_application import require_search_candidate_ready
                    candidate = ScrollCandidate.from_effect_sequence(generate_ng3_certified_effect_sequence(params['seed'], rarity=params['rarity'], level=params['level']))
                    candidate = replace(candidate, auxiliary=generate_complete_auxiliary(params['seed'], 3))
                    require_search_candidate_ready(candidate)
                    result = {'candidate': candidate_payload(candidate, jobs.service),
                              'transfer': export_candidate(candidate, jobs.service.context.context_digest, params['level'])}
                elif method == 'candidate.export':
                    result = jobs.export(params['job_id'], params['candidate_id'])
                elif method == 'job.snapshot':
                    result = jobs.snapshot(params['job_id'])
                elif method == 'job.current':
                    result = jobs.current()
                elif method == 'job.cancel':
                    result = jobs.cancel(params['job_id'])
                else:
                    jobs.shutdown()
                    result, shutdown = {'stopped': True}, True
                response = {'protocol': 1, 'id': request_id, 'ok': True, 'result': result}
            except (ValueError, RuntimeError) as error:
                response = {'protocol': 1, 'id': request_id, 'ok': False,
                            'error': {'code': getattr(error, 'code', 'INVALID_REQUEST'), 'message': str(error)}}
            write_frame(sink, response)
            if shutdown:
                break
    finally:
        jobs.shutdown()
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
