# Experiment failure ledger

This is a non-canonical engineering notebook for failed or abandoned research
approaches. It exists to preserve evidence without turning each failure into a
project rule, current conclusion, or Codex instruction.

Do not load this file for routine development. Consult it only when planning a
closely related experiment or reviewing whether a failure pattern has repeated.

## Promotion policy

- Record the research question, failed approach, root cause, evidence location,
  and disposition. Do not turn symptoms into universal advice.
- Raw captures remain under `audit/`; this ledger summarizes rather than copies
  them.
- A lesson may move to a skill or runbook only after it recurs independently or
  a verified invariant shows that it applies broadly.
- Accepted conclusions belong in `docs/knowledge/`, not here.

## 2026-09-13: downstream mode branch mistaken for the upstream mechanism

**Question:** Why does seed `86872488` expose different enemy configurations in
normal solo and one-person expedition?

**Failed approach:** A live observer was designed around configured-count
branches inside `0x1029E80` before proving where the session mode was encoded or
where the generator request was constructed.

**Root cause:** The selected sites were downstream consequences. They could
describe one local branch but could not identify the upstream mode/session
object or isolate the causal input. The expedition and solo request snapshots
also differed in more than the assumed byte.

**Secondary symptoms:** Two attempts exposed ordinary observer implementation
defects, but fixing those defects would not repair the causal-site mistake.

**Evidence:** Raw Run D and the two subsequent normal-solo attempts are retained
under `audit/possessed_enemy_capture/86872488/`. The validated assignment-origin
result remains current evidence; the normal-solo attempts are not mode-mechanism
evidence.

**Disposition:** The downstream mode collector, comparator, and active
instructions were removed. The static sections, prior Pro analysis, validated
Run D, and raw attempts were packaged for a new Pro analysis of the correct
upstream mechanism. The failed downstream interpretation is not a current
mode-mechanism conclusion.

**Reproduction status:** Reproduced across the attempted mode-materialization
line of inquiry; not independently reproduced as a native causal mechanism.
The secondary ECX-width and repeated-hit observations are symptoms of the
collector and do not establish the root cause.

**Follow-up state:** Closed as a failed approach. Do not load this entry by
default; consult it only when designing a closely related upstream experiment.

**Skill promotion:** None. Revisit only if an independent research task shows
the same pattern or Pro establishes a reusable probe-selection invariant.

## 2026-09-13: repeated test-environment selection failure

**Objective:** Run the bounded possessed-enemy sequence regression suite in the
repository's prepared test environment.

**Symptoms:** The first attempt used `D:\Python\python.exe`; `pytest` was
missing and collection reported four errors. A second attempt used `uv run
--with pytest`; `pytest` was present, but the Lua 5.4 shared library was
unavailable and collection failed. A later probe using the bundled Codex
Python exposed an incomplete user-site `pytest` installation (`pluggy` was
missing).

**Root cause:** Ambient interpreters were invoked instead of the repository's
prepared dependency environment, and the Cheat Engine Lua shared-library
binding was omitted. These are test-environment selection/configuration
failures, not project test failures.

**Evidence:** A valid direct rerun used
`.codex_tmp\\title-save-test-env\\Scripts\\python.exe` with
`LUA54_LIBRARY=C:\\Program Files\\Cheat Engine\\lua53-64.dll` and reported
`161 passed in 1.33s`. The new tracked wrapper selected
`.codex_tmp\\v2-build-env\\Scripts\\python.exe`, resolved the same CE Lua
library, and reported `161 passed in 1.31s`.

**Disposition:** The repository now has a deterministic test wrapper and a
direct `AGENTS.md` rule requiring it. Preserve this entry as failure evidence
only; do not promote it to a skill or project conclusion.

**Reproduction status:** Reproduced across multiple ambient interpreter
selection attempts; valid rerun succeeded in the prepared environment.

**Follow-up state:** Closed. `tools/run_python_tests.ps1` and the direct
repository guidance now provide the deterministic entry point.

**Skill promotion:** None.

## 2026-09-14: v0.7.4 rebase whitespace cleanup broke hash-bound capture replay

**Objective:** After the v0.7.4 rebase, remove the trailing blank line that
`git diff --check` reported while preserving reproducible live-parameterized
session replay.

**Symptom:** The complete Python test
`tests/test_mode_transaction_join.py::test_live_parameterized_session_transaction_is_reproducible_from_raw_capture`
failed with `ValueError: capture source hash mismatch`.

**Root cause:** The hash-bound source file
`research/possessed_enemy_capture/mode_transaction_join_ce.lua` had its
trailing blank line removed. That changed the original collector bytes bound
by `capture_metadata.source.phase_sha256` in
`live_parameterized_86872488.json`.

**Evidence:** The failing test and the capture metadata/source pair identified
above.

**Disposition:** Restore the original bytes of the hash-bound source file and
exclude the exact original/hash-bound file from generic whitespace validation.

**Reproduction status:** Reproduced once after the rebase.

**Follow-up state:** Resolved. The focused project runner passed
`tests/test_mode_transaction_join.py`, `tests/test_entry_transaction_evidence.py`,
`tests/test_mode_upstream_sequence.py`, and
`tests/test_materialization_frontier.py` (`181 passed in 2.27s`).
`.gitattributes` pins `research/possessed_enemy_capture/*.lua` to LF, and
hash-bound source/fixture replay passed.

**Skill promotion:** None.

## 2026-09-14: v0.7.4 Electron acceptance-harness cleanup did not settle

**Objective:** Verify the v0.7.4 startup update prompt and Settings manual
check in an isolated Electron UI run.

**Symptom:** All assertions passed, `verification.json` was written, and the
success marker was printed, but `Playwright electronApplication.close()` did
not settle after more than 60 seconds on the second run. Only test-owned
Electron/Node processes remained.

**Root cause:** Unknown; the symptom likely belongs to the shutdown path or
worker close handling, not the update UI logic.

**Evidence:** `deliverables/v074-update-acceptance-20260914/verification.json`;
the command `npx tsx apps/workshop/verify-hotfix.mjs`; and executable/command-
line validation showing that the remaining processes were test-owned before
they were terminated.

**Disposition:** The verifier now exits its isolated Electron test host
directly with `app.exit(0)`, waits boundedly, and retains a validated
process-tree fallback. The acceptance evidence remains valid; root product
shutdown is out of scope because this was a mocked legacy Electron UI host,
not the shipped Tauri executable.

**Reproduction status:** Resolved on rerun in 20.0 seconds with
`V074_JAPANESE_STARS_UPDATE_PROMPT_OK` and no forced-cleanup warning.

**Follow-up state:** Closed. The bounded exit and validated process-tree
fallback are in place; the rerun completed successfully.

**Skill promotion:** None.

## 2026-09-14: low-pool-only Possessed UI acceptance used a stale Tauri frontend

**Objective:** Verify the new low-pool-only Possessed UI, including its user-facing
note and state restrictions, through the native Tauri harness.

**Symptom:** All earlier preview checks passed, but the native harness timed out
while waiting for the new note text assertion in
`apps/tauri/verify-enemy-states.mjs`.

**Root cause:** The rebuild ran `npm run build` plus Cargo, which updated
`apps/desktop/dist` but did not run the project Tauri frontend build entry
`node apps/tauri/build.mjs`. The debug Tauri executable therefore embedded the
stale `apps/tauri/dist` frontend.

**Evidence:** `apps/tauri/verify-enemy-states.mjs` timed out at the new text
assertion; `apps/desktop/dist` had newer timestamps and contained the new
string, while `apps/tauri/dist` had the older timestamp and did not contain it.

**Disposition:** Use the project Tauri frontend build entry before rebuilding
Cargo, then rerun the native harness. This records a build-entry/acceptance
failure and does not imply a Possessed filtering or UI-contract conclusion.

**Reproduction status:** Confirmed.

**Follow-up state:** In progress.

**Skill promotion:** None.

## 2026-09-13: v2 handoff patch used the wrong EVIDENCE_MAP context

**Objective:** Apply the multi-file v2 handoff documentation patch.

**Symptom:** Whole-patch validation failed because the expected context lines
for `EVIDENCE_MAP.md` were at the wrong location. `apply_patch` changed no
files.

**Root cause:** The target sentence was assigned to the wrong section context.

**Disposition:** Split the patch and locate each edit against the actual file
content before retrying.

**Skill promotion:** None.

## 2026-09-13: v2 handoff packaging used LiteralPath with a wildcard

**Objective:** Create the v2 handoff package with assignment-origin Pro return
and `resource_v3` contents.

**Symptom:** Two `Copy-Item` calls combined `-LiteralPath` with `*`, so
PowerShell treated the asterisk literally. Those contents were not copied;
the v2 directory existed and other explicitly named files copied successfully.

**Root cause:** `-LiteralPath` does not expand glob patterns.

**Disposition:** Enumerate source-directory items and copy each with
`Copy-Item -Recurse`, retaining and completing the existing v2 directory.
There is no evidence of source-file or evidence corruption.

**Skill promotion:** None.

## 2026-09-13: final ZIP hash patch had an invalid hunk boundary

**Objective:** Synchronize the final ZIP hash in the delivery documentation.

**Symptom:** `apply_patch` failed during patch validation because the patch
contained an extra empty `@@` hunk. No file was changed.

**Root cause:** Invalid patch format.

**Disposition:** Retry as three correctly formed update hunks.

**Skill promotion:** None.

## 2026-09-13: composite ZIP refresh command was rejected before execution

**Objective:** Refresh the final ZIP using a generate/verify/replace/cleanup
PowerShell command.

**Symptom:** Command safety policy rejected the composite command before any
execution because it combined `Copy-Item -Force` and `Remove-Item`. No files
changed; the old ZIP remained intact.

**Root cause:** Generation, verification, replacement, and cleanup were
combined into one command containing destructive operations.

**Disposition:** Separate non-destructive candidate generation and verification
from precise replacement and cleanup steps.

**Skill promotion:** None.

## 2026-09-13: CE readiness used a session that disconnected during setup

**Objective:** Retry CE readiness after fixing the temporary script's
repository import path.

**Symptom:** The bridge query executed successfully, but session `ce-12064`
was no longer present in the session list. The script failed closed with
`RuntimeError: CE session is not connected`. No attach, debugger startup, or
breakpoint placement occurred.

**Root cause:** The task script started the bridge but did not wait for the
existing CE plugin to reconnect, so the session list was temporarily empty.

**Disposition:** Add a bounded reconnect wait of at most 20 seconds, then use
the returned session ID. The retry connected to `ce-12064`, attached to new
game PID `26892`, ran with VEH interface 2, and confirmed an empty breakpoint
list. All preceding failures stopped before breakpoint placement.

**Reproduction status:** Reproduced in the readiness retry; no game-side probe
was attempted.

**Follow-up state:** Resolved after bounded reconnect and attach readiness
verification.

**Skill promotion:** None.

## 2026-09-13: CE readiness temporary script missed the repository module path

**Objective:** Retry CE readiness through the task-local temporary script after
the inline quoting failure.

**Symptom:** Starting `.codex_tmp/ce_readiness.py` by file path failed during
import with `ModuleNotFoundError: research`. Because `sys.path[0]` was
`.codex_tmp`, the repository root was not available. No bridge call was made
and there was no CE or game-side impact.

**Root cause:** The temporary script did not explicitly add its parent
repository root to `sys.path` when launched from `.codex_tmp`.

**Disposition:** Add the repository root explicitly to `sys.path` before
imports, then rerun the readiness script.

**Reproduction status:** Reproduced at import time only; no runtime probe was
attempted.

**Follow-up state:** Open pending the corrected-script retry.

**Skill promotion:** None.

## 2026-09-13: CE readiness probe failed during PowerShell inline quoting

**Objective:** Use the project Python to call the local bridge and complete
attach/debugger readiness.

**Symptom:** PowerShell failed while parsing nested `-c` quoting for the
complex multi-line bridge operation. Python did not run; no CE connection or
modification, breakpoint placement, or game-side effect occurred.

**Root cause:** A multi-line bridge operation was embedded in a shell
`python -c` command, making the nested quoting invalid at the PowerShell
parsing stage.

**Disposition:** Do not retry this inline form. Use the existing runner/bridge
tool's native interface or a task-local temporary script.

**Reproduction status:** Reproduced at PowerShell parse time only; no runtime
probe was attempted.

**Follow-up state:** Open pending the native-interface or temporary-script
retry.

**Skill promotion:** None.

## 2026-09-13: entry-transaction evidence tests retained the Pro source layout

**Objective:** Run the two newly integrated modules from
`Nioh3_Entry_Transaction_v201_20260913` through
`tools/run_python_tests.ps1`.

**Observed result:** The first integration run reported `42 passed, 16
failed`. All 42 `tests/test_mode_transaction_join.py` tests passed. All 16
failures came from `tests/test_entry_transaction_evidence.py`, whose default
evidence root resolved to `F:\evidence\...` and could not find the four raw
captures.

**Root cause:** The Pro attachment's tests assumed its independent `source/`
layout (`parents[2]/evidence`) and were not adapted to the repository's
authoritative `audit` paths. This is an integration-layout failure, not a
research or evidence conclusion.

**Disposition:** Precisely copied the four authoritative read-only captures
into self-contained `tests/fixtures/entry_transaction/` fixtures, added
`PROVENANCE.md`, and changed the test's default root to that repository-owned
fixture directory while retaining the `NIOH3_FRONTIER_EVIDENCE` override.
The official wrapper rerun of both modules reported `58 passed in 0.37s`.

**Reproduction status:** Reproduced in the first repository integration run.

**Follow-up state:** Resolved after the fixture-backed rerun passed.

**Skill promotion:** None.

## 2026-09-13: skill validator used a project environment without PyYAML

**Objective:** Validate the updated `nioh3-ce-research` runtime skill using
the system `skill-creator/scripts/quick_validate.py` required by AGENTS.

**Symptom:** Running the official validator with the project's fixed Python
`.codex_tmp/v2-build-env/Scripts/python.exe` failed with
`ModuleNotFoundError: yaml`.

**Root cause:** The system skill validator's PyYAML dependency is not part of
the project's test environment. This is an environment mismatch, not a
failure in the skill content.

**Disposition:** Without modifying the project virtual environment, use the
workspace dependency Python with task-local
`.codex_tmp/skill-validator-deps` containing `PyYAML==6.0.2`; inject that
dependency via `sys.path` and run the same official validator.

**Reproduction status:** Reproduced with the project fixed Python environment.

**Follow-up state:** Resolved. The official validator returned `Skill is valid!`.

**Skill promotion:** None.

## 2026-09-13: materialization-frontier Run B produced no observable events

**Objective:** Collect materialization-frontier evidence for seed `86872488`
after arming the observer.

**Observed result:** Run B at
`audit/possessed_enemy_capture/86872488/20260913-materialization-frontier-b/`
mounted all four observation points successfully through VEH interface 2 and
ran for `120.344s`, but recorded `0 events`, `0 hits`, and `0 invocations`.
Cleanup was verified: `verified=true`, observer inactive, owned and global
breakpoint inventories empty, and `debugger_broken=false`.

**Validation boundary:** The validator correctly rejected the capture because
`empty/oversized events are not mechanism evidence`. The user later confirmed
that they did not enter the scroll within the observation window because prior
debugger troubleshooting had taken too long; this does not implicate the probe
sites.

**Root cause:** The user did not perform the entry action within the window
after ARMED because preceding troubleshooting consumed the available time.
This is an incomplete/invalid research capture, not evidence that
materialization did not occur or evidence for any other mechanism conclusion.

**Disposition:** Preserve the raw capture and cleanup evidence. The procedural
lesson is to complete troubleshooting before arming and trigger promptly once
ARMED. Run C later produced 22 events, closing the zero-event reproduction;
do not promote a mechanism conclusion from Run B.

**Reproduction status:** Run B's zero-event capture is explained by the
missing in-window trigger; a later Run C with 22 events provides closure of
this reproduction. Native mechanism status is not inferred from Run B.

**Follow-up state:** Closed as a procedural no-trigger capture; Run C supplied
valid follow-on observations.

**Skill promotion:** None.

## 2026-09-13: materialization-frontier CE setup reused an occupied port

**Objective:** Prepare the CE bridge for the materialization-frontier run.

**Symptom:** `mcp ce_bridge_status` showed an existing backend on
`127.0.0.1:5556` with `session_count=0`. The primary agent then ran
`ensure_ce_mcp.py` without arguments, which failed with Windows
`OSError`/`Errno 10048` because port `5556` was already occupied.

**Root cause:** The existing runtime.md branch was not followed: when `5556`
belongs to an active backend, setup must use `--port 5566`, and subsequent
runners must use the same port and session.

**Disposition:** Do not retry port `5556`; switch immediately to the isolated
port `5566` and carry that port/session through the runner. The rule already
exists in runtime.md; no skill change is warranted.

**Reproduction status:** Reproduced in the current preparation attempt.

**Follow-up state:** Closed as a CE setup/process failure; no native or
materialization conclusion is inferred.

## 2026-09-13: returned-Pro augmentation test used an incompatible import layout

**Objective:** Run the returned Pro package's augmentation frontier contract
test through the repository's prepared Python test entry point.

**Symptom:** `tests/test_augmentation_frontier_contract.py` initially imported
`test_materialization_frontier` as a top-level module. With
`tools/run_python_tests.ps1` collecting the repository's `tests` package,
pytest failed during collection with
`ModuleNotFoundError: No module named 'test_materialization_frontier'`.

**Root cause:** The returned package used an independent `source/` test layout,
whose top-level import convention differs from the repository's `tests`
package import convention. This was an integration/import-layout mismatch,
not a research implementation failure or Cheat Engine failure.

**Evidence:** The failed pytest collection output from
`tools/run_python_tests.ps1`, identifying the import in
`tests/test_augmentation_frontier_contract.py` and the missing
`test_materialization_frontier` module.

**Disposition:** Change the import to
`from tests.test_materialization_frontier import ...` and rerun through the
repository wrapper. Final verification is recorded as `297 passed, 3 skipped
in 3.04s`; this import-layout failure is closed.

**Reproduction status:** Reproduced during the one-time integration attempt;
the corrected import made test collection succeed. The final corrected-rerun
result was `297 passed, 3 skipped in 3.04s`.

**Follow-up state:** Closed for the import-layout failure. The implementation
and offline exporter passed the bounded regression; no game or Cheat Engine
runtime test was performed.

**Skill promotion:** None.

## 2026-09-13: augmentation closure tests required unavailable native tools

**Objective:** Run the augmentation regression suite after correcting the
returned package's repository import layout.

**Observed result:** The initial related regression run reported `297 passed,
2 failed, 1 skipped`; after the bounded test handling fix, the final run
reported `297 passed, 3 skipped in 3.04s`. The three skips had explicit
boundaries: two optional GNU objdump decoding tests skipped because the Windows
environment lacked the tool, and one isolated raw-machine-code harness skipped
because it supports Linux x86-64 only. The offline exporter itself successfully
exported 8 groups from the fixed `.text`/`.rdata`/`.pdata` inputs.

**Symptom:** Both failures came from the new
`test_augmentation_code_closure.py`, whose two explicitly objdump-dependent
tests unconditionally invoked optional GNU `objdump`. Neither `objdump` nor
`llvm-objdump` was available on PATH or in common Windows installation paths,
and the project Python environment had no `capstone` package. Other boundary
tests in the same file were therefore not independently at fault. Linux native
fragment tests were already designed to skip.

**Root cause:** Optional native disassembly tooling was treated as mandatory by
the two tests in the Windows project dependency environment. This was a
repository test-environment integration failure, not an offline exporter or
Cheat Engine failure.

**Evidence:** The corrected regression output showing `297 passed, 2 failed,
1 skipped`, the missing-tool diagnostics for `objdump`/`llvm-objdump` and
`capstone`, and the successful 8-group offline export.

**Disposition:** Make only the two tests with an explicit `objdump` dependency
skip with a clear reason when the tool is unavailable. Do not skip unrelated
boundary tests in the same file.

**Reproduction status:** Reproduced in the Windows project dependency
environment; corrected-rerun completed with `297 passed, 3 skipped in 3.04s`.

**Follow-up state:** Closed for this tooling-availability failure. The
implementation and offline exporter passed the bounded regression; no game or
Cheat Engine runtime test was performed.

**Skill promotion:** None.

## 2026-09-13: returned-Pro review found the mode-upstream collector incomplete

**Question:** Is the `Nioh3_Mode_Upstream_v201_20260913` four-site collector
ready for live use?

**Failed approach:** The returned collector and validator accepted the selected
causal sequence without proving every documented producer-local address/stack
relation or the outer arm-verification boundary. Ignored and nonmatching
callback paths were also outside a global hit/deadline budget, allowing
high-frequency no-read hits to continue until timer scheduling.

**Root cause:** Tests and validation focused on the successful causal sequence
and selected error cases without asserting that every callback path consumes a
global bound.

**Evidence:** Returned package files
`Nioh3_Mode_Upstream_v201_20260913/mode_upstream_ce.lua`,
`Nioh3_Mode_Upstream_v201_20260913/validate_mode_upstream_capture.py`, and
`Nioh3_Mode_Upstream_v201_20260913/tests/test_mode_upstream_lua.py`, together
with the independent collector review.

**Disposition:** Patch before integration or live use. Add total callback
hit/deadline checks, offline producer and arm-proof checks, and tests covering
inactive, error, ignored, and bound continuation paths.

**Reproduction status:** Static code review only; no live CE reproduction.

**Follow-up state:** Open until the collector is repaired and targeted tests
and static verification pass.

## 2026-09-13: default CE MCP port was already occupied

**Objective:** Establish the CE MCP bridge with the approved
`ensure_ce_mcp.py` procedure before the mode-upstream capture.

**Symptom:** Binding `127.0.0.1:5556` failed immediately with Windows
`OSError 10048` because the port was already owned; the current task had no
callable surface for that backend. An existing CE process (PID `40264`) was
present, but no Nioh3 process was attached.

**Root cause:** The existing backend listener owned the default port. This was
an environment/bridge-ownership condition, not a CE collector defect.

**Evidence:** The exact `ensure_ce_mcp.py` command and output from 2026-09-13,
including the existing CE PID `40264` and the absence of a Nioh3 process.

**Disposition:** Preserve the existing backend and process. Use the documented
isolated port `5566`, then pass that same port and the selected session
explicitly to the runner.

**Reproduction status:** Reproduced in the current local environment.

**Follow-up state:** Open until the isolated bridge is established.

## 2026-09-13: CE session retained an exited prior target

**Objective:** Arm the mode-upstream runner for Nioh3 PID `26476` through CE
session `ce-40264` on port `5566`.

**Symptom:** The runner failed closed before arming with the exact error:
`Cheat Engine is already attached to PID 35580; refusing to retarget it to
26476`. No breakpoint was armed and no game action occurred.

**Root cause:** The CE session retained stale target identity from the exited
prior game process (`35580`) across the game restart.

**Evidence:** The current runner output for PID `26476`, session
`ce-40264`, port `5566`, including the target-identity refusal above.

**Disposition:** Preserve the old session. Establish an approved isolated fresh
CE session on port `5567`, then select its exact session ID explicitly before
retrying.

**Reproduction status:** Reproduced in the current run.

**Follow-up state:** Open until the new session arms and its cleanup is later
verified.

## 2026-09-13: first mode-upstream run was procedurally invalid

**Objective:** Prepare and run the bounded mode-upstream capture through
isolated CE session `ce-14644` on port `5567`, attached to current Nioh3 PID
`26476`, for run `mode-upstream-86872488-expedition-a-20260913`.

**Symptom:** Fresh-phase initialization first failed closed with the exact
error `Debugger event is still stopped`; no observer breakpoint was armed at
that point. After the stopped event was resumed, arm verification succeeded
with all four owned breakpoints. The owner later confirmed that no mission
entry was performed during the 210-second window.

**Root cause:** The initial preparation failure was the stopped CE debugger
state. The run itself was procedurally invalid because no trigger action was
performed. It is not evidence that the native path failed.

**Evidence:** `audit/possessed_enemy_capture/86872488/20260913-mode-upstream-a/`
`mode-upstream.json` and `mode-upstream.cleanup.json`, plus the owner's
confirmation that no mission entry occurred. The files record successful
ARMED verification for four owned breakpoints and cleanup verification.

**Disposition:** Classify Run A as procedural no-trigger / invalid experiment,
not native-path failure. Cleanup was verified true with an empty breakpoint
inventory and `debugger_broken=false`.

**Reproduction status:** The stopped-event preparation symptom was reproduced
and cleared. The no-trigger procedural condition is confirmed for this run.

**Follow-up state:** Superseded by the successful Run B capture below; no mode
mechanism conclusion is promoted from Run A.

## 2026-09-13: mode-upstream Run B captured the producer request

**Objective:** Execute a bounded one-person-expedition mode-upstream capture
after ARMED confirmation for seed `86872488`.

**Observed result:** Run `mode-upstream-86872488-expedition-b-20260913`
returned four events. The `owned_scroll_branch` return was RVA `0xF1E4F1`;
request was `A8912D05B700010301000000`, with `+0x9=0`, tail `0000`, generator
extra `0`, and context `0x8E` / path 2 / counts `[1,1,1,1,0]`. The validator
passed and cleanup was verified.

**Evidence boundary:** The mode is owner-observed one-person expedition. This
is not a native mode-enum capture and does not by itself identify a universal
mode field or settle the mode mechanism.

**Follow-up state:** Requires Pro reconciliation with Run D's
`+0x9=1`, `+0xA=2`, and 10-task result before promoting a causal conclusion.

## 2026-09-13: mode-upstream handoff packaging used Path.with_suffix incorrectly

**Objective:** Package the mode-upstream live-result Pro handoff.

**Symptom:** Applying `Path.with_suffix('.zip')` to a directory name containing
`v2.01` generated the incorrect name `Nioh3_PC_v2.zip`.

**Root cause:** `with_suffix` treated `.01...` as the directory-name suffix.

**Disposition:** Use `root.parent / (root.name + '.zip')`; the incorrect file was
precisely removed and the correct ZIP was verified. No product or research
conclusion is affected.

## 2026-09-13: apply_patch added extra EOF bytes to packaged files

**Objective:** Preserve exact bytes while preparing the mode reconciliation
handoff.

**Symptom:** A dynamic patch operation added an extra blank EOF line to seven
new files, making each file 2 bytes longer and causing the package hash check
to fail.

**Disposition:** Precisely removed the redundant EOF blank lines; all 8/8
source byte matches then passed. This was a packaging-byte hygiene failure,
not a source or native-research finding.

## 2026-09-13: static verifier path and PowerShell exit status obscured failure

**Objective:** Run the static verifier against the mode reconciliation package.

**Symptom:** The verifier initially targeted a `disassembly` directory absent
from the Reconciliation package. A PowerShell semicolon then allowed a later
`py_compile` success to obscure the earlier command's failure status.

**Disposition:** Repointed verification to the prior archived Pro disassembly
directory and checked `$LASTEXITCODE` explicitly; verification passed. This
was a tooling/path and status-reporting failure, not a native conclusion.

## 2026-09-13: mode-upstream-sequence trigger arrived after the deadline

**Objective:** Observe multiple requests, consumers, and generator returns in
run `mode-upstream-sequence-86872488-expedition-a-20260913` using the unchanged
Pro collector and its 120-second boundary.

**Observed result:** The observer armed correctly with four owned breakpoints;
the window ended naturally. Cleanup was verified true, the global breakpoint
inventory was empty, and the debugger was not broken. Offline validation
reported `no_target_generation_return` with an empty `events` list, while the
raw capture recorded `total_hits=2`, `ignored_hits=0`, and `elapsed_ms=174703`.
The two target breakpoints therefore executed after the 120,000 ms hard limit;
the callback correctly only finished/resumed and did not invoke the handler.

**Root cause and boundary:** This was a late-trigger/deadline procedural
invalid, not a native-path negative. It does not prove that no later request
exists. A contributing process factor was that the primary agent used a
30-second first yield after runner start before sending the ARMED notice to the
owner, consuming part of the pre-trigger window.

**Disposition:** Repeat with the same Pro collector and 120-second boundary,
but use the Windows-minimum 10-second first `exec` yield, notify immediately
when ARMED, and have the owner trigger promptly from the final confirmation
screen.

**Evidence:**
`audit/possessed_enemy_capture/86872488/20260913-mode-upstream-sequence-a/`
`mode-upstream-sequence.json` and `cleanup.json`.

## 2026-09-13: mode-upstream-sequence Run B depended on an unscheduled CE timer

**Objective:** Complete the same-run sequence observation after prompt ARMED
notification and owner trigger for
`mode-upstream-sequence-86872488-expedition-b-20260913`.

**Valid observation:** ARMED and trigger timing were prompt. Four events
completed at elapsed `12922`/`25390` ms: one request, one invocation, and one
generated return. The `owned_scroll_branch` request was
`A8912D05B700010301000000`, with extra `0`; the generated result contained six
descriptors across waves `[1,1,2,2]`, class0=6 and class1=0.

**Failure and boundary:** CE `createTimer` did not schedule in the MCP/off-GUI
execution context. The Lua probe remained active until the outer runner's
210-second timeout. Cleanup was verified true and the global breakpoint
inventory was empty. Because the capture had no `stop_reason` and was an
active snapshot before forced stop, the official validator correctly rejected
it as an invalid completed-window claim. Do not infer that no later request
exists.

**Root cause:** The sequence collector relied on direct `createTimer` for
authoritative window closure despite repository guidance that CE timers are
unreliable off-GUI.

**Disposition:** `run_possessed_enemy_observer.py` now uses a Python
`time.monotonic` 120-second fallback to call
`p.stop('observation_window_elapsed')`, retains the Lua timer as backup, and
rejects an outer timeout at or below 120 seconds; targeted regression coverage
now reports 161 passed. This entry records the runtime failure and valid
partial observations, not a mode-mechanism conclusion.

**Evidence:**
`audit/possessed_enemy_capture/86872488/20260913-mode-upstream-sequence-b/`
`mode-upstream-sequence.json` and `cleanup.json`.

## 2026-09-13: returned-Pro augmentation export failed at an assumed function boundary

**Objective:** Run the returned Pro package's offline augmentation function-body
export against the pinned Nioh 3 v2.0.1.0 runtime sections.

**Symptom:** The package archive was valid, and the exporter was run from
`deliverables/Nioh3_Augmentation_Fork_v201_20260913/source/research/possessed_enemy_capture/dump_augmentation_function_bodies.py`
against `audit/runtime_sections/v2.0.1.0_20260902_title/Nioh3_v2.0.1.0.text.bin`
with SHA `F879...8023` and matching pdata SHA `928D...B904`. It failed closed at
target RVA `0x13684C` with `CaptureError: no unique exception-directory
function BEGIN at 0x13684c`.

**Output boundary:** Only
`captures/augmentation-code-v201/CAPTURE_FAILED.json` was produced; no
`FUNCTION_BODIES.json` was produced. The valid archive does not make this a
successful export.

**Root cause:** Pending exact pdata inspection. The failure is a substantive
tooling-assumption failure about runtime-function/chain-info boundaries, not
native mechanism evidence.

**Evidence:** The returned package archive, the pinned text and pdata sections
listed above, the exporter path above, and the preserved
`captures/augmentation-code-v201/CAPTURE_FAILED.json` artifact.

**Disposition:** Primary agent is diagnosing the exact runtime-function and
chain-info boundaries. Any corrected rerun must use a new output directory and
must preserve this failed artifact. Do not promote the failed assumption to a
skill or active research conclusion.

**Reproduction status:** Reproduced for the returned package and pinned runtime
inputs in the current offline run; exact boundary cause remains unverified.

**Follow-up state:** Open pending pdata inspection and a separately directed
corrected rerun.

**Skill promotion:** None.

## 2026-09-13: first materialization-frontier arm failed in debugger attach

**Objective:** Arm the materialization-frontier observer for seed `86872488`
at the confirmed one-person expedition screen.

**Symptom:** With the current game PID, port `5566`, and session `ce-40264`,
fresh initialization called `debugProcess(1)` from
`materialization_frontier_ce.lua` and failed to start the debugger. The runner
reported `Debugger attachment failed`; the CE UI reported failure to attach,
with Windows debugger start failure `87`. None of the four observation points
mounted successfully, and the user did not enter the mission.

**Root cause:** `celua.txt` defines `debugProcess(1)` as the Windows debugger
and `debugProcess(2)` as VEH. The observer had hard-coded `debugProcess(1)`,
so Windows debugger startup failed with error `87`.

**Evidence:** The runner failure output and the CE UI attachment dialog for
the current game/session preparation described above.

**Disposition:** Change the observer to use `debugProcess(2)` (VEH). Focused
regression coverage passed 51 tests, and the subsequent Run C armed
successfully and completed. This closes the attach failure without implying a
game or mechanism conclusion.

**Reproduction status:** Reproduced during the first formal arm attempt; no
observation window started.

**Follow-up state:** Closed after the VEH correction, focused regression, and
successful Run C arm/completion.

**Skill promotion:** None.

## 2026-09-14: clean v0.7.4 portable build blocked by target-directory access

**Objective:** Build the v0.7.4 portable package from the clean worktree
`F:\Nioh3_ScrollEditor\.codex_tmp\release-clean-v074-0d628f8`.

**Symptom:** `npm typecheck` passed, but Cargo could not create
`apps\launcher\target` and returned `Access is denied (os error 5)`. The
`build_tauri.ps1` script terminated with Cargo exit code 101.

**Root cause:** The `F:` volume is exFAT. The first Cargo invocation could not
create each crate's deep `target` directory, although manually creating the
exact target directory succeeded immediately and allowed the rerun to
continue.

**Evidence:** Clean worktree path
`F:\Nioh3_ScrollEditor\.codex_tmp\release-clean-v074-0d628f8`, candidate
commit `0d628f8421b2ea10110abf6dc75cae06e6b1163f`, and the recorded command
output showing the typecheck pass and Cargo access-denied failure.

**Disposition:** Resolved by precreating `apps/launcher/target` and
`apps/tauri/src-tauri/target`. The clean build from commit
`0d628f8421b2ea10110abf6dc75cae06e6b1163f` completed and produced portable
version 0.7.4 with 738 files, 41,673,715 installed bytes, and an inner EXE of
12,402,688 bytes.

**Reproduction status:** Reproduced once; resolved on the subsequent rerun.

**Follow-up state:** Closed after the successful clean portable build rerun.

**Skill promotion:** None; retained as a one-off exFAT observation.

## 2026-09-14: v0.7.4 debug rebuild blocked by insufficient disk space

**Objective:** Rebuild the Tauri debug acceptance executable after changing the
preview marker from an icon to `zh` 附身 / `en` Wraith / `ja` 憑き.

**Symptom:** Cargo failed while copying incremental objects and reported
`There is not enough space on the disk (os error 112)`.

**Root cause:** Duplicate Rust `target` caches in the main worktree and the
clean worktree filled the external `F:` volume.

**Evidence:** `apps/tauri/src-tauri/target` and
`.codex_tmp/release-clean-v074-0d628f8/apps/*/target`.

**Disposition:** Ran `cargo clean` only for the two task-specific clean-worktree
Rust `target` caches, freeing approximately 1.7 GiB while preserving source
files and candidate packages. The main debug Cargo build then passed. Native
enemy-state UI acceptance also passed with the final short localized preview
labels `zh-CN` 附身, `en-US` Wraith, and `ja-JP` 憑き.

**Reproduction status:** Reproduced once; resolved after targeted cache cleanup
and the successful debug rebuild and native UI acceptance.

**Follow-up state:** Closed after the bounded rebuild and native enemy-state UI
acceptance passed.

**Skill promotion:** None.

## 2026-09-15: v0.7.5 release-checkout LF fixture hash mismatch (autocrlf)

**Objective:** Run the v0.7.5 search-continuation hotfix regression suite from a
checkout of `a1601bb242ebfdc509ab853016cc64639f69320c` in the new hotfix
worktree `F:\Nioh3_ScrollEditor\.codex_tmp\v075-search-hotfix`.

**Symptom:** `tests/test_mode_transaction_join.py` failed in its live
parameterized case: the recorded transcript compares the LF fixture hash
`0e1f54e9...` for `research/owned_breakpoint_lifecycle_ce.lua`, but the working
copy hashed to `7bb81b4a...`. Search behavior was not involved.

**Root cause:** Windows line-ending materialization, not a product defect. With
`core.autocrlf=true` the fixture was checked out as a 3494-byte CRLF file
(93 `\r\n` pairs) instead of the 3401-byte LF blob the transcript hashes. The
backend independently observed the same CRLF content in older clean worktrees,
which rules out a search-regression cause.

**Evidence:** Worktree at `a1601bb`, `git config core.autocrlf` = `true`,
`git cat-file -s HEAD:research/owned_breakpoint_lifecycle_ce.lua` = 3401,
working copy 3494 bytes with 93 CRLF pairs,
sha256 LF blob
`0e1f54e959dfbe3eec1cfd91b5f8360777caacb7d8b7e64dfc955c47e3e7cd7d` versus
working copy
`7bb81b4acf031cc60deb5bd55e901db89a78c465fd913a482012357a98e945d5`, and the
backend failure log for `tests/test_mode_transaction_join.py`. After the
approved repair the worktree file is 3401 bytes with 0 CRLF pairs and matches
the expected LF hash above.

**Disposition:** Root approved a targeted `eol=lf` rule for this fixture path
plus normalization of the working file, with no change to the expected hash.
The worktree now carries
`research/owned_breakpoint_lifecycle_ce.lua text eol=lf` in `.gitattributes`
(uncommitted there) and the fixture is LF. Fixture-hash re-verification is
pending the backend owner's test run.

**Reproduction status:** Reproduced from the clean `a1601bb` checkout on this
Windows host. Re-check by deleting/re-checking the fixture and comparing the two
sha256 values above.

**Follow-up state:** Open until the normalized checkout passes
`tests/test_mode_transaction_join.py`. If the hash still mismatches after
normalization, treat it as a real fixture-contract change and stop.

**Skill promotion:** None; one-off checkout artifact, not a rule.

## 2026-09-15: v0.7.5 hotfix worktree Tauri cargo gate blocked by disk pressure

**Objective:** Run the local release gate
`cargo test --locked --manifest-path apps/tauri/src-tauri/Cargo.toml` for the
v0.7.5 search-continuation hotfix in the isolated worktree
`F:\Nioh3_ScrollEditor\.codex_tmp\v075-search-hotfix` at base `a1601bb`.

**Symptom:** The cold Tauri debug build failed while linking and copying build
scripts: `error: failed to link or copy ... build-script-build.exe`, `Caused by:
There is not enough space on the disk. (os error 112)`, and
`LINK : fatal error LNK1318: Unexpected PDB error; LIMIT (12)`. Cargo exited
101. No source or test assertion failed; the same commit's Python, Node,
TypeScript, and launcher-crate gates passed.

**Root cause:** Local disk pressure on the exFAT `F:` volume, not a product
regression. The volume hosts the main checkout's
`apps/tauri/src-tauri/target` (measured 4,339,545,395 bytes) plus a fresh cold
Tauri `target` in the hotfix worktree, which grew to 2,195,710,520 bytes before
the link stage ran out of space.

**Evidence:** `apps/tauri/src-tauri/target` in the main checkout and in the
hotfix worktree; `cargo test --locked --manifest-path
apps/tauri/src-tauri/Cargo.toml` failure output with os error 112 and LNK1318;
`Get-PSDrive F` free space reaching 0 bytes during the attempt;
`deliverables/v075-search-hotfix/source_gates_report.md` records the gate totals.
The frontend agent independently measured 0.40 GB free of 931.48 GB (exFAT) with
no cargo/rustc processes running, and reported no frontend gate failure
attributable to disk; its WebView2 continuation acceptance later passed on a
legacy-built debug host.

**Disposition:** Ran `cargo clean` for the aborted hotfix-worktree Tauri target
(2.0 GiB) and for the same worktree's launcher target (270.4 MiB), preserving all
source and other agents' artifacts. The Tauri crate gate is left to the hosted
release workflow rather than a repeated cold local build, which also matches the
runbook's preference for one clean hosted package after the frozen SHA.

**Reproduction status:** Reproduced once on this host. Re-checks: confirm
`F:` free space before a cold Tauri build; `cargo clean` only task-specific
target caches; keep `CARGO_TARGET_DIR` off `F:` when a local host build is truly
needed.

**Follow-up state:** Open as a local environment constraint until the hosted
release workflow reports the Tauri `cargo test` result for the frozen candidate.

**Skill promotion:** None; local volume capacity is not a product rule.
