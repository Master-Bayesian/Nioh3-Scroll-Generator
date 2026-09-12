# v0.7.2 follow-up fixes prepared for review

> Status on 2026-09-11: the Pro patch has been integrated and verified on
> Windows. This pre-review note is retained as history. Read
> [the integration result](V072_PRO_REVIEW_INTEGRATION_20260911.md); the
> historical title-screen release gate was superseded by the owner
> decision in [the approach reset](TITLE_SAVE_APPROACH_RESET_20260912.md).

## Diagnostic logging

The reported log screenshot came from v0.7.1 and contained only UI status
messages. The current Tauri path records broker requests, responses, protected
job transitions, worker stderr, and errors with ISO timestamps. Candidate
records and save paths are intentionally retained for reproduction.

Copy Log now replaces the clipboard with diagnostics plus the newest 128,000
bytes across `desktop.4.log` through `desktop.log`, rather than reading only the
current segment. A tail that starts inside a multibyte UTF-8 character discards
that incomplete character rather than adding replacement noise. On an operation
failure the renderer invokes the same copy path automatically without replacing
the original user-visible error.

Storage remains bounded to the current 4 MiB log and four 4 MiB archives.

## Intermittent live-add failure

`LiveAddAdapter` previously set its in-memory `pending` owner before calling the
native transport. If the transport rejected synchronously before creating a
native receipt, the adapter retained a false owner until its protected worker
was restarted. This matches the reported pattern where restarting the
application made live addition work again.

The adapter now clears `pending` and `pending_pid` only when the default native
transport proves that the operation ID has no in-memory or durable receipt.
Timeouts, disconnects, a failure after receipt creation, and uncertain redirects
continue to retain ownership and remain non-replayable. Regression tests cover
preview and insertion rejection plus the ambiguous-failure retention case.

This is a concrete repaired failure mode, not proof that every reported live
failure had the same cause. The next real failed-operation log should be kept
for comparison.

## Title-screen save insertion

The current candidate implementation permits append-only generated-scroll
installs while the game is at the title screen. It samples the main character
save, game backup, and system save twice before preparation and again before
commit. Any drift aborts without writing. It writes through an application
backup and durable journal, decrypts and parses the installed file, and restores
the original main save when post-commit verification fails.

Synthetic fault injection covers sibling-file drift, post-commit corruption,
rollback, and reproduction data. Delayed overwrite by the still-running game
has not been reproduced or ruled out. This path requires Pro review and a
controlled title-screen exit/restart acceptance test before release.

## Other checklist items

- Add to cart has a high-contrast gold treatment and a distinct selected state.
- v0.7.2 already ships a standalone 27 MB setup EXE; the ZIP is the portable
  and update payload, not the only user download.
- Earlier live acceptance inserted NG1-NG3 *scroll payloads* across R3-R5 and
  then verified save/reload. It did not vary or record the running character's
  actual progression.
- A separate current-progression test inserted R3 seed `10032001` while the
  running game was actually set to NG1. Inventory increased 41 to 42, serial
  `2446282` occupied slot 29, all 41 previous records and the native index were
  preserved, the source save remained unchanged, and native ownership cleaned
  up. The user stopped the matrix before NG2 and before normal save/reload, so
  NG1 persistence and actual-NG2 compatibility remain unaccepted. Do not test
  them proactively; collect evidence if a user reports a matching problem.
- Cross-installation user and feature statistics are outside this follow-up.
- Possessed-enemy selection is frozen under the 2026-09-11 research note.
