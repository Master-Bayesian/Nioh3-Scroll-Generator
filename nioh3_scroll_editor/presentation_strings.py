"""Small locale-aware catalog labels; never use localized text as an identity."""
LABELS = {
    'en-US': {'none': 'None', 'no_terrain': 'No terrain effects', 'contains': 'Contains {name}',
              'unknown_terrain': 'Unknown terrain row {value}', 'unknown_terrain_effect': 'Unknown terrain effect {value}',
              'unknown_rule': 'Unknown rule {value}', 'unknown_enemy': 'Unknown enemy {value}', 'item': 'Item {value}'},
    'zh-CN': {'none': '无', 'no_terrain': '无地形效果', 'contains': '包含{name}',
              'unknown_terrain': '未知地形行 {value}', 'unknown_terrain_effect': '未知地形效果 {value}',
              'unknown_rule': '未知规则 {value}', 'unknown_enemy': '未知敌人 {value}', 'item': '道具 {value}'},
    'ja-JP': {'none': 'なし', 'no_terrain': '地形効果なし', 'contains': '{name}を含む',
              'unknown_terrain': '不明な地形行 {value}', 'unknown_terrain_effect': '不明な地形効果 {value}',
              'unknown_rule': '不明なルール {value}', 'unknown_enemy': '不明な敵 {value}', 'item': 'アイテム {value}'},
}


def label(locale, key, **values):
    return LABELS.get(locale, LABELS['en-US'])[key].format(**values)
