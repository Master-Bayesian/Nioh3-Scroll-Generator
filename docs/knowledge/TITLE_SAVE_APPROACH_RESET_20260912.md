# Title-save investigation: approach reset

## Final pre-release clarification on 2026-09-12

The owner explicitly requires save-file addition and permanent scroll editing
to remain usable at the title screen without closing the game. The editor's
transactional edit/delete actions therefore use the same title-or-closed
acknowledgement as append and restore. Automatic backups, reviewed source
hashes, generation checks and durable no-replay receipts remain mandatory.
This extends the restore-only clarification below; process presence alone
cannot identify whether the player is at the title screen.

The owner also requires an explanation of the completed changes and a local
review candidate before any push or publication. Do not publish this candidate
until the owner reviews those changes and authorizes the subsequent release.

## Current product decision: recovery-backed risk accepted

On 2026-09-12, after the approach reset below, the project owner accepted the
unreproduced title-save corruption report and delayed-overwrite exposure as a
known product risk, relying on the application's automatic backups and restore
workflow. Title-screen append remains available. This concern alone no longer
blocks release, and implementing a native save/cache-ownership protocol is not
a prerequisite for the follow-up release.

Pause C1/C2 reproduction, broad tracing, and further native-save research.
Preserve the existing evidence and Pro findings for a future concrete failure
report or explicit research request. No corruption fix or game-native ownership
guarantee has been demonstrated; the owner's risk acceptance changes release
policy, not the evidence. Do not relabel incomplete observer captures as passed.

Keep automatic backup validation, transaction diagnostics, generation checks,
and uncertain-result handling. The owner subsequently clarified that backup
restoration may run at the title screen; closing the game is not required.
This restores the v0.6.10 restore-entry policy. The UI confirms title-screen or
closed state, while the application keeps source/backup identity checks and a
pre-restore checkpoint. This is a product policy, not a native state detector.
Restoration returns to the selected backup's contents; subsequent progress is
not guaranteed to survive. Backup integrity and restore defects remain ordinary
release concerns. Other verification requirements are not waived, and this
decision does not by itself establish overall release readiness.

The restore-entry correction is implemented in SaveApplication, the legacy Tk
restore action, and both Workshop restore confirmations. The focused 28-test
save-operation/transaction suite passed, including a simulated running-process
restore with a pre-restore checkpoint and durable no-replay verification.
TypeScript checking and the 560-message localization audit passed. The connected
UI test now expects the title-screen-or-closed confirmation; its full UI run and
a live-game restore were not performed for this policy correction.

The earlier decision and experiment plan below are retained as deferred history.

## Earlier decision (superseded)

The next experiment must isolate the reported failure stage. Stop broad WPR
collection, full-ETL buffered parsing, and further tracing-tool development.
Preserve the existing ETL locally without making its analysis a prerequisite.
This document supersedes the mandatory C0 writer-hit prerequisite and the
preselected native-protocol implementation in the original research plan.
Publication remains blocked; no title-save fix has been accepted.

## Evidence and correction

- The Pro review's delayed-flush test explicitly writes old bytes back using a
  simulated writer. It proves a missing concurrency guarantee, not the cause
  of the player's failed load.
- Static PC v2.01 analysis recovered a real asynchronous save chain. It does
  not establish that a cold title exit serializes the character inventory.
- C0.files.01 reached title, exited normally, and cold-started a second process
  without an application mutation. The selected CE files profile recorded no
  events. Cleanup could not be reverified against the exited process, so its
  analyzer correctly retained an incomplete result.
- Between the prelaunch and title snapshots, BACKUP.BIN changed to match the
  main save. Subsequent captured main/backup/system files were unchanged.
  The writer is not attributed. Second-process responsiveness is recorded;
  visual title confirmation is not.
- The merged ETL is 7,899,971,584 bytes over about 32 minutes. Broad collection
  and reader troubleshooting added cost without resolving the causal question.
  No lightweight replacement reader has successfully extracted the evidence.
  Preserve the ETL and failed-reader diagnostics as unprocessed local evidence.

## Smallest next experiment

Use the existing lifecycle collector and production append entry point. Record
the exact source/build identity; patched-source results must not be relabeled
as results for the player's original binary. Do not change game functions,
memory flags, or save behavior to make the experiment pass.

1. Start with a known cold-title state and a verified automatic three-file
   backup. Capture the three original encrypted files locally, not just hashes.
2. Append one uniquely identified test scroll once. Preserve the operation
   receipt and capture the files immediately after the application returns.
3. Exit normally without loading a character. Capture the files after the
   process exits, before restarting it.
4. Cold-start to title and capture again, then attempt to load the character.
   Record actual acceptance or the exact error. Decode captured copies offline
   and compare the appended record, all existing inventory records, file
   structure, and related-file changes.

This is one C1 reproduction cycle, not a claim that file stability proves
shared save ownership. Stop immediately on a failed backup, ambiguous commit,
unexpected existing-record change, or a failed load. Preserve the state; do not
repeat the append or automatically restore over a potentially newer save.

If C1 succeeds, perform at most one C2 cycle (load a character, return to title,
append, exit without loading again, cold-start). If neither reproduces the
problem, stop local reproduction and deliver the negative evidence to Pro.
Do not escalate into repeated C0 runs or infer that the report is fixed.

## Evidence-driven branches

- Invalid immediately after append: investigate the application transaction,
  serialization, checksum, inventory structure, and related-file handling.
- Changed on exit or cold start: target that observed interval and writer.
  Use an existing bounded observer, after a relevant positive control, rather
  than collecting all system activity. Identify ownership only as required to
  explain and correct the observed transition.
- Unchanged on disk but rejected by the game: inspect load validation and
  cross-file/object consistency; do not assume an overwrite.
- No reproduction: keep the user report unresolved and ask Pro to distinguish
  outstanding concurrency risks from the unobserved corruption cause.

Codex collects these bounded vectors and packages source, raw relevant evidence,
negative results, and a concrete Pro task. Pro performs unresolved causal and
binary analysis. A native save/reload protocol remains a candidate repair;
its prerequisites must be proved before implementation. Any alternative also
needs a defensible concurrency argument, regression coverage, and direct-exit,
cold-start, normal-save/reload acceptance. A few successful cycles alone do not
justify publication or removal of the known concurrency concern.

No new live experiment was performed during this decision reset.
