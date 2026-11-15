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

Build the separately named private test EXE from a clean source commit, and retain
native acceptance and delivery hashes in the external task folder. The actual
outer-EXE gate is `apps/tauri/verify-enemy-preview.mjs`; future signed releases
require its report in the release inventory. This document does not claim a new
public release or update-feed change.
