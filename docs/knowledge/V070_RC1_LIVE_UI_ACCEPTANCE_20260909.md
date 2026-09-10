# RC1 packaged live UI acceptance — 2026-09-09

User confirmed menus closed. Read-only PC v2.01 capture found 39 entries.
The packaged UI generated seeds 10030609 and 10030610, favorited both, moved both into the cart and selected only 10030609 (NG3 R4).

Preparation was rejected before native insertion: `Saved defined record fields differ`.
Comparison against the automatically backed-up, decrypted save identified exactly the two rarity bytes (0x30/0x31) for seeds 10031015, 10031025, 10031035: saved 5/5, runtime 4/4. This matches the previously accepted R5 conversion. No whitelist or weakened backup comparison was introduced. The user was asked to save at a shrine and close menus before preparing again.

The UI error mapping now gives that concrete recovery step in all three languages. The presentation fix is included in `deliverables/frontend-v2/portable-v070-rc1-live-ui-20260909`. The original RC1 artifact remains unchanged. Both worker binaries were reused from the verified RC1 build; the protected-worker SHA256 matches exactly. Strict TypeScript and the 530-message locale audit passed.

Evidence: deliverables/frontend-v2/rc1-ui-live-20260909. Test profile and operation state are isolated; the actual save is only read/backed up. No native insertion has occurred at this checkpoint.

## Successful packaged UI insertion

After the user saved and closed menus, the final presentation-fixed package prepared and executed exactly one selected candidate through its actual favorites/cart UI.
- Batch: b940e3e8-a58d-4c75-b413-ae334153f86f.
- Child operation: c0e8eda7-e644-4b9f-bdf7-7d2d01162401.
- UI completion: verified 1/1; completed/total progress 1/1; selected item automatically unselected.
- Independent readback: inventory 39 -> 40; only seed 10030609 added; unselected 10030610 absent.
- Automatic backup exists and its SHA256 matches the reviewed source hash. Source save remained unchanged by insertion.
- Awaiting normal save/reload acceptance and subsequent temporary-cap UI acceptance.

A minor UI issue was observed after successful auto-unselection: the obsolete preparation card still said selection changed. Source now dismisses a completed batch plan; receipts remain durable. This cleanup does not affect the proven native operation.

## Save/reload acceptance

The user saved, returned to title, loaded the same slot and confirmed seed 10030609 in details. Read-only capture found the same 40 serials. `verify_persistence(..., allow_new_marker_clear=True)` verified all 40 records; only the expected new-item bit at record+0x18 cleared for serial 2421712. This closes the packaged favorites/cart subset insertion and normal persistence gate. Temporary capacity acceptance is in progress.

## First capacity attempt: visibility limit and clean stop

The packaged editor selected seed 10030609, disabled enemy/terrain/rule overrides and armed capacity=4. The runtime reported armed_no_hit with no pending calls. The user pointed out that this new scroll is unrevealed, so its capacity cannot be visually checked. No challenge completion was requested.

The override was stopped through the same UI: stopped, pending_remote_calls=0, safe_to_shutdown=true. Independent inventory comparison found only the normal new-item marker clearing on another browsed scroll (10031035, 0x18: 130 -> 128); no numerical inventory fields changed. A previously written boolean whole-inventory check was false because it included that marker; the detailed diff resolves it.

The user was asked to select any already revealed scroll and provide seed/current-count/capacity for a visible test. Do not count the unrevealed-scroll attempt as live capacity acceptance.

Final UI-cleanup package: `deliverables/frontend-v2/portable-v070-rc1-final-20260909`; it includes the success-card cleanup and a stable accessible label for the capacity checkbox. Full manifest and packaged favorites/three-language viewport checks pass; numerical workers remain the same verified binaries.

## Explicit two-field experiment on revealed scrolls

The user identified revealed NG3 R3 seed 10030565 at 6/7 and R4 seed 36526331 at 3/4, and explicitly requested testing changed capacities and remaining count=2.

Read-only identity distinguished the first target by full serial 2398468 (remaining 6), because another R3 instance has the same seed and remaining 7. The R4 target is serial 2375803 (remaining 3). Remaining-count writes are therefore full-instance scoped. The capacity hook is intentionally seed scoped.

A bounded local research helper, `.codex_tmp/challenge-field-probe.py`, copied and verified a fresh automatic save backup, retained complete before records, validated PC v2.01 / process creation time / manager ownership / serial / seed / rarity / expected old count, and wrote only record+0x33 on the two targets. Both record+0x0E fields were zero, so the native setter side-field reset would have no effect in these samples. Readback and whole-inventory comparison confirmed exactly the two count-byte changes (6 -> 2 and 3 -> 2). No native setter invocation or other field write occurred.

Remaining-count editing is an experiment at this checkpoint, not an already shipped UI feature. Restore is guarded by the same process instance and expected count=2; it refuses to overwrite intervening gameplay changes. The user was told not to challenge or save during the experiment. R3 capacity=5 was armed through the production editor, with enemy/terrain/rule overrides disabled; visible confirmation is pending.

### R3 visual acceptance and restoration preference

The user confirmed 10030565 R3 displayed 2/5 and 36526331 R4 displayed 2/4. R3 runtime status was applied_hit, hit_count=1. The R3 capacity override was then stopped and cleanup confirmed stopped / no pending calls / safe_to_shutdown=true.

The user explicitly cancelled manual restoration: do not run the remaining-count restore command. Current remaining counts are left at 2. An unsaved reload can restore the saved values; stopping the capacity override alone does not revert an inventory count byte. The R4 capacity=6 test is next. User confirmation of these displays validates current-memory behavior, not changed-count persistence or multiplayer propagation.

### R4 visual acceptance and final cleanup

The user confirmed R4 seed 36526331 displayed 2/6. Runtime reported applied_hit with two hits. The override was stopped through the packaged editor; final state is stopped, pending_remote_calls=0, safe_to_shutdown=true, error=null. No manual remaining-count restore was executed, as explicitly requested.

Final full-inventory comparison against the pre-experiment capture found exactly two changed bytes: serial 2398468 / seed 10030565, +0x33 6 -> 2; serial 2375803 / seed 36526331, +0x33 3 -> 2. All other bytes and the full serial set remained unchanged. This accepts independent current-count and capacity changes on these revealed R3/R4 samples. The counts were left at 2; their changed-value save/reload and propagation were not tested. Capacity cleanup was verified by the owned runtime, not by another user-requested visual restoration pass.

The packaged capacity feature and the separately backed-up remaining-count experiment are distinct. Remaining-count editing still needs a product command, validation and UI before it can be advertised as a shipped feature. No such UI support is claimed by this experiment.

Final artifact manifest SHA256: 54670b44f3a1728bb6159f3aa71384926a2858bc287cc90a97f97c7cc752aa13 (152 files). The artifact remains an unsigned local RC, with no commit, push or public release performed.
