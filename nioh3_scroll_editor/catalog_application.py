"""Layout-independent option identities and exact terrain row selections."""
from dataclasses import asdict
from functools import lru_cache
import struct

from .auxiliary_catalog import load_auxiliary_name_catalog
from .auxiliary_generation import (load_default_auxiliary_generation_tables, TERRAIN_DISPLAY_CRUCIBLE_KEY,
    TERRAIN_DISPLAY_SPECIAL_KEYS, legal_special_rule_keys, describe_special_rule)
from .runtime_auxiliary_override import _enemy_role_by_lookup_key
from .presentation_strings import label
from .recommended_level import native_recommended_level_curve, resolve_recommended_level


def recommended_level_metadata():
    """Compact input semantics; this is curve prediction, not in-game acceptance."""
    curve = native_recommended_level_curve()
    minimum_displayed, maximum_displayed = curve.displayed_level_bounds()
    return {
        'minimum_internal_level': curve.minimum_internal_level,
        'maximum_internal_level': curve.maximum_internal_level,
        'minimum_displayed_level': minimum_displayed,
        'maximum_displayed_level': maximum_displayed,
        'selection_policy': 'lowest_canonical_internal_level',
        'evidence': 'captured_native_curve_prediction',
    }


def resolve_recommended_level_payload(displayed_level):
    result = asdict(resolve_recommended_level(displayed_level))
    result['canonical_internal_levels'] = list(result['canonical_internal_levels'])
    return {**result, 'metadata': recommended_level_metadata()}


@lru_cache(maxsize=1)
def terrain_choices():
    tables = load_default_auxiliary_generation_tables()
    combinations = {}
    for index, row in enumerate(tables.terrain.rows()):
        keys = []
        if struct.unpack_from('<H', row, 0x2C)[0]:
            keys.append(TERRAIN_DISPLAY_CRUCIBLE_KEY)
        special = TERRAIN_DISPLAY_SPECIAL_KEYS.get(row[0x30])
        if special is not None:
            keys.append(special)
        combinations.setdefault(tuple(keys), set()).add(index)
    choices = {('exact:' + ','.join(f'{key:X}' for key in keys)): (keys, frozenset(rows), False)
               for keys, rows in combinations.items()}
    for key in sorted({key for keys in combinations for key in keys}):
        matching = [rows for keys, rows in combinations.items() if key in keys]
        if len(matching) > 1:
            choices[f'contains:{key:X}'] = ((key,), frozenset().union(*matching), True)
    return choices


def resolve_terrain_selections(selection_ids):
    choices = terrain_choices()
    if any(value not in choices for value in selection_ids):
        raise ValueError('Unknown terrain option; refresh the context-bound catalog')
    return frozenset().union(*(choices[value][1] for value in selection_ids))


def auxiliary_catalog(playthrough, locale):
    names = load_auxiliary_name_catalog(locale)
    terrains = []
    for option_id, (keys, _rows, aggregate) in terrain_choices().items():
        display = ' + '.join(names.terrain_effect_name(key) for key in keys) or label(locale, 'no_terrain')
        if aggregate:
            display = label(locale, 'contains', name=display)
        terrains.append({'option_id': option_id, 'name': display, 'effect_keys': list(keys), 'aggregate': aggregate})
    enemies = [{'lookup_key': key, 'role': role, 'name': names.enemy_name(key)}
               for key, role in sorted(_enemy_role_by_lookup_key().items())]
    rules = []
    for key in sorted(legal_special_rule_keys(playthrough)):
        detail = describe_special_rule(key)
        rules.append({'key': key, 'name': names.special_rule_name(key), 'variant': asdict(detail)})
    # Preserve native family grouping without inventing semantic categories.
    families = [{'name': name, 'keys': sorted(keys)} for name, keys in names.special_rule_key_groups(allowed_keys=legal_special_rule_keys(playthrough)).items()]
    return {'terrain_options': terrains, 'enemy_options': enemies, 'special_rule_options': rules, 'special_rule_families': families}
