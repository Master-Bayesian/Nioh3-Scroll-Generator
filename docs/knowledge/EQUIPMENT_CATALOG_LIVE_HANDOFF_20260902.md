# Equipment catalog live-verification handoff — 2026-09-02

This note preserves the exact state of the community equipment-effect supplement
and full-item catalog work. It is not a release note. No game-memory address in
this document is stable across a game restart.

## Delivered workbooks

- `deliverables/catalogs/仁王3词条装备库v2.1_完整缺漏补充表_PC_v2.01_20260902.xlsx`
  - 360 deduplicated PC v2.01 supplement candidates.
  - 49 ordinary-category candidates and 311 promoted/star candidates.
  - 5 definite corrections.
  - 841 rows from the old supplement remain explicitly pending live review.
- `deliverables/catalogs/仁王3_PC_v2.01_全物品列表_简体中文_20260902.xlsx`
  - All 3,362 native PC v2.01 item IDs are present.
  - 2,727 rows currently have a Simplified Chinese name source.
  - 635 rows remain semantically unresolved: 416 use placeholder text ID
    `0x0110475F`, 137 use text ID `0x0116C671`, 59 use zero, and only 23 use
    distinct non-placeholder text IDs that still need a runtime/menu context.

The full-item workbook is therefore complete as an ID inventory, but it is not
yet complete as a user-facing name/use catalog.

## Evidence boundary

The static PC v2.01 pool audit establishes native pool membership from item
rows, effect rows, weight slots, playthrough/rarity gates, promotion flags, and
fixed-effect fields. Live injection establishes a different fact: whether an
effect ID can be rendered and retained by a selected equipment instance.

Never relabel an injected/displayed effect as a naturally droppable effect
unless the native pool data independently supports that conclusion.

## Live result established today

- Selected item: `神通弓`, native item ID `0x3009`.
- Probe effect: `0x0000197E`, `对极近距离敌人的弓箭伤害`.
- Probe raw value: 135, displayed as `+13.5%`.
- Probe category key: `0x1B`.
- The effect was written temporarily to the first mutable slot of a persistent
  inventory copy. After selecting another item and returning to the bow, the
  game UI displayed the target name and value.
- All modified bytes were restored. The UI retained the rendered line as a
  cache after restoration; this is not a persistent save change.

This proves that the first non-view copy identified below was a source used by
the selection refresh path. It does not prove native drop legality for
`0x0000197E`.

## Session-only memory layout

These values were valid only for `Nioh3.exe` PID 47516 and must be rediscovered
after restart:

- Volatile selected-item view header: `0x000001D5082335E0`.
- Volatile selected-item first effect: header `+0x50` =
  `0x000001D508233630`.
- Persistent inventory copy header used by the successful refresh:
  `0x000001D509A5338E`.
- Persistent inventory copy first effect: header `+0x50` =
  `0x000001D509A533DE`.
- Second identical mirror header: `0x000001D5316E79A0`.
- Each effect slot is 0x18 bytes. The effect ID is at `+0x04` from the slot
  record start in the underlying equipment structure used by the inspection
  helpers; for the addresses above, the helper receives the effect-ID address
  directly.
- At an effect-ID address: ID is `+0x00` (u32 LE), raw is `+0x04` (u32 LE),
  and metadata begins at `+0x08`; metadata byte 1 low seven bits carry the
  category key.

Important failure mode: writing the selected-item view buffer and then changing
selection causes that same address to be overwritten by the newly selected
item. Restoring old bytes to the view address after a selection change can
write the wrong item's slot. Future probes must write the persistent inventory
copy and restore the view only when its effect ID still equals the temporary
probe ID.

## Durable resume package

The curated resume package is:

`deliverables/catalogs/Nioh3_PC_v2.01_catalog_resume_20260902.zip`

It contains the workbook builders, source-analysis JSON, effect-name/range
exports, item-name captures, memory scanners, inspection helpers, and the
recovering live-probe scripts used today.

## Tomorrow's first steps

1. Start the game, load the same offline backup, open the ranged-weapon inventory,
   and select the same `神通弓`.
2. Find the selected view plus persistent copies again with
   `scan_selected_bow_memory.py`; do not reuse the addresses above.
3. Confirm the inventory copy with one temporary `0x0000197E` probe and one
   selection-away/selection-back refresh.
4. Extend the persistent-copy probe to write four candidate effect slots at
   once, capture the refreshed panel, log the four expected names/values, and
   restore every 12-byte payload in a `finally` block.
5. Verify the 59 old ranged-supplement rows in about 15 four-effect batches.
   Record at least: source row, effect ID, expected Chinese name, raw value,
   category key, rendered result, reset/rejection result, and screenshot.
6. Update the supplement workbook with separate columns for native-pool evidence
   and live injection/display evidence. Do not collapse them into one status.
7. After ranged validation, select representative melee, armor, and accessory
   items for the corresponding old-supplement batches.
8. Rebuild with the artifact-tool pipeline, re-import the final workbook, scan
   formulas/errors, render representative sheets, and deliver only the final
   workbook from `deliverables/catalogs`.

## Bulk-probe correction — 2026-09-04

The earlier four-effects-per-refresh design was rejected as needlessly
repetitive. The replacement runner is
`research/run_equipment_effect_live_bulk.py`.

- Keep ten suitable ranged weapons next to one another, each with four populated
  mutable effect slots, and select the last one in the group.
- The runner first registers all ten selected-item fingerprints with scripted
  `W` navigation, returns to the starting item, and resolves all ten persistent
  inventory records inside one bounded mirror-memory scan.
- It patches all ten records before capture, then walks the same ten items once,
  captures each rebuilt detail panel, restores every modified record in a
  `finally` block, and returns to the starting item.
- Ten weapons cover 40 supplement rows. The 59 ranged rows therefore require two
  bulk rounds: batches 1-10, then batches 11-15 using five of the same weapons.
- The runner rejects items without four populated mutable slots and rejects
  duplicate discovery fingerprints before making any writes.
- The evidence boundary remains unchanged: this confirms live rendering and
  selection-refresh retention, not native drop-pool membership.

Windows Computer Use was explicitly stopped with the physical Escape key near
the end of this session. Do not issue further UI input until the user says the
game is ready again.
