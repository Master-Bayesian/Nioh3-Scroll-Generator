# v0.7.3 Pro review closure

## Scope and result

This audit compared both original Pro archives with the current working tree,
including uncommitted files. The supplied patches were fully integrated; there
was no partially applied production patch. The incomplete portions were native
title-save research and live acceptance, which must not be conflated with source
integration or simulated regression coverage.

The owner's [2026-09-12 decision](TITLE_SAVE_APPROACH_RESET_20260912.md) accepts
the unreproduced title-save/delayed-overwrite risk and allows backup restoration
at the title screen. That supersedes the title-specific release prohibition in
the original Pro reports. It does not prove a corruption fix or native save
ownership. Possessed-enemy research and actual-NG2 testing remain deferred.

## Original archive integrity and coverage

| Input | Verified SHA-256 | Source comparison before this audit's restore correction |
| --- | --- | --- |
| `D:/Downloads/Nioh3_v072_review_patch_20260911.zip` | `FB7810431C5F5249799A5925CB7AF35412C74614571D1B20A464B184B1937647` | All 22 patched files present: 15 exact bytes, one newline-only difference, five Rust formatting-only changes, one approved restore-entry policy difference in `save_application.py`. |
| `D:/Downloads/Nioh3_TitleSave_v201_20260911.zip` | `B78D1AA3697D9C9E0647B7C201F45116780EA7D37BC89793C98D1A8EB8C6E9CC` | All 47 source payload files present, including the unchanged owned-breakpoint helper: 41 exact bytes and six reviewed Windows/CE integration or documentation changes. |

Both archives passed ZIP CRC checks again. The six second-package differences
are CE main-thread synchronization/.NET executable hashing, their mock tests,
the Lupa fallback, LLVM disassembly support, and updated observation/testing
notes. These differences retain the supplied signatures and read-only scope.

## First Pro review: item-by-item closure

Test names below are bounded evidence. Python native lifecycle cases use mock
Win32 state; the isolated native fixture uses its own process, not Nioh 3.

| Finding | Implemented code | Verification and remaining boundary |
| --- | --- | --- |
| F01 / P0: no game-native title-save/cache ownership | No invented native protocol added. Existing title append retains backups, related-file checks and journal/readback. `SaveApplication._kind_requires_game_closed` permits append, edit, delete and restore at title following the owner's final clarification; all preserve transactional guards. | `test_delayed_game_flush_after_return_remains_an_explicit_release_blocker` remains a negative capability test despite its historical name. It simulates a later external write; it is not the user's proven corruption cause. Native ownership research/C1-C3 are deferred under the owner's accepted-risk decision. |
| F02 / P0: rollback could overwrite a later generation; unknown downgraded | `savegame.SaveCommitUncertain`, `_restore_main_from_application_backup`, `commit_encrypted_main_save`, `SaveApplication.commit` bind checkpoints, preserve unknown, retain original errors and transaction context. | `test_changed_peer_during_failed_readback_prohibits_rollback`, `test_backup_tamper_is_not_restored_even_if_copy_matches_tampered_file`, `test_application_does_not_downgrade_uncertain_to_not_committed_when_old_bytes_return`, plus journal-failure tests pass. This audit additionally extended the same principles to the separate backup-restore transaction below. |
| F03 / P0: PID reuse and cross-worker native ownership | `process_instance.py`, `native_submission_guard.py`, `NativeLiveAddTransport`, `LiveAddAdapter`, `LiveAddApplication`, inventory/index capture and durable claims bind PID plus creation FILETIME and serialize admission. | Worker restart, corrupt receipt, PID reuse/access denial, process-lifetime readback and shared-admission tests pass. No old operation is replayed. Successful normal-game acceptance and injected live-game fault coverage remain distinct from this evidence. |
| F04 / P1: staging/readback generation gaps | `commit_encrypted_main_save` rechecks peers before replace and after decrypt/readback; uncertain replacement is retained. | Staging main/peer mutation, post-decrypt peer mutation, and post-readback replacement tests pass. Checks narrow observed races; they are not atomic coordination with the game's writer. |
| F05 / P1: append silently repaired existing serials | Single and batch `SaveInstaller.install*` reject `APPEND_ONLY_REPAIR_REQUIRED` when old records would need repair. | Single/batch duplicate-serial tests verify unchanged source records and retained backups. New-record identity allocation is preserved. |
| F06 / P1: late receipts and dead protected host cache | Adapter pending reconciliation, proven-unaccepted business-claim settlement, Rust `Worker.can_replace` and `Broker` replacement after safe shutdown or actual exit. | Late preview/wrong-active receipt/proof-error/unaccepted-claim tests pass. Rust exit-proof test was executed in the prior Windows integration. Dead-pipe status alone never authorizes replacement or write replay. |
| F07 / P1: transient scheduler rejection, unbounded events, receipt I/O cleanup | Native transport waits for an idle callback inside one deadline, checks deadline on all events, retains cleanup ownership despite receipt persistence failure, closes exited-thread handles. | Busy-to-idle, changed-owner, continuous-event deadline, and receipt-I/O tests pass. Current isolated native fixture additionally passed normal dispatch, caller rejection, repeat operations and five controlled failure scenarios after its process-identity inputs were updated. |
| F08 / P1: completed job hid nested failures; diagnostics could hang | `invoke-with-diagnostics.ts` detects nested `unknown`, `not_committed`, rejected and partial business results; bounded diagnostic calls and UTF-8 message limits preserve the original outcome. | The four added Node diagnostic regressions cover nested failure, hung sinks, unsafe serialization and multibyte bounds. They remain in the complete Node suite; final release evidence is recorded separately from this source audit. |
| F09 / P1: log rotation/UTF-8 boundaries and lost first failure | Rust `storage.rs` performs per-chunk projected-byte rotation; worker stderr uses `Utf8LogDecoder`; a bounded first-failure capsule supplements rotated tails. | All five supplied Rust additions were previously executed on Windows. Tests cover oversize payloads, UTF-8 tails and pipe splits, rotation and pinned first failure. Current plus four 4 MiB log segments remain bounded. The capsule is session-local; durable operation receipts remain the longer-lived evidence. |
| F10 / P1: failed verification lost evidence; broken package broke Copy Log | `_finish` writes a unique pre-verification execution/inventory/index directory; `protected_jobs` logs context; Tauri reports package verification errors as data and uses nonblocking worker diagnostics. | Verification-failure then recovery test retains both attempts and proves one insertion call. Save errors retain source path, candidate bytes, public/transaction IDs and journal paths. Actual packaged clipboard/manifest-error behavior belongs to the final package UI acceptance. |

### Retained conservative behavior and optional suggestions

- A protected host that may own a redirected native operation is retained until
  release/exit is proved. Replacing it on timeout would undo the review's fix.
- Worker timeout request IDs retain the existing bounded late-response window.
  The Pro report suggested tombstone/recovery RPCs only if further shrinking
  that window is desired; this was not an omitted mandatory fix.
- The first-error capsule is bounded to the current session and is not claimed
  to preserve every historical error across restarts. Old oversized archives
  are not silently truncated; new writes use the fixed rotation cap.
- Actual NG1 has an earlier in-memory positive; its persistence and actual NG2
  compatibility must not be inferred from the candidate payload matrix. The
  owner deferred the additional progression experiments.
- Cold-title/returned-title native acknowledgement, forced post-redirect lost
  response against Nioh 3, and all live fault permutations are not claimed as
  completed because simulated or isolated-fixture tests passed.

## Second Pro package: research and tooling closure

| Deliverable | Current status |
| --- | --- |
| v2.01 snapshot/staging/async-write/completion static reconstruction | Integrated under `research/title_save_v201`, with raw function disassembly and scoped evidence. The earlier independent verifier passed 28/28 locator signatures, uniqueness, unwind bounds and instruction boundaries. |
| Read-only CE observer, eight staged four-breakpoint profiles and bounded output | Integrated with PID/creation/attachment identity, owned-breakpoint cleanup and schema/analyzer. Added Windows GUI-thread synchronization and cmdlet-independent SHA-256 before any successful live arm. |
| Lifecycle capture numbering and FILETIME fix | Integrated in `tools/capture_title_save_lifecycle.py`; stages reserve monotonic directories atomically. |
| Supplied 75 observer/capture tests | Integrated; the prior Windows integration ran both Lua DLL and Lupa modes. The subsequent identity-attestation regression adds one case. Mock tests are not a live observer acceptance claim. |
| Live C0 | Attempted; selected CE profile had no events and old-process cleanup was unverified. Preserved as incomplete, not passed. |
| C1-C3, title authoritative inventory/request-thread/generation acknowledgement | Not implemented or accepted. Research is paused by the owner's later decision; not a forgotten production patch or current release prerequisite. |

## Additional restore transaction defect closed in this audit

The Pro patch repaired F02/F04 in append transactions, but the older, separate
`SaveInstaller.restore_backup` still had equivalent race windows. This was an
additional code path, not a failed application of the supplied patch.

The restore transaction now:

- captures the full initial main/backup/system generation, including absent
  files, and binds its automatic checkpoint to those exact hashes;
- keeps each selected backup's manifest digest fixed through staging instead
  of accepting a new digest after a backup changes;
- checks the full expected generation before and after each replacement,
  accounting only for its own verified changes;
- verifies every required rollback checkpoint and the whole current generation
  before undoing any replacement, then checks again after rollback staging;
- preserves a later game write or changed checkpoint as an uncertain outcome,
  retains the first error, and attaches source/checkpoint/journal identity for
  automatic support diagnostics.

The title-screen restore entry remains enabled. These guards do not claim to
prevent game writes after the method returns or remove the last compare/rename
window; that remains part of the owner's accepted native-ownership limitation.

`tests/test_v073_restore_races.py` adds eight synthetic-file fault cases. Four
were also run against the actual original Pro archive's unmodified restore
method, loaded in an isolated Python process: three failed assertions and one
raised the old unsafe-rollback error, as expected. The current implementation
passes all eight. The existing safe-failure rollback and checkpoint-restoration
tests also remain green.

## Verification performed during this audit

Explicit Python executable:

```text
C:/Users/oudeb/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe
```

```powershell
& $python -m unittest tests.test_v072_save_races tests.test_v072_live_lifecycle -v
# 37 passed in 4.431 seconds.

& $python -m unittest tests.test_v073_restore_races tests.test_backend_freeze.SaveTransactionFreezeTests tests.test_v2_operations.SaveOperationsTests tests.test_save_commit_guard tests.test_v072_save_races -q
# 44 passed in 12.491 seconds; includes eight new restore cases.
```

The two runs overlap and their counts must not be summed as unique tests.
All fixtures used isolated temporary files. This audit did not write a real
game save or inject a game process. Final clean-candidate, package, updater and
publication evidence is recorded by the release workflow, not implied by this
review document.
