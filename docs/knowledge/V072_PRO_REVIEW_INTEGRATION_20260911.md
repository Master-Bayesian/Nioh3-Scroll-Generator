# v0.7.2 Pro review patch integration

> Historical review and integration checkpoint. The project owner's
> [2026-09-12 risk decision](TITLE_SAVE_APPROACH_RESET_20260912.md) supersedes
> the title-save-specific release block below. Technical findings and test
> limitations remain valid; accepting backup-based recovery is not a root-cause
> fix, native ownership proof, or waiver of unrelated release checks.

## Decision

The follow-up working tree remains **blocked from publication**. The Pro patch
repairs the reviewed save-transaction, native lifecycle, worker recovery and
diagnostic defects, but it intentionally does not claim to solve title-screen
save-cache ownership inside the game.

The reviewed input archive was
`Nioh3_v072_review_patch_20260911.zip`, 201,864 bytes, SHA-256
`FB7810431C5F5249799A5925CB7AF35412C74614571D1B20A464B184B1937647`.
Its 46 manifest entries were checked for path, size and SHA-256 before
`SOURCE.patch` was applied to the current dirty worktree based on commit
`6264cbd355729e0b434ba5f540232a5d1362a79d`. The pre-application worktree diff
was preserved under `.codex_tmp/pre-pro-review-patch-20260911-r2/`.

## Integrated corrections

The patch adds or strengthens the following behavior without changing search,
RNG, R4 finalization or candidate-generation semantics:

- save transactions preserve `unknown` after an uncertain commit, bind rollback
  to the original backup generation and recheck related files before replace
  and after readback;
- append-only installation refuses a duplicate-serial repair that would modify
  an existing scroll;
- native operations bind PID to process creation time, serialize admission
  across protected workers, scan durable ownership receipts and separate
  resource release from business verification;
- live-add preview retries only a proved idle miss, while insertion is never
  replayed; temporary scheduler activity shares one bounded deadline;
- late receipts and exited protected workers can be reconciled without clearing
  an uncertain owner or assigning an old operation to a reused PID;
- operation failures retain pre-verification execution, inventory and native
  index evidence;
- completed worker jobs with nested business failure states trigger diagnostic
  capture; logger and clipboard calls are bounded and cannot replace the
  original error;
- Tauri log rotation is byte-bounded, preserves UTF-8 across pipe and tail
  boundaries, and retains a bounded first-failure capsule for the current app
  session.

## Windows verification after integration

All commands ran from the real current worktree on Windows:

| Check | Result |
| --- | --- |
| Focused Pro save/lifecycle tests | 37 passed |
| Full Python discovery | 614 passed in 78.546 seconds |
| Desktop Node tests with source workers | 51 passed |
| TypeScript `tsc --noEmit` | passed |
| Rust unit tests | 11 passed, including all five Pro additions |
| Electron production build | passed |
| Production surface verification | 15 checks passed |
| Electron-to-Python encrypted synthetic-save flow | 20 checks passed |
| Tauri release build with both frozen workers | passed |
| Desktop Node tests with both packaged workers | 51 passed |
| Strict source/package GPU parity for R3, R4 and R5 | passed |
| Packaged Tauri startup smoke | remained alive for the 12-second probe |
| Portable manifest | 728 declared files, no missing or mismatched files |

The first packaging attempt used the Codex runtime Python and was correctly
rejected because its `altgraph` and `pyinstaller-hooks-contrib` versions did not
match `packaging/requirements-v2.lock.txt`. Rebuilding with the repository's
locked `.codex_tmp/v2-build-env` succeeded. After Rust formatting, the current
release binary was relinked and repackaged with the already verified workers.
The verification-only portable tree is
`.codex_tmp/v072-pro-integration/portable-pro-review-r4/`; it is not a release
asset.

## Remaining release blocker

Title-screen installation still writes the disk save while the running game may
retain an older in-memory generation. A successful backup, replace, decrypt,
parse, hash and readback sequence cannot prevent the game from later flushing
that older generation when the player exits without loading the character.
`test_delayed_game_flush_after_return_remains_an_explicit_release_blocker`
documents this legal ordering. Its passing result proves that the safety
capability is missing.

The title path requires a version-specific native protocol that owns or joins
the game's save queue, then either reloads the committed generation or
invalidates the stale cache and receives an acknowledgement bound to the same
process lifetime, account and save generation. Do not replace this gate with a
sleep, process suspension, repeated external overwrite, or a requirement that
the player enter the game once.

No live-game mutation was performed during this integration. Actual NG1 still
has only the earlier in-memory insertion evidence without normal save/reload;
actual NG2 remains unaccepted by prior user decision. A future release must use
a version newer than the already published v0.7.2, run from the locked build
environment, and remain blocked until the title ownership protocol and the
specified live lifecycle tests pass.
