# Effects, Grace, and Seed solving

## RNG foundation

The recovered LCG is:

```text
state = state * 0x10DCD + 1 mod 2^32
output = state >> 16
```

The multiplier is odd, so state advancement has a modular inverse. This makes
a fixed draw and a known output interval mathematically reversible. It does not
make every final game result a fixed-draw function.

## What is directly invertible

- Confirmed Grace first-draw partitions can be inverted to complete natural
  Seed families for their exact version, record type, rarity, and generation
  stage.
- LCG state can be advanced or rewound by an arbitrary number of fixed draws.
- In theory, a table lottery with a proven fixed pool and no retry can be
  represented as output intervals and inverted to finite high-16 preimages.
  In the current product this is certified for the fixed-draw Grace families
  and matching-context NG1/NG2 primary maps, not for ordinary effects.

The R4 Grace map describes the stage-one slot-5 candidate, not necessarily the
final effect. Exact finalizer replay is required to prove that the Grace
survives.

## What requires exact replay

- Primary output cannot be identified solely by a conditioned draw-2 high-16
  bucket. Seeds `255766105` and `264410626` share draw-2 high16 `0` and Grace
  `0x6553`, yet produce different primary IDs (`0x512D` and `0x23E8`).
- Ordinary secondaries depend on category allocation, conflicts, quota,
  weighted pools, promotion, retry, rejection, and previous accepted effects.
  They are not independently invertible, but the product can still use an
  exact forward GPU filter before independent exact replay.
- R4 completion derives a scoped RNG from the complete stage-one record and
  target slot, then may retry and normalize the complete record.
- Enemy and special-rule output is path-dependent on terrain, gates, budgets,
  scratch exclusions, and changing pool sizes.

For those systems, mathematics is a prefilter only. The final acceptance test
is complete offline replay against captured native tables.

## Certified search flow

1. Reject impossible slot, conflict, category, role, and playthrough
   combinations before opening a Seed family.
2. Construct an exact mathematical pivot family when a proven fixed-draw
   constraint exists; otherwise enumerate the complete natural Seed domain in
   bounded deterministic chunks rather than an arbitrary prefix.
3. Use GPU bulk prefilters only for predicates whose implementation has parity
   evidence. The product does not silently fall back to a full-domain CPU or
   Python replay.
4. Replay the complete effect and auxiliary generators for each survivor.
5. Stream accepted candidates and preserve a resume cursor; never materialize
   the full domain in memory.

The current accelerator hard-chunks pivot construction to 1,000,000
mathematical trials and exact primary batches to 65,536 surviving Seeds. Those
are memory bounds, not a claim that only the first million Seeds are searched.

### R4 finalizer-aware partial ordinary-effect filter

For one or two independent requirement groups, the DirectCompute matcher now
generates all four stage-one ordinary effects, their percentile rolls, the
transient Grace, and the exact completion-finalizer selection before evaluating
the final five slots. It preserves primary-only versus any-ordinary-slot
semantics and supports mandatory or OR-group conditions, including an effect
created only by the finalizer.

For three or more independent groups, the cheaper lossless rule remains useful:
because the finalizer changes at most one physical slot, stage one must already
match at least `N - 1` merged groups. The GPU evaluates that necessary
condition and leaves final acceptance to exact replay.

This is an exact forward filter, not a closed-form inverse. Every GPU survivor
is still replayed independently by the certified CPU generator before it is
shown or installed. Distributed 2,048-Seed two-group masks and contiguous
1,024-Seed single-group masks produced zero differences; level-1 value-one
eligibility and primary-position checks were also compared separately. Those
local checks do not replace the existing native stage/final parity corpus.

## Rarity and stage notes

- Rarity 3 uses its own native path; its final growth slot is token `0x00000001`
  and is excluded from ordinary secondary matching.
- Rarity 4 builds a stage-one record and runs the completion finalizer. Slot 5
  may retain its Grace candidate or become an ordinary completed effect.
- Rarity 5 has complete third-playthrough offline generation and a 10,000-Seed
  full-record parity report. Hiding it from the current UI is a product policy,
  not a claim that the offline algorithm lacks technical verification. The
  policy anticipates the game developer's announced propagation fix.

## `0xBABD` lesson

Final-record `0xBABD` resolves to `月读的恩宠`. In the R4 stage-one slot-5
namespace, the same number is a Grace candidate, but it is not a guarantee that
the completed record will retain that Grace. Seed `183696634` starts with the
candidate and finalizes to `0xAE5A` (`技之深奥`). That is one complete-state
outcome, not a fixed `0xBABD -> 0xAE5A` conversion table. The correct semantic
key therefore includes game version, record type, rarity, generation stage,
slot role/index, raw ID, and full slot payload.

## Sources

- current solver boundaries: `research/EFFECT_SEED_SOLVER.md`
- native table layout: `research/EFFECT_GENERATION_TABLE_INDEX.md`
- math: `nioh3_seed_math.py`
- exact generation: `nioh3_scroll_editor/effect_sequence.py`
- R4 finalizer: `nioh3_scroll_editor/r4_finalizer_engine.py`
- Grace partitions: `nioh3_scroll_editor/grace_map.py`
- GPU/CPU acceleration: `nioh3_scroll_editor/seed_accelerator.py` and
  `research/native_seed_accelerator.cu`
