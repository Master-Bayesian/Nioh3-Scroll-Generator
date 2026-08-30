# Special-rule legality and item-name status

Context: PC v2.00.02, Simplified Chinese product catalog, playthrough 3.

## Native legality gate

The captured `scroll_special_rule` table contains 301 rows. A row is exposed
as a selectable playthrough-3 rule only when both native gates pass:

- row `+0x36`, bit 0 is set;
- the playthrough-3 weight is nonzero.

Exactly 277 rows pass. The other 24 rows remain useful reverse-engineering
evidence, but they are not presented to players as legal search targets.
This removes misleading entries such as disabled `Wind Shikigami`,
`Ki Burst Talisman`, and raw `item 0x3619` variants.

## Automatic-activation item names

Parameterized automatic-activation rules use the native uint16 item key, not
the special-rule row key, to resolve the embedded Onmyo item name. Known item
keys are stored in `nioh3_scroll_editor/data/special_rule_item_names.json`.
Legal rows with an unresolved item key remain selectable, but must be labelled
as an unidentified native item together with its raw key and legality status.
They must never be silently translated or shown as a bare English fallback in
the Simplified Chinese UI.

The only currently legal unresolved automatic-activation item key is `0x3011`.

## Priority-drop Grace coverage

The native priority-drop rule family contains 60 rows: 20 Grace IDs with three
value variants each. The rarity-4 final Grace map contains 21 Grace IDs.
The set difference is exactly:

```text
0x4192  Shinatsuhiko's Grace / 志那都彦的恩宠
```

Therefore the missing priority-drop option is not a UI truncation or catalog
name bug. PC v2.00.02 has no corresponding native priority-drop rule row for
`0x4192`. The application must not fabricate a rule that the generator cannot
produce. No other rarity-4 Grace is missing from this native rule family.

## Evidence boundary

These conclusions describe the captured PC v2.00.02 parameter tables. A game
update can change row availability or weights, so a future version must rebuild
the legality set from newly captured tables instead of carrying these counts
forward unchecked.
