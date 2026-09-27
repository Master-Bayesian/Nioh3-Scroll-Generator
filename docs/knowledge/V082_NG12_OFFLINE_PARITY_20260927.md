# NG1/NG2 offline scroll generation: live native parity (PC v2.02, 2026-09-27)

Owner present, game at the title screen (pid 3404), no save access. The native
generator ran in isolated remote buffers through `nioh3_scroll_editor.native`
with `nioh3_scroll_editor/data/game_versions/pc_v2_02.json`.

## Method

Scripts (not in the reviewed public `research/` allow-list) are kept with the
evidence in `deliverables/v082-ce-research/ng12-parity/scripts/`; copy them into
`research/` of a checkout to run them.


- `dump_playthrough_native_records_live.py` generates native records
  for one playthrough/rarity/level from the NG3 capture template with its record
  type replaced (`CATEGORY_TO_TYPE`: NG1 `0x1E82`, NG2 `0x516D`). Rarity 4 also
  runs the native finalization (`complete_native_batch` of the NG3 R4 gate).
- `capture_playthrough_special_map_live.py` measures a first-u16
  special map (65536 native generations), the same loop as
  `grace_map.build_live_grace_output_map`.
- `crates/nioh3-worker/examples/playthrough_parity.rs` regenerates every record
  offline with the playthrough-generic domain paths and compares all 0xE8
  bytes. `+0x1B` (runtime header) is ignored as in the NG3 gates; rarity 5
  accepts only the documented header cap (native `+0x30/+0x31` = 4/4, offline
  5/5); the recommended level at `+0x10` is taken from the native record.

## Results

Reports: `deliverables/v082-ce-research/ng12-parity/parity-*.json`.

| Context | Seeds | Result |
| --- | --- | --- |
| NG3 R3 / R4 / R5 (controls) | 500 / 1000 / 1000 | pass |
| NG1 R3, NG2 R3 (level 180) | 10000 each | pass |
| NG1 R4, NG2 R4 stage one + finalized (level 180) | 10000 each | pass |
| NG1 R5, NG2 R5 (level 180) | 10000 each | pass (header cap only) |
| NG1/NG2 R3, R4, R5 (level 120) | 2000 each | pass |

- The captured NG3 R4 map equals the shipped `grace_output_map_e604_r4_current.json`
  range for range, although it was measured at the title screen without a save:
  the maps do not depend on the loaded save.
- The NG1 and NG2 R4 stage-one maps are identical to each other and at levels
  180 and 120 (10 ranges).
- NG1/NG2 rarity 5 has **no Grace**: a first-u16 capture of slot 6 gives 59065
  ranges over 39 ordinary effects. Native records carry six ordinary effects
  (primary in the first slot) and an empty seventh slot. The offline path
  `generate_rarity5_plain_effect_sequence` (promotion draw with slot limit 6,
  six ordinary slots from source 0, no special) matches every record. This is
  why the old capture refused NG1/NG2 rarity 5.
- At level 120 with recommended level 123 the game wrote recommended level 156
  (`0x9C`) into `+0x10/+0x12`; the effect bytes are unaffected.

## Domain changes

`record_type_for_playthrough`, `generate_rarity3_effect_sequence`,
`generate_rarity4_stage_one_effect_sequence`,
`generate_rarity5_plain_effect_sequence`, the playthrough-generic
materializers and `R4FinalizerEngine::for_playthrough`. The NG3 entry points
and their certification are unchanged; the search worker still gates NG1/NG2
until its product integration lands.
