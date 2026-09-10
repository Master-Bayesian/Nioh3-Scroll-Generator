"""Export reviewed UI strings and existing game-localized names for V2."""
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from nioh3_scroll_editor.catalog import native_effect_name
from nioh3_scroll_editor.auxiliary_catalog import load_auxiliary_name_catalog
from nioh3_scroll_editor.catalog_application import auxiliary_catalog

base = ROOT / 'apps/workshop'
source = json.loads((base / 'catalog.json').read_text(encoding='utf-8'))
ui = {}
for line in (base / 'ui-translations.tsv').read_text(encoding='utf-8').splitlines():
    key, english, japanese = line.split('\t')
    if key in ui:
        raise ValueError('Duplicate UI source key: ' + key)
    ui[key] = [english, japanese]
games = {}
for locale in ('en-US', 'ja-JP'):
    translated = {}
    names = load_auxiliary_name_catalog(locale)
    effects = source['editorEffects'] + [row for context in source['contexts'].values() for row in context['effects'] + context['graces']]
    for row in effects:
        name = native_effect_name(int(row['id']), locale)
        if int(row['id']) == 47147:
            name = 'Increased Damage Taken (Winded Enemy)' if locale == 'en-US' else '敵の気力切れで被ダメージ増加'
        if name and any(token in name for token in ('~BUFF~', '~DEBUFF~', '{}', '^09')):
            name = ('Effect ' if locale == 'en-US' else '効果 ') + f"0x{int(row['id']):04X}"
        if name:
            translated[row['name']] = name
    for row in source['enemies']:
        translated[row['name']] = names.enemy_name(row['keys'][0])
    for row in source['rules']:
        translated[row['name']] = names.special_rule_name(row['keys'][0])
    for row in auxiliary_catalog(3, locale)['terrain_options']:
        original = next((r for r in source['terrains'] if r['option_id'] == row['option_id']), None)
        if original:
            translated[original['name']] = row['name']
    games[locale] = translated
(base / 'ui-locales.json').write_text(json.dumps({'ui': ui, 'game': games}, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
print(f'Exported {len(ui)} UI messages and {sum(map(len, games.values()))} game names')
