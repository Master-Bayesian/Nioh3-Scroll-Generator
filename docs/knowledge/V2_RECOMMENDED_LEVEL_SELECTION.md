# V2 displayed recommended-level selection

Updated: 2026-09-07. This is a read-only input foundation for the future UI.

Recommended level stored in a scroll is different from its displayed recommended
level. A raw value of `350` predicts display level `238`. The captured forward
curve predicts display level `350` for both canonical raw values `585` and `586`.
The resolver returns both alternatives and selects `585`, using the explicit
`lowest_canonical_internal_level` policy. Choosing between matching raw values
is visible to callers because other generation behavior can depend on the raw
input; equal displayed levels do not imply interchangeable generated records.

## Read-only API

Python consumers call `resolve_recommended_level(displayed_level)` from
`nioh3_scroll_editor.recommended_level`. It returns
`RecommendedLevelResolution` with the requested display level, status, every
matching canonical raw value, and the deterministically selected raw value.

The offline worker accepts:

```json
{
  "protocol": 1,
  "id": "level-350",
  "method": "recommended_level.resolve",
  "params": { "displayed_level": 350 }
}
```

The usual handshake is required first. The sandboxed Electron API exposes this
as a typed read-only method:

```typescript
const result = await window.nioh.resolveRecommendedLevel(350);
if (result.status === 'exact') {
  // 585; retain result.canonical_internal_levels ([585, 586]) for inspection.
  const rawRecommendedLevel = result.selected_internal_level;
}
```

Resolution does not start a job, access a save, call the game, or install a
record. Existing save/runtime command fields named `recommended_level` continue
to take the raw internal value. The resolver does not change their defaults or
automatically rewrite a submitted request. The current engineering workbench
retains its existing raw JSON controls; Figma can decide the final interaction.

`search.catalog` includes compact `recommended_level` metadata:

- Canonical internal bounds: `156` through `1400`.
- Displayed bounds from that canonical range: `142` through `700`.
- Selection policy: `lowest_canonical_internal_level`.
- Evidence: `captured_native_curve_prediction`.

Bounds are not a promise that every intervening target is reachable. Consumers
must check the resolution status before accepting a target.

## Exactness and failures

- `exact`: one or more raw values match; `selected_internal_level` is the lowest.
- `out_of_range`: target is outside the canonical curve's displayed bounds;
  the raw-value list is empty and the selected value is `null`.
- `unreachable`: target lies inside those bounds but no canonical integer raw
  value produces it; the raw-value list is empty and the selected value is `null`.

Unavailable targets are never rounded, clamped, or substituted. The strict
Python domain helper rejects non-integers, including booleans. The IPC contract
accepts bounded JSON integers and rejects fractional numbers, booleans, strings,
and unknown fields. Integral JSON numbers such as `350.0` are normalized to an
integer after schema validation, consistent with JSON Schema integer semantics.

The inverse enumerates the bounded canonical domain and calls the existing
float32 forward method without modifying it. It does not introduce a separate
approximate formula. This also preserves the upper plateau: raw values
`1301` through `1400` all predict displayed level `700`.

## Verification and remaining acceptance

Focused tests cover all 1,245 canonical raw inputs, complete inverse membership,
deterministic selection, a synthetic unreachable gap, both range boundaries,
strict input handling, response-schema consistency, and the real framed worker.
The TypeScript IPC test exercises the same Python worker. The Electron smoke
test now asserts the added preload method and exact level resolution.

This subtask passed 10 focused Python tests, the existing 23 V2 Python tests,
TypeScript checking, and all 10 source IPC/controller tests. Parent verification
then passed all 496 Python tests, 10 rebuilt-worker IPC tests, source/packaged
R3/R4/R5 parity and actual portable Electron preload, locale, search and level
resolution smoke. All 89 manifest entries were verified. The current artifact
is `deliverables/frontend-v2/portable-v2-level-input`. The changed request/response digests
intentionally reject mixing older workers with the newer application files.

The result is a prediction from the captured native curve. Actual in-game
display acceptance for the `350` target remains pending. No forward numerical
semantics, generator/search defaults, Tk behavior, or installation semantics
were changed by this work.

A separate loaded-game comparison uniquely matched an existing level-170 scroll
to slot 7, seed 112905143 and serial 2321636: raw recommended level 592 displays
as 353. Its full record was unchanged across the Items and Battle Scrolls
captures. See `deliverables/frontend-v2/live-acceptance/20260907T225708Z/selected-scroll-ui-match.md`.
This corroborates one existing raw/display pair; the record's origin is unknown
and this does not accept the 350 target or natural generation rules.

The later controlled acquisition seed 10030565 / serial 2398468 supplied a
second pair: raw 561 displays as 343, both before and after its first reveal.
The screenshot and full-record lifecycle are recorded in
`deliverables/frontend-v2/live-acceptance/20260907T225708Z/r3-10030565-first-reveal-analysis.md`.
The existing R4 regression seed 43723117 additionally displays raw 183 as 160
in its pre-clear screenshot. These are bounded forward-display corroborations;
the requested 350 target and a default change still await their own acceptance.
