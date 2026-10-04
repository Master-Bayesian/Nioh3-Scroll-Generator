"""Export reviewed UI strings and existing game-localized names for V2."""
import json
import re
import struct
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
RUBY = re.compile(r'\^(?:20|21)~default~|\^FE~RUBY~|\^FF~RUBY,[^~]*~', re.I)
SLOT = re.compile(r'\^09~(BUFF|DEBUFF)~\{([^}]*)\}\^09~~(状态)?')
BARE = re.compile(r'(?<!~)\{([^{}":]+)\}')
TABLES = ROOT / 'nioh3_scroll_editor/data/r4_finalizer/pc_v2_02/resource_v1/tables'
UNNAMED = {'en-US': 'Unnamed Ninjutsu (0x{:04X})', 'ja-JP': '名称のない忍術（0x{:04X}）'}


def has_slot(text):
    return bool(re.search(r'\^09~(?:BUFF|DEBUFF)~|\{\}', text))


def zh_display(name):
    """The editor's Chinese effect name (model.ts arguments, then game-text.ts fillTemplateSlots)."""
    name = RUBY.sub('', name)
    if re.search(r'\^09~(?:BUFF|DEBUFF)~\{\}|\{\}', name):
        return None  # still generic; no exact game name to pair it with
    name = SLOT.sub(lambda m: m.group(2) + (m.group(3) or ''), name)
    return BARE.sub(r'\1', name)


def captured_names(locale, effects):
    """Exact Chinese display name -> game name, from a live capture of the game in `locale`.

    The capture maps text IDs to the game's own strings; the item, effect-group
    and skill tables give each name its text ID, so every pair is the game's.
    """
    path = base / 'locale-captures' / f'{locale}.json'
    if not path.exists():
        return {}
    capture = json.loads(path.read_text(encoding='utf-8'))
    texts = capture['texts']
    text = lambda tid: texts.get(tid) if tid and tid != '0x00000000' else None
    item_name = lambda item: text(capture['items'].get(str(item)))
    arguments = json.loads((base / 'effect-arguments.json').read_text(encoding='utf-8'))
    resolved = dict(arguments['names'])
    for effect_id in arguments['unnamed_in_game']['ids']:
        resolved.setdefault(str(effect_id), f'未命名忍术（0x{effect_id:04X}）')
    item_names = json.loads((base / 'item-names.json').read_text(encoding='utf-8'))['items']
    zh_items = {}
    for item, row in item_names.items():
        zh_items.setdefault(row[0], int(item))
    table = (TABLES / 'effect.bin').read_bytes()
    group_of = {}
    for offset in range(8, len(table), 0xD8):
        effect_id, group = struct.unpack_from('<HH', table, offset)
        group_of.setdefault(effect_id, group)
    pairs = {}

    def add(zh, name):
        # Unfinished rows share the game's placeholder text ("DUMMY"); a
        # curated Chinese name is better than that.
        if zh and name and not re.search(r'DUMMY|UNUSED', name):
            # localize() collapses whitespace before it looks a name up.
            pairs.setdefault(re.sub(r'\s+', ' ', zh), RUBY.sub('', name))

    # Effects: the editor's name, with the argument the game would fill in.
    curated = {}
    for context in source['contexts'].values():
        for row in context['effects'] + context['graces']:
            if not has_slot(row['name']):
                curated[row['id']] = row['name']
    for row in effects:
        effect_id = int(row['id'])
        zh = row['name']
        if has_slot(zh) and row['id'] in curated:
            zh = curated[row['id']]
        if row['id'] in resolved:
            zh = zh.replace('{}', '{' + resolved[row['id']] + '}')
        zh = zh_display(zh)
        group = capture['groups'].get(str(group_of.get(effect_id)))
        name = group and text(group['name'])
        if not zh or not name:
            continue
        if has_slot(name):
            if effect_id in arguments['unnamed_in_game']['ids']:
                argument = UNNAMED[locale].format(effect_id)
            elif re.search(r'\^09~(?:BUFF|DEBUFF)~', name):
                argument = text(group['arg'])
            else:
                subjects = capture['subjects'].get(str(int(group['subject'], 16)), [])
                argument = (text(subjects[0]) if subjects else None) or item_name(int(group['item'], 16))
                if not argument:
                    zh_argument = resolved.get(row['id'], '')
                    argument = item_name(zh_items.get(zh_argument)) or ui.get(zh_argument, [None, None])[locale == 'ja-JP']
            if not argument:
                continue
            name = re.sub(r'\^09~(?:BUFF|DEBUFF)~\{\}\^09~~', lambda _: argument, name).replace('{}', argument)
        add(zh, name)
    # Items, with soul cores built as the game builds them ("{} Soul Core").
    cores = json.loads((ROOT / 'test_fixtures/soul_core_yokai.json').read_text(encoding='utf-8'))['cores']
    core_format = text('0x010D29F1')
    for item, row in item_names.items():
        core = cores.get(item)
        if core and not core.get('placeholder') and core_format:
            add(row[0], core_format.replace('{}', core[locale]))
        else:
            add(row[0], item_name(item))
    hell = json.loads((base / 'hell-skill-names.json').read_text(encoding='utf-8'))['skills']
    for skill, zh in hell.items():
        add(zh, text(capture['skills'].get(skill)))
    return pairs


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
    for key, value in captured_names(locale, effects).items():
        translated.setdefault(key, value)
    games[locale] = {key: re.sub(r'\^(?:20|21)~default~|\^FE~RUBY~|\^FF~RUBY,[^~]*~', '', value, flags=re.I)
                     for key, value in translated.items()}
(base / 'ui-locales.json').write_text(json.dumps({'ui': ui, 'game': games}, ensure_ascii=False, indent=2)+'\n', encoding='utf-8', newline='\n')
print(f'Exported {len(ui)} UI messages and {sum(map(len, games.values()))} game names')
