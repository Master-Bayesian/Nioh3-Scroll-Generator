"""Reuse legacy measured maps; every proposed seed is still native-verified."""
import os
from pathlib import Path
from dataclasses import asdict
from .app_settings import load_app_settings
from .cache_application import primary_map_cache_path, grace_map_cache_path
from .primary_map import (PrimaryFirstDrawOutputMap, PrimaryOutputMap, load_primary_map,
    save_primary_map, build_primary_first_draw_output_map, build_primary_output_map)
from .grace_map import load_grace_map_cache, save_grace_map_cache, build_live_grace_output_map


def prepare_maps(oracle, template, playthrough, rarity, level, recommended_level, criteria, context, cancelled, progress):
    root = Path(__file__).resolve().parents[1]
    state = Path(os.environ.get('NIOH3_STATE_ROOT') or load_app_settings(fallback_root=root).data_root)
    fingerprint = template['save_fingerprint']
    digest = context.context_digest
    target = criteria.get('grace_effect_id')
    primary = criteria.get('primary_effect_ids')
    result = {}
    if target is not None:
        path = grace_map_cache_path(state, save_fingerprint=fingerprint, playthrough=playthrough,
                                   rarity=rarity, generation_context_digest=digest)
        if path.is_file():
            mapping = load_grace_map_cache(path, expected_context_fingerprint=fingerprint,
                                           expected_generation_context_digest=digest)
        else:
            mapping = build_live_grace_output_map(oracle, template=bytes.fromhex(template['template_hex']),
                category=playthrough, rarity=rarity, level=level, recommended_level=recommended_level,
                cancel_event=cancelled, progress=lambda value: progress({'phase': 'capture_special_map', **asdict(value)}))
            save_grace_map_cache(path, mapping, context_fingerprint=fingerprint, generation_context_digest=digest)
        result['grace_output_map'] = mapping
    first = playthrough in (1, 2) and bool(primary) and target is None
    second = bool(primary) and target is not None and rarity == 5
    if first or second:
        path = primary_map_cache_path(state, save_fingerprint=fingerprint, playthrough=playthrough,
            rarity=rarity, grace_effect_id=target, generation_context_digest=digest)
        if path.is_file():
            mapping = load_primary_map(path, expected_context_fingerprint=fingerprint,
                                      expected_generation_context_digest=digest)
            if not isinstance(mapping, PrimaryFirstDrawOutputMap if first else PrimaryOutputMap):
                raise ValueError('Primary map kind does not match the search')
        else:
            kwargs = dict(template=bytes.fromhex(template['template_hex']), rarity=rarity, level=level,
                recommended_level=recommended_level, cancel_event=cancelled,
                progress=lambda value: progress({'phase': 'capture_primary_map', **asdict(value)}))
            mapping = build_primary_first_draw_output_map(oracle, category=playthrough, **kwargs) if first else build_primary_output_map(
                oracle, grace_effect_id=target, mapping=result['grace_output_map'], **kwargs)
            save_primary_map(path, mapping, context_fingerprint=fingerprint, generation_context_digest=digest)
        result['primary_first_output_map' if first else 'primary_output_map'] = mapping
    return result
