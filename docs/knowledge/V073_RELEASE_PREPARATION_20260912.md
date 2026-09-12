# v0.7.3 release preparation

## Reviewed scope

The candidate incorporates both returned Pro packages and the additional
backup-restore race correction documented in
[the item-by-item closure](V073_PRO_REVIEW_CLOSURE_20260912.md).
This is a preparation record, not proof of publication.

The title-screen append/restore policy follows the owner's accepted-risk
decision. No native title-save ownership protocol or demonstrated fix for the
reported corruption is claimed. Possessed-enemy research and the broader
actual-progression matrix remain deferred.

## Local source evidence

Validation used Python 3.12.14 at
`.codex_tmp/v2-build-env/Scripts/python.exe`, installed from
`packaging/requirements-v2.lock.txt` plus `requirements-dev.txt`.

| Check | Result |
| --- | --- |
| Full Python unittest discovery | Final scope: 624 passed, 91.267 seconds |
| Explicit CPU-only runner | 123 tests passed, two hardware-related skips |
| Title observer/capture pytest | 75 passed, one unavailable Lua-DLL-path skip |
| Rust tests with explicit `NIOH3_PYTHON` | 11 passed |
| Native isolated-process fixture and five fault modes | Passed with the real fixture process-creation identity |
| TypeScript, generated contracts/locales and native manifest | Passed; no unexpected generated changes |
| Original Pro regression subset | 37 passed; overlaps the full suite |
| Additional restore race regressions | Eight passed; four exposed the original restore defect |

The initial Node run passed 50/51. Its synthetic save-edit test was correctly
refused because the real Nioh 3 process was running. The full suite must be
rerun after normal game exit; weakening that production gate is not a fix.
The first Rust attempt omitted the explicit Python environment and the old
native fixture omitted the new process-lifetime argument; both harness issues
were corrected before the successful runs above.

Local logs and raw acceptance records are retained under `.codex_tmp/v073-*`
and `deliverables/releases/0.7.3-validation/`, outside the public source tree.
Private save contents and account paths are excluded from the release commit.

## Bounded live-game acceptance

PC v2.01 was loaded at a shrine. The product materialization,
`LiveAddApplication`, and native adapter inserted one R3 scroll, seed
`10033001`, with automatic backup, one dispatch, verified cleanup, full
container/index verification and all 41 previous records preserved.

The user then performed a normal shrine save and title-screen reload.
Independent read-only persistence verification matched all 42 saved records
against both the post-insertion evidence and the reloaded inventory, including
serials and defined fields. The native serial index matched. Verification did
not modify the source save.

This proves the observed insertion and persistence path. It does not prove all
mission states, forced native faults against the game, or the causal resolution
of every intermittent player report. Computer Use keyboard injection did not
reliably activate the shrine, so the normal save/reload step was user-operated
and is explicitly attributed as such.

## Final scope clarification and review hold

Before committing, the owner clarified that permanent edits must also work at
the title screen without closing the game. The application and editor now
allow append, edit, delete and restore under the same title-or-closed
acknowledgement. Existing transaction protection remains in place. A focused
37-test save-policy/transaction suite passed, including running-process edit
and delete simulations, backups, read-only preparation and stale-file refusal.

The updated debug WebView2 acceptance passed actual UI edit/delete/restore
against a synthetic encrypted save. Each write produced the expected complete
backup, and the invalid read request automatically replaced the real clipboard
with backend diagnostics. These checks do not claim a live-game disk-edit
experiment. The earlier Node environment failure is resolved: all 51 tests
passed after normal game exit, before the final policy expansion.

The owner subsequently requested review before any push or publication.
Prepare a local candidate from an isolated clean checkout and validate its
packaged workers, WebView2, installer/uninstaller and updater replacement/cleanup.
Do not dispatch hosted builds, push commits, create tags or publish an update
until the owner reviews the concrete change list and authorizes release.
Record the candidate SHA, artifact hashes and final checks in a separate local
review report. A signed public update manifest remains a future publication
step; do not substitute a test signature or advertise an unpublished feed.
