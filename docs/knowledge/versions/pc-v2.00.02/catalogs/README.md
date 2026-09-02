# Versioned localization and native-key catalogs

The machine-readable registry is [catalog-manifest.json](catalog-manifest.json).
It records hashes, coverage, source paths, and semantic boundaries for every
catalog used by PC v2.00.02.

## Final effect catalog

Canonical asset:
`nioh3_scroll_editor/data/effect_names_multilingual.json`

- 3,609 native effect keys;
- Simplified Chinese and Japanese names for all 3,609;
- English names for 3,604;
- names are indexed by stable native IDs/text IDs and include provenance;
- R4 stage-one slot-5 tokens are explicitly outside the final-effect namespace.

Unknown or missing names remain numeric IDs. They must never be guessed from a
different slot, stage, rarity, or old Cheat Engine list.

## Auxiliary catalog

Canonical assets:

- `nioh3_scroll_editor/data/auxiliary_names/zh-CN.json`
- `nioh3_scroll_editor/data/auxiliary_names/ja-JP.json`
- `nioh3_scroll_editor/data/auxiliary_names/en-US.json`

Each contains 238 display-terrain rows, 301 ordered special-rule keys, and 960
enemy lookup keys from the native localization pool. The 20-row generator
terrain table is not the same namespace as the 238-row display catalog.

Known localization gaps are retained as gaps: some display-terrain rows have no
name, one special-rule name is missing in English/Japanese, and item-qualified
special rules still have incomplete Chinese/Japanese item qualifiers.

## Enemy role catalog

- [Grouped Markdown](enemy-roles.md)
- [Complete JSON](enemy-roles.json)
- [Complete CSV](enemy-roles.csv)

These files join all 487 native enemy-candidate rows to three-language native
names without collapsing raw lookup-key variants. Regenerate them with:

```powershell
python tools/export_enemy_role_catalog.py
```

The export preserves the complete `0x1C` raw row so future field discoveries do
not require reconstructing old capture bytes.

## Player enemy-combination catalog

- [中文版玩家指南](enemy-combinations.zh-CN.md)
- [Player guide and trilingual family table](enemy-combinations.md)
- [Machine-readable combination rules](enemy-combinations.json)
- [Spreadsheet-friendly player catalog](enemy-combinations.csv)
- [Native names unavailable to the scroll generator](enemy-unavailable.csv)

These files translate the 487 raw candidate rows into 148 player-visible name
entries and the O/A/B family rules used for structural preflight. They preserve
candidate keys because a localized display name is not a globally unique enemy
identity. Known same-name human/yokai and character-identity variants are
qualified from the shared `nioh3_scroll_editor/enemy_variants.py` registry.
Regenerate them after the role catalog with:

```powershell
python tools/export_enemy_combination_guide.py
```

`structurally compatible` is deliberately weaker than `proven legal`. The
former means that at least one native branch class can fit the requested role
families; the latter requires exact forward replay under the selected
playthrough, terrain, parameter gates, budgets, linked groups, and RNG path.
