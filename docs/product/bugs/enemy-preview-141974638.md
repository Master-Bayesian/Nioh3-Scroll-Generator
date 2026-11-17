# Known enemies hidden by an unavailable state preview

Reported on 2026-11-15. The supplied screenshots show scroll 141974638,
NG3/R4, level 180, recommended level 330. The game lists Enenra, Kappa and
Red Kappa, while the application's enemy section is blank.

## Reproduction and cause

The exact published 0.8.10 worker was extracted from the verified public ZIP.
Its real packaged handshake, binary/contract identity and candidate preview
were recorded under `D:/Nioh3_v080_deliverables/deliverables/enemy-preview-141974638`.
The existing resource identity used for reproduction is PC v2.02 (`2.0.2.0`).

The worker returns these auxiliary base groups:

| Wave | Enemy lookup keys | Resolved enemies |
| --- | --- | --- |
| 1 | 405836, 193845 | Red Kappa, Kappa |
| 2 | 193845, 163259 | Kappa, Red Kappa |
| 3 | 488671 | Enenra |

Both detailed enemy-state variants have empty `occurrences`, null `terrain`,
`possessed_complete: false`, and the explicit missing-input reason
`selector-nonzero branch needs independent parity; use explicit generate_roster for research`.
An unavailable detailed state preview is not an empty base roster.

The frontend mapped the detailed array even when it was empty. JavaScript
arrays are truthy, so that empty array suppressed the auxiliary fallback for
enemy names and keys. The card then rendered the empty detailed array.

## Repair and support boundary

- The adapter preserves known auxiliary enemies when detailed occurrences are
  absent. The reported scroll displays the three names visible in the game.
- An exhaustive presentation state distinguishes detailed, base-only, confirmed
  empty and unknown rosters. Only an explicitly resolved roster can establish
  absence; old cached samples without that evidence remain unknown until refreshed.
- The card identifies unconfirmed Crucible Wraith state. The expedition variant
  also identifies unconfirmed additional enemies. No special state is inferred
  from the base names.
- The local sample filter uses the same state and cannot promote a base roster
  into a Possessed/Curse match. Server-side generation/filter semantics are unchanged.

The change affects presentation and sample adaptation. It does not extend the
certification of the native nonzero-selector state branch.

## Verification

Frontend lint, type checking, the E2E script's JavaScript lint/syntax checks and
the 1,592-message locale audit pass. The TLA+ runner checks five correct models
and eleven exact named fault controls, including three enemy-preview faults.

The production-component E2E calls the actual published read-only worker for the
reported ID, checks the screenshot's three names, preserves an ordinary detailed
preview, and exercises explicit unknown/empty response metadata controls. Those
two controls test presentation only and are not claimed as generated rosters.
Three locales, both themes, minimum-window geometry, previous known-ID ownership,
cart spacing and recovery presentation pass 74 checks with 22 screenshots. The
existing full text-fit gate also passes. No game/user save is accessed.

The separately named private test EXE was built and accepted first. The repair
is now published in [v0.8.11](../releases/v0.8.11.md). The actual outer-EXE gate,
`apps/tauri/verify-enemy-preview.mjs`, passes for the final signed file and is a
required source/hash-bound member of the thirteen-report release inventory.
The adjacent release record identifies the immutable product commit and exact
public file hash; the external task folders retain native captures and receipts.

## Selector-state follow-up on the development branch

The follow-up completes the PC v2.02 NG3 normal-solo selector-1 state path.
Two independent native positive entries and one negative entry matched the
seeded roster, source RNG/trials and persistent task flags, with owner visual
confirmation and verified cleanup. A separate read-only comparison matched all
1022 enemy keys/eligibility gates, all 20 declared placement slices and source
configuration against the retained replay inputs.

The reported ID now returns five detailed solo occurrences and marks only its
wave-2 Red Kappa possessed. `DescriptorSelectorSupport` restricts this extension
to the verified version/mode/selector and exact table bytes. Cache identity is
revised so old unavailable-state previews cannot silently remain current.
Nonzero expedition, other versions, changed tables and Curse remain outside
this new certification. The original base-roster fallback is retained.

The sanitized native controls are in
`data/enemy_states/pc_v2_02/selector_one_certification.json`; the collection and
reproduction procedure is in
`research/possessed_enemy_capture/SELECTOR_V202.md`. This follow-up is separate
from the already published v0.8.11 presentation repair.
