# Rarity-4 Final Grace Prediction

## Scope

This note describes the certified PC v2.00.02, NG3 (`record_type=0xE604`),
rarity-4 path. It predicts the completed record shown after the challenge, not
the transient stage-one record.

## Exact result

The finalizer does not draw a second Grace. Rarity 4 has exactly two possible
final Grace outcomes:

1. the stage-one Grace in physical effect slot 5 is retained; or
2. physical effect slot 5 is completed and replaced by an ordinary effect, so
   the final record has no Grace.

It cannot change one Grace into another Grace.

## Forward prediction

For a displayed 32-bit Seed `seed`:

```text
state1 = (seed * 0x00010DCD + 1) mod 2^32
draw1  = state1 >> 16
stage_grace = rarity4_draw1_map[draw1]
stage_record = GenerateR4StageOne(seed, stage_grace)
completion = FinalizeR4(stage_record)

if completion.accepted_index == 4:
    final_grace = none
else:
    final_grace = stage_grace
```

The first-draw map is the complete 65,536-bucket partition in
`nioh3_scroll_editor/data/grace_output_map_e604_r4_current.json`.

`FinalizeR4` must replay the whole completion path. Its scoped RNG Seed
depends on every stage-one effect ID and percentile roll:

```text
S_i = displayed_seed
    + salt16
    + rarity * salt16 * (displayed_seed >> 16)
    + 7 * (target_index << 16)
    + sum(int32(raw_id[j]) * min(roll[j], 100))
    mod 2^32
```

It then discards `(target_index + salt16) & 31` LCG draws and runs category
assignment, candidate filtering, conflict and quota checks, weighted selection,
retry, percentile roll, and completion acceptance. Therefore final Grace
retention is deterministic but is not a single fixed-draw interval by itself.

The public offline API is:

```python
from nioh3_scroll_editor.effect_sequence import (
    predict_ng3_rarity4_final_grace,
)

prediction = predict_ng3_rarity4_final_grace(183696634)
```

It returns the draw-1 bucket, stage-one Grace, final Grace or `None`, accepted
target index, attempted indexes, selected effect IDs, and both stage-one and
final effect-ID sequences.

## Inverse prediction

For a requested Grace `G`, the complete inverse is:

```text
candidate_seeds = inverse_lcg(draw1 in mapped_ranges[G])
answers = {
    seed in candidate_seeds
    where FinalizeR4(GenerateR4StageOne(seed)).accepted_index != 4
}
```

This is exact and complete. The first step is mathematical construction, not a
linear Seed scan. The second step is exact offline replay because the result is
path-dependent. The existing solver already follows this structure.

## Why the proof is complete

- The measured rarity-4 draw-1 map contains 21 Grace IDs and covers all 65,536
  high-16 output buckets without gaps.
- All 21 Grace IDs fail the finalizer's `candidate_context_allowed` gate for
  `record_type=0xE604` and have no finalizer candidate weight.
- The finalizer constructs each attempt from the unchanged stage-one source and
  writes only the attempted target slot.
- If an earlier slot is accepted, slot 5 remains byte-for-byte unchanged. If
  target index 4 is accepted, slot 5 is replaced by an ordinary candidate. If
  no slot is accepted, the original record is returned unchanged.
- All 10 native stage/final corpus pairs match the offline finalizer byte for
  byte and obey the preserve-or-remove invariant.
- A deterministic 10,000-Seed diagnostic sample produced 8,655 retained Grace
  records, 1,345 records with the Grace replaced, and zero records changed to a
  different Grace.

## Regression vectors

```text
Seed 183696634:
  draw1              = 6247
  stage-one Grace    = 0xBABD
  accepted index     = 4
  final slot 5       = 0xAE5A
  final Grace        = none

Seed 2965:
  stage-one Grace    = 0xCE68
  accepted index     != 4
  final Grace        = 0xCE68
```

These vectors distinguish a transient stage-one Grace from the completed
rarity-4 record and must remain in the test suite.
