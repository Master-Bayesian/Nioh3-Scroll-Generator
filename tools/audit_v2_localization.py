"""Audit exact bundled locale coverage, without hiding gaps behind fallbacks."""
import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from nioh3_scroll_editor.catalog import _NATIVE_NAMES_BY_EFFECT
from nioh3_scroll_editor.auxiliary_catalog import load_auxiliary_name_catalog, _load_special_rule_item_names
from nioh3_scroll_editor.catalog_application import auxiliary_catalog


def audit():
    raw_effects = json.loads((ROOT / 'nioh3_scroll_editor/data/effect_names_multilingual.json').read_text(encoding='utf-8'))['effects']
    report = {'schema': 'nioh3-v2-localization-coverage/v1', 'locales': {},
              'scope': 'Exact bundled data coverage and stable numeric option identities; not linguistic review'}
    reference = None
    for locale in ('zh-CN', 'en-US', 'ja-JP'):
        names = load_auxiliary_name_catalog(locale)
        catalogs = [auxiliary_catalog(playthrough, locale) for playthrough in (1, 2, 3, 4, 5)]
        identities = [{
            'terrain': [(row['option_id'], row['effect_keys']) for row in catalog['terrain_options']],
            'enemies': [(row['lookup_key'], row['role']) for row in catalog['enemy_options']],
            'rules': [(row['key'], row['variant']) for row in catalog['special_rule_options']],
            'families': sorted(sorted(row['keys']) for row in catalog['special_rule_families']),
        } for catalog in catalogs]
        if reference is None: reference = identities
        missing_groups = {}
        for effect in raw_effects.values():
            if effect['names'].get(locale):
                continue
            text_id = effect.get('text_id')
            # Classification uses the native text identity and both captured names.
            placeholder = (text_id == '0x03441371'
                           and effect['names'].get('zh-CN') == '\u5907\u7528'
                           and effect['names'].get('ja-JP') == '\u30c0\u30df\u30fc')
            group = missing_groups.setdefault(text_id, {
                'text_id': text_id, 'effect_ids': [], 'known_placeholder': placeholder})
            group['known_placeholder'] = group['known_placeholder'] and placeholder
            group['effect_ids'].append(f"0x{effect['effect_id']:04X}")
        report['locales'][locale] = {
            'missing_effect_text_groups': list(missing_groups.values()),
            'effect_count': len(_NATIVE_NAMES_BY_EFFECT),
            'missing_exact_effect_names': [f'0x{key:04X}' for key, translations in _NATIVE_NAMES_BY_EFFECT.items() if not translations.get(locale)],
            'item_qualifier_count': len(_load_special_rule_item_names()),
            'missing_exact_item_qualifiers': [f'0x{key:04X}' for key, translations in _load_special_rule_item_names().items() if not translations.get(locale)],
            'empty_auxiliary_names': {kind: [key for key, row in getattr(names, kind).items() if not row.get('name')] for kind in ('terrain', 'special_rules', 'enemies')},
            'all_playthrough_option_identities_equal': identities == reference,
        }
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    result = audit()
    args.output.write_text(json.dumps(result, indent=2)+'\n', encoding='utf-8')
    print(json.dumps({locale: {key:value for key,value in row.items() if key != 'empty_auxiliary_names'} for locale,row in result['locales'].items()}))
