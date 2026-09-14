# PC v2.01 enemy-states Pro integration — 2026-09-14

## Package and integration

The returned Pro package is preserved as
`deliverables/Nioh3_EnemyStates_v201_20260914.zip` (652,332 bytes, SHA-256
`CE29D25B5DC53B2C9E8F1B7D2A4FDD75DF753547879DC59134E0622B9D90B6D4`). CRC
passed; 98/98 MANIFEST payloads matched and all 99 ZIP entries were verified.
`SOURCE.patch` check passed. Eighteen files were integrated and matched
`SOURCE_FILES` individually. The only local integration fixes were exporting
`auxiliary_generation.__all__` and allowing Git to track
`research/enemy_states_v201` while continuing to ignore `pycache`.

## Native-table capture and current bounded capability

The supplied PC v2.01 native-table capture completed successfully as a
zero-breakpoint, read-only collection with no Cheat Engine involvement. The
capture is recorded under
`audit/enemy_states_v201/20260914-native-tables-a/` and is covered by its
read-only manifest. Offline replay against the captured native fixtures now
passes all three required Possessed controls:

- `86872488` solo: no Possessed spawn (expected/actual empty)
- `86872488` expedition: expected/actual spawn `0xF3F`
- `156062997` solo: expected/actual spawn `0xF40`

This is offline replay versus native fixtures, not a new native generation run.

For seed `86872488`, solo has six occurrences with no possessed enemy, while
expedition has 10 occurrences including possessed spawn `0xF3F`. Seed
`156062997` now replays the expected solo Possessed spawn `0xF40` against the
captured fixtures. Curse (中文一难) remains `unknown` by default and retains
its observed condition range; this capture does not establish that Curse is
exact.

Validation reports 73 passed and 3 skipped (optional NumPy unavailable), the
original auxiliary suite reports 42 passed, and the full Python suite reports
1,252 passed and 7 skipped. The control verifier reports 2 pass and
1 `missing_tables`; therefore the three vectors are not all accepted.

## Boundary and next step

This is a backend/reference foundation only. It is not connected to the UI or
worker and is not a released filter. The native-table capture closes the
bounded offline control-validation prerequisite, but does not authorize UI,
worker, or release claims. Do not repeat the mode or materialization
experiments. No version number is fixed here.
