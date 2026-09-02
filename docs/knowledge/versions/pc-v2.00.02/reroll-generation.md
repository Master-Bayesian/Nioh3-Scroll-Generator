# Scroll reroll candidate generation — PC v2.00.02

## Current status

Project decision (2026-09-01): this research is frozen and abandoned as a
product TODO. The evidence and offline research code are preserved for archival
value, but no further implementation or live capture is planned. The application
must not expose or promise reroll prediction.

The paid/manual five-candidate reroll path is recovered to a
`native-static-candidate` grade.
The offline implementation reproduces the visible control flow and captured
parameter-table semantics, but it is not yet `native-byte-parity`: no live
reroll candidate vector was retained before this recovery.

Do not expose the predictor as guaranteed product output until the controlled
capture described below passes.

The post-challenge replacement screen is a second mechanism and must not be
treated as this same path.  A valid controlled vector for that mechanism is
now retained, but its generator remains unresolved.

## What determines one candidate pool

Native RVA `0x20C4BD0` takes:

1. the complete current `0xE8` final scroll record;
2. the zero-based effect slot selected for reroll;
3. an output vector owned by the scroll UI.

It derives a scoped RNG seed as:

```text
reroll_rng_seed = (displayed_seed + uint16(record[0x0C:0x0E])) mod 2^32
```

The LCG is the same recovered generator used elsewhere:

```text
state = state * 0x10DCD + 1 mod 2^32
output = state >> 16
```

Before the first candidate ticket, the function consumes one unused
`random_int(0x10000)` draw.  It then builds a legal weighted pool from:

- scroll type, rarity, and current playthrough progress;
- the currently selected slot's effect group;
- all six other effect groups;
- category capacity remaining after those other effects;
- group equality and conflict masks;
- candidate context flags and progress gates;
- the native weight selector from the scroll item row;
- one small save-scoped effect-group eligibility set queried by RVA
  `0x2167804`.

The function draws at most five candidates.  Each selected candidate consumes
one weighted ticket and normally two percentile draws.  After selection, every
variant with the same effect-group key is removed from the pool.  Therefore the
five displayed choices cannot contain two variants from the same group.

## Why the same Seed changes after another action

The displayed Seed does not change.  A `uint16` counter at record `+0x0C`
changes the scoped RNG seed:

| Native RVA | Observed static action | Counter behavior |
| --- | --- | --- |
| `0x20C1718` | paid/manual candidate refresh | increments `+0x0C` before rebuilding candidates |
| `0x20C1764` | apply the selected candidate | writes the effect, normalizes the record, then increments `+0x0C` |
| `0x20DD008` | completion/rebuild wrapper | increments `+0x0C` after the completion path |

Consequently, “the result after N rerolls” is a branching sequence rather than
one list indexed only by N:

- rejecting or refreshing advances the counter while preserving the current
  effect set;
- accepting a choice advances the counter and changes the effect set, which
  changes later conflict and category-capacity gates;
- completing another challenge may advance the same counter before the next
  pool is shown.

An exact prediction UI must record the initial counter and the user's action at
each branch.  Seed alone is insufficient once reroll history exists.

## Remaining save-scoped gate

Most currently weighted scroll effects carry candidate flag `0x0100` and
bypass RVA `0x2167804`.  Three weighted effect families do not.  The helper
looks up their group keys in a per-save runtime structure and returns an
eligibility byte.

The offline predictor therefore accepts an explicit set of eligible dynamic
group keys.  If that set is omitted, it excludes conditional rows and marks the
result incomplete.  The capture analyzer enumerates all relevant subsets and
reports which subset reproduces the native vector; no guessed default is
promoted to product truth.

## Code and evidence

- Offline predictor: `nioh3_scroll_editor/reroll.py`
- CLI preview: `research/predict_scroll_rerolls.py`
- Live CE capture: `research/capture_scroll_reroll_ce.lua`
- Capture analyzer: `research/analyze_scroll_reroll_capture.py`
- Regression tests: `test_reroll.py`
- Candidate builder disassembly:
  `audit/p1_static/reroll_effect_helper_callers_20260831.json`
- Caller and state-transition disassembly:
  `audit/p1_static/reroll_candidate_pipeline_20260831.json` and
  `audit/p1_static/reroll_state_functions_20260831.json`

## Required controlled acceptance capture

On the exact PC v2.00.02 executable:

1. Preserve a raw `0xE8` record for one completed scroll before touching the
   reroll UI.
2. Arm `capture_scroll_reroll_ce.lua` and open one rerollable slot.
3. Capture the initial five native candidates.
4. Perform exactly one manual refresh and capture the next five candidates.
5. Accept exactly one candidate, reopen the same slot, and capture the next
   pool.
6. Run `analyze_scroll_reroll_capture.py` and require every effect ID and roll
   percentile to match in native order.
7. Repeat on at least two rarities and two different starting counter values.

Only after those vectors pass may this subsystem be raised to
`native-byte-parity` and integrated into the player-facing application.

## Controlled post-challenge replacement vector

Seed `203900415`, rarity 4, record type `0xE604`, supplied the first clean
controlled vector for the challenge-completion UI.  The complete evidence is
stored at:

`captures/reroll_live/seed_203900415/controlled_completion_vector.json`

The UI offered one replacement for each of the four occupied non-primary
slots, rather than five alternatives for one selected slot:

| Physical slot | Current effect | Completion candidate |
| --- | --- | --- |
| 2 | `0xD411` Sprint Ki Consumption | `0xDAC2` Ultimate Constitution |
| 3 | `0x6AAF` Ultimate Magic | `0xDFF0` Ultimate Skill |
| 4 | `0xFBEE` Guard Ki Consumption | `0x6CE3` Untouched Familiar Talisman, `+1.4%` |
| 5 | `0xBABD` Tsukuyomi's Grace | Anima Charge Bonus group `0x3194`, `+9.3%`; exact candidate effect ID was not captured |

The player selected slot 2.  A read-only decrypted-save comparison proved:

- record `+0x0C` advanced from 1 to 2;
- visible challenge count byte `+0x33` changed from 5 to 4;
- slot 2 became canonical `0xDAC2`, value 150, prefix `0xF8B5`, metadata
  `0x00040D59`;
- the other effect IDs did not change;
- the unselected slot-5 prefix normalized from `0xB991` to `0xA1B1` while
  retaining ID `0xBABD` and metadata `0x00020C00`.

The pre/post records changed at exactly 12 byte offsets, all enumerated in the
capture JSON.  The save path and account identifier are intentionally omitted
from the frozen evidence.

The slot-5 screenshot does not expose an internal effect ID.  Several native
effect rows share the displayed Anima Charge Bonus name and group `0x3194`, so
the earlier `0xBC51` assignment was only a catalog guess and is withdrawn.  An
exact ID requires a live candidate-object capture before the player confirms a
replacement.

This native vector does **not** match the paid/manual predictor rooted at RVA
`0x20C4BD0`: none of the enumerated save-scoped dynamic-gate subsets produced
the observed per-slot vector.  Therefore the challenge-completion UI either
uses a different generator or additional state not modeled by that function.
The product must keep the two predictors separate until that path is recovered
and at least one additional controlled vector passes.

## Rejected live samples

Seed `99183032` from 2026-08-31 must not be used for native parity.  Its
pre-resolution record visibly contained both Tsukuyomi's Grace and Amaterasu's
Grace, which is a modified, structurally non-native final effect set left from
earlier local editing.  The completion UI also showed an intermediate Luck
candidate before the confirmed replacement, and the final normalized record
removed the stale Tsukuyomi slot.  This sample is useful only as evidence that
the game may normalize modified records during completion; it cannot establish
the native candidate sequence or counter rules.
