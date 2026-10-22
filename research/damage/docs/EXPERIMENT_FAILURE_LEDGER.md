# Experiment failure ledger

## 2026-10-21: Installed owned-toy held-event cleanup

- Objective: validate installed CE held context, TID/TLS/MXCSR, same-lease
  epoch/guard, ACK and four-site cleanup before any further game session.
- Symptoms: early harness runs failed heartbeat startup or observed remaining
  breakpoints when asserting asynchronous cleanup inside a2second window.
- Cause: heartbeat GO-barrier initialization was incomplete; cleanup timing
  was inadequately budgeted/observed. A unique CE internal cause is unproved.
  Debugger removal return truthiness was not adequate evidence of absence.
- Disposition: initialize heartbeat before the barrier, separate the2second
  event lease from8second bounded read-only cleanup polling, retain exact
  context/identity/deadline checks and require final breakpoint absence,
  exactly-once resume, child exit0 and owned CE closure. Final positive,
  timeout and cancel modes pass with unchanged final dependency hashes.
- Evidence: F:/Nioh3_r16_build/deliverables/r16-real-data-bridge-20261021/
  owned-platform-01 through09, historical timeout/cancel receipts, and the
  accepted platform-final-positive/timeout/cancel RECEIPT.json files. Early
  full source revisions were not all retained; final-source receipts are the
  accepted reproduction evidence, not a retroactive repair of failed runs.
- Follow-up: port the demonstrated core protocol to the production observer/
  paused service and retest. Toy acceptance is not natural game acceptance.
  No Nioh3 attachment, game/save write or global debugger change occurred.

## 2026-10-20 real-gear optimizer integration

- Offline equipment preview/selection controls initially attempted an optional
  Unicorn import and an inherited fixture missing native_constants.json.
  Neither ran target code or installed dependencies. The accepted controls
  reuse the pinned existing instruction interpreter and explicit authenticated
  item/effect/group/level-curve bindings. The initial unknown descriptor flags
  were then resolved by64 original setter executions; C0 was recovered from
  an already byte-matched LFM772 asset. Earlier refused artifacts remain
  separate from the final5233-instruction converter result. Evidence:
  `equipment-add-frontier/REPORT.md`, clone-control JSONs and
  `test-outfit-selection/REPORT.md`. Follow-up: no live write is enabled by
  fixture or native converter success.

- Root checked and falsified an index hypothesis without writes: record+24
  is not the physical slot number in the current2500 F0 records (2499
  mismatches; the known helmet's field is0). The exact native copy preserves
  this destination lane, so uninitialized/context-owned is the correct grade,
  not proven padding or a slot ID. The actual slot/index contract is derived
  from54DDF1/54E084 and54C34C/54C468 instead. Evidence: ordinary readback
  code/16 controls and raw `inventory-02/` records. No inferred field was used
  for a write or accepted item identity.

- Live-session follow-up, objective: fill the first original fresh-constructor
  dependency. The initial quick classification treated module+44B7F30 as a
  pointer and requested a64-byte pointee. The exact8-byte global was retained,
  but ReadProcessMemory refused the invalid pointee and the handle closed.
  Native disassembly corrected the field to an opaque /GS security cookie;
  the subsequent request reads it as u64 only and separately captures the
  authenticated mutex/refcount/import fields. No game/save write or game call
  occurred. Evidence: `r16-real-gear-live-20261020/CRT-frontier-01/` (partial
  cookie, refused traversal), `CRT-sync-01/` (five observed named regions),
  and `native-followup/new-actor-constructor/`. Reproduction is the retained
  initial request; do not dereference the cookie again. Disposition: corrected
  role, next native frontier is external GetCurrentThreadId, not a missing
  gameplay measurement. Follow-up: explicit private OS primitive semantics
  or a typed refusal; no further owner wait for operating-system imports.

- The same follow-up's quick action-state note reversed fields+20 and+40.
  Raw observations were preserved. Exact native2E98F0/2E99A0 proves+20 points
  to flags, +40 to the modifier pointer array, +48 is start index and+4A count.
  The final action-flags capture extends the actual flags block through+DC;
  observed count0 makes stale nonnull array entries inactive. No guessed
  linked-list traversal or node+0 NEXT walk was performed. Evidence:
  `action-state-01/`, `action-flags-01/` and the native geometry export.

- Objective: execute the original2A0 helper and preserve repeatable evidence.
  Symptom: first private helper entry completed but failed the conservative
  unknown-read audit while restoring unspecified RBX/RDI; repeated receipt
  sealing also hashed its own prior receipt. Cause: incomplete private caller
  initialization and an artifact-glob self-inclusion. Disposition: declare known
  private callee-saved values, exclude the receipt from its artifact manifest,
  pin native runner dependencies and rerun primary/fallback/MISS/UNKNOWN controls.
  Evidence: `r16-real-optimizer-20261020/damage/constructor-input-audit/lookup-e2e/`
  and `FAILURE_LEDGER_ENTRY.md`. All five controls pass with unchanged native
  instructions. The worker removed its initial private trial file; the bounded
  diagnostic note and reproduction instructions preserve the failure, not a
  retained raw initial artifact. An auxiliary saved-capture reader also needed
  to skip metadata-only unreadable records before mask evaluation. The repaired
  actual S0 selector preflight reads48 known bytes. Follow-up: real2A0 data is
  still absent; fixture repair does not establish current-game lookup results.

- Objective: transport candidate effects into the retained native damage path.
  Symptom: early damage/01-02 receipts still used the baseline SOURCE count60
  for the helmet, although D had changed. Cause: the first transport covered
  STAT/D but omitted source-map/count lanes. Disposition: superseded; selected
  and core SOURCE are now independently produced and patched, actual source
  materialization is checked, and public-e2e-04 plus CLI E2E reproduce714/720.
  Helmet instruction count changes10051->10079. Evidence:
  `F:/Nioh3_r16_build/deliverables/r16-real-optimizer-20261020/damage/` and
  `numeric/NUMERIC_COMPARISON.json`. Reproduction: final positive/negative
  controls pass; stale SOURCE fails closed. Follow-up: preserve the earlier
  receipts as intermediate, never as the accepted candidate transport.

- Objective: certify the observed-pair packet binding. Symptom: a draft
  certificate pinned mutable numeric output aliases; integration regenerated
  those files and public-e2e-03 refused their hashes before native scoring.
  Cause: evidence publication raced its input producer. Disposition: v2 pins
  immutable input snapshots and complete blob/mask/SOURCE structures. Evidence:
  `damage/packet-scope/v2/` and `damage/public-e2e-04/`. Reproduction: two positive
  and21 mutation controls pass. Follow-up: keep this admission observational;
  it does not repair the incomplete causal packet constructor.

- Objective: close the full constructor from retained S0 inputs. Symptom:
  concrete entry stops first at an uncaptured refcount lock; exploratory
  lock-skipping reaches a real query pointer gap. Longer conditional native
  packet execution crosses authenticated parameter130 then refuses at the
  uncaptured parameter2A0 role. Cause: saved inputs lack required dependencies;
  original SOURCE+B426/query+50 and manager+2A0 also have no known bytes.
  Disposition: no full-constructor claim, default substitution or old-epoch
  actor borrowing. Evidence: `damage/packet-tail/` and
  `damage/constructor-input-audit/`; the bounded audit covers173 receipts.
  Reproduction: original missing reads remain typed refusals; private suffix
  proofs and calibration are separately scoped. Follow-up: one coherent new
  declared-scenario collection, with native caller/ABI reduction and bounded
  sorted-vector collection prepared before asking the owner to open the game.

- Objective: run existing optimizer E2E regression using the resolved build
  root. Symptom: initial setup found a missing temporary parent, then two
  historical tests required a literal D: path despite the F: build override.
  Cause: test path assumptions, not evaluator behavior. Disposition: create
  the bounded temp root and check containment against the shared build-root
  resolver. Evidence: `native-dispatch-regression-20261020/` and current
  `r16-real-optimizer-20261020/synthetic-regression.xml`. Reproduction:80 E2E
  tests pass. Follow-up: retain failed receipts; no checkout-wide path changes.

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

## 2026-09-19: PC v2.02 Pro return - coordinate, key-assumption, label, and parity-scope defects

**Objective:** Independently reproduce the Pro return for the PC v2.02 添画 / level-clamp / changed-table packet
against our retained baseline and target tables, and produce a bounded collector-fix specification. Read-only lane.

**Pro-reported symptoms, recorded as proposed until reproduced:** (1) the v5 export's `item` row-3358 window omitted
the 8-byte table header, so the changed field was published as window-relative `+0x8C` instead of the true in-row
`+0x84`; (2) the three target tail rows of `optional_multiplier` are not proven new semantics, because their keys
already exist in the baseline; (3) `native_recommended_displayed` is a Python curve prediction, not a UI observation;
(4) R3's "full record parity" masks `0x1B`, and R5's original 10,000-record mismatch cannot be retroactively
reclassified as a pass.

**Reproduction:** All four reproduced. The Pro packet ZIP matched its expected hash and the embedded v5 archive
matched ours; all 24 member hashes recomputed; Pro `baseline_supplement/item.bin` and `optional_multiplier.bin` matched
our retained baseline files exactly. Pro offline tests passed 18/18, v5 packaged tests passed 4/4, and the Pro
analyzer reproduced `evidence/offline_analysis.json` field for field. Direct diffs of our retained tables showed:
exactly two differing bytes in `item.bin` at `0x15514C/0x15514D`, which is row 3358 start `0x1550C8` plus in-row
`+0x84` (`0x380 -> 0x0`), with real `+0x8C` unchanged and the exported window equal to `baseline[0x1550C0:0x155260]`;
`optional_multiplier` 2951 -> 2954 rows with tail keys `0x0AE3/0x8A46/0xC869` already present at baseline rows
2948/2949/2950, zero removed keys, and three genuinely new keys `0x3472/0xAA65/0xD56F` at target rows 2847-2849 plus
three changed `+0x10` payloads (`0x39E8` 80 -> 35, `0xA899` 30 -> 15, `0xD7C3` 1400 -> 600).

**Root cause:** Coordinate-origin error in the exporter (window start versus true row start), a tail-position
heuristic used in place of a validated-key comparison, a derived curve value labelled as a native or UI observation,
and parity scope summarised without its mask or cap. Not a game-mechanism error, and not a product regression.

**Evidence:** Pro packet `D:\Nioh3_v080_deliverables\deliverables\game-version-update-20260919\pro-return-20260919\Nioh3_v202_Pro_Analysis_20260919\`
(report, five outputs, `evidence/code_evidence.txt`, `evidence/bounded_objdump.txt`, `evidence/offline_analysis.json`);
independent verification
`...\pro-return-20260919\repro-20260919\PRO_RETURN_VERIFICATION.md`, `check_tables.py`, `recheck.json`; retained tables
under `nioh3_scroll_editor/data/r4_finalizer/pc_v2_00_02/resource_v1/tables/` and
`deliverables/game-version-update-20260919/resource-v2.02-r4/tables/`.

**Disposition:** P0 collector-fix pending, owned by the producer: separate header, row and window origins and publish
`+0x84`; compare the full target table by validated key instead of the file tail; rename the derived display prediction
away from native wording; keep the R3 mask and R5 mismatch visible in every parity summary; add the seed to raw
filenames so the second seed cannot overwrite the first. No product, collector, or package file was modified here, and
no compatibility, approval, or release claim follows.

**Reproduction status:** Deterministic offline from the frozen archives plus the retained tables; no game, save, or
native call was made.

**Follow-up state:** Open with the producer. This entry stays a bounded record, not a product rule.

**Skill promotion:** None. Route the mechanics into the existing collector or export checks if they recur.

## 2026-09-19: P0 table diff used the wrong key origin, and a table hash was cited for the wrong curve

**Objective:** Correct the PC v2.02 table/offset evidence after the Pro return and
the producer's own follow-up counts, then re-verify from the retained bytes.

**Observed symptom:** Two independent numbers described the same
`optional_multiplier` change. The producer first published 105 keys added, 101
removed, 546 payload changes and duplicate keys 85 -> 84; the independent
re-derivation published 0 removed, 3 added, 3 payload changes. Both were
internally consistent, so the disagreement had to be a schema error rather than a
measurement error. Separately, the shipped summary cited the equal hash of
`tables/level_curve.bin` (501 rows x 10 bytes) as evidence that the 42-row
recommended-level display curve did not change.

**Root cause:** The rejected comparison read an 8-byte key from each row start and
collapsed rows into one dict entry per key. The shipped loader
(`r4_table_bundle.FixedStrideTable`) requires an 8-byte header with the row count
at offset 4, and the shipped consumer
(`auxiliary_generation._find_optional_multiplier_row`) documents the key as a u32
at row `+0x14` with the threshold base at `+0x10`. With the wrong key origin the
keys slide across row boundaries and duplicate ordinals are destroyed, which
manufactures added and removed keys and inflates payload changes: repeating the
rejected method reproduces exactly 105/101/546/85 -> 84, while the corrected
schema gives 0/3/3 with the changed payloads `0x39E8` 80 -> 35, `0xA899`
30 -> 15 and `0xD7C3` 1400 -> 600. The curve citation is a separate label error:
the 501-row table has no relation to the 42-row display curve, whose retained
PC v2.02 capture does verify 42/42 identical points but only with signature-bound
identity (no PID, module base, or executable hash recorded).

**Secondary finding:** The one game-recognized save is unchanged since
2026-09-14, and its single high-level scroll (slot 45, `0xE604`, seed 180443387,
serial 2375795, key 50409) stores raw internal recommended 1400 - display 700 -
and is identical in the 9/2-era product backup. The owner reports that some
scrolls now show 356, but that observation is not yet identity-joined to this
record: the stable 1400 shows only that the durable value has not been rewritten
and that no post-update save exists yet.

**Evidence:** `deliverables/game-version-update-20260919/reports/P0-corrections-20260919.md`
with `reports/recommended-level-curve-42point-verified-20260919.json`,
`reports/private-save-discovery-20260919.json`,
`reports/private-record-scan-20260919.json`,
`reports/backup-record-comparison-20260919.json`; corrected collectors
`tools/table_diff.py`, `tools/parity_scope.py`,
`tools/inspect_private_save_records.py`, `tools/scan_backup_save_records.py`,
`tools/compare_recommended_curve_42.py`; tests
`tests/test_table_and_field_functions.py`,
`tests/test_structured_table_offsets.py`, `tests/test_parity_scope.py`
(16 passed on the project runtime).

**Disposition:** Closed as an evidence-quality defect. The corrected coordinates
and keyed diff are canonical for the lane; the 501-row hash may no longer be
cited for the display curve. No product, generation, profile, or release change
follows, and PC v2.02 stays unapproved.

**Reproduction status:** Deterministic offline. Re-running the corrected diff
against the retained baseline and target tables reproduces 0 removed / 3 added /
3 payload changes, and running the rejected method reproduces the superseded
counts.

**Follow-up state:** Open only as the bounded static lead: key `0xD7C3` is
consumed at `0x110DE06` and `0x227FE4B`, the latter through the parameter
manager `+0x230` into a write of record `+0x10`/`+0x12`. That relation is static,
not causality, and needs the prepared bounded probe before any claim.

**Skill promotion:** None. The mechanics belong in the existing collector tests,
which now execute the functions on independently constructed fixtures.

## 2026-09-19: PC v2.02 Pro package v5 final closure accepted (docs-only)

**Scope:** v5 closes the two v4 documentation defects and changes nothing else.

**Verified:** v5 ZIP sha256
`74E1576DF78F804D7734F2C948B3413665F2418255A79A5BE10D931B7764EDC9`, 137,038 bytes, 60 members, no `.pyc`.
Repository validator returned `ok: true` with 60 files, 59 hashes verified, and the ZIP SHA-256 matching the
producer's pre-published value. The v4 -> v5 diff was computed member by member from the two archives rather than
from working directories: 4 members changed, 56 byte-identical, none added, none removed.

**Concrete correction:** the producer's "57 non-doc unchanged" claim is off by one. The accurate figures are 56
byte-identical members and 4 changed members - `KNOWN_LIMITS.md`, `README.md`, `TASK_FOR_PRO.md` and the mandatory
`SHA256SUMS.txt` - so exactly one non-documentation member changed, and it had to.

**Documentation closure:** `TASK_FOR_PRO.md` is standalone (PC v2.02, file version 2.0.2.0, Steam build 25297068,
owner wording verbatim, three questions with `outputs/` paths, no reference to any earlier archive), `KNOWN_LIMITS.md`
now reads exactly one vector, one seed and one stage per raw file, and `README.md` records the docs-only change and the
`candidate` / `product_enablement_allowed: false` boundary.

**Disposition:** v5 accepted as the final bounded handoff for Pro analysis and next-probe design. No mechanism
completion claim: Q1 has no revision artifact, Q2 is construction-only with the load path unproven, Q3 field meanings
are unknown. Seed-1 raw bytes stay absent and disclosed; the collector fix adds the seed to the raw filename without
re-capturing. No package, source, product, game, build, or publication action was taken, and packaged tests were not
rerun because every code and evidence member is byte-identical to v4, where the checker and four portable tests
already passed.

**Reproduction status:** Deterministic from the frozen v4 and v5 archives plus the repository validator.

**Follow-up state:** Closed in this lane; Pro analysis of the v5 packet is the next step.

**Evidence:** report
`D:\Nioh3_v080_deliverables\deliverables\game-version-update-20260919\reports\PRO_HANDOFF_ADEQUACY_REVIEW.md`
(sha256 `EB0B7D859203C379587307FB8112E8ADCCBE5E472B696E1BFE3EB321C7789C6F`).

**Skill promotion:** None.

## 2026-09-15: downloaded hosted candidate packaged test found worker digest mismatch

**Objective:** Locally verify downloaded hosted candidate `df438ed` from run
`34921969962`.

**Symptom:** ZIP/EXE sidecars, production Ed25519 signature, manifest,
CRC/member hashes, source SHA, one-file direct launch/cache, and one-file
update replacement all passed. Hosted `Verify the release binaries` also
passed. After extraction under
`deliverables/v074-hosted-candidate-df438ed-20260915/extracted-portable`,
`npm run test:packaged` failed because offline/protected worker contexts had
different `resources_digest`/`context_digest` values: actual search resources
`984440...` versus expected protected `a92736...`.

**Root cause:** The long-lived local worktree contains CRLF raw bytes in 12
tracked JSON resources despite clean filters/eol policy. `runtime_resource_digest`
intentionally hashes raw bytes, while the hosted clean checkout and PyInstaller
package use LF. The embedded candidate contains 48 resources with digest
`a927362d...`; only those 12 text files differ from local raw source by line
endings.

**Evidence:** Local extracted candidate path and run `34921969962` hosted
verification results.

**Disposition:** Resolved; a fresh detached worktree at exact `df438ed` produced
`CLEAN_SOURCE_RESOURCE_DIGEST=a927362d...` and
`PACKAGED_R3_R4_R5_PARITY_OK` against the downloaded hosted search worker.
Package defect ruled out.

**Reproduction status:** Reproduced locally; not reproduced in hosted CI; resolved
after clean-source comparison.

**Follow-up state:** Closed.

**Skill promotion:** None.

## 2026-09-14: hosted signed-candidate release-binary acceptance failed

**Objective:** Pass signed Windows candidate release acceptance for commit
`b1e578acf0cf1d55b7f45dd7fb28a71da060312a`.

**Symptom:** The clean-checkout build, archive, and one-file EXE build passed,
but the hosted step `Verify the release binaries` failed on a visible-switch
click assertion.

**Evidence:** [GitHub Actions run](https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/actions/runs/34920594896).

**Root cause:** Acceptance attempted `.check()` on a visually hidden input
after the `ToggleSwitch` refactor. The fix clicks the visible `<i>` track and
asserts checked state in both acceptance drivers.

**Disposition:** Resolved by the visible-track acceptance fix. Successful run
`34921969962` at source
`df438ed3a9a1b92e68b3c77da0b1c094e0663327` passed clean source/build,
671-test inventory, npm/typecheck/native/Rust suites, release-binary UI
acceptance, direct one-file launch/cache, outer update replacement/rollback,
signing, and artifact uploads.

**Reproduction status:** Reproduced in hosted CI.

**Follow-up state:** Closed.

**Skill promotion:** None.

## 2026-09-14: hosted release-binary verification failed on Settings focus ring

**Objective:** Verify the release binary for source
`e1ee62dbd523883e153ead4d509bf879bd249059` in hosted run `34918229861`.

**Symptom:** Build and all test suites passed, but
`apps/tauri/verify-add-layout.mjs:343` failed with
`AssertionError: Focused Settings switch has a visible focus ring`; actual
value was `undefined`. Diagnostic artifact upload independently failed during
`FinalizeArtifact` with HTTP 403, but this was not the primary job failure.

**Root cause:** Unknown.

**Evidence:** [GitHub Actions run](https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/actions/runs/34918229861)
and `gh run view 34918229861 --log-failed`.

**Disposition:** Inspect the release-binary focus-ring state and failed-step
diagnostics before selecting a repair; keep the artifact-upload 403 separate.

**Reproduction status:** Reproduced once in hosted CI.

**Follow-up state:** Open pending diagnosis and rerun.

**Skill promotion:** None.

## 2026-09-14: hosted Rust one-file helper test failed on Windows runner

**Objective:** Complete the hosted release workflow for exact source
`6e0cdd6461c2959d656a720cb588d4b0444c4aeb`.

**Symptom:** GitHub Actions run
[#34917013537](https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/actions/runs/34917013537)
passed inventory, CPU-only tests, full unittest, title-save pytest, npm tests,
typecheck, native faults, and the Tauri build. The Rust step
`cargo test --locked --manifest-path apps/tauri/src-tauri/Cargo.toml` then
reported 14 passed and 1 failed. Test
`onefile::tests::real_onefile_helper_rechecks_prepared_bytes_after_waiting_for_exit`
at `src/onefile.rs:351` expected JSON error `Replacement changed while waiting`
but received `Application did not exit safely`.

**Root cause:** Unknown; likely a timing/process-exit race in the real helper
test on the GitHub Windows runner.

**Evidence:** Hosted run `34917013537` and its Rust test-step log.

**Disposition:** Inspect the helper's timing and process-exit behavior on the
GitHub runner before choosing a repair.

**Reproduction status:** Reproduced once in hosted CI.

**Follow-up state:** Open pending inspection and rerun.

**Skill promotion:** None.

## 2026-09-14: local r4d one-file acceptance exited before debug connection

**Objective:** Run one-file acceptance for candidate source
`000c701504e3a530b523a07363176a6fadcdd2de` and artifact
`deliverables/v074-local-candidate-final-r4d-20260914`.

**Symptom:** Packaging succeeded; the inner app, launcher, and both worker
hashes matched previously accepted r4c, and
`PACKAGED_R3_R4_R5_PARITY_OK` passed. However,
`node apps/tauri/verify-onefile.mjs` failed before debug connection:
`onefile-acceptance.mjs connect()` reported `Error: Launcher exited 0` at line
39. `verify-onefile-update` did not run.

**Root cause:** A residual r4c test instance/process tree remained:
`deliverables/v074-local-candidate-final-r4c-20260914/Nioh3Studio-0.7.4-win-x64.exe`
with cache payload `0ef472...`. Correct single-instance forwarding caused the
new r4d launcher to exit 0 before its debug connection.

**Evidence:** Candidate artifact path, packaging/hash parity output, and the
`verify-onefile.mjs` / `onefile-acceptance.mjs` console failure.

**Disposition:** Terminated only the verified residual r4c test process tree.
The r4d rerun passed `TAURI_ONEFILE_DIRECT_LAUNCH_CACHE_OK` and
`TAURI_ONEFILE_REAL_UPDATE_RESTART_CLEANUP_OK`; post-check found no r4d
outer/cache processes. This was test-isolation residue, not a product failure.

**Reproduction status:** Reproduced once; resolved on isolated rerun.

**Follow-up state:** Closed.

**Skill promotion:** None.

## 2026-09-14: hosted signed-release preparation failed during test inventory

**Objective:** Prepare the GitHub hosted signed release workflow at source
`44809e40a549f2f3bd71619b29a358bfa8c454f2`.

**Symptom:** Workflow run
[#34914327826](https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/actions/runs/34914327826)
failed after 1m59s at
`python tools/write_test_inventory.py --output deliverables/release/test-inventory.json`
with exit code 1. All later tests, build, and package steps were skipped; no
portable or acceptance artifacts existed.

**Root cause:** Not yet known.

**Evidence:** Hosted workflow run `34914327826` and its failed-step log.

**Disposition:** Inspect the exact failed-step log and repair the issue without
rerunning the known-bad SHA.

**Clean-checkout reproduction:** At candidate
`277c69653c8a8d177a3f52082ea1c4beb16ad94e`, `write_test_inventory` succeeded
with 670 IDs and the missing ignored research scripts/Lua DLL issue was fixed.
The exact clean `python -m unittest discover -s tests -t . -v` then failed 3 of
670 because Capstone was absent from `requirements-dev` and existed only in the
main machine's ignored `audit/runtime_deps`. The failing tests were
`test_call_xrefs_prefilter_before_bounded_disassembly`,
`test_static_field_inventory_distinguishes_reads_and_writes`, and
`test_string_xrefs_disassemble_only_bounded_pdata_functions`; each raised
`ModuleNotFoundError: capstone`.

**Updated root cause:** Undeclared development/test dependency.

**Updated disposition:** Pin the compatible Capstone version in
`requirements-dev.txt` and rerun from a clean checkout.

**Resolution evidence:** Candidate
`000c701504e3a530b523a07363176a6fadcdd2de` tracks all three
`find_runtime_*_xrefs.py` scripts, Lua54 falls back to locked `lupa 2.8`, and
`requirements-dev` pins `capstone 5.0.6`. Clean-checkout preflight passed;
inventory produced 670 IDs, CPU-only tests passed 123 with 2 skipped, unittest
passed 670, and title-save pytest passed 75 with 1 skipped.

**Reproduction status:** Reproduced once in hosted workflow and once in the
clean-checkout test run.

**Follow-up state:** Open only pending a successful fresh hosted workflow run.

**Fresh workflow recurrence:** Workflow run
[#34915567134](https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/actions/runs/34915567134)
at exact source `000c701504e3a530b523a07363176a6fadcdd2de` confirmed the prior
dependency-install, `write_test_inventory`, and `run_cpu_only_tests` blockers
fixed in CI. The full
`python -m unittest discover -s tests -t . -v` failed after approximately
4m05s, so later steps were skipped. UI annotation references
`tests/test_v2_operations.py#224` (`Expected operation failure`), which may be
intentional test logging; root cause remains unknown pending failed-step log
extraction.

**Updated follow-up state:** Open pending successful hosted rerun; not fully
closed until GitHub passes.

**Failed-step detail:** In run `34915567134`, dependency installation,
inventory, and CPU-only tests passed. Full unittest ran 670 tests and failed
exactly two `DocumentationAuditTests`: `test_audit_accepts_links_images_references_and_fenced_examples`
reported unindexed `CURRENT.md` and `INDEX.md`;
`test_audit_reports_missing_local_target_and_unindexed_document` expected one
index finding but got two because `INDEX.md` self-indexed. The GitHub temporary
root used `C:/Users/RUNNER~1/...`; a Windows short/long path identity mismatch
is suspected but not confirmed.

**Confirmed root cause:** `audit_knowledge_index` compared unresolved glob
`Path` objects with canonical `Path` objects returned by `resolve_destination()`.
Windows 8.3/long aliases therefore made identical `INDEX.md`/`CURRENT.md`
files unequal.

**Resolution evidence:** Fix commit
`6e0cdd6461c2959d656a720cb588d4b0444c4aeb` resolves every expected top-level
knowledge path before set comparison and adds alias/`..` regression coverage.
Focused `DocumentationAuditTests` passed 4/4; repository audit passed; main
worktree full unittest passed 671/671; clean exact-SHA inventory produced 671
IDs; CPU-only passed 123 with 2 skipped; clean full unittest passed 671/671.

**Updated disposition:** Resolved locally; retain the entry pending a
successful GitHub hosted rerun.

**Skill promotion:** None.

## 2026-09-14: final r4b update/hotfix acceptance timed out without stage output

**Objective:** Verify the startup automatic-update prompt and Settings manual
check on the exact-source candidate
`77ce8ac25176cf72abb3886dc325706ec6d6e9a8` using
`apps/workshop/verify-hotfix.mjs`.

**Symptom:** The legacy verifier produced no stage output and Playwright's
ElectronDispatcher timed out at the 180000 ms global timeout in
`progress.js:86`.

**Root cause:** The verifier still used legacy Playwright `electron.launch`
against the migrated Tauri/WebView2 app.

**Evidence:** Current console output and candidate path
`deliverables/v074-local-candidate-final-r4b-20260914`.

**Disposition:** Migrated the verifier to native WebView2/CDP with an isolated
local update response and stage output. Final source
`44809e40a549f2f3bd71619b29a358bfa8c454f2` candidate
`deliverables/v074-local-candidate-final-r4c-20260914` passed all listed
package and UI acceptance gates: `PACKAGED_R3_R4_R5_PARITY_OK`,
`TAURI_WEBVIEW2_SEARCH_FAVORITES_INVENTORY_RESTORE_OK`,
`TAURI_REAL_UPDATE_RESTART_CLEANUP_OK`, `ENEMY_STATE_UI_ACCEPTANCE_OK`,
`V074_JAPANESE_STARS_UPDATE_PROMPT_OK`,
`TAURI_ONEFILE_DIRECT_LAUNCH_CACHE_OK`, and
`TAURI_ONEFILE_REAL_UPDATE_RESTART_CLEANUP_OK`.

**Reproduction status:** Reproduced once.

**Follow-up state:** Closed after the native WebView2/CDP rerun and all listed
acceptance gates passed.

**Skill promotion:** None.

## 2026-09-14: v0.7.4 r4 packaging blocked by stale worker path

**Objective:** Assemble the r4 candidate at exact source
`77ce8ac25176cf72abb3886dc325706ec6d6e9a8` while reusing prior validated
workers.

**Symptom:** `tools/package_tauri.py` failed immediately with
`FileNotFoundError` while copying `nioh3-search-worker.exe` and
`nioh3-protected-worker.exe`; the summarized worker path
`F:\Nioh3_ScrollEditor\.codex_tmp\tauri-build-eab0ee32f07c45fb910c408b1454c7ce\workers`
no longer exists.

**Root cause:** Stale/nonexistent temporary worker path, not a product or
build failure.

**Evidence:** Console traceback in the current turn and incomplete target
`deliverables/v074-local-candidate-final-r4-20260914`.

**Disposition:** The worker path was confirmed to belong under the clean
worktree. Using the correctly located clean-worktree workers,
`package_tauri.py`, archive, and onefile assembly succeeded for final source
`44809e40a549f2f3bd71619b29a358bfa8c454f2` and candidate
`deliverables/v074-local-candidate-final-r4c-20260914`. The policy-blocked
incomplete r4 shell remains unused and is not a candidate.

**Reproduction status:** Reproduced once; resolved in the final r4c assembly.

**Follow-up state:** Closed after successful final assembly and identity
verification.

**Skill promotion:** None.

## 2026-09-14: packaged UI verification blocked by intercepted language control

**Objective:** Run `apps/tauri/verify.mjs` against the final portable EXE built
from SHA `0b16f9d284db12ed46ed9d4120d15ec9353fd1b9`.

**Symptom:** On rebuilt candidate SHA
`92a934580cceda85cbb42d12a70e0d1125b1df7f`, the previous
`.language-button` interception no longer occurred. The comprehensive
verifier progressed further but timed out at `apps/tauri/verify.mjs` line 157
while waiting for button `确认写入存档`.

**Root cause:** The original interception was traced to the removed
`--ui-font-size` CSS variable invalidating the expanded `en`/`ja` grid
template. It was fixed with a fixed 150px localized-sidebar rule and a new
native-shell placement assertion. The new timeout at line 157 remains unknown.

**Evidence:** `apps/tauri/verify.mjs` runs against the candidate portable EXEs
for SHAs `0b16f9d284db12ed46ed9d4120d15ec9353fd1b9` and
`92a934580cceda85cbb42d12a70e0d1125b1df7f`, including the line-157 timeout.

**Recurrence:** SHA `1435825cf1896cd1dea40b67b08ea65dfb558d2a` passed
`apps/tauri/verify.mjs` completely, confirming the sidebar fix and backup-refresh
race fix. The packaged enemy-state verifier then failed because the new
`expandedShellPlacement` helper used `:scope > main:not([hidden])`, while the
search `main` is nested under a `display: contents` wrapper rather than a
direct shell child. Product metrics were all fit/no-overflow before the
assertion.

**Disposition:** The CSS/layout failure is fixed and the acceptance advanced;
the line-157 timeout is resolved. The current failure is an over-strict
acceptance selector; use the actual visible `main` descendant and rerun. Do not
promote it to a skill or active conclusion.

**Reproduction status:** Original interception reproduced once and resolved;
the line-157 timeout reproduced once and resolved; the expanded-shell
placement assertion failure reproduced once.

**Follow-up state:** Open pending diagnosis and rerun.

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

**Recurrence:** A clean release build for SHA
`0b16f9d284db12ed46ed9d4120d15ec9353fd1b9` completed launcher/Tauri release
compilation and both PyInstaller workers, but `tools/package_tauri.py` then
failed with WinError 112 while creating
`deliverables/v074-local-candidate-short-labels-20260914/portable/licenses/rust/windows-link-0.1.3`.

**Reproduction status:** Reproduced twice; the first occurrence was resolved
after targeted cache cleanup and the successful debug rebuild and native UI
acceptance, while this recurrence remains unresolved.

**Follow-up state:** Open pending resolution of the recurrence.

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

## 2026-09-15: migration parity gates failed at setup on a full repository volume

**Question:** Why did the M3 migration parity gates report `there is not enough
space on the disk` instead of their real result?

**Observed symptom:** `tests/migration/test_save_read_parity.py`,
`test_save_transaction_parity.py`, `test_search_worker_parity.py` and
`test_rng_parity.py` failed at setup (24 failed, 91 errors in one full-directory
run) while the gates whose build cache lived on `D:` passed. `F:` held 0 bytes
free.

**Root cause:** Those gates defaulted `CARGO_TARGET_DIR` to a path under the
repository (`F:\Nioh3_ScrollEditor\.codex_tmp\<gate>-target`), so a fresh target
tree had to be written to a volume with no space. The failure was environmental,
not a product or parity regression.

**Evidence paths:** gate stderr `could not create incremental compilation
session directory ... There is not enough space on the disk. (os error 112)`;
`F:` free space 0.00 GB; the peer-owned caches
`F:\Nioh3_ScrollEditor\.codex_tmp\m3-save-target` (681 files, 172,410,027 B) and
`...\m23-worker-target` (767 files, 354,377,394 B).

**Disposition:** The two current-goal caches were relocated, not deleted, to
`D:\Nioh3_v080_deliverables\recovered-f-caches-20260915-092427\` with identical
file counts, byte totals and sampled SHA-256, restoring 652 MB on `F:`. Every
migration gate now resolves its target through `tests/migration/cargo_target.py`
(explicit `CARGO_TARGET_DIR` wins, otherwise the platform temp directory), and
`tests/migration/test_cargo_target_defaults.py` is the executable regression for
that rule. A recursive delete was refused by command policy and was not retried.

**Reproduction status:** Reproduced once on this host with `F:` at 0 bytes.
Re-checks: confirm `F:` free space before a cold parity run; never default a
cargo target into the repository; keep the target on `D:` or the platform temp
directory.

**Follow-up state:** Closed as an environment/workflow fix; the gates pass with
the target on `D:` and the shared resolver is in place.

**Skill promotion:** None; the rule lives in `AGENTS.md` and the executable
regression, not in a skill.

## 2026-09-15: shipped save-crypto tool exits 0 without writing its output

**Question:** Why did `test_save_transaction_parity.SaveProductTransactionTests::test_edit_commit_installs_the_composed_record`
fail with "the shipped save crypto component failed" while the tool itself
reported success?

**Observed symptom:** `bin/Nioh_Savefile_decrypt.exe` exited 0, printed
`Success!` and `Press Enter To Exit...`, and never created the requested output
file, so the caller saw only its own generic message with no tool error text.

**Failed hypothesis:** An earlier guess blamed NTFS path length. The save peer's
re-measurement disproved it: 61, 120 and 170-character output paths all
succeeded whenever the output's parent directory was the tool's working
directory.

**Root cause:** Invoking the tool from a working directory that is not an
ancestor of the output path's parent, with an absolute output path outside that
working directory. Bounded reproduction on `D:` with a task-local fixture and a
known-good encrypted container:

| cwd | output | rc | output created |
| --- | --- | --- | --- |
| `source.parent` | `<root>\installed-A.bin` | 0 | no |
| `<root>` | `<root>\installed-B.bin` | 0 | yes |
| `source.parent` | `source.parent\plain.txt` | 0 | yes |
| `<root>` | `source.parent\out.txt` | 0 | no |

**Evidence paths:** original failing output path
`D:\Nioh3_v080_deliverables\m3-save-acceptance\tmp1xs4oej3\installed-edit.bin`
(76 characters) from a 101-character source path; the reproduction table above
was measured by the save peer in `tests/migration/test_save_transaction_parity.py`
territory.

**Disposition:** Fix landed by the save peer: `native_transform_short()` runs the
shipped tool from a short `%TEMP%` working directory with both paths inside it and
then copies the artifact to its destination; the save gates no longer call the
tool with a cross-tree absolute output.

**Reproduction status:** Deterministically reproduced four ways, `D:` only.
Re-checks: keep the short-working-directory helper for every shipped-tool
invocation; treat "rc 0 and no output file" as the tool's silent-failure
signature rather than as success.

**Follow-up state:** Open as a tool-behaviour constraint until every shipped-tool
call site uses the short working directory; the affected gate passes with the
new helper.

**Skill promotion:** None yet; this is one tool's behaviour, recorded so a second
independent occurrence can decide whether it becomes a rule.

## 2026-09-15: save-lane "quiescence" was a snapshot and the guard was mis-reported

**Objective:** Verify the M3-b save lane's three review points against the actual
code and tests: the shipped multi-file timed quiescence, the independence of the
product checksum assertion, and the batch comparison's codec provenance.

**Observed symptom:**

1. `SaveTransactionHost::require_quiescent` slept 2 ms between two full
   main/`BACKUP.BIN`/system fingerprint passes while the shipped reference sleeps
   `SAVE_QUIESCENCE_SECONDS = 0.20`. The Rust commit also revalidated the
   generation only before the checkpoint, never after staging; the shipped
   `commit_encrypted_main_save` rechecks immediately before the replace.
2. `M3B_SAVE_COMMIT_COMPARISON.json` and the handoff claimed the shipped
   `SaveInstaller.edit_many` paid a 0.20 s window. It calls no quiescence helper at
   all; the windows live on `install`/`install_many`/`restore`.
3. The handoff and a read-gate comment claimed the shipped tool rewrites the
   trailing user-checksum field on decrypt, which would make every
   decrypt-then-fold assertion tautological.

**Failed hypothesis / wrong assumption:** That a two-pass fingerprint comparison
with a nominal delay was "the same guard" at any interval, and that the shipped
decryptor normalizes `0x900194`. Both were recorded as facts without measuring
the reference or the tool.

**Root cause:** The port reproduced the shape of the shipped guard (two
fingerprints, digest comparison) but not its timing constant or its second
revalidation point, and the report described the intended semantics rather than
the implemented ones. The oracle's normalizing field was misidentified: a direct
oracle probe (plaintext with a deliberately non-fold checksum, both directions)
shows it preserves `0x900190` and `0x900194` and zeroes only the final 8 bytes
(`0x9001A8..0x9001AF`). Those 8 bytes sit outside the transformed body
(`USER_BODY_BYTES = 0x900058` is not a multiple of the 16-byte block), so no
implementation can return them and both read back zeros; the checksum field is
well inside the transformed region and is preserved byte for byte.

**Evidence paths:** `crates/nioh3-save/src/transaction.rs` (`require_quiescent`,
`install`), `nioh3_scroll_editor/savegame.py` (`SAVE_QUIESCENCE_SECONDS`,
`capture_quiescent_save_fingerprints`, `commit_encrypted_main_save` lines 919/943,
`edit_many`), `deliverables/v080-completion-readiness/M3B_SAVE_HANDOFF.md`,
`M3B_SAVE_COMMIT_COMPARISON.json`; probe artifacts under
`%TEMP%\n3probe-*` (regenerable from the fixture builder).

**Disposition:** Fixed and re-gated. `SAVE_QUIESCENCE_MILLIS = 200` is the
default with `with_quiescence_interval`/`without_quiescence_delay` changing only
the delay, and `install` revalidates the generation across a fresh window before
the replace. `test_save_transaction_parity.py` gained
`test_an_external_writer_inside_the_quiescence_window_is_refused` (writer inside
the window refused; same writer with a zero window passes; the guard's own
elapsed time >= the shipped constant) and the crate gained
`default_quiescence_window_matches_the_shipped_constant` and
`an_external_writer_inside_the_window_is_refused`. The guarded comparison now
runs `install_many` on both sides (three windows each): Rust 1.883 s vs 3.032 s
median-of-5 in the last logged run. The checksum assertion is now anchored on a byte-preserving Rust
decode plus a corruption negative, with the oracle's real behaviour pinned by
`test_the_shipped_tool_preserves_the_checksum_and_drops_only_the_trailer`.

**Reproduction status:** Deterministic. Windowed refusal and the zero-window
control both reproduce on every run; the oracle's trailer-only normalization is a
pure byte diff.

**Follow-up state:** Closed for the synthetic-fixture lane. Live acceptance (a
real save and a running game) remains open and unclaimed, as for the rest of this
lane.

**Skill promotion:** None. The general lesson (reproduce the shipped constant and
every revalidation point, and measure an oracle before describing it) stays in
this ledger until a second independent occurrence.

## 2026-09-15: final v0.8.0 packaged-host acceptance timed out waiting for the backend connection

**Objective:** Complete same-candidate packaged-host acceptance for the final
v0.8.0 Rust-backend candidate (clean source commit
`122c1027c00002e2ebb845716386dbf591406684` at
`D:\Nioh3_v080_deliverables\v080-candidate-source`).

**Symptom:** On the same built artifact bytes, the Rust three-role identity check
and the `npm test:packaged` worker-parity leg passed, but four packaged-host
acceptance gates did not complete. `verify.mjs`, `verify-add-layout.mjs` and
`verify-update.mjs` timed out while waiting for the packaged app's backend to
connect, and `verify-host-package.mjs` read the host's own startup record as
`graph=null` where `rust-packaged` was expected. The failure is in the packaged
host startup/handshake chain, not in the worker binaries themselves.

**Root cause:** Unknown. Reproduction is confirmed across four artifact gates;
diagnosis is active under `/root/m4_final_candidate`. Do not state a cause, and do
not treat the passing identity/parity legs as evidence that the host startup
chain works.
Resolved on 2026-09-19; see the entry below.

**Evidence:** Candidate source `122c1027c00002e2ebb845716386dbf591406684` at
`D:\Nioh3_v080_deliverables\v080-candidate-source`. Artifact identity: portable
749 files / 30,941,042 bytes, ZIP 10,398,657 bytes, outer EXE 11,022,329 bytes.
Passing: Rust three-role identity and `npm test:packaged` parity. Failing gates:
`verify.mjs`, `verify-add-layout.mjs`, `verify-update.mjs` (backend-connect
timeout) and `verify-host-package.mjs` (`graph=null` vs `rust-packaged`).
Diagnostic logs are under `D:\Nioh3_v080_deliverables\quality`; the candidate
owner can supply exact artifact log references if that is cheap.

**Disposition:** Left open for the candidate owner to reproduce and fix; this
entry is documentation only. No code was inspected or changed and no gate was
re-run by the documentation task. Distinguish this substantive packaged-host
startup/handshake failure from an ordinary wording or assertion-message test
failure, which is not logged here.

**Reproduction status:** Reproduced across four artifact gates on the same
candidate bytes; root cause not yet isolated. Re-check: re-run the four gates
against the exact artifact and capture the host startup record and the
backend-connect wait before comparing any worker-level result.

**Follow-up state:** Open pending the candidate owner's diagnosis and a fixed
candidate. No live game, user save, or remote write is involved; the candidate
was not pushed, tagged, packaged for release, or published.

**Skill promotion:** None. Promote nothing until the root cause is known and the
failure recurs independently.

## 2026-09-19: final v0.8.0 packaged-host acceptance closed: four root causes fixed

**Objective:** Explain the 2026-09-15 packaged-host acceptance failure recorded above and re-verify a fixed candidate on
the same machine.

**Symptom:** The same four gates as that entry: `verify-host-package.mjs` read `graph=null` instead of `rust-packaged`,
and `verify.mjs`, `verify-add-layout.mjs` and `verify-update.mjs` timed out waiting for the packaged backend to connect.

**Root cause:** Four distinct defects, three of them in the acceptance harness.

1. Product, release host, fixed on 2026-09-15 in commit `fab3ce9`: the packaged root from Tauri `resource_dir()` is
   canonical (`\\?\D:\...`), while `worker-backend.json` declares package-confined paths with POSIX separators
   (`<runtime>/worker/runtime/...`). A canonical path is never normalized by the filesystem, so the joined path was
   looked up literally and every worker resolution failed with `WORKER_BACKEND_RESOURCE_MISSING`; that is why
   `verify-host-package` read `graph=null` and the three UI gates never connected. The test-only override
   `NIOH3_TAURI_PACKAGE_ROOT` is compiled out of a release build (`apps/tauri/src-tauri/src/main.rs`,
   `(!packaged).then(...)`), so no development shape could reproduce it. Direct evidence:
   `D:\Nioh3_v080_deliverables\quality\hostprobe\profile\logs\desktop.log` and
   `D:\Nioh3_v080_deliverables\deliverables\v080-backend-review\evidence\add-layout\failure.txt` (identical error string
   inside the running UI).

2. Harness, commit `86ae430`: `apps/tauri/verify-host-package.mjs` compared paths with `resolve()`, which keeps the
   `\\?\` prefix, so the gate failed on the host's correct resolution. It now folds the extended-length prefix.

3. Harness, commit `28fe250`: `apps/tauri/verify-worker-identity.mjs` called `main()` at import time, so
   `verify-onefile.mjs` ran that CLI with its own argv, printed `TAURI_WORKER_IDENTITY_FAILED: --runtime is required`
   and forced exit 1 after its real assertion had passed. It now runs the CLI only as the entry point.

4. Product, commit `b20e493`: a Rust restore reused `plan.backup_id` and wrote no pre-restore checkpoint, silently
   dropping the shipped behaviour (`nioh3_scroll_editor/savegame.py` creates a bundle whose manifest action is
   `pre-restore-checkpoint`, plus `restore-journal.json`). The Rust transaction now writes that checkpoint as a new
   bundle before reading the selected backup and carries the journal prepared -> committed, or rolled_back /
   recovery_required on failure. Regression gate: `tests/migration/test_save_transaction_parity.py`.

**Evidence:** Candidate source `b20e493ff0b2374978d008451a461cb9caa6d44b` in clean detached worktree
`D:\Nioh3_v080_deliverables\v080-candidate-source`; artifacts unchanged since. Portable
`D:\Nioh3_v080_deliverables\deliverables\v080-backend-review\portable-b20e493`: 749 files / 30,954,866 bytes,
`build-manifest.json` git=`b20e493` dirty=false, all 748 member hashes re-verified. Outer EXE
`Nioh3Studio-0.7.5-win-x64-b20e493.exe` 11,027,487 bytes, sha256
`546be5fa25a2df8f78ff41d0775b7f72c16065a2fd244f0963f773adc12581e8`; ZIP 10,403,815 bytes, sha256
`1ef50eef2c4290c6309620ea55a7c914a0a97f7cc3e69480a608f0a8c543c8b2`. Gates on those exact bytes:
`TAURI_HOST_PACKAGE_RESOLUTION_OK` (`verify-host-package.mjs`, graph `rust-packaged`),
`TAURI_WEBVIEW2_SEARCH_FAVORITES_INVENTORY_RESTORE_OK` (`verify.mjs`), `verify-add-layout.mjs` all legs,
`TAURI_REAL_UPDATE_RESTART_CLEANUP_OK` (`verify-update.mjs`), three-role identity, `PACKAGED_R3_R4_R5_PARITY_OK`
(`npm run test:packaged`), `TAURI_ONEFILE_DIRECT_LAUNCH_CACHE_OK`, `TAURI_ONEFILE_REAL_UPDATE_RESTART_CLEANUP_OK`,
`TAURI_ONEFILE_ROLLBACK_RESTORES_ORIGINAL_OK`, `PACKAGED_COLD_START_OK` (median cold 4210 ms / warm 2113 ms,
`rust-packaged` for both roles). Migration gates rerun after the save change: `test_save_transaction_parity.py` 31
passed, `test_protected_save_acceptance.py` + `test_save_performance_parity.py` + `test_save_batch_install_parity.py` 22
passed, `nioh3-save` 33 unit tests, `nioh3-protected` crate tests, `cargo fmt` / `clippy` clean.

**Disposition:** Closed for the four startup gates on the `b20e493` candidate; the earlier entry's symptom is fully
explained. Nothing was pushed, tagged, published or installed; no game process, real save or user state was involved.

**Reproduction status:** Deterministic; each of the four defects was reproduced and then fixed on the same machine.

**Follow-up state:** Closed for this lane; no open action against the 2026-09-15 entry.

**Skill promotion:** None for items 2-4 (single occurrence each); the canonical-root lesson in item 1 is a general
Windows packaging hazard worth promoting if it recurs.

## 2026-09-19: release-host packaged-frontend gate sampled worker readiness once

**Objective:** Run the remaining final UI acceptance gate for the v0.8.0 Rust-backend review candidate in release-host
mode: `apps/tauri/verify-packaged-frontend.mjs --host release` against the extracted one-file runtime
`D:\Nioh3_v080_deliverables\deliverables\v080-backend-review\portable-b20e493` (inner `Nioh3Studio.exe` sha256
`6460a6bc318e9a28caf581f51fdb89980269358051073cc9e8b7120f7b29db19`), outer EXE 11,027,487 bytes sha256
`546be5fa25a2df8f78ff41d0775b7f72c16065a2fd244f0963f773adc12581e8`, outer ZIP 10,403,815 bytes sha256
`1ef50eef2c4290c6309620ea55a7c914a0a97f7cc3e69480a608f0a8c543c8b2`, artifact source commit
`b20e493ff0b2374978d008451a461cb9caa6d44b`, dirty=false.

**Symptom:** First run exited 1 after roughly 50 seconds at the harness's final diagnostics assertion:
`AssertionError: Expected values to be strictly equal: + actual 'starting' - expected 'ready'` for the offline_search
worker's `connection`. Every earlier leg had already passed on those exact bytes: three locales, the v0.7.5 regression
seed 226061463 at cursor 158614759, cancel/resume, the editor commit with byte readback, delete, restore, and the cart
plan. So the failure looked like a release-host worker-startup defect.

**Root cause:** Harness timing assumption, not a product defect. `apps/desktop/src/worker-client.ts` reports
`connection: 'ready'` only after a handshake has been recorded, else `'starting'`. The shipped UI
(`apps/workshop/main.tsx`) mounts the shell first, loads its catalogs, and only then calls the search controller's
`connect()`, so right after a restart the host can legitimately report `starting`. The harness waited only for
`#root .shell` and then sampled `support:diagnostics` once, so it raced the UI's own startup sequence. It passed in
the earlier debug/staged shape only because that shape lost the race in the harness's favour.

**Evidence:** Failed run log
`D:\Nioh3_v080_deliverables\deliverables\v080-backend-review\evidence\packaged-frontend-b20e493\run1-failed.log`;
its partial evidence `partial-evidence.run1.json` in the same directory shows all earlier legs green. After the fix
the recorded settle time was 824 ms (`diagnostics.readyWaitMs` in `packaged-frontend.json`), which is the direct
measurement that this was a race and not a hang.

**Disposition:** Fixed in the harness (test-only file, not an artifact member): the post-restart readiness check is now
a bounded wait (60 s deadline, 250 ms poll) that still fails immediately on an `unavailable` worker and still asserts
`ready` at the end. The same edit also made screenshot capture and raw artifact hashes opt-in (`--screenshots`,
`--artifact-exe`, `--artifact-zip`). No product source changed, no rebuild, no push. Re-run on the same bytes:
`TAURI_PACKAGED_FRONTEND_OK`, exit 0.

**Reproduction status:** Reproduced as a race; the failing observable was captured once, then the same command passed
on the same bytes with the bounded wait. Re-check by running the release-host gate twice and comparing
`diagnostics.readyWaitMs`.

**Follow-up state:** Closed for this candidate. Any future release-host run that again reports the search worker as
`starting` at the end should check this bounded wait before treating it as a product startup defect.

**Skill promotion:** None. This is a bounded harness assertion defect, not a product rule; do not encode it as a skill.

## 2026-09-19: PC v2.02 revision and level-clamp Pro package failed acceptance on packaging closure

**Objective:** Deliver a self-contained Pro research package for the PC v2.02 添画 (Divine-rarity scroll revision)
and level-clamp questions, plus an independent adequacy review of that package.

**Symptom:** The v2 package was rejected for full mechanism handoff. Root review and the independent review agreed on
three defects: (1) the export's baseline side carries only `baseline_sha256` and pdata ranges, no baseline bytes or
disassembly, so the quoted 148/29/17/8 byte differences cannot be classified as displacement-only versus semantic
changes despite the old/new framing; (2) the packaged probe path is not closed, because `probe_level_fields.py`
imports `validate_ng3_rarity4_native_parity_live` and `nioh3_scroll_editor.native`/`recommended_level`, none of which
are packaged, and the in-package fixture reads `parents[1]/reports/*.json` while the files live under `evidence/`;
(3) a self-test import wrote `project-source/__pycache__/recommended_level.cpython-312.pyc` after the sums and archive
were produced, so the directory no longer matched `SHA256SUMS.txt` or the ZIP.

**Root cause:** Packaging selection, dependency closure, and coverage verification. Not a game mechanism, not a native
conclusion, and not a contradiction of the level-mapping evidence.

**Evidence:** v2 package and `...pro-handoff-20260919-v2.zip` (97,681 bytes, sha256
`79D5FCEC75335E744F949EF203E0A4D22F58577632CF9BBF30351DC151FBD765`; ZIP members byte-identical to the directory,
57/57, so the archive itself is internally consistent). Adequacy review
`D:\Nioh3_v080_deliverables\deliverables\game-version-update-20260919\reports\PRO_HANDOFF_ADEQUACY_REVIEW.md`
(sha256 `E720FA7107BC3D3748064A8FE901EA7FF4FDACDEAE4CA96650A22212B92B6BCC`). Coverage was checked by reading
`tools/validate_research_handoff.py` and re-implementing its set-equality, per-file hash, and ZIP-versus-directory
comparisons in PowerShell; the validator script itself was not executed.

**Disposition:** v2 is adequate for bounded Pro probe design only; it is not accepted as a full mechanism handoff. The
producer is preparing v3. No package, product, or handoff-builder file was modified by the reviewer, and no push, tag,
game, save, or native call was made.

**Reproduction status:** Deterministic by static archive inspection: unlisted `.pyc`, sums/ZIP/directory set
mismatch, and absence of baseline bytes are all reproducible from the frozen v2 archive and directory.

**Follow-up state:** Open with the producer for v3: add baseline bytes for the exported bodies, close the oracle and
fixture import paths, give row/field schema for the two changed tables, and revalidate after the final copy. Root will
request an independent v3 review when it exists; do not re-review v2.

**Skill promotion:** None. Document the class-level lesson only if the same packaging defect recurs.

## 2026-09-19: PC v2.02 Pro package v3 accepted for bounded analysis, with residue

**Objective:** Independent acceptance of the v3 revision/clamp handoff from the ZIP alone, then close the v2 packaging
entry.

**Symptom:** The v2 defects are fixed, but two of the same class survive in smaller form. `verify_package.py` passes
twice from a fresh isolated unpack (`curve_points 42`, `vectors 12`, `raw_records 24`) with no repository import
available, and all eight bodies now carry baseline and target bytes whose recomputed differential counts match the
v2 report exactly (13/5/148/29/7/3/8/17). However `project-source/test_package_fixtures.py` asserts 48 raw `.bin`
records while 24 ship, so the packaged self-test fails `1 failed, 2 passed`: the reference probe names raw files
`L{level}-R{recommended}-{label}.bin` without the seed, so seed 2 overwrites seed 1. The probe JSON keeps both seeds'
SHA-256 values and both seeds store identical field values per vector, so the mapping claims stay reproducible, but
seed-1 raw bytes are absent. Secondary: 14 of 160 level-scan hits carry no `owning_function`, `gate_evidence` still
points at `reports/...` while four referenced files are absent from the package, the reference fixture copy keeps a
repository-relative `reports/` path, and the bundled curve resource is labeled `PC v2.00.02`.

**Root cause:** Same class as the v2 entry - packaging selection and coverage verification - now limited to a stale
test expectation, a filename collision in the reference probe, and stale path pointers. Not a game mechanism and not a
contradiction of the level or parity evidence.

**Evidence:** v3 ZIP 127,604 bytes, sha256
`521AE29F09DDD80C7EF0D3FC71CC94F0CABDF4A2AA9A103C84365389F5282EA6`, 55 members, sums coverage 54/54, no `.pyc`.
Isolated unpack under `%TEMP%\v3-acc-f1ee2c87249f`; checker run with
`F:\Nioh3_ScrollEditor\.codex_tmp\v2-build-env\Scripts\python.exe -I -X utf8` and empty `PYTHONPATH`, with
`find_spec("nioh3_scroll_editor")` returning `None`. Updated adequacy report
`D:\Nioh3_v080_deliverables\deliverables\game-version-update-20260919\reports\PRO_HANDOFF_ADEQUACY_REVIEW.md`
(sha256 `28B3E1DCA6B49D36D3A64B30B8311A21388CD61868F12A85F43879C2D953CCFE`).

**Disposition:** v3 accepted as adequate for bounded Pro analysis and next-probe design; a captured revision pair is
not demanded, because `TASK_FOR_PRO.md` explicitly accepts naming the exact probe instead. No package, product, or
handoff-builder file was edited; no game, save, native, push, or publish action.

**Reproduction status:** Deterministic by static inspection plus the packaged checker: the failing fixture assertion,
the 24-versus-48 raw count, and the unannotated scan hits all reproduce from the frozen v3 archive.

**Follow-up state:** v2 entry closed. Remaining items are disclosure or minor-fix level: align the fixture expectation
with the seed-less raw naming (or add the seed to the filename), annotate or disclose the 14 unowned scan hits,
repoint `gate_evidence` to `evidence/...`, mark the reference fixture as repository-only, and state that the curve
resource is the byte-equal `PC v2.00.02` copy. Do not re-review v2 or v3 unless the producer ships a corrected archive.

**Skill promotion:** None. The recurring lesson - checksum and self-test coverage must be verified after the final
copy, from the archive rather than the build tree - is already a runbook rule; promote only on a third recurrence.

## 2026-09-19: v3 closeout scope corrected - seed-agnostic raw filename overwrote the first seed

**Scope correction:** Only the v2 core gaps are closed by the v3 work: baseline bytes with pdata ownership, the oracle
interface spec, the record contract, and honest unknowns. The v3 portable-test and raw-data disclosure gaps remain
open, so delivery requires v4. This note amends, and does not replace, the two entries above.

**Observed:** The reference level probe names raw payloads `L{level}-R{recommended}-{label}.bin` with no seed
component, so each vector's second seed overwrote the first seed's stage and final bytes. The package therefore ships
24 raw bins (12 vectors x stage/final, one seed each) while the packaged `test_package_fixtures.py` asserts 48 and
fails `1 failed, 2 passed`.

**Not a game failure:** This is a capture-filename and coverage-disclosure defect. It says nothing about native
behaviour, causality, or the revision mechanism. Both seeds' SHA-256 values and stored field values survive in
`evidence/level-field-probe-v2.02.json`, the surviving 24 bins are reproducible second-seed bytes, and the level and
clamp mapping claims stay reproducible from them.

**Evidence:** v3 ZIP sha256 `521AE29F09DDD80C7EF0D3FC71CC94F0CABDF4A2AA9A103C84365389F5282EA6` (127,604 bytes);
updated adequacy report
`D:\Nioh3_v080_deliverables\deliverables\game-version-update-20260919\reports\PRO_HANDOFF_ADEQUACY_REVIEW.md`
(sha256 `99BFF8A57DB16A52BF10ECA2372CFE2A8D842EC78AE988D67F28E4B47954F9BA`).

**Disposition:** v3 core adequacy accepted; portable test closure and raw-data disclosure deferred to v4. Fixes are
seed-unique raw naming plus an explicit 24-record coverage statement. Do not re-run the live probe and do not
fabricate seed-1 bytes. No package, source, or product file was edited, and no game, save, native, or publish action
was taken.

**Reproduction status:** Deterministic from the frozen archive: 24 shipped bins, two seeds per vector in the probe
JSON, one surviving seed's bytes on disk, and the failing 48-record assertion.

**Follow-up state:** Open until v4 ships with seed-unique naming and honest coverage. No further broad review is
required before v4 is ready.

**Skill promotion:** None. Reiterate the existing "verify coverage after the final copy" runbook rule; no new skill.

## 2026-09-19: PC v2.02 Pro package v4 delta acceptance - data closed, docs pending v5

**Scope:** v4 closes the archive-level gaps left open by v3. Raw seed-1 bytes remain absent and are now disclosed as
unrecoverable; the planned collector fix adds the seed to the raw filename and does not re-capture.

**Verified:** v4 ZIP sha256 `0293B258A962162226DDC89D494606EC63A6DAC8E0F623A682C395A2F5170511`, 135,622 bytes, 60
members, no `.pyc`. Repository validator `ok: true` (60 files, 59 hashes, ZIP SHA-256 match). Package checker
`verify_package.py` reported `vectors 12`, `raw_files 24`, `raw_seed_coverage "second seed only"`, `bodies 8`,
`curve_points 42`. Packaged pytest passed 4 of 4. All 24 shipped bins matched the seed-2 `stage_sha256`/`final_sha256`
entries with zero mismatches, and no vector had equal seed-1 and seed-2 stage hashes. Every `gate_evidence` path token
resolved inside the package. The eight v3 bodies survived with differential counts unchanged
(13/5/148/29/7/3/8/17).

**Remaining, documentation only:** `TASK_FOR_PRO.md` is not standalone (it opens with "Same three questions as v3"),
and `KNOWN_LIMITS.md` says "2 vectors per stage/final file", which is wrong because each file is one vector, one seed,
one stage. The reference probe still writes seed-less raw names by design of the deferred collector fix.

**Disposition:** v4 data, checker, tests, and coverage disclosure accepted for bounded Pro analysis and next-probe
design. No mechanism completion claim; load-path and revision semantics remain open as documented. Final delivery
waits on the docs-only v5 delta plus hash closure; no whole-package audit is needed for that delta.

**Reproduction status:** Deterministic from the frozen v4 ZIP; every check above reruns from the archive alone.

**Follow-up state:** Open on v5 documentation corrections only. No live re-capture, no fabricated seed-1 bytes, no
package or source edits by the reviewer.

**Skill promotion:** None.

## 2026-09-19: level-setter observer reported READY before the CE bridge existed

**Objective:** Arm the four owned breakpoints inside `assemble_scroll` for the PC v2.02 level-setter observation (the
`0xD7C3` key request, the returned threshold, and the write into record `+0x10`/`+0x12`) inside one bounded
120-second window, with the owner holding the title / save-select screen.

**Symptom:** The lane reported the observer as built, mocked, and preflighted, and told the owner that arming could be
immediate. The owner then waited at the title / save-select screen for more than two minutes without an `armed`
confirmation. On escalation the CE MCP bridge and its CE session had never been started and the CE adapter was
absent, so no arm transaction could have occurred; hardware breakpoint slot availability was never verified either.
Counts for this attempt: zero breakpoints armed, no 120-second capture window opened, no game function called, and no
game or save write.

**Cause:** Mock API validation plus a direct-read process preflight were promoted to operational readiness.
`deliverables/game-version-update-20260919/tools/observer_preflight.py` reads process memory only, so it can validate
process identity and probe-site signatures but cannot observe the CE bridge, the CE session, the adapter, debugger
state, or hardware slot availability; the mocked suite exercises the observer against a fake CE API. The resulting
`ready` therefore described the artifact, not the live transport. The readiness narrative also asked the owner to
hold a confirmation screen before any bridge existed, inverting the documented order in
`.agents/skills/nioh3-ce-research/references/runtime.md` ("Complete bridge, target-attachment, debugger, and cleanup
readiness before asking the owner to hold a confirmation screen"). The owner was called prematurely.

**Secondary symptom:** `deliverables/game-version-update-20260919/reports/level-setter-observer-readiness-20260919.md`
states "12 mocked assertions", while `tests/test_level_setter_observer.py` actually contains 16 named mocked checks
inside one pytest function. The readiness narrative undercounted its own offline evidence, so no mock count quoted
from that document should be reused.

**Evidence:** `reports/observer-preflight-20260919.json` (schema `nioh3-observer-preflight/v1`, `status: ready`,
`read_only: true`, PID 18568, module SHA-256 `E22C4A63...AE130`, five site signatures matched);
`tests/test_level_setter_observer.py`; `reports/level-setter-observer-readiness-20260919.md`;
`reports/precapture-backup-20260919.json`; and the existing remediation path
`.agents/skills/nioh3-ce-research/scripts/ensure_ce_mcp.py` with `references/runtime.md` sections 1 and 2. No durable
CE or session transcript from this attempt exists; the two JSON reports and the readiness document are the only
retained machine artifacts, and the bridge/session absence is reported from the owner's escalation rather than from a
captured log.

**Disposition:** No capture and no result. The observer, its sites, and its offline evidence remain valid as
artifact-level work; only the readiness claim is withdrawn. Remediation is assigned to the `pro_handoff_review` lane:
it must document and demonstrate actual CE auto-start, adapter presence, session identity, target attachment,
debugger state, and hardware slot availability as the readiness gate, and confirm bridge and cleanup state before the
owner is asked to hold a screen again. Status is open until real evidence exists.

**Reproduction status:** Not independently reproduced as a native or game behaviour, because no live phase ran. The
transport gap is deterministic from the artifacts: the direct-read preflight path has no CE dependency, so a `ready`
result cannot arise from it. The premature owner call is a process observation, not a mechanism.

**Follow-up state:** Open. Do not treat this as resolved until the `pro_handoff_review` lane produces real
bridge/session/attachment evidence. A mock suite or a direct-read preflight must not be quoted as operational
readiness again.

**Skill promotion:** None. The correct ordering is already a rule in `references/runtime.md`; this is the first
recorded violation of it. Promote only if it recurs.

## 2026-09-19: stale injected vehdebug DLL wedged the CE session and the target crashed during debugger recovery

**Objective:** Run the read-only dispatch-boundary gate `G1-V202-SCHEDULER-READONLY-DISPATCH-BOUNDARY` against the
long-lived PC v2.02 title-screen process (PID 18568, born 2026-09-19 06:06:43 local) that earlier stages 1-5 used.

**Observed symptom:** In the inherited CE session (`ce-29176`, CE PID 29176) `debug_isBroken()` reported true while
the target kept executing (12-20 CPU-seconds per 3 wall seconds), `debug_continueFromBreakpoint(co_run)` returned
true without changing that flag, and a 25 s positive control on `kernel32.GetTickCount64` delivered zero hits. A
forced recovery attempt (detach, then `debugProcess(1)`) produced a modal "Debugger attach timeout" dialog; CE's
main thread then stopped answering `ce.lua_exec` (every call timed out) although the process stayed alive and idle.
A freshly launched CE (PID 35196) attached successfully with interface 1 and delivered 5 control hits on
`kernel32.WaitForSingleObject`, with hardware registers proven in all threads; but a follow-up
`detachIfPossible(); debugProcess(2)` (VEH) hung again with a modal "Error" dialog. During that VEH recovery attempt
the game exited: APPCRASH `0x80000004`, faulting module `KERNEL32.DLL`, at 2026-09-19 11:03:34 local.

**Suspected root cause (not proven):** the target process still carried the injected `vehdebug-x86_64.dll` from the
earlier lane session, so CE's VEH (re)attach could not complete its handshake; the failed attach then left the CE
session and, on the second attempt, the target in a faulting state. Not proven: whether the crash came from the
stale DLL, from CE's aborted attach, or from an unrelated game fault. `THREADID` also returns 0 for auto-resumed
callbacks in this CE build, which is a separate, unresolved observation, not part of this crash.

**Evidence:** `deliverables/game-version-update-20260919/deepseek-v202-scheduler-live-gate/evidence/` -
`phase-plumbing.raw.json`, `ce-state-diagnostic.json`, `ce-state-diagnostic-resume.json`,
`debugger-reset-experiment.json`, `callback-path-experiment.json`, `dr-register-experiment.json`,
`multi-control-experiment.json`, `interface1-identity-experiment.json`, `attach-interface1-experiment.json`, plus
`PENDING_OWNER_ACTION.md` and Windows Error Reporting events 1000/1001 in the Application log. No CE or session
transcript beyond these JSON reports was retained.

**Verified clean state after reset:** CE relaunched via the approved
`.agents/skills/nioh3-ce-research/scripts/ensure_ce_mcp.py --port 5566` helper (plugin/core hashes pinned and
verified), session `ce-15928`, no target attached, breakpoint inventory empty. No game function call, no redirect,
no register write, and no memory or save write occurred at any point in this line of work.

**Disposition:** Exploration halted at the plumbing boundary; the gate capture was never armed. The long-lived
process is gone, so the gate now requires a fresh game process (and the owner has been asked for exactly that one
action). The lane's read-only tooling, dispatch-site identity checks, and owned-breakpoint roundtrip remain valid
artifact-level work.

**Reproduction status:** One occurrence, not reproduced. The wedged-session symptom (blocked `ce.lua_exec`) was seen
twice in the same inherited session; the VEH re-attach hang was seen once on a fresh CE. Both need a fresh game
process before this can be retested.

**Follow-up state:** Open. Next attempt must start from a new Nioh 3 process without a preloaded CE debug DLL, keep
the session identity recorded per phase, and re-run the read-only plumbing checkpoints before arming.

**Skill promotion:** None. This is a one-off session-state failure; revisit only if a second occurrence confirms it.

## 2026-09-19: VMProtect-protected trainer sample blocks equipment mechanism recovery

**Objective:** Recover equipment add/edit/generation structure (item offsets,
AOBs, add-call ABI, effect/rarity/level/seed handling) from the third-party
`Nioh3Trainer-70.0.0.exe` sample by static analysis only, without executing the
trainer, without touching the game, and without any protection circumvention.

**Observed symptom:** Static metadata extraction succeeded, but no equipment
mechanism could be recovered. The sample is a native x64 Windows GUI PE whose
executable body sits in a VMProtect `.vmp1` section (raw `0x400`-`0x1C38A00`,
29,591,040 bytes, entropy 7.9987) containing the entry point RVA `0x3221650`,
with an uninitialized `.vmp0` section (`RawSize=0`, virtual 23,060,480 bytes)
and a DOS stub message replaced by `The program is protected by VMP.`. The only
plaintext regions are the `0x0`-`0x400` headers, the `.rsrc` resource entries
(`0x1C38A00`-`0x1C5BAE8`: 13 icons, one version resource, one manifest) and the
import table at the tail of `.rsrc` (`0x1C5BAE8`-`0x1C5C800`: 41 descriptors,
44 named imports). There is no overlay and no nested container.

**Root cause:** Protection, not a tooling defect. Every probe of interest -
code, equipment strings, the declared export directory (RVA `0x3006560`), TLS
and load-config data - lies inside the encoded section, so the file bytes carry
no readable equipment semantics. High entropy is consistent with both
encryption and compression and does not identify which; the protection mode
stays unknown. The equipment-relevant string probe is an exact negative
(2 false-positive hits: the manifest `requestedExecutionLevel` line at
`0x1C5B949` and `DoDragDrop` at `0x1C5C51C`), and a UTF-16 CJK candidate scan
sits at the deterministic random noise floor (217,899 candidates versus 218,297
expected, ratio 0.998), so its silence is explicitly not evidence about
features.

**Evidence:** `deliverables/equipment-trainer-research-20260919/evidence/structure/`
- `STRUCTURE_FINDINGS.md`, `identity.json`
  (sha256 `6925848936e42cb6202ce2a8d5637bef042c10abbd075facde6d79e7b39c6c65`),
  `pe_headers.json`, `pe_sections.json`, `pe_directories.json`,
  `file_layout_map.json`, `pe_resources.json`, `resource_probe.json`,
  `pe_imports.json`, `pe_exports.json` (marked `valid: false`),
  `protection.json`, `container_identification.json`, `container_markers.json`,
  `randomness_assessment.json`, `cjk_candidate_report.json`,
  `strings_candidates` summary files, `tools_crosscheck_7zip.txt` (independent
  7-Zip 25.01 listing), `ARTIFACT_SHA256SUMS.txt`. Full random-region candidate
  corpus stays in scratch (`.codex_tmp/trainer-structure-20260919/scratch/`).
  Sample path of record: `D:\BaiduNetdiskDownload\仁王3修改器\Nioh3Trainer-70.0.0.exe`.

**Disposition:** Bounded static extraction only; closed as a static research
blocker for mechanism recovery. No second-stage payload was located by these
static probes, and equipment structures were not recovered by this static
analysis. This says nothing about whether the trainer implements equipment
features. Not attempted by ticket boundary: executing the trainer, memory
dumping, or any protection or licensing bypass.

**Reproduction status:** Deterministic offline. Re-running
`tools/run_python_tests.ps1` with the structure test path regenerates every
evidence file from the frozen sample hash above; the 7-Zip listing reproduces
the section/resource structure independently.

**Follow-up state:** Open only as an owner decision: any attempt to read the
protected body would require running the trainer in a controlled environment
under explicit owner authorization and a separate ticket, and would still be a
different evidence class from this static report.

**Skill promotion:** None. The transferable part - plaintext metadata versus
protected body, exact negatives, and entropy not proving encryption - belongs
in the structure report, not in a new skill.

## 2026-09-19: PC v2.02 noop live attempt timed out, then the game process exited

**Question:** Does the accepted PC v2.02 dispatch boundary survive one bounded
read-only noop dispatch (redirect and return, no builder call, no insertion)?

**Observed symptom:** The live worker's noop attempt timed out, and the game
process (PID 40060) exited afterwards. The cleanup receipt may show a settled
owned state, but the actual cause of the game exit is unproven: a timeout is not
by itself an explanation, and a settled receipt does not establish why the
process ended.

**Root cause:** unknown.

**Secondary symptom (independent, test-harness hygiene):** an earlier
full-suite pytest run of mine was believed finished but was still alive as
PIDs 17148/43848/3380 and had spawned `nioh3-protected-worker.exe` PID 39888 at
13:59:29. Those processes are stopped; no pytest or protected-worker process
remains. This is a harness-hygiene defect, not evidence about the game exit.

**Evidence:** The live attempt's raw capture belongs to the live worker and its
path must be confirmed with that owner before being cited. Process-table
readings (PID, parent PID, birth time, command line) for the orphaned run are
recorded in the completion report under
`deliverables/game-version-update-20260919/go-v202-candidate-profile/`.

**Disposition:** The noop attempt is closed as a failed attempt, not as a
product finding. The live lane is now read-only failure diagnosis with no game
restart. No product code, resource or registry changed as a result, and the
v2.02 runtime write gate stays disabled.

**Reproduction status:** Not reproduced. The timeout and the process exit have
each been observed once and are not separately explained.

**Follow-up state:** Open: confirm the raw evidence path with the live worker
and decide whether a single controlled retry is warranted. Do not treat the
timeout as evidence for or against dispatch-boundary correctness.

## 2026-09-19: routed worker attempts ended in repeated summary finals and raw DSML text

**Question:** Did two routed worker sessions (`/root/go_v202_rust_candidate`
repeatedly finalizing with compaction summaries, and
`/root/go_v202_serial_boundary` repeating text and emitting raw DSML markup)
fail from transport abort, provider quota exhaustion, excessive context, or a
successful but malformed upstream response?

**Failed approach:** Two routed workers were left running long enough to reach a
very large working set, and their turns were accepted as if status 200 implied
a usable response. Provider-side response content was never independently
captured, so the client-visible symptoms could not be tied to a specific
upstream response.

**Root cause:** unknown. The bounded router review found canceled turns
(status 0) and unusually large context (up to ~766k input tokens), but no
quota exhaustion and no content-level evidence that identifies the trigger.
Correlation between high context and the failure window is not proof that
compaction or context size caused the behavior.

**Secondary symptoms:** One worker produced repeated prose instead of
terminating, and another emitted raw DSML markup that the client could not
consume. Both shapes appeared alongside successful HTTP 200 responses in router
timings, which record status and token counts but not content or finish reason.

**Evidence:** Bounded read-only review is documented in
`deliverables/router-recovery-20260919/LOG_DIAGNOSIS.md`. Sources were
`C:\Users\oudeb\.codex\codex-router\usage-events.jsonl` (last 150 rows, file
lines 14169-14318) and focused windows of
`C:\Users\oudeb\.codex\codex-router\router.log` (last 400 lines, plus lines
10388-10400 and 13533-13645). No prompts, keys, or session transcripts were
read, and no raw log content is copied here.

**Disposition:** Closed as a failed attempt at the diagnosis level, not as a
product finding. Containment: game writes stay paused and any resumed work uses
fresh bounded workers with small scopes instead of extending the two oversized
sessions. No product code, router configuration, or game state changed.

**Reproduction status:** Not reproduced. The client-visible symptoms were
reported by the worker owners; router metadata alone cannot regenerate them.

**Follow-up state:** Open: if the pattern repeats in a fresh bounded worker,
capture the client-side transcript for that turn so a content-level conclusion
becomes possible. Do not treat context size or compaction as the established
cause.

## 2026-09-20: deadline-fixed PC v2.02 noop was accepted once, then cleanup failed and the game faulted

**Objective:** Determine whether the corrected dispatch window ("deadline
fixed") reaches the accepted PC v2.02 dispatch entry for one read-only noop -
redirect once and return, with no builder call, no insertion, no serial
allocation and no save write.

**Observed symptom:** The redirect was accepted and the cleanup then failed,
leaving the worker's own state uncertain; after the capture the game raised a
fatal Windows error dialog. Attempt `v202-noop-39932` ran on PID 18956
(creation filetime `134343176877073549`, `Nioh3.exe`, module
`0x7FF69A840000` / `0x52FB000`, image SHA-256
`E22C4A635E4EC1E27A177B76E27D7F6A637F426C0ED3928B60F5693BC52AE130`) - the
same process instance as the earlier title-screen idle miss. The dispatch leg
succeeded in its own terms: `entry_hits 1`, `entry_hits_accepted 1`,
`acknowledgement_hits 1`, `redirect_count 1`, `stop_reason "accepted"`,
`elapsed_ms 49`, with `rounds 491`, `threads_armed 340`, `create_thread 339`,
`load_dll 147`, `exit_thread 1` and zero `entry_ownership_rejects`. Cleanup then
reported `GetThreadContext(0) failed with error 6`, so the receipt settled
`phase: "uncertain"`, `released: false`, `breakpoint_count: -1` and a retained
`allocation` (`2385526849536`); the run report records `dispatch_failed: true`
with `failure: "Native dispatch result is uncertain; allocation retained, do not
retry"`. Inventory read before and after was identical (`entries 43`,
`index_nodes 45`, `serial 2500806`, container
`07db828286787e7edc1c5deac85302d668993851a1427fe532e01a9f430b09db`) and the
debugger detached, but that equality is bounded evidence about visible
inventory only and is not a safety proof for the redirected path. After the
capture the user's game displayed a fatal error dialog `0xC0000005` at
`0x00007FF69AA272D6`; the arithmetic RVA `0x1E72D6` is a subtraction against the
recorded module base, not a causal attribution.

**Root cause:** unknown. The cleanup failure (`GetThreadContext(0)` error 6), the
retained allocation and the later access violation are separately unexplained,
and no evidence joins them: whether the retained allocation or armed-thread
state contributed to the fault is not established, and the reverse (that the
fault caused the cleanup failure) is equally unproven.

**Evidence:** `deliverables/router-recovery-20260919/native-noop-deadline-fixed/evidence/`
- `noop-report.json` (schema `nioh3-live-add-v202-noop/v1`), the raw
`v202-noop-39932.receipt.json`, `noop-stdout.txt`, `preflight-before.txt`,
`preflight-after.txt`, `fault-last-state.json` (`ObservedDialogCode 0xC0000005`,
`ObservedDialogAddress 0x00007FF69AA272D6`, `ArithmeticRva 0x1E72D6`) and the
saved fatal-error dialog capture `fatal-error-dialog.png`. That directory holds
no top-level report; the attempt summary reached this ledger through the
producer's final report message. The raw receipt was preserved without edits.

**Disposition:** Closed as a failed attempt, not as a mechanism or regression
finding. No success claim: the single accepted redirect is not a completed noop,
and no Pro or package claim follows. Live acceptance stays blocked, a retry is
not allowed, and the declared scope contains no builder, insertion, serial
allocation or save write (`serial` and `slot` stay null).

**Reproduction status:** Not reproduced. One attempt, one accepted redirect, one
cleanup failure and one later fault dialog; none of the three is repeated or
independently explained here.

**Follow-up state:** Blocked on an owner decision, not on more probing: the
worker left a retained allocation with `released: false`, so the session is
uncertain and must not be retried. The game process was still the user's after
the capture, and the root asked the owner to exit it. Any further work needs a
fresh game process and its own bounded ticket.

**Skill promotion:** None. The cleanup and fault shapes are single occurrences
with unknown causality.

## 2026-09-20: T5b context-identity worker degenerated into a repetition loop and delivered nothing

**Objective:** Produce `deliverables/rust-partial-repair-20260920/T5B_CONTEXT_IDENTITY.md` for the
context-identity layer of the partial-repair lane, with schema proof fields applied through real tool calls.

**Observed symptom:** An OpenCode Go DeepSeek V4.1 Flash worker repeatedly stated that it would apply the schema
proof fields and then re-emitted the same intent text without acting. The turn produced no tool call, no report
file, and no implementation that can be attributed to that worker. The root verified that the claimed artifact did
not exist (`Test-Path` on
`deliverables\rust-partial-repair-20260920\T5B_CONTEXT_IDENTITY.md` returned `False`; only the pre-existing
directory `deliverables\rust-partial-repair-20260920\` existed). The raw repeated text is not copied here.

**Root cause:** Model/tool-call degeneration in the routed worker turn. This is a runtime behavior failure, not a
repository, architecture, or mechanism finding, and it carries no information about the context-identity design.

**Impact:** Zero accepted progress. No result from that worker is trusted, and nothing in the lane may be cited as
having been produced by it.

**Evidence:** Root-side artifact check of
`deliverables/rust-partial-repair-20260920/T5B_CONTEXT_IDENTITY.md` (absent) and `deliverables/rust-partial-repair-20260920/`
(directory present, no T5b report); root-observed turn behavior. No client-side content capture was retained, so the
degeneration trigger is not identified beyond the visible repetition, and context size is not established as the
cause.

**Disposition:** Closed as a failed worker attempt, not as an architectural finding. Containment: the worker was
stopped, no work from it was accepted or merged, and existing dirty work was preserved untouched. The bounded
evidence boundary is root-side absence checks only; no provider response content and no reproduction exist.

**Reproduction status:** Not reproduced. Observed once in one routed worker turn; no independent rerun was performed
because the task is being reissued in a smaller shape.

**Follow-up state:** Open as reissued work: split T5b into smaller single-layer, single-file contract tickets instead
of one context-identity ticket, each requiring one observable outcome. Prevention for the reissued tickets: require an
explicit success artifact or report path (and its existence check), and reject empty or repetitive finals as a failed
turn instead of treating them as a report.

**Skill promotion:** None. This is a one-off routed-worker degeneration; revisit only if the same empty repetitive
final recurs across independent tickets.

## 2026-09-20: T5b migration-harness worker narrated a PowerShell parameter-binding observation instead of running the tool

**Objective:** Produce the T5b migration-harness repair (preview/search worker identity and argv gates) with executed
test evidence, as one bounded worker ticket.

**Observed symptom:** The OpenCode Go DeepSeek V4.1 Flash worker re-emitted the same PowerShell parameter-binding
observation as prose instead of invoking `tools/run_python_tests.ps1`. The turn produced no tool call, no test run and
no accepted report; the root interrupted it before any final was taken as a result. The repeated text is not copied
here.

**Cause / evidence boundary:** Model/tool-call degeneration in the routed turn. The evidence is the root-observed
turn shape plus the absence of an accepted report; no client-side transcript was retained, so context size,
transport and quota are all unestablished as causes. This says nothing about the harness or its parameter block.

**Impact:** Zero accepted progress for that turn; no result from it is trusted.

**Evidence:** No report exists from that turn. The reissued work landed as
`deliverables/rust-partial-repair-20260920/T5B_MIGRATION_CORE_PARITY.md` (45 passed, plus a 5-test targeted rerun) and
`deliverables/rust-partial-repair-20260920/T5B_APPLICATION_BOOTSTRAP_HARNESS.md` (8, 6 and 10 passed).

**Containment / recovery:** The degenerate turn was interrupted and the ticket was reissued as a fresh DeepSeek API
ticket split across two files. Both landed green on the repository test wrapper, and the parameter-binding detail the
first worker only narrated is exercised explicitly in the reissued form (`-TestPath` passed by name).

**Reproduction status:** Not reproduced. Observed once in one routed worker turn.

**Follow-up state:** Open as process guidance: keep the two-file split, and require an executed command line with its
pass/fail output as the acceptance artifact instead of prose about a command.

**Skill promotion:** None. Revisit only if the same narrated-without-executing shape recurs across independent
tickets.

## 2026-09-20: T5b Tauri worker wrote partial code, then looped on final-verification narration

**Objective:** Implement the T5b Tauri game-file-version plumbing (bounded Steam discovery, `--game-file-version`
argv identity, fail-closed behaviour) and verify it.

**Observed symptom:** The OpenCode Go worker did write partial implementation to disk, then entered repetitive
"run final verification" narration without running the verification. No usable final or verification result came
from that turn, so the root rejected the final.

**Cause / evidence boundary:** Model/tool-call degeneration in the routed turn, not an architecture finding; the
degeneration carries no information about the Tauri version-plumbing design. The retained on-disk edits were judged
separately, on their own evidence, not on the worker's narration.

**Impact:** Partial implementation only. Nothing that worker claimed about verification is trusted.

**Evidence:** `deliverables/rust-partial-repair-20260920/T5B_TAURI_VERSION_PLUMBING.md` (implemented and verified as
bounded evidence by the finisher, with `CARGO_TARGET_DIR=D:\nioh3-t5b-tauri-finish\target`) and the retained edits
under `apps/tauri/src-tauri/src/` (`game_version.rs`, `worker.rs`, `tests.rs`, `main.rs`).

**Containment / recovery:** The root rejected the repetitive final, kept the disk changes, and delegated verification
and reporting to a fresh independent finisher, who completed both. No peer file in that worktree was reverted.

**Reproduction status:** Not reproduced. Observed once in one routed worker turn.

**Follow-up state:** Open as process guidance: when a routed worker narrates a verification step without a tool call,
interrupt and hand the residual to a fresh finisher ticket that owns the executed command output.

**Skill promotion:** None.

## 2026-09-20: R3 worker stashed the shared worktree, hiding peer work during a baseline check

**Objective:** Check the R3 baseline in the shared `codex/v080-rust-backend` worktree.

**Observed symptom:** The R3 worker ran a worktree-wide `git stash` while checking its baseline, which removed other
owners' uncommitted work from the working tree, and then entered a repetition loop. The retained stash entry is
`stash@{0}: On codex/v080-rust-backend: r3-baseline-check`.

**Cause / evidence boundary:** An unsafe worktree-wide state change in a shared dirty tree, followed by
model/tool-call degeneration. `git stash show --name-only stash@{0}` lists nine paths: two under
`apps/tauri/src-tauri/src/` and seven under `crates/nioh3-protected/` (the seven restored), including peer T2b and
context work. `crates/nioh3-protected/src/jobs.rs` is not in the stash and is a separate concurrent change. No
report file records the incident, so the stash is the on-disk evidence and the recovery actions are root-observed.

**Impact:** Peer T2b and context work temporarily disappeared from the shared working tree. Nothing committed was
lost and no current change was overwritten.

**Evidence:** `git stash list` and `git stash show --name-only stash@{0}` for the nine captured paths; the retained
stash `r3-baseline-check` itself.

**Containment / recovery:** The root inspected the stash read-only, confirmed the seven exact target paths' worktree
state and that `jobs.rs` was separate, then restored only those seven paths with
`git restore --source=stash@{0} --worktree -- <exact paths>`. The stash was kept as recovery evidence and the
combined acceptance was delegated rather than run from the damaged turn.

**Reproduction status:** Not reproduced. Observed once in one routed worker turn.

**Follow-up state:** Open as a rule now recorded in the root `AGENTS.md` Boundaries: workers in the shared worktree
must not run `git stash`, `git clean`, reset, or checkout/restore another owner's paths, or make other worktree-wide
state changes; isolation should come from a managed worktree.

**Skill promotion:** None.

## 2026-09-20: OpenCode Go DeepSeek workers in the Pro r3 repairs ended turns without acceptance evidence

**Objective:** Execute the Pro r3 repairs lane - the save-core repair and the Python R5 repair - on routed OpenCode Go
DeepSeek V4.1 Flash workers, with each ticket requiring one observable outcome, executed checks, and a report.

**Observed symptoms:** (1) The save-core worker twice ended its turn with a summary while the lane still carried red
tests or an unmet gate, so neither turn delivered the required green evidence. (2) The Python R5 worker repeatedly
re-emitted the same sentence about preparing to add tests, added nothing, and finished no acceptance run and no
report. No result from either worker is accepted, and the repeated text is not copied here.

**Root cause:** unknown. The observable shape is a routed worker turn ending in text - a summary, or a repeated intent
sentence - instead of the required tool work and its output. No client-side transcript, provider response content, or
reproduction was retained, so context size, transport, and quota all stay unestablished as causes. This entry says
nothing about the save-core or R5 implementations.

**Impact:** Zero accepted progress from those turns, and the red tests and unmet gate stayed open until the tasks were
stopped, so the lane's dependency work had to be reassigned. Nothing unverified was merged, and no existing dirty work
was reverted.

**Evidence:** Root-observed turn shapes in this lane plus the absence of the expected green runs, acceptance output,
and reports for those tickets. Evidence is bounded to those root-side observations; no artifact path, raw log, or
provider response from the failed turns is cited here.

**Immediate correction:** The root stopped both worker tasks and reassigned the two repair lanes to the official
DeepSeek V4.1 Flash worker (`deepseek/deepseek-v4.1-flash`) on fresh bounded tickets. That reassignment is recorded as
the containment action only; this entry makes no claim about the outcome of the replacement work.

**Owner assessment and decision (owner judgment, not an independent measurement by this team):** The owner decided
that OpenCode Go DeepSeek is no longer to be used, on the owner's assessment that it is a quantized, degraded model.
That reason is the owner's assessment and decision, not a measurement this team performed: no benchmark, scoring run,
or model comparison was carried out here, and the recorded failure facts above stand on their own.

**Reusable diagnostic signals:** A final that summarizes intent instead of reporting executed checks; the same intent
sentence repeated across turns with no tool call; a worker ending its turn with a known red test or missing gate still
open; an acceptance claim with no report path that can be checked. Any of these means the turn failed regardless of
the text, so stop the turn, do not accept the result, and reissue a smaller ticket.

**Disposition:** Closed as failed worker attempts in this lane, not as a finding about the r3 repair design, the
product, or the routed model's capability. These symptoms stay evidence, not rules; no product code, document, or
skill was changed for them.

**Reproduction status:** Not reproduced. Two workers and three degenerate turns were observed once each inside one
lane, and no independent rerun was attempted because the tickets were reissued in a smaller shape.

**Follow-up state:** Open as process guidance for reissued tickets: require an explicit success artifact or an
executed command with its visible pass/fail output as the acceptance evidence, and treat a red test or unmet gate as
an unaccepted turn.

**Skill promotion:** None. Keep this a ledger record and reissue smaller tickets instead of promoting symptoms into
rules.

## 2026-09-20: r4 final closure gates exhausted the D: build volume

**Objective:** Re-run the RF02-RF05 closure gates on one final working tree before assembling the next Pro review
package.

**Observed symptom:** The protected-host subset reported six failures while creating synthetic save/resource
fixtures, the packaged-host resolver reported seven setup errors while linking the release host, and the save fault
matrix began reporting failures. Every captured failure was an operating-system write failure: Windows error 112 /
`Errno 28` (`No space left on device`), including linker `LNK1201` while writing a PDB. The tests did not reach the
assertions whose product behavior they were intended to verify.

**Root cause:** Confirmed environmental capacity exhaustion. At the failure boundary, `D:` had approximately 0.10 GB
free. Duplicate, reconstructible Cargo targets under `D:\Nioh3_v080_deliverables\build-cache` occupied approximately
7.8 GB beyond the retained shared `python-tests` target; failed and historical test temporary trees occupied
additional space.

**Evidence:** Failed run roots
`D:\Nioh3_v080_deliverables\tmp\pytest-26332-e8a9426e`,
`D:\Nioh3_v080_deliverables\tmp\pytest-34264-2573b358`, and
`D:\Nioh3_v080_deliverables\tmp\pytest-44432-8a3a80e6`; the failing test entry points are
`tests/migration/test_protected_save_acceptance.py`,
`tests/migration/test_packaged_host_resolver.py`, and
`tests/migration/test_save_transaction_parity.py`.

**Disposition:** The runs are invalid as product evidence. They are capacity failures, not red behavioral verdicts.
The root stopped the remaining runs and used `cargo clean --target-dir` on six explicitly named duplicate Cargo
targets plus the obsolete `m3-save-acceptance/cargo-target`, recovering approximately 7.5 GB while preserving the
shared project target, source tree, research packages, and delivery artifacts.

**Reproduction status:** Reproduced concurrently across Python fixture writes, Rust metadata writes, and the MSVC
linker on the same full volume. No reproduction is needed after capacity recovery.

**Follow-up state:** Closed. The affected gates were re-run sequentially against the retained shared target: the
save-transaction matrix passed 54 tests, the RF02/RF05 closure subset passed 24 tests, and the packaged-host resolver
passed all seven cases. The final report keeps the capacity failure distinct from those behavioral results.

**Skill promotion:** No new skill. `tools/run_python_tests.ps1` now fails before a run when its build volume has less
than the configured free-space floor, exposes project-script execution through the same prepared environment, and
`AGENTS.md` requires routine gates to reuse the shared Cargo target. Disposable isolated targets remain explicit and
are cleaned with `cargo clean --target-dir`; there is no unbounded automatic deletion policy.

## 2026-09-20: first r4 restore-evidence export used an invalid short main-save fixture

**Objective:** Preserve raw restart evidence for the four journal update crash cuts and one A/B/C mixed-target case.

**Observed symptom:** The first export stopped during `prepare` with `encrypted save has 0x15 bytes, which is not a
shipped container size`. No commit or target replacement ran.

**Root cause:** Confirmed exporter fixture defect. The first implementation used short label bytes for the main-save
role, while the real restore planner correctly requires a valid encrypted container. The already-green parity matrix
builds that role through the shipped transform and was not affected.

**Evidence:** The root-observed first-run stderr carried the exact size refusal; the failed synthetic staging was
removed after the corrected compact export passed. The retained implementation and successful evidence are
`tools/export_r4_closure_evidence.py` and `deliverables/r4-closure-final/restore-crash-evidence/`.

**Disposition:** Closed. The exporter now creates three distinct full fixture containers through
`native_transform_short`, retains only raw receipt/journal/checkpoint JSON plus role hashes, and removes its bulky
synthetic work tree after success. The corrected run produced five cases, all crash exits were 9, all receipts
classified `unknown`, and the A/B/C case classified System=A, GameBackup=B, Main=external-C.

**Reproduction status:** Reproduced once by the first exporter run and not reproduced after the fixture correction.

**Follow-up state:** Closed; compact evidence is under `deliverables/r4-closure-final/restore-crash-evidence/`.

**Skill promotion:** None. This was a one-off fixture construction bug, not a reusable workflow decision.

## 2026-09-20: live runtime helper tests raced their Win32 lifecycles

**Objective:** Run the final `nioh3-runtime` all-features test gate after the RF03 repair.

**Observed symptom:** All 136 library tests passed, then one of five `native_helper_api` cases failed while reading the
disposable helper's stdout with Windows error 6 (`The handle is invalid`). The same test passed immediately when run
alone, and the five-case binary passed on its next unconstrained run.

**Root cause:** The integration binary allowed four real Win32 helper/debugger/remote-thread lifecycles to execute in
parallel without declaring that shared OS-test boundary. The exact scheduler interleaving that invalidated the pipe
handle was not retained, so the lower-level Windows causal sequence remains unclaimed.

**Evidence:** `crates/nioh3-runtime/tests/native_helper_api.rs`; the initial all-features gate; the isolated rerun;
three repeated five-case runs; and the final all-features gate.

**Disposition:** Closed as test-harness reliability, not a product-runtime failure. The four live helper cases now
share a poison-tolerant process-local mutex. Three consecutive five-case reruns passed, followed by the complete
all-features gate with 148 tests passed across the library and integration binaries.

**Reproduction status:** Observed once in the parallel gate; the failing node was green alone before the harness fix.

**Follow-up state:** Closed. Clippy and rustfmt are also green for the runtime crate.

**Skill promotion:** None. The correction is executable in the owning integration test rather than prose guidance.

**Follow-up (2026-09-21, preview disposable-helper acceptance).** A new bounded
acceptance - real Windows debug session over the owned disposable helper, no game
- first stopped inside the accepted preview window with `entry_hits_accepted` 1
and `redirect_count` 0. Root corrected the attribution: the helper fixture had
collapsed the product's two pointer hops, so the manager slot named the data
object directly and the second hop read the acquisition counter (11) as the data
address; the container then resolved to `0x224A6B` and the capacity cell to
`0x23B4EB`, both `ReadProcessMemory` error 299. Fixed in the helper with a
separate manager object holding the data address. Two product defects were
separated from that fixture bug and repaired independently by the runtime owner:
cleanup refused `DebugBreakProcess` while a debug event was still pending (the
genuine negative-path defect; the primary error is now recorded beside
`cleanup_error`), and the preview fingerprint derived its digest from container
bytes instead of the real native serial index, so the proof now reads the
shipped `capture_index` traversal under the stopped owner and records
`native_index_digest`, `index_node_count` and `index_bucket_count`. Result: four
real-OS acceptance cases pass (matched; mismatch to `rejected_after_preview`
with unchanged container and index digests, terminal cleanup and zero builder
reruns on recovery; no-ack stays unknown) and the five existing helper cases
still pass. No game was attached and no save was touched.

## 2026-09-20: r5 archive initially carried a nested checksum manifest

**Objective:** Validate the self-contained r5 Pro review directory and ZIP with both repository and package-local
verifiers.

**Observed symptom:** The first repository validation failed with one ZIP-only extra member:
`evidence/prior-review-r4/SHA256SUMS.txt`.

**Root cause:** Confirmed package-assembly mismatch. The imported r4 reviewer return carried its own checksum file,
while both generic handoff validators intentionally reserve the name `SHA256SUMS.txt` for the current package's root
manifest and exclude nested files with that name from their directory inventory.

**Evidence:** First validator result and `deliverables/Nioh3_v080_RepairWave_r5_Final_Closure_Review_20260920/assemble_package.py`.

**Disposition:** Closed. The prior review's checksum bytes are retained as
`evidence/prior-review-r4/ORIGINAL_SHA256SUMS.txt`; the r5 root manifest and ZIP were rebuilt. Both validators then
verified 155 files, 154 hashes, archive membership, CRC, and byte identity.

**Reproduction status:** Reproduced once by the first archive and absent from the rebuilt archive.

**Follow-up state:** Closed; final ZIP SHA-256 is
`1da548d9d8aa1878e9f4d3615596fe0638ddd19754f1f0d435ab4e71d6d0a631`.

**Skill promotion:** None. The package-local assembler now renames an imported checksum manifest before finalization.

## 2026-09-20: r5 evidence-link package repeated the nested checksum collision

**Objective:** Retain the final seven-node debug-host identity link in a
self-contained, validator-clean Pro handoff directory and ZIP.

**Observed symptom:** The seven acceptance nodes passed, the package directory
validated, but ZIP validation reported one extra member:
`evidence/pro-r5-review/SHA256SUMS.txt`.

**Root cause:** Confirmed exporter assembly defect. The evidence-link exporter
copied the imported r5 review's root checksum file under its reserved original
name. This repeated the already documented r5-package collision above; it did
not affect any build, binary identity, verifier result, or test outcome.

**Evidence:** The failed validator output; the preserved first archive at
`deliverables/Nioh3_v080_R5_Evidence_Link_20260920_prevalidation-invalid.zip`;
and the final package's `evidence/seven-node-results.json` and
`evidence/link-validation.json`.

**Disposition:** Closed. `tools/export_r5_evidence_link.py` no longer imports a
nested file named `SHA256SUMS.txt`. The redundant copied manifest was removed
from the package, the ZIP was rebuilt, and the repository validator verified 94
files, 93 root-manifest hashes, and every archived byte.

**Reproduction status:** Reproduced once in the initial evidence-link archive
and absent from the rebuilt archive.

**Follow-up state:** Closed. The validated ZIP is 241,340 bytes with SHA-256
`714e7194b39053d6c9d64106ae64636ee530e6493441ab9de764f954b608958a`.

**Skill promotion:** None. The correction lives in the owning deterministic
exporter; the symptom remains a packaging failure record, not an active product
rule.

## 2026-09-20: local v0.8.0 RC clean-checkout Python gate failed on a missing ignored test oracle

**Objective:** Run the full Python gate for the local v0.8.0 RC candidate
`ade0dd205b2ca7c00ead2c00a25a6153e43d5029` from a clean worktree at
`D:\Nioh3_v080_deliverables\source-ade0dd2-local-rc`.

**Observed symptom:** The gate ran through the project runner
(`tools/run_python_tests.ps1`) with an explicit project Python over `tests` and
pytest `-x -vv`. It stopped on the first failure,
`tests/migration/test_application_worker_parity.py::ApplicationWorkerParityTests::test_search_catalog_matches_the_frozen_zh_reference`,
with `FileNotFoundError` for `deliverables/m23c-application/catalog_reference_zh.json`.
That path is git-ignored, so it is absent from a clean checkout even though it
exists in populated developer worktrees.

**Root cause:** The test oracle depends on a build artifact that a clean
checkout does not contain. This is a fixture-hermeticity defect, not a
product-code failure; no product behavior has failed.

**Evidence:** Clean worktree `D:\Nioh3_v080_deliverables\source-ade0dd2-local-rc`
at candidate commit `ade0dd205b2ca7c00ead2c00a25a6153e43d5029`; the failing node
id and its `FileNotFoundError` for the ignored
`deliverables/m23c-application/catalog_reference_zh.json`.

**Disposition:** Open. A bounded fixture-hermeticity repair is assigned; the
failure is a test-oracle dependency on an ignored artifact, so no backend lane
is reopened.

**Reproduction status:** Reproduced in the clean checkout. `-x` stopped the run
at this first failure, so the remaining tests were not exercised.

**Follow-up state:** Open. Targeted repair of the oracle fixture, then a rerun
of the same gate; no backend reopening.

**Skill promotion:** None. This is a bounded test-hermeticity failure record.

## 2026-09-20: clean v0.8.0 RC candidate TypeScript gate failed on stale test fixtures

**Objective:** Typecheck the clean v0.8.0 RC candidate `ade0dd2` at
`D:\Nioh3_v080_deliverables\source-ade0dd2-local-rc` before running its tests.

**Observed symptom:** The gate failed before any test executed. Running the
direct project dependency `tsc.cmd --noEmit` reported `TS2740` in
`apps/desktop/tests/controller.test.ts` (near line 9) and
`apps/desktop/tests/search-policy.test.ts` (near line 21): the test context
fixtures omit required resolved-generation fields (`game_file_version`,
`versioned_resource_dir`, `bundle_digest`, `versioned_digest`,
`legacy_context_digest`, `production_authority`).

**Root cause:** The two test fixtures are stale relative to the expanded
resolved-generation contract. Production code is not implicated at this point;
only the fixture shape failed the typecheck.

**Evidence:** Clean worktree `D:\Nioh3_v080_deliverables\source-ade0dd2-local-rc`
at candidate `ade0dd2`; the `tsc.cmd --noEmit` run and its two `TS2740` reports
against `apps/desktop/tests/controller.test.ts` and
`apps/desktop/tests/search-policy.test.ts`.

**Disposition:** Open. A bounded two-file fixture repair is delegated, after
which the typecheck and the affected tests are rerun. The contract is not
weakened to make the fixtures pass.

**Reproduction status:** Reproduced in the clean checkout; the gate stopped at
the typecheck, so no test ran.

**Follow-up state:** Open. Repair the two fixtures, rerun `tsc.cmd --noEmit`,
then rerun the affected desktop tests.

**Skill promotion:** None. This is a bounded stale-fixture failure record.

## 2026-09-20: closure - repository catalog oracle promoted to a tracked fixture

**Objective:** Close the recorded failure cause from the earlier entry
"local v0.8.0 RC clean-checkout Python gate failed on a missing ignored test
oracle".

**Closure evidence:** The ignored oracle was promoted byte-exactly to
`tests/fixtures/m23c-application/catalog_reference_zh.json` (172,037 bytes,
SHA-256 `8E81756E6DC36E79C025D203CAF44BABE48EC254D3F2E40353F78ED3230484C5`),
and its test now points there. The combined targeted rerun over
`test_application_worker_parity.py`, `test_live_add_identity.py` and
`test_game_version_v202_resources.py` passed 22/22 in 47.62s through
`tools/run_python_tests.ps1`.

**Disposition:** Closed as the recorded fixture-hermeticity failure cause. The
clean-checkout full RC gate still remains to run as the normal release gate;
this entry does not claim the full RC is accepted.

**Follow-up state:** Closed for this failure cause; full RC gate pending.

**Skill promotion:** None.

## 2026-09-20: closure - Handshake test fixtures gained the resolved-context fields

**Objective:** Close the recorded failure cause from the earlier entry
"clean v0.8.0 RC candidate TypeScript gate failed on stale test fixtures".

**Closure evidence:** The two Handshake fixtures gained the six required
resolved-context fields without weakening the contract. The direct project
`tsc --noEmit` passed, and the two targeted TypeScript test files passed 6/6.

**Disposition:** Closed as the recorded stale-fixture failure cause. The
clean-checkout full RC gate still remains to run as the normal release gate;
this entry does not claim the full RC is accepted.

**Follow-up state:** Closed for this failure cause; full RC gate pending.

**Skill promotion:** None.

## 2026-09-20: clean v0.8.0 RC candidate Node gate failed on stale source-worker call sites

**Objective:** Run the desktop Node gate for clean v0.8.0 RC candidate
`28103fff5124d06acb2bc279853ea660e36e6deb` at
`D:\Nioh3_v080_deliverables\source-28103ff-local-rc` with `NIOH3_PYTHON`
correctly supplied.

**Observed symptom:** The direct project `tsc` run passed, but the full
`apps/desktop/tests/*.test.ts` run returned 67 passed, 5 failed, 1 skipped out
of 73. The five failures were recommended-level, save-workflow, the two
worker-client-call-shape source tests, and worker IPC; every one reported
`WORKER_EXITED: 2`.

**Root cause:** The TypeScript source-worker call sites and API are stale after
the Python worker adopted fail-closed explicit identity via
`--game-file-version` or `--legacy-test-context`. The earlier run without
`NIOH3_PYTHON` was only an invocation setup error and is not the recorded
product or test failure.

**Evidence:** Clean worktree `D:\Nioh3_v080_deliverables\source-28103ff-local-rc`
at candidate `28103fff5124d06acb2bc279853ea660e36e6deb`; the passing direct
`tsc` run; and the full `apps/desktop/tests/*.test.ts` totals with the five
`WORKER_EXITED: 2` failures listed above.

**Disposition:** Open. A bounded 7-file fix is delegated: preserve fail-closed
semantics, use explicit legacy opt-in for source tests and local dev, bind the
source to the packaged handshake `game_file_version` in packaged parity, and
leave packaged argv behavior unchanged.

**Reproduction status:** Reproduced in the clean candidate with `NIOH3_PYTHON`
correctly set; the direct project `tsc` passed in the same tree.

**Follow-up state:** Open. The full clean RC must be refrozen and retested after
the fix.

**Skill promotion:** None.

## 2026-09-20: closure - source-worker call sites and identity injection repaired

**Objective:** Close the recorded failure cause from this entry, "clean v0.8.0
RC candidate Node gate failed on stale source-worker call sites".

**Closure evidence:** The fix expanded beyond the initial 7-file estimate to
include the workflow and runbook after independent review found the packaged
parity path still lacked staged Rust worker identity injection. `WorkerClient` now
forwards explicit source argv; only exact source argv
`['--legacy-test-context']` may validate the non-production legacy response,
while all production and source `--game-file-version` and packaged responses
remain strict. Packaged parity requires a four-part
`NIOH3_PARITY_GAME_FILE_VERSION`, injects it into staged argv and source argv,
and asserts `production_authority` true.

**Verification on the working tree:** The bundled Codex node `tsc --noEmit`
exited 0. The full `apps/desktop/tests/*.test.ts` run with explicit project
Python and version yielded 73 total, 72 passed, 0 failed, and 1 env-gated
packaged skip.

**Disposition:** Closed as the recorded failure cause. The actual current
staged packaged parity remains pending a final rebuilt artifact, so the full
clean RC and the rebuilt packaged parity are still pending and are not claimed
accepted here.

**Follow-up state:** Closed for this failure cause; full clean RC and rebuilt
packaged parity pending.

**Skill promotion:** None.

## 2026-09-20: local-RC cleanup probe crossed the exact allowlist and deleted a live Git admin directory

**Objective:** During local-RC source prep, recover D: space by removing exactly
four obsolete registered detached worktrees
(`D:\Nioh3_v080_deliverables\source-ade0dd2-local-rc`,
`source-28103ff-local-rc`, `source-5dc3bff-local-rc`,
`source-1b94181-local-rc`) with `git worktree remove --force`, plus the verified
temporary Cargo target `D:\Nioh3_v080_deliverables\build-cache\save-parity`,
while leaving the F: main checkout, `build-cache\tauri-target` and the live
`D:\Nioh3_v080_deliverables\v080-candidate-source` candidate worktree intact.

**Observed symptom:** After the four removals, the deleted worktrees' Git admin
directories under `F:\Nioh3_ScrollEditor\.git\worktrees\` were still visible by
name while every read failed with access denied. A diagnostic loop meant to
compare one of those names with one live entry also carried the recursive delete
call, so it ran `[System.IO.Directory]::Delete(<admin dir>, $true)` against
`F:\Nioh3_ScrollEditor\.git\worktrees\v080-candidate-source`, the admin
directory of the live `v080-candidate-source` worktree, which was not on the
removal list. That name then held in the Windows delete-pending state and
`git worktree list` no longer registered the worktree.

**Root cause:** Operator error, not a product or tooling defect. The probe loop
enumerated one path outside the prevalidated exact allowlist and executed a
recursive delete on each entry instead of only inspecting it. A misread symptom
invited the wider probe: while the Codex host process holds directory handles on
the workspace, any delete under `.git\worktrees` reports as Windows
delete-pending (name visible, access denied, ACL unreadable), which reads like a
stale ACL or ownership problem rather than a completed deletion.

**Impact:** No tracked content and no working-tree file changed.
`D:\Nioh3_v080_deliverables\v080-candidate-source` and every file in it stayed
intact. Lost were that worktree's Git admin metadata: `HEAD`, `index`, `logs/`,
`ORIG_HEAD` and `FETCH_HEAD` for the detached commit
`28fe2500ec06a308fd545a131c78140f4961b04e`.

**Evidence:** `F:\Nioh3_ScrollEditor\.git\worktrees\v080-candidate-source`
(deleted, now a delete-pending name) and
`F:\Nioh3_ScrollEditor\.git\worktrees\v080-candidate-source-recovered`
(rebuilt `HEAD` = `28fe2500ec06a308fd545a131c78140f4961b04e`); the unchanged
worktree `D:\Nioh3_v080_deliverables\v080-candidate-source`; and the cleanup
report to `/root` for the same session. The incident exists only as filesystem
and Git metadata state, so this ledger entry is its repository record.

**Repair:** Recreated the admin directory as
`F:\Nioh3_ScrollEditor\.git\worktrees\v080-candidate-source-recovered` with
`gitdir` pointing at the worktree's `.git` file, `commondir` = `../..` and
`HEAD` = `28fe2500ec06a308fd545a131c78140f4961b04e`; repointed
`D:\Nioh3_v080_deliverables\v080-candidate-source\.git` at that directory; and
rebuilt the index with `git -C <worktree> read-tree HEAD`.

**Verification:** `git worktree list --porcelain` registers
`D:/Nioh3_v080_deliverables/v080-candidate-source` again at detached
`HEAD 28fe2500ec06a308fd545a131c78140f4961b04e`, and
`git status --porcelain` in that worktree is empty, so it was clean and no
staged or unstaged work was lost. Only the reflog and the `ORIG_HEAD` /
`FETCH_HEAD` caches are unrecoverable. The assigned cleanup still completed on
the same pass: four obsolete worktrees (138.02 MiB), the `save-parity` target
(1429.48 MiB) and a new clean detached worktree at `ea50b19` were all verified.

**Prevention:** Never probe or delete `.git/worktrees` entries outside the
prevalidated exact list; the allowlist is per entry, never "everything that
looks stale". Remove worktrees only through `git worktree remove --force <exact
absolute path>` and never run a recursive delete against an admin directory.
Unlink a worktree's `node_modules` junction before removal so no shared
dependency tree can be traversed. Treat a delete-pending name (exists, access
denied, ACL unreadable) as already deleted and closed until the host process
restarts rather than probing it. Pending names dissolve on host restart; the
repaired admin directory keeps the non-standard id
`v080-candidate-source-recovered` until then.

**Reproduction status:** The delete-pending behaviour reproduced deterministically
with a fresh empty probe directory under the same parent (created, deleted, then
still visible and unreadable). The accidental deletion of the live admin
directory is a one-off operator error and is not reproducible as a product or
gate failure.

**Follow-up state:** Closed for the incident. The worktree is functional and the
RC/source state is unaffected. Optional cosmetic follow-up after the Codex host
restarts: rename the admin directory back to its original id and run
`git worktree repair`.

**Skill promotion:** None. One-off operator failure; recorded for the bounded
allowlist and delete-pending lessons only.

## 2026-09-20: packaging environment - `pnpm run` converted the shared `node_modules` during the v0.8.0 RC build

**Objective:** Build and verify the final local v0.8.0 Tauri 2 RC from the
isolated checkout `D:\Nioh3_v080_deliverables\source-03174b9-lf-rc` (detached
`03174b9201958ebbab65fc4ede4054d5b9dba675`) using the bundled/project dependency
environment, no `C:` writes and no `node_modules` copy; a temporary junction to
the shared `F:\Nioh3_ScrollEditor\node_modules` was allowed for resolution.

**Symptom:** The first build attempt stopped immediately because
`tools/build_tauri.ps1` runs `npm.cmd run typecheck` and this host provides no
`npm` CLI on `PATH`. To obtain a script runner, `pnpm run typecheck` was invoked
with the bundled pnpm. Instead of only running the script, pnpm began an install
over the npm-shaped shared tree (`Progress: resolved 101, reused 39, added 38`),
moved the direct dependencies into
`F:\Nioh3_ScrollEditor\node_modules\.ignored\<name>`, created
`node_modules\.pnpm\...`, and aborted with
`ERR_PNPM_EISDIR [symlinkAllModules] ... symlink '...\@tauri-apps+cli-win32-x64-msvc@2.11.4\...'`.
The top-level packages were then absent while no replacement symlinks existed.

**Root cause:** Unverified environment assumption inside a bounded packaging
ticket. The ticket required the existing dependency tree and no install, but did
not check which package managers exist before linking that tree. pnpm treats an
existing npm-installed `node_modules` as an import source and rewrites it
(move to `.ignored` plus a `.pnpm` virtual store) as part of its dependency-status
check, so `pnpm run <script>` can mutate a shared dependency cache even when the
script itself only reads. The shared tree was reachable through a junction from
the candidate checkout, so the mutation looked local while it affected the
user's main workspace.

**Impact:** Temporary damage to the shared dependency cache: ten direct-dependency
directories were relocated and their transitive packages duplicated into
`.pnpm`. No tracked file, product source, save, or artifact changed; no build
output was written to `C:`; the RC bytes were unaffected. pnpm also refreshed its
own cache directory under `C:\Users\oudeb\AppData\Local\pnpm`, which is a `C:`
cache write against the ticket's no-`C:`-writes intent.

**Evidence:** the incident and repair are recorded for the same candidate in
`D:\Nioh3_v080_deliverables\deliverables\v080-local-rc-03174b9\REPORT.md`; the
workaround shim is `D:\Nioh3_v080_deliverables\tmp\v080-03174b9-package\shim\npm.cmd`
with `npm-shim.js`; the restored tree is `F:\Nioh3_ScrollEditor\node_modules`
(only `.bin` and `.package-lock.json` hidden entries remain) and the successful
same-candidate gates are under the same RC directory. The first attempt's console
error and the pnpm `ERR_PNPM_EISDIR` text exist only in the session transcript:
that run's `logs\build-tauri.log` was overwritten by the successful rebuild, so
the ledger entry is its durable record.

**Repair:** Every entry of `node_modules\.ignored` was moved back to the tree
root, including the nested `@tauri-apps\{api,cli}` and
`@types\{node,react,react-dom}` packages; `.pnpm` and `.ignored` were deleted
with verified literal paths inside the `node_modules` root. A minimal
`npm run <script>` shim (outside the checkout, `cmd.exe` with
`node_modules\.bin` on `PATH`, exactly as npm executes a script) completed the
build, and the candidate's `node_modules` junction was removed after
verification.

**Verification:** `node -e require.resolve(...)` resolves every pinned
dependency (`typescript`, `esbuild`, `playwright`, `tsx`, `react`, `react-dom`,
`ajv`, `json-schema-to-typescript`, `@tauri-apps/api`); `tsc --noEmit` exits 0 in
the shared checkout and `tsx --version` reports `v4.23.13`. The frozen RC stayed
at `03174b9` with `git status --porcelain --untracked-files=all` empty before and
after, and every same-candidate package gate passed (packaged parity, WebView2
shell, add-layout, portable update, host package, three worker identities,
packaged frontend, one-file launch/update/rollback, native faults).

**Prevention:** Never invoke `pnpm` or `pnpm run` in this npm-lock/npm-shaped
shared dependency tree. Preflight package-manager availability before linking a
shared `node_modules`, and prefer the documented bundled `npm` shim or explicit
`node <entrypoint>` invocations for repository scripts. Treat dependency-cache
mutation as forbidden during RC builds: link a shared tree read-only, never let
the runner's own dependency-status check run against it, and remove the junction
before finishing.

**Reproduction status:** The missing `npm.cmd` is deterministic on this host. The
pnpm layout conversion reproduced once, partially, and was not retried; the
package-relocation step is pnpm's documented behaviour for an npm-shaped tree,
so a repeat is expected if pnpm is used again there.

**Follow-up state:** Closed. The shared tree is functional, the candidate
junction is removed, and the RC is unaffected and still frozen at `03174b9`.
No repository, runbook, or skill file was changed by this incident; the
prevention rules above are the candidate promotion if a later packaging ticket
needs them.

**Skill promotion:** None for skills. This is a packaging-environment mistake,
not a reusable workflow; if it recurs, add the package-manager preflight to the
release runbook instead of a skill.

## 2026-09-21: v2.02 candidate insertion refused before dispatch by an upper-case pinned executable hash

**Objective:** Execute one root-authorized research insertion of R4 seed
`226061463` on game PC v2.02 through the reviewed runner
(`runtime_read_probe.exe --live-add-candidate-insert`), to exercise the
candidate live-add binding with the real (non-fake) transport.

**Symptom:** The runner exited `0` but refused before dispatch:
`NATIVE_DISPATCH  Candidate live addition requires the pinned PC v2.02 executable`
with `dispatch_failed true`. Nothing was dispatched and nothing was written.

**Root cause:** A case-sensitive executable-hash comparison in the product
binding gate. `native_abi.rs:150-151` pins
`PC_V202_CANDIDATE_EXECUTABLE_SHA256` in upper case (`E22C4A63...130`), while
`NativeDebugTransport::executable_sha256` (`native_executor.rs:1427-1430`)
returns the lower-case `sha256_hex` output (`count.rs:910-918`), and
`require_accepted_binding` (`native_executor.rs:370-375`) compares the pair with
exact equality. With the real transport the comparison can never succeed
(`e22c4a63...` != `E22C4A63...`). The refusal happens inside
`LiveAddApplication::prepare` -> `executor.inspect()` -> `require_accepted_binding`,
so it precedes the checkpoint, the preview dispatch and any write-capable handle.
It was not seen earlier because the offline dry run builds
`FakeLiveAddTransport` with the same upper-case constant, the probe's own noop
path compares with `eq_ignore_ascii_case`, and the shipped PC v2.01 binding sets
`executable_sha256: None`.

**Impact:** Zero writes. Before/after inventory is identical (44 entries, serial
`2500807`, acquisition order `51151`, container `c61b6b93...`; `records_added 0`,
`serial_advanced false`, `container_changed false`), 46 index nodes on both
sides, no new operation directory or backup, no new receipt
(`unsettled_receipts 0`), `debugger_attached false`, the same game process
(pid `40936`) still responding, and the game save untouched. The save/live
agreement question stays open because the refusal precedes the save read.

**Evidence:** `D:\Nioh3_v080_deliverables\deliverables\v202-native-acceptance-20260921\INSERT_ATTEMPT_20260921-042809.md`,
with `insert-request-20260921-042809.json`, `insert-stdout-20260921-042809.txt`,
`insert-report-20260921-042809.json`,
`insert-inventory-{before,after}-20260921-042809.json`,
`go-post-insert-preflight.txt` and the earlier settled
`stale-receipt-v202-noop-39932.json` in the same directory.

**Repair:** Assigned as a runtime-crate change (normalize the comparison or the
returned digest); not implemented, reviewed, or accepted in this ticket, and the
two fix options remain the recorded candidates in the attempt file. The attempt
was not retried, re-armed or replayed.

Later status: the runtime-crate hash-case fix was accepted through its
regression, and the next authorized attempt cleared this gate (see the attempt-3
entry below).

**Verification:** The no-write state is verified by the attempt's before/after
preflight. The assigned repair has no accepted verification yet, so this entry
records the refusal, not a fix.

Later status: the fix is accepted via its regression, and attempt 3 progressed
past the executable-identity gate and past the save/live agreement gate that had
also been blocked.

**Prevention:** None promoted. A single casing mismatch does not justify a rule;
if the same exact-equality-against-a-pinned-digest pattern blocks another
candidate run, treat it as recurring before writing any guidance.

**Reproduction status:** Not reproduced. The authorized attempt is a single run
under the standing no-replay rule; the mismatch is deterministic from the named
constants, so the same comparison would refuse again until the assigned repair
lands.

**Follow-up state:** Closed for this gate. The runtime-crate hash-case fix was
accepted through its regression, and the next authorized attempt (attempt 3,
2026-09-21) progressed past the executable-identity gate and past the save/live
agreement gate that had also been blocked. The candidate insertion itself
remains open: attempt 3 stopped at the preview builder-output review gate and
left an unsettled preview receipt (see the entry below).

**Skill promotion:** None. Recorded for the casing-mismatch and fake-transport
blind-spot lessons only; no skill, runbook, or product rule was changed.

## 2026-09-21: v2.02 candidate insertion attempt 3 stopped at the preview builder-output review gate

**Objective:** Run the authorized R4 seed `226061463` candidate insertion on game
PC v2.02 (after the owner's normal in-game save) now that the
executable-identity case fix and the save/live agreement gate were cleared.

**Symptom:** The runner stopped at the preview review gate with
`NATIVE_DISPATCH  Native builder output differs from reviewed record` and
`dispatch_failed true`. No insertion happened: no serial, no slot, no new
record. No replay or reconciliation was attempted.

**Observed divergence:** The preview dispatch did run (redirect 1, entry hit and
accepted, 90 ms), then the loop rejected its own builder output against the
reviewed record at `native_executor.rs` (`source[..0x24] != expected[..0x24]`).
Exactly one byte differs in `[0x00,0x24)`: `+0x1B` is `0x02` in the reviewed
record and `0x00` in the game builder output, i.e. the lifecycle word reads
`0x02800002` reviewed versus `0x00800002` emitted. The `[0x24,0x30)` window is
excluded from the comparison (the builder writes `0xFFFFFFFFFFFFFFFF` and the
preview path expects exactly that because no serial is allocated), and
`[0x30,0xE4)` is byte-identical, as are type, seed and rarity. The single bit is
`0x02000000`; its runtime meaning is **not established by this attempt**, and
this entry does not claim an unrevealed-state or new-item explanation for it.
Whether the reviewed preview expectation should carry the builder-consistent
word or keep the bit is a root/owner decision.

**Impact:** No insertion and no write. Inventory is identical before and after
(44 entries, serial `2501562`, container `c61b6b93...`, index 46;
`records_added 0`, `serial_advanced false`, `container_changed false`), the
debugger is detached, the same game process (pid `40936`) is responding, and the
game save was not written by this path (the new backup checkpoint
`f27a475f-...-001` copy hash equals both its manifest hash and the fresh save
hash `6afd998f...`; the decryptor ran only on that copy).

**Receipt and cleanup:** Remote cleanup was clean - preview receipt
`state\live-add\native-executor\178625a6-1f46-4edc-9c45-6e61f6b7f38f.json` reports
`mode preview`, `released true`, `active false`, `breakpoint_count 0`,
allocation `null`/`freed`, `debugger_state detached`, `remote_execution
quiescent`, `redirect_count 1`, with thread cleanup 337 `original_restored`,
1 `not_armed`, 1 `exited`. The receipt is nevertheless **not settled**:
`phase uncertain` because `business_outcome` is `unknown` for a preview whose
output was rejected, so the post-attempt preflight reports `receipt_files 2`,
`unsettled_receipts 1` (`unsettled 178625a6-...json`). The unsettled receipt was
preserved deliberately; any further dispatch on that state root would refuse
until root decides how to reconcile it.

**Evidence:** `D:\Nioh3_v080_deliverables\deliverables\v202-native-acceptance-20260921\INSERT3_ATTEMPT_20260921-043528.md`,
with `insert3-request-20260921-043528.json`,
`insert3-stdout-20260921-043528.txt`, `insert3-report-20260921-043528.json` and
`insert3-inventory-{before,after}-20260921-043528.json` in the same directory.
The attempt-2 save/live agreement refusal (`INSERT2_ATTEMPT_20260921-043238.md`
and its `insert2-*` files) is an ordinary unsaved-state gate and is not recorded
as an incident.

**Repair:** None attempted. The expectation for the preview lifecycle word and
the reconciliation of the unsettled preview receipt are root/owner decisions,
and the attempt was not retried, re-armed or replayed.

**Verification:** The no-insertion state is verified by the attempt's
before/after inventory and the receipt's released/detached cleanup; the
unsettled receipt is intentionally left unresolved, so this entry records a
fail-closed stop, not a fix.

**Prevention:** None promoted, and no skill, runbook or product rule was
changed. The useful pattern - a preview that dispatches and then rejects its own
builder output must leave a durable unsettled receipt instead of rewriting one -
is already how the shipped review gate behaves.

**Reproduction status:** Not reproduced. This is a single authorized attempt
under the standing no-replay rule; the one-byte divergence is deterministic for
the reviewed record and descriptor that were used, so the same gate would stop
again until the expectation is decided.

**Follow-up state:** Open. Pending root/owner decisions on the preview
lifecycle-word expectation and on reconciling the preserved unsettled preview
receipt; the candidate insertion has still not been performed.

**Skill promotion:** None.

**Corrected diagnosis (static follow-up, 2026-09-21).** An offline read of the
pinned PC v2.02 runtime text (`4CEC8FB6...`) settled the mechanism this attempt
left open and corrected two earlier readings. The builder has one verified
conditional bit-25 writer: `0x227FD96 cmp rdx, r8` / `0x227FD99 jne` guard
`0x227FD9E call 0x228758C`, which passes `{out+0x18, 0x19}, true` to the
`.pdata`-bounded `0x1A7D7F4` (`0x1A7D7F4..0x1A7D812`, 30 bytes); that function
is a thin adapter onto the bitfield helper `0x4273E8`, and index 25 in the set
direction ORs exactly `0x02000000` into the dword at `out+0x18`. Bits 1
(`0x55315A`) and 23 (`0x227FDBF`) explain the observed `0x00800002`; bit 25
additionally needs that identity-equality branch to be taken.

The reviewed record's identity instead came from the tracked corpus donor
(`test_fixtures/r4_native_corpus`, sanitized origin-account bytes equal to the
repository test constant `1111222233334444`), not from an account read. The
ambient identity `A` was never observed, so "this run's `J` differed from `A`,
so the wrapper was skipped" remains an inference about this one historical run,
not a proven v2.02 version change; nothing here shows the builder stopped
producing bit 25. Production flags are unchanged, no constant or comparison was
altered, and the unsettled preview receipt `178625a6-...` plus preview
settlement remain a separate fix.

A later offline zero-template artifact (the Rust product materializer driven
with a product-shaped donor) is retained only as non-submittable research
output. Its raw record was rejected by a direct descriptor call, but that call
bypassed the application's `new_assembly_record` normalization - the app builds
the dispatch descriptor from the normalized record
(`live_add_application.py:50-51`, `live_add_adapter.py:253`) - so the refusal is
not evidence of a product live-add defect. The next live acceptance uses real
product materialization with a genuine bound context rather than a standalone
example.

## 2026-09-21: hosted signed-release preparation failed at the source-test gate

**Objective:** Prepare the hosted signed Windows release for the v0.8.0
candidate `df7a2bd8fc42ab36b9d9fa67498e57d5fe77dc92` on
`codex/v080-rust-backend` through `.github/workflows/release.yml`.

**Observed symptom:** Workflow run
[#35593863273](https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/actions/runs/35593863273)
(`workflow_dispatch`, `worker_backend=rust`) failed at the step
`python -m unittest discover -s tests -t . -v`, reporting
`Ran 762 tests in 127.070s` / `FAILED (failures=6, errors=1, skipped=4)`.
Every step before it passed (checkout, Python 3.12 and Node 24 setup,
`pip install packaging/requirements-v2.lock.txt`, `pip install
requirements-dev.txt`, `npm ci`, version validation). Every step after it was
skipped, so no build, package, verification, signature or upload step ran and
no release artifact exists for this SHA.

**Failing node IDs (all Python, single gate):**

- `ERROR tests.test_python_r5_table_selection.SelectedTablesReachTheSequenceTests.test_batched_primary_ids_use_the_selected_index`
  - `RuntimeError: native primary batch accelerator rejected valid input`,
  raised from `nioh3_scroll_editor/seed_accelerator.py` line 593 via
  `generate_ng3_primary_effect_ids_native` and
  `effect_sequence.py` line 1148.
- `FAIL tests.test_resolved_context.CanonicalEncodingTests.test_pinned_goldens_for_both_versions`
- `FAIL tests.test_resolved_context.CanonicalEncodingTests.test_python_payload_matches_rust_golden_bytes_and_digest`
- `FAIL tests.test_resolved_context.LegacyIsProofOnlyTests.test_legacy_capture_is_not_the_shipped_production_identity`
- `FAIL tests.test_resolved_context.LegacyIsProofOnlyTests.test_legacy_digest_matches_rust_and_is_not_the_primary_digest`
- `FAIL tests.test_resolved_context.LegacyIsProofOnlyTests.test_production_payload_declares_authority_and_carries_proof_fields`
- `FAIL tests.test_python_production_context_wiring.GoldenIdentityTests.test_worker_context_matches_both_pinned_version_goldens`
  - observed `AssertionError: '2c2cc737a1b5783920806125f085ccd7b0242d270e1a33c3c60011388b2198fb'
  != '6f1292895f25937005f736b3170ccfd11b295aa7c284d3744339f6bbfd1a8712'` on
  `v202.context_digest` against a pinned golden.

The six failures form one resolved-context family (pinned digests and the
worker-context wiring that asserts them). The R5 error is a separate native
accelerator rejection on the same gate.

**Root cause:** Not yet known. This is recorded objectively at the point of
failure, before any repair report. Correlation with the candidate's
`nioh3_scroll_editor/data/game_versions/pc_v2_02.json` resource change is only
a hypothesis: the tree was seen to be dirty on that file during the run window,
and the mismatched pinned digests could be either a stale golden or a real
resolved-context change. No cause is asserted here. The R5 accelerator error is
likewise unexplained by this record; a prior worker statement that the
accelerator identity was unaffected is not yet proven.

**Measured cause - resolved-context family (2026-09-21, follow-up):** The
candidate had written the PC v2.02 approval into its own profile document.
`runtime_resource_digest` (`nioh3_scroll_editor/core_services.py:104-126`)
hashes *every* file under the runtime data root, so that one document edit moved
`resources_digest` `411866d7...` -> `b175d07b...`; the value propagates through
`legacy_context_digest` and `context_digest`, so all six pinned-golden and
wiring assertions moved together. Measured, not inferred: the digest returned
`b175d07b...` with the edit and `411866d7...` after the edit was reverted, and
the hosted log's observed value equals the edited value exactly. The pinned
goldens themselves were never wrong, and they were not re-pinned.

**Measured cause - R5 accelerator error (2026-09-21, follow-up):** The batched
route reaches the shipped ABI-v2 Seed accelerator DLL, whose default execution
policy is strict GPU. The hosted runner has no CUDA device - the same log skips
four CUDA/GPU-device-gated tests (`test_cuda_special_rule_masks_match_exact_python_replay`,
`test_cuda_and_explicit_cpu_preserve_exact_pivot_cursor_results`,
`test_amd_d3d11_device_round_trip_when_present`,
`test_ng3_rarity5_fixed_solver_needs_no_game_or_save`) - so the fixture's valid
batch request was refused before the selected index was used. On this host
`cuda_seed_acceleration_available()` is `True` and the same gate ran with zero
skips, which is why the error never reproduced locally.

**EOL/CRLF hypothesis:** Refuted for this failure. `.gitattributes` pins
`*.json text eol=lf`; `git ls-files --eol` reports `i/lf w/lf` for the document
with zero CRLF bytes; and a CRLF variant would have produced a third digest
rather than the edited-value digest the run reported.

**Repair:** Two bounded changes. The operation-scoped approval moved out of the
hashed data root into `crates/nioh3-runtime/src/profile.rs` as
`LIVE_ADD_APPROVED_VERSIONS = [FileVersion::new(2, 0, 2, 0)]`, consumed by
`ProfilePurpose::LiveAdd` while `NativeWrites` still requires the document's
blanket approval, with a new `file_version` guard so a renamed or misplaced
document cannot resolve. The profile document is restored byte-identically
(blob `2d963e040373b6c96f2a8b3c6c86bac841037d6d`, identical to the
pre-approval revision `d66b070`). The R5 fixture opts into the accelerator's
existing bulk-CPU policy, and a new regression
(`HostedNoGpuBatchRouteTests.test_cpu_opt_in_carries_the_batch_and_strict_gpu_still_refuses`)
forces the CUDA-failure path so the no-device condition is exercised on a host
that does have a device; the product default stays strict.

**Evidence:** Hosted run `35593863273` (job `106314122392`) and its failed-step
log preserved at
`D:\Nioh3_v080_deliverables\deliverables\v080-hosted-release-20260921\run-35593863273-log-failed.txt`,
summarized in `FAILURE_35593863273.md` in the same directory.

**Disposition:** Repaired in the v0.8.0 freeze commit that follows `df7a2bd`
(7 files: the restored profile document, the version-scoped live-add approval
in `profile.rs`, the two test files, the two docs and this entry). The failed
SHA `df7a2bd` is non-promotable and was not re-dispatched.

**Clean-checkout reproduction:** A clean-checkout harness
(`D:\Nioh3_v080_deliverables\deliverables\v080-hosted-release-20260921\run_hosted_gate.ps1`)
runs the exact hosted step `python -m unittest discover -s tests -t . -v`
through the project Python runner in a detached LF worktree.

- Failed candidate `df7a2bd`: reproduced the same six resolved-context
  failures, `Ran 762 tests in 127.360s`, zero checkout-converted CRLF files
  (the hosted R5 error did not reproduce, because this host has a CUDA device -
  the one environment difference, now covered by the forced-failure
  regression).
- Repaired 7-file tree: `Ran 763 tests`, `OK`, exit 0, zero checkout-converted
  CRLF files, and every previously failing node green, on two independent runs
  (136.2s and 128.8s). The hosted-equivalent
  `--require-clean --expected-sha <candidate>` preflight also returned
  `ok: true` with `dirtyEntries: 0`.

**Reproduction status:** Reproduced on both sides - failure on the old SHA,
clean pass on the repaired SHA - from clean checkouts. Line endings are
measured, not assumed (`git ls-files --eol`: 0 files converted to CRLF).

**Follow-up state:** Source-level closed. The repaired SHA still needs the
hosted run and the re-checked artifacts; this entry claims only the local
source-gate reproduction, not a release acceptance.

**Skill promotion:** None. This is a bounded hosted-gate failure record.

## 2026-09-21: no-CUDA hosts - CI frontend ordering, ignored vectors, missing test policy, DirectCompute preference

**Objective:** make the CI host gate and the packaged parity gate pass on a
runner without a usable CUDA device, without changing any shipped Rust
semantics.

**Observed symptoms:**

1. Run `35600529595` (Tests / `rust-packaging`), step "Build and verify the
   packaged Rust graph": compiling `nioh3-studio` died with
   `error: proc macro panicked ... The frontendDist configuration is set to
   "../dist" but this path doesn't exist` (`main.rs:502`,
   `tauri.conf.json` `build.frontendDist = "../dist"`).
2. A clean checkout also lacks the offline preimage vectors the worker's own
   tests read, because they lived under the git-ignored
   `deliverables/m23d-preimage/evidence`.
3. On a host without CUDA, three worker tests (`query_compile.rs` twice,
   `search_backend.rs` once) inherited the strict-GPU default and refused a
   valid page: 124/127 of the masked worker tests passed.
4. Run `35600538284` (release, step 31 `npm run test:packaged`): packaged-worker
   vs source-Python parity failed on `auxiliary.enemy_groups[*]` and
   `special_rules[*]` plus the candidate DTOs at seed 168712443.

**Root causes:** (1) that CI job builds the Tauri host but never built
`apps/tauri/dist`, which the host embeds at compile time; `release.yml` already
built the frontend first, which is why the release build got past it. (2) and
(3) are test-hermeticity and environment defects: the vectors existed only under
a git-ignored path, and the three tests carried no explicit execution policy, so
they silently depended on a usable CUDA device. (4) the legacy Python reference
preferred the DirectCompute fixed-draw collector whenever CUDA was absent; that
collector publishes a pivot-value-major cursor which deliberately differs from
the canonical low16-major cursor shared by the CUDA accelerator, the native CPU
enumeration and the ported worker, so one identical request produced different
candidates on a no-GPU host.

**Repair (bounded):** the CI job builds the
frontend (`setup-node` 24, `npm ci --no-fund`, `node apps/tauri/build.mjs`)
before the host build, with a contract assertion in
`tests/migration/test_ci_optin_wiring.py`; the six offline vectors are tracked
beside the crate at `crates/nioh3-worker/tests/fixtures/m23d-preimage/evidence`
(1.30 MiB, synthetic numeric masks/plans, no user or account data) and the
preimage/effect test modules read them there; the three tests pin
`ExecutionPolicy::AllowBulkCpu` for their own page while the production default
stays `StrictGpu` and the strict-refusal tests keep asserting refusal; and the
Python reference keeps the DirectCompute route only while CPU replay is refused,
taking the certified native enumeration under an explicit CPU allowance so
cursor and candidate order are host-independent. The Rust worker additionally
changed in `native.rs` and `native_search.rs`: the accelerator load/identity
probe no longer writes hard-coded strict GPU into the loaded library, because
that cancelled the bulk-CPU opt-in of an operation-scoped guard that was already
installed. A probe now re-installs the authoritative policy - strict GPU while
no guard is active - so a load racing an operation cannot break it.

**Independent follow-up on that policy re-install (verified):** the first form
of the repair read the lock's mirror and then called the setter separately, and
independent review showed the same defect could still occur in that window: a
probe could re-install an opt-in after the owning guard restored strict GPU
(leaking CPU allowance past the guard) or overwrite a freshly installed one. The
accepted repair makes every native policy mutation happen inside one acquisition
of the process-global policy lock: a guard install, a guard restore on drop, and
a probe re-install each read the mirror, write the library and write the mirror
under the same lock; a rejection leaves the lock untouched; nested same-owner
depth is preserved so the outermost drop still restores strict GPU. Three
permanent regressions were added in `native_search.rs`
(`a_probe_setter_cannot_interleave_an_install`,
`a_load_probe_cannot_cancel_or_leak_the_policy_guard`,
`the_policy_lock_nests_and_releases_by_owner`), the crate's 130 library tests
pass across the eight runs and clippy is clean, and the two negative controls
fail deterministically. Because these two files are production code, this
record does not claim that shipped Rust semantics are unchanged.

**Evidence and disposition:** the packaged parity gate now passes both normally
and with `CUDA_VISIBLE_DEVICES=-1` against the actual hosted workers with
unchanged assertions, and the Python gates (26 + 178 + 151) pass. Closed for
these causes in the freeze commit that follows the last non-promotable SHA; the
failed SHAs are not re-dispatched.

**Reproduction status:** all four reproduced (two in hosted runs, two in clean
checkouts and no-CUDA runs) and verified fixed by the same gates.

**Follow-up state:** source-level closed; hosted signed bytes still need their
own acceptance before any publication claim.

**Skill promotion:** None. Bounded defect record.

## 2026-09-21: Supplied Pro-response validators are locale-dependent and die on a GBK host

**Objective:** Re-run the two bounded validators shipped inside
`Nioh3_v081_Integration_Contracts_Pro_Response_20260921.zip` against the frozen v0.8.1 input
package, to reproduce its 22 + 43 checks.

**Observed symptom:** Both scripts failed before doing any work. `validation/audit_inputs.py`
raised `UnicodeDecodeError: 'gbk' codec can't decode byte 0x80 in position 724565` at
`audit_inputs.py:82` while reading `evidence/ct/features.json`; `validation/validate_response.py`
raised `UnicodeDecodeError: 'gbk' codec can't decode byte 0xae in position 2571` at
`validate_response.py:22`. Exit code 1 for both, with no check executed.

**Root cause:** The scripts call `Path.read_text()` without an encoding in several places, so they
inherit the interpreter's locale default. This host's default is GBK, while the package's JSON and
Markdown files are UTF-8. The failure is in the tooling's portability, not in the frozen input: the
same inputs decoded correctly once the interpreter ran in UTF-8 mode. Two other reads in the same
scripts already pass `encoding='utf-8-sig'`, so the omission is local and inconsistent rather than
a deliberate contract.

**Evidence:** `deliverables/v081-pro-response-20260921/acceptance/REPRODUCTION.json` records both
verbatim tracebacks, the remedy, and the reproduced results; the rerun outputs are
`acceptance/STATIC_AUDIT_RESULTS.rerun.json` (22 checks, 86 manifest entries) and
`acceptance/DELIVERY_VALIDATION.rerun.json` (43 checks, 0 failures), both deep-equal to the
versions shipped in the response.

**Disposition:** Worked around, not fixed: the response package is being accepted as delivered, so
its scripts were left byte-identical and the runs were made with `PYTHONUTF8=1` through
`tools/run_python_tests.ps1` with the explicit repository interpreter. Any future run of these two
scripts on a non-UTF-8 default host needs the same setting, or an encoding argument added to the
offending reads.

**Reproduction status:** Deterministic. The failure reproduces whenever `PYTHONUTF8` is unset on a
GBK-default host, and the pass reproduces whenever it is set.

**Follow-up state:** Open only as a note for the response author. No repository code depends on
these scripts, so nothing here blocks product work.

**Skill promotion:** None. Environment-portability record, not a reusable technique.

## 2026-09-21: hosted run 35608064429 - Python unittest fixture missed the bulk-CPU policy

**Objective:** explain the three Python `unittest` failures in hosted run
35608064429 before the next candidate dispatch.

**Observed symptom:** `tests/test_python_r5_table_selection.py::`
`ThreeRouteTablePropagationTests` failed through `_collector_page` ->
`collect_offline_rarity5_search_batch` -> `collect_effect_seed_page` ->
`joint_solver` -> `seed_accelerator.collect_natural_pivot_seeds` with
`RuntimeError: native Seed accelerator rejected a valid pivot range`; the NG4
and NG5 subtests failed the same way.

**Root cause:** test isolation, not a Rust or product defect. The fixture calls
the rarity-5 collector directly with `allow_cpu_fallback=True` but never
installed the DLL's `AllowBulkCpu` policy, unlike its sibling fixture in the
same file. Product callers install that policy (SearchJobs and the packaged
app) whenever CPU replay is allowed, so no product path is exposed. The
defect only became visible because the explicit CPU allowance now selects the
certified native enumeration instead of the DirectCompute fixed-draw route.

**Repair:** 11 added lines in `tests/test_python_r5_table_selection.py`: a
`setUp` that enters `seed_acceleration_execution_policy(allow_bulk_cpu=True)`,
mirroring the sibling fixture and the product contract. No assertion or skip
was removed, and no Rust, product or GPU-policy file was touched.

**Evidence:** with `CUDA_VISIBLE_DEVICES=-1` and the project interpreter the
module reports 11 passed, and `python -m unittest discover -s tests -t .`
(same no-CUDA shape as CI, D: temp and cargo cache) reports `Ran 766 tests ...
OK (skipped=4)` in 104 s.

**Disposition:** closed as a test-configuration defect under freeze of the CPU
parity repair; the failed run's artifacts are not promoted. Release-pipeline
isolation from the legacy Python worker is being handled by the preview agent
and is not claimed complete here.

**Skill promotion:** None. Bounded defect record.

## 2026-09-21: hosted run 35613544980 - worker policy test raced its own prober

**Objective:** explain the single worker failure in hosted run 35613544980 at
commit `8363b81` before the next dispatch.

**Observed symptom:** `WORKER` failed 129/130 on
`native_search::policy_consistency_tests::`
`a_load_probe_cannot_cancel_or_leak_the_policy_guard`, panicking at the
`"no load probe ran"` assertion on `probes > 0`. The other 129 worker tests
passed; the later release stages did not run.

**Root cause:** the regression test raced its own probe thread, not a product
or algorithm defect. The prober was spawned without a start/first-probe
handshake, so on a fast or single-core host the main thread could finish all 64
guard cycles and reach the assertion before the prober recorded its first
probe. This failure did not demonstrate a product policy error. The final
strict-GPU assertion was after the failing assertion and was not reached.

**Repair:** the root agent added a start plus first-probe-completion channel to that
test and waits for both probes to complete while the first `AllowBulkCpu` guard
is held, so the observation the assertion needs exists before the loop
proceeds. No assertion was skipped or deleted and no product code changed.

**Evidence:** with a single core and `CUDA_VISIBLE_DEVICES=-1`, that test
passed 10/10 repetitions. The three policy tests also passed. The full
single-core run then exposed a separate test scheduling assumption in
`jobs::tests::concurrent_starts_admit_exactly_one_job`: terminal status can
precede the owner's thread exit. That test now reuses the existing gated
collector instead of a four-millisecond sleep and waits for actual owner
exit before asserting it. All original acceptance assertions remain. The
subsequent single-core, no-CUDA worker run passed 130 library plus 6 binary
tests, with exit 0. Both code changes are confined to test modules.

**Disposition:** closed as a test-synchronisation defect under the same freeze;
the failed run is not re-dispatched and its artifacts are not promoted.

**Skill promotion:** None. Bounded defect record.

## 2026-09-21: hosted run 35615344505 - protected protocol test pre-seeded its own polls

**Objective:** explain the single protected failure in hosted run 35615344505 at
commit `e90ada6` without disturbing the shipped protocol surface.

**Observed symptom:** the first five crates passed;
`crates/nioh3-protected/tests/host_protocol.rs`
`handshake_then_job_current_and_shutdown` failed 24/25 with `job remains
running`.

**Root cause:** the existing acceptance scripted the 200 `job.current` requests by
pre-loading them into a `Cursor` and never waited for the background job to be
scheduled, so it could assert before the host had work in flight. That shape
does not demonstrate a production job hang, and no product path is implicated.

**Repair:** the root agent rewrote the existing protocol acceptance using
`ChannelReader`/`SharedSink`, response-driven polling with a deadline, and the
same host through completion and safe shutdown. Three other tests with the
same pre-queued-poll defect now use the shared interactive driver. The panic
finalization fixture injects exactly two panics instead of depending on a
separately scheduled releaser to produce an exact retry count. Original
result/error/ownership assertions remain. Only test code changed.

**Evidence:** the 25 protocol tests passed on a single core; the full protected
suite subsequently passed 30 library, 5 binary and 25 protocol tests. Commands,
the `HOST_PROTOCOL_E2E_OK` JSON transcripts and full output are retained under
`D:/Nioh3_v080_deliverables/deliverables/protected-protocol-sync/` in
`host-protocol-single-core.log` and `protected-all.log`.

**Disposition:** source-level closed; hosted publication still requires a new
successful run. No product logic changed, no test was skipped, and the failed
run is not re-dispatched.

**Skill promotion:** None. Bounded defect record.

## 2026-09-21: hosted run 35617410872 - verify.mjs waited on a host without the game

**Objective:** explain the packaged-UI timeout in hosted run 35617410872 at
commit `07b74236315847ca491c4b966e0b986995160a72`.

**Observed symptom:** all six Rust suites and the packaged R3/R4/R5 parity gate
passed, but `node apps/tauri/verify.mjs` timed out waiting for backend ready and
never produced its acceptance.

**Root cause:** the hosted runner has no installed game. `desktop.log` records
`GAME_EXECUTABLE_NOT_FOUND: no installed Nioh3.exe under the Steam roots this
host checks` for the `worker-backend` resolution and for the `search.catalog`,
`save.discover` and `operations:execute` calls, so the shell rendered but no
backend could become ready. The missing input is the game's PE `VERSIONINFO`
that the packaged host reads from an installed executable; this is an
environment gap, not a product defect.

**Repair in progress:** the owner is adding
`tools/prepare_ci_game_identity.ps1` plus a `release.yml` step that stages a
synthetic, never-executed version PE inside an isolated Steam root, with no
product override and no game authority. Compiler and version validation passed
under `deliverables/ci-game-identity-validation` on the D root; the hosted
retest is still pending and this record does not claim it passes.

**Evidence:** `deliverables/v080-ui-diagnosis-07b7423/failure.txt` and
`deliverables/v080-ui-diagnosis-07b7423/failed-logs/desktop.log` on the D root.

**Disposition:** open pending the hosted retest; no product logic changed and no
assertion or skip was removed.

**Skill promotion:** None. Bounded defect record.

## 2026-09-21: hosted run 35620759369 - protected retention test raced its own sampling window

**Objective:** explain the protected-suite failure in hosted run 35620759369 at
commit `0f3457a93248a86038a72295a17085665f7327cd`.

**Observed symptom:** the exact test expected `firstRetainedAttempt == 1` and
observed `2`; its bounded sibling carried the same vulnerability.

**Root cause:** a test race, not a product defect. The assertion sampled the
cumulative `finalize_attempts` counter inside a 5 ms window, which can already
include valid degraded polls that occur after the terminal or bound event, so the
sampled value is not the first retained attempt the test meant to pin.

**Repair:** the owner rewrote the two existing integration tests as subprocess
acceptance that reads the actual first retained attempt from the child's stderr
report (accepting exactly `1` or `3`), asserts the process is live before the
owner releases it, and asserts a clean exit after the stdin release. The
`HOST_PROTOCOL_E2E_OK` transcripts from the earlier single-core run remain the
protocol evidence, and the new
`deliverables/protected-protocol-sync/retained-process-e2e.log` carries the
`FINALIZATION_E2E_OK` machine JSON. The 25/25 `host_protocol` run passed on a
single core with the explicit D-root cargo target `build-cache/migration`.
Production code was not changed.

**Disposition:** closed as a test-synchronisation defect; the failed run is not
re-dispatched. A new immutable candidate
`789844856c52c5ed48aae0f5a78a884aec42347e` was pushed and its hosted runs
`35622344384` and `35622344428` started; nothing is published from either until
their promoted bytes are proven.

**Skill promotion:** None. Bounded defect record.

## 2026-09-21: hosted run 35622344384 - packaged-frontend CPU gate used the GPU deadline

**Objective:** explain the packaged-frontend timeout in hosted run 35622344384 at
commit `789844856c52c5ed48aae0f5a78a884aec42347e`.

**Observed symptom:** the crate suites, the CI game fixture, the packaged
R3/R4/R5 parity gate, `verify.mjs`, the add-layout and update gates, the host
resolution check and all three worker-identity roles passed; the packaged-frontend
regression then timed out at 180000 ms while a CPU-only search was still running.
The old error did not retain its cursor; 158614759 is the required match cursor,
not an observed timeout cursor.

**Root cause:** the acceptance driver applied its 180 s GPU deadline to a
CPU-only search, while the sibling worker-parity lane already bounds the same
search at 900 s. The gate was too short for the CPU path, not a stalled search.

**Repair:** the driver's CPU bound is now 900000 ms, matching the existing
900 s worker-parity bound, and a timeout now emits a snapshot instead of only
failing. The exact expectations stay in force: seed `226061463` at cursor
`158614759`. The GPU bound is unchanged at 180000 ms. The pipeline now passes an
explicit `--host release`, so its evidence cannot be mislabelled as a debug
build.

**Evidence:** the hosted failure is preserved under
`D:/Nioh3_v080_deliverables/deliverables/v080-final-7898448/`.
The successful local CPU replay is
`D:/Nioh3_v080_deliverables/deliverables/v080-cpu-frontend-7898448/packaged-frontend.json`;
its regression record contains the CPU flag, deadline, elapsed time, exact seed
and exact cursor. The full driver also passed the remaining synthetic save,
updater and persistence flows (`TAURI_PACKAGED_FRONTEND_OK`).

**Disposition:** local CPU acceptance passed using the existing diagnostic
portable; hosted retest of a new immutable candidate remains required. No
production code changed and the failed run will not be re-dispatched.

**Skill promotion:** None. Bounded defect record.

## 2026-09-21: closure - hosted run 35625590622 passed the open gates

The open items above are resolved by the v0.8.0 release run, not by a new
mechanism: hosted run `35625590622` at commit
`3798693c48cef2238480da66dc0cc0d2a098c78b` succeeded in 29m40s with every
required gate passing, including the CI game-identity fixture that closed
`35617410872` and the packaged-frontend regression that closed `35622344384`
(CPU-only search, allow-CPU true, seed `226061463` at cursor `158614759`,
158,765 ms). The three legacy Python steps were intentionally skipped as the
retired parity/oracle lane. The six suite results, the packaged parity gate and
the host/identity checks all passed in that same run.

The public release was then re-verified from an unauthenticated re-download
(27/27 checks) at
`D:/Nioh3_v080_deliverables/deliverables/v080-final-3798693/PUBLIC_VERIFICATION.json`.
No production code changed for either closure, and no failed run was
re-dispatched.

**Skill promotion:** None. Bounded defect record.

## 2026-09-21: v0.8.1 reader worker - out-of-scope rustfmt, CRLF stat noise, and a disposable Cargo target

**Objective:** add the read-only `runtime.inventory_snapshot` operation to the
Rust protected worker (`crates/nioh3-runtime`, `crates/nioh3-protected`).

**Observed symptom:** a `rustfmt` invocation meant for the new worker files was
given `crates/nioh3-runtime/src/lib.rs`, so it followed the module tree and
reformatted twelve files outside the ticket. They were restored with
`git restore --source=HEAD --` on exactly those twelve paths; because
`core.autocrlf=true`, that smudged them to CRLF, so `git status` kept listing
them as modified while `git diff` and `git diff --cached` were both empty. A
task-named Cargo target, `build-cache/v081-reader`, was also created instead of
reusing an existing shared gate target.

**Root cause:** rustfmt recursion from a crate root file, and choosing a fresh
target name from the resolver rather than reusing an established gate cache.

**Evidence:** the twelve paths are `crates/nioh3-runtime/src/profile.rs` and
`crates/nioh3-runtime/src/mutation/{evidence,historical_preview,historical_preview_tests,live_add,live_fakes,live_tests,mod,native_abi,native_executor,native_executor_tests,native_fakes}.rs`.
For each, `git diff --quiet` and `git diff --cached --quiet` both exit 0,
`git ls-files --eol` reports `i/lf w/crlf` (untouched clean files report
`w/lf`), and disk size exceeds blob size by the CRLF expansion. The initial
session status listed none of them as modified.

**Disposition:** no substantive content loss found; the twelve files are left
as-is on owner instruction (line-ending/index-stat noise only) and no further
restore is authorized. The disposable target was removed after resolving and
verifying it strictly below `D:/Nioh3_v080_deliverables/build-cache` with
`cargo clean --manifest-path crates/nioh3-runtime/Cargo.toml --target-dir
D:/Nioh3_v080_deliverables/build-cache/v081-reader`: 4114 files, 1.8 GiB
recovered. The external JSON evidence under `deliverables/` is untouched.
Future runs should format only their own files, never a crate root, and reuse
an existing shared Cargo target.

**Skill promotion:** None. Bounded defect record.

## 2026-09-21: v0.8.1 equipment-browser host - absolute frontendDist directory embeds no assets

**Objective:** build an authorized debug Tauri host whose embedded frontend is
the equipment-browser bundle produced on the D: delivery volume, so the
read-only `runtime.inventory_snapshot` acceptance never writes build output into
the checkout.

**Observed symptom:** with `TAURI_CONFIG` overriding `build.frontendDist` to the
absolute D directory, three debug-host launches rendered a blank shell at
`http://tauri.localhost/`; the page body was the literal text `asset not found:
index.html` and the console reported two HTTP 500s. `#root` never existed.

**Root cause:** the absolute `frontendDist` **directory** form does not produce a
usable embedded asset set in this configuration, while the config-file relative
`../dist` does. The checkout volume is exFAT, so the relative default could not
be redirected to D with a junction or symlink (`New-Item -ItemType Junction`
failed with "Incorrect function"), and `apps/tauri/build.mjs` hardcodes
`apps/tauri/dist` with no outdir override.

**Evidence:** single-variable rebuild plus probe for each variant.
`{"build":{"frontendDist":"D:/.../integration/dist"}}` -> `asset not found:
index.html`; the same key with the relative `../dist` -> shell renders;
`{"build":{"frontendDist":["D:/.../dist/index.html","D:/.../dist/app.js",
"D:/.../dist/app.css"]}}` -> shell renders with all six nav entries including the
equipment page. `tauri-build-2.6.3/src/lib.rs:487` and
`tauri-codegen-2.6.3/src/lib.rs:83` merge `TAURI_CONFIG`; the directory branch is
`tauri-codegen-2.6.3/src/context.rs:182-192`.

**Successful configuration:** the `frontendDist` **file-list** form with absolute
D file paths, then `cargo build --manifest-path apps/tauri/src-tauri/Cargo.toml`.
Recorded in the integration `PREPARE.md`; no repository file was edited and
`apps/tauri/dist` on F: was left untouched.

**Superseded artifact:** `integration/verify/equipment-failure.png` is the blank
shell from the superseded absolute-directory attempt. It is retained on owner
instruction and labelled historical in `integration/verify/visual-review.md`; it
is not evidence for the passing run.

**Skill promotion:** None. Bounded build-configuration defect record.

## 2026-09-21: v0.8.1 equipment-browser native bundle - esbuild define expression

**Objective:** build the first D-rooted debug Tauri host with the reviewed
equipment-browser frontend for bounded native UI acceptance.

**Observed symptom:** the host launched, but the CDP page reported
`production is not defined` and the document remained an empty
`<div id="root"></div>`. No game, runtime, save, or write path was reached.

**Root cause:** the helper invocation passed the esbuild define value as the
identifier expression `'production'` instead of the JSON string literal
`'"production"'`; the generated `app.js` therefore referenced an undefined
identifier at runtime. This is an invocation/build-input defect, not a product
runtime failure.

**Evidence:**
`D:/Nioh3_v080_deliverables/deliverables/v081-integration-continuation-20260921/scroll-audit/native/native-v081-evidence.json`
(run `NSPKmY`, 2026-09-21 23:24–23:26 local) records the page error and empty
root. The affected D-root bundle/host workspace is under
`D:/Nioh3_v080_deliverables/deliverables/v081-local-catalog-20260921/native/`.

**Disposition:** repair in progress. The native worker corrected the define to
the JSON-string expression, rebuilt/re-embedded the host, and will retry the
bounded acceptance. No repository product source, game process, save, or native
write was changed by the failed attempt.

**Reproduction status:** reproduced once in the first D-root helper invocation;
the corrected invocation has not yet been accepted in this entry.

**Follow-up state:** open pending the native worker's corrected build and
focused rerun. Do not promote this one-off helper mistake to a product rule or
skill.

**Skill promotion:** None. Bounded build-input failure record.

## 2026-10-13: PC v2.02 combat intake - historical section path absent

**Objective:** Capture fresh PC v2.02 `.text`, `.rdata`, `.pdata`, and the
known read-only effect/auxiliary parameter tables for a bounded Nioh 3 combat
source inventory.

**Observed symptom:** The first invocation of
`research/dump_r4_finalizer_tables.py` with the unchanged
`nioh3_scroll_editor/data/game_versions/pc_v2_02.json` profile failed before
process access with `FileNotFoundError`: the profile's historical section
manifest resolved to a missing `Nioh3_v2.0.2.0.text.bin` path under the old
game-version-update delivery tree.

**Root cause:** The historical `.text` artifact referenced by the profile was
absent on this host. This is an evidence-path availability failure, not a game
or parameter-table failure.

**Evidence:** The failed and successful attempts are recorded in
`D:/Nioh3_v080_deliverables/deliverables/combat-data-intake-20261012/runtime/run-20261013-pid21384/capture-attempts.json`.
The fresh sections are in
`D:/Nioh3_v080_deliverables/deliverables/combat-data-intake-20261012/runtime/run-20261013-pid21384/sections/manifest.json`;
the successful effect capture is
`D:/Nioh3_v080_deliverables/deliverables/combat-data-intake-20261012/runtime/run-20261013-pid21384/tables/manifest.json`.
The original profile was left unchanged and the run-local profile copy is
`D:/Nioh3_v080_deliverables/deliverables/combat-data-intake-20261012/runtime/run-20261013-pid21384/profile-pc_v2_02-fresh-sections.json`.

**Disposition:** Created a run-local profile copy that changed only
`section_dump.manifest` to the fresh same-session section manifest. The fresh
read-only capture then passed the executable hash, version, code-signature,
parameter-manager and table reads. No product profile, source data, game
memory, or save was modified.

**Reproduction status:** Reproduced once during the intake on this host; the
missing historical path is deterministic for the unchanged profile. The
run-local correction succeeded without retrying the absent artifact.

**Follow-up state:** Closed as a bounded intake failure. Use the retained fresh
run artifact for this task; do not treat the run-local profile copy as product
compatibility approval.

**Skill promotion:** None. This is a host-specific missing-evidence-path
record, not a new workflow requirement.

## 2026-10-13: PC v2.02 combat intake - uncapped static xref helper interrupted

**Objective:** Find bounded current-version RIP-relative references for selected
action/motion and damage/Ki combat-anchor strings after the fresh section dump.

**Observed symptom:** The existing `research/find_runtime_data_xrefs.py` pass
had not produced a report after more than 60 seconds. Its implementation
iterates every parsed `.pdata` function and disassembles each function without
an explicit decoded-byte or function budget.

**Root cause:** The helper's resource bounds were not explicit for this fresh
large section capture. This is an analysis-tool bounding defect, not a game
bug, xref conclusion, or runtime failure.

**Evidence:** The interrupted attempt and its no-report result are recorded in
`D:/Nioh3_v080_deliverables/deliverables/combat-data-intake-20261012/runtime/run-20261013-pid21384/xref-helper-attempt.json`.
The complete bounded replacement run is recorded in
`D:/Nioh3_v080_deliverables/deliverables/combat-data-intake-20261012/runtime/run-20261013-pid21384/combat-anchor-xrefs.json`
and
`D:/Nioh3_v080_deliverables/deliverables/combat-data-intake-20261012/runtime/run-20261013-pid21384/combat-consumer-xrefs.json`;
the aggregate run manifest is
`D:/Nioh3_v080_deliverables/deliverables/combat-data-intake-20261012/runtime/run-20261013-pid21384/run-manifest.json`.

**Disposition:** The uncapped helper was stopped without interpreting partial
state. A run-local `bounded_rip_xrefs.py` prefiltered selected RIP opcode forms
and enforced a maximum of 32 candidate functions, 65,536 decoded bytes, and
eight instruction snippets. It produced 3 and 6 scoped references respectively
for the two reports. Full sections remain private.

**Reproduction status:** The slow/no-report behavior occurred once in the
fresh intake. The bounded replacement completed successfully in the same
prepared Python environment; no second uncapped attempt was made.

**Follow-up state:** Closed as a bounded-analysis failure. The bounded reports
are diagnostic leads only and do not establish action-table ownership or field
semantics.

**Skill promotion:** None. Keep this as a one-off resource-boundary record;
promote only if the same helper limitation recurs independently.

## 2026-10-13: Damage Pro return - optional-key evidence selected the wrong manager table

**Objective:** Independently audit the Pro return's `0x4647` association in the
PC v2.02 damage investigation and determine whether it supports an effect-table
combat coefficient.

**Observed symptom:** The earlier evidence selection treated the `0x4647`
immediate at `0x5725F6` as an effect-table query through manager `+0xC8`.

**Root cause:** The caller at `0x5725E3` resolves parameter manager RVA
`0x45B9E30`, then loads `+0x230` before passing `0x4647` to `0x608508`.
The direct callee reads signed `row+0x10` and float `row+0x18` after the
unrecovered `0x608530` lookup. The current optional row 1638 is key `0x4647`,
integer `120`, scale `1.0`; it is not an effect-table row. The independently
counted 165 effect/optional numeric intersections are namespace collisions.

**Evidence:** The immutable Pro return and its input checks are under
`D:/Nioh3_v080_deliverables/deliverables/damage-pro-followup-20261013/pro-return/nioh3_damage_analysis`.
The reproduced byte audit is in
`D:/Nioh3_v080_deliverables/deliverables/damage-pro-followup-20261013/review/reproduction/audit_results.json`
and `optional_lookup_sites.json`; the retained PC v2.02 body/section controls
are under
`D:/Nioh3_v080_deliverables/deliverables/combat-data-intake-20261012/runtime/run-20261013-pid21384`.
The final bounded review is
`D:/Nioh3_v080_deliverables/deliverables/damage-pro-followup-20261013/review/REVIEW.md`
with machine-readable verification in `verification.json`.

**Disposition:** Corrected the review classification only. Keep the optional
route and counter field as qualified static evidence; do not promote it to a
damage or Ninjutsu coefficient contract. The package and product code remain
unchanged.

**Reproduction status:** Reproduced independently from the current `.text`
section and optional/effect table bytes; section and table hashes match their
retained manifests. No game, process, debugger, network, memory, or save action
was used.

**Follow-up state:** Closed as a bounded noncanonical evidence-selection error.
The `0x608530` row lookup and exact `this+0x1DB0` units/object remain open for
the next bounded research capture. This entry does not block product work.

**Skill promotion:** None. Keep the correction in the review and evidence
ledger until a matching runtime consumer is captured.

## 2026-10-13: Damage Pro return - descriptor candidates selected 0x78 bytes before the name pointer

**Objective:** Independently audit the four descriptor-function selections in
the Pro return and verify the correct pointer positions in the current `.rdata`
section.

**Observed symptom:** Each old exported candidate was read from
`name-pointer-storage-0x78`, while the descriptor window's immediate following
qword was treated as unrelated.

**Root cause:** In all four retained 264-byte windows, the qword at the named
string pointer storage slot is followed immediately by a qword resolving to the
candidate callback RVA. The corrected adjacent candidates are `0x23C785C`,
`0x23C75F4`, `0x23C7540`, and `0x23C76A8`; the old candidates are exactly at
the `-0x78` positions. The descriptor registration consumer and complete
`.pdata`/unwind association were not supplied, so callback binding remains an
inference.

**Evidence:** The four window hashes and pointer positions are reproduced in
`D:/Nioh3_v080_deliverables/deliverables/damage-pro-followup-20261013/review/verification.json`.
The source `.rdata` section is
`D:/Nioh3_v080_deliverables/deliverables/combat-data-intake-20261012/runtime/run-20261013-pid21384/sections/Nioh3_v2.0.2.0.rdata.bin`
with SHA-256
`112BE451AB3B2ABEF3BD90C608BF7853C2607E96250894C82670FABBD74EFA81`.
All four retained windows match that section byte-for-byte; the four old body
ranges also match the current `.text` section.

**Disposition:** Corrected the next export targets in the audit only. Treat the
four `+8` values as strong descriptor-pair candidates and retain the old bodies
as historical mistaken selections. Do not claim callback semantics until the
registration consumer is recovered.

**Reproduction status:** Reproduced independently from the retained current
`.rdata` and `.text` bytes with zero window or body mismatches. No game,
process, debugger, network, memory, or save action was used.

**Follow-up state:** Closed as a bounded noncanonical evidence-selection error.
Export the four corrected ranges and recover their registration consumer in the
next bounded C01 pass. This entry does not block product work.

**Skill promotion:** None. The adjacency correction is specific to this
versioned descriptor evidence until the consumer is closed.

## 2026-10-13 — Damage curve reference accepted zero caps

Objective: reproduce manager+720 curve and attack-defense correction in native floating-point order.
Symptom: the first reference rejected D6A4 at input zero with ValueError.
Cause: an overly restrictive positive repeat-cap guard; the captured curve returns zero and the native residual loop still terminates through decay below one.
Evidence: damage-direct-research-20261013/pipeline/CURVE720_AND_AFFIX_JOIN.md, selected_curve720_rows.json, and exact FBFE00 loop exports.
Disposition: allow nonnegative caps with decay in [0,1); reject negative caps and nonterminating decay domains.
Reproduction: integrate_curve720.py includes the formerly rejected zero input among 481 offline predictions. Native same-hit parity remains pending.
Follow-up: root records live checkpoint comparisons. No product behavior changed.

## 2026-10-13 — Scalar table key offset corrected

Objective: recover native scalar coefficients for attack-defense correction.
Symptom: initial searches found none of thirteen expected keys.
Cause: treating row+0 or row+8 as the key; the native hashEntries-to-rowIndex lookup identifies the key at row+0x14, value at +0x10 and scale at +0x18.
Evidence: damage-direct-research-20261013/pipeline/REPORT.md and scalar reconstruction artifacts.
Disposition: corrected the offline parser against the native lookup, preserved original captures.
Reproduction: coefficients now resolve in the captured manager+230 table. No live parity claim from this repair alone.
Follow-up: use current-session signatures and captures for native comparisons.
# Active-agent ledger additions for root integration

The root owns repository paths; this agent owns only pipeline/. These bounded records are ready to append to `docs/research/EXPERIMENT_FAILURE_LEDGER.md` without altering another owner's work.

## Tail ordering overview overstated final cap

- Objective: derive complete outgoing HP/Ki tail arithmetic.
- Symptom: initial ordinary-core overview could imply999999 is a final channel bound and the integer conversion is the last element arithmetic.
- Cause: omitted post-cap physical replacements and post-truncation element state multipliers from the concise overview.
- Evidence: main795EF1..795FA3 physical replacement;7960CD..796133 element factors; captured scalar6331=5/B4FE=2; no second cap at7962F0.
- Disposition: repaired REPORT.md/typed stage map and complete tail reference; actual current-table coefficients preserve integral element values but can raise the earlier cap to9,999,990.
- Reproduction: inspect proof_complete_tail.asm lines98..247, run tail_reference.py caseelement_cap_before_times10.
- Follow-up: native pre-tail/final checkpoint comparison must include the activating gates; ordinary vectors do not establish the exceptional path.

## Third attack-defense channel overview overstated curve scaling

- Objective: recover all attack-defense stage channels.
- Symptom: initial overview grouped XMM11 element-copy with the two curve-scaled channels.
- Cause: compressed three-call summary did not distinguish the missing FBFD40 before the third FBFE00 invocation.
- Evidence: main78E65A..78E67C directly passesXMM11 toFBFE00; compare_runtime_math.py models this route separately.
- Disposition: reports corrected; same-hit19-vector comparison57/57 low32-bit matches, with only zeroXMM11/element controls.
- Reproduction: inspect proof_attack_defense_calls.asm at78E65A onward; run compare_runtime_math.py through project runner.
- Follow-up: nonzeroXMM11 and nonzero-element native vectors remain outside the current acceptance corpus.


## 2026-10-13 — Tail observer signature preflight rejection

Objective: observe native pre-tail channels at795D43 and later independent truncation.
Symptom: the initial arm was rejected before any breakpoint was installed.
Cause: the handwritten signature omitted an intervening mov rcx,r14 instruction.
Evidence: live/observations/tail_arm.json, tail_signature_cleanup.json and exact795D43 bytes in the pinned main-function export under damage-direct-research-20261013.
Disposition: corrected the expected signature against raw source bytes; cleanup proved empty. Improved observer preflight to stop its namespace and report expected/actual bytes on rejection.
Reproduction: corrected arm passed; the first corrected collection had zero events and is retained without positive acceptance.
Follow-up: collect only when a naturally triggered target hit is confirmed. No code/game/save patch occurred.

## 2026-10-13 Attack-source multi-hop observer unavailable thread API

Objective: observe prepared attack -> constructor copies -> HP evaluator in one native run. Symptom: attack_source3 stopped after5callbacks because debug_getCurrentThreadID was nil. Cause: unsupported API assumption in added optional metadata. Evidence: damage-direct-research-20261013/live/observations/attack_source3_final.json and attack_source3_cleanup.json. Disposition: preserve failed run, mark thread identity unavailable, remove unsupported call; always-resume and owned cleanup succeeded, global breakpoint inventory empty. Reproduction: first retained callback calls missing global; verified in raw error and installed celua.txt lacks this API. Follow-up: fresh source4 run with unchanged causal pointer/block guards and explicit missing thread metadata.
# Bounded experiment failure entry

Objective: locate an8A17 item-local consumer by following group flag80.
Symptom: the obvious direct byte+50/mask80 match at4AAE0C and a later packed-bit helper21F8E3C do not establish the effect-group namespace;119 lookup-edge and92-function surveys produce no joined flag80 arithmetic route.
Cause: reused structure offsets and unproven flag meaning; treating bit80 as an item-local routing flag was only a hypothesis.
Evidence: group_flag_access_windows.json, flag80_helper_candidates.json, group_lookup_tail_edges.json and exported4AAE0C evidence in the parent effects directory; byte-verified group table flags include Max Ammo groups with active actor handlers.
Disposition: reject those matches as familiarity consumers. Keep8A17 unresolved globally; retain specific route exclusions and the separately proven generic familiarity calculator.
Reproduction: project runner with inspect_group_flag_routes.py and locate_flag80_helpers.py in this directory.
Follow-up: root-owned actual8A17 gear observation at557BA0/819400 and same-phase item/ordinal/prepared-attack writer correlation. No live state was changed by this subtask. The global unresolved status in this historical entry is superseded by the direct on-hit consumer documented below.

## Oversized-function omission hid the actual consumer

Objective: locate all direct native 8A17 group consumers in the captured executable.
Symptom: the literal locator reported no joined damage calculator, while the already exported main function contains `mov edx,8A17` at RVAs7914DE and791A35 and calls the actual grade reader921BC8.
Cause: the `immediates` command silently skipped every pdata function larger than32768 bytes, including the 39134-byte 78CE8C..79676A main function. The exporter separately allowed that known function to40000 bytes. I did not reconcile the inconsistent limits or revisit the existing complete export soon enough; this materially prolonged the false lead through item preparation.
Evidence: exact main bytes at7914DE and791A35, their921BC8 calls, the2081C50 native helper and DD41 curve join, and the old `if e[1]-e[0]>32768:continue` locator branch. The group materializer819400 default and unchanged prepared attack remain valid bounded observations, but do not exclude on-hit aggregate readers.
Disposition: supersede all global-unresolved Fam conclusions with the on-hit formula. Repair the owned locator to explicitly permit these already approved large functions and report all other oversized candidates. Preserve earlier artifacts as historical evidence; provide a catalog correction to the parent.
Reproduction: run `inspect_familiarity.py immediates 0x8A17 --name repaired_familiarity_immediates` through the project Python runner and compare verified sites with the complete78CE8C export.
Follow-up: extract every DD41 grade knot and compare the natural0-to15000 familiarity control against helper and pair observations. Live observation and cleanup remain exclusively root-owned.

# Active-agent ledger additions for root integration

The root owns repository paths; this agent owns only pipeline/. These bounded records are ready to append to `docs/research/EXPERIMENT_FAILURE_LEDGER.md` without altering another owner's work.

## Tail ordering overview overstated final cap

- Objective: derive complete outgoing HP/Ki tail arithmetic.
- Symptom: initial ordinary-core overview could imply999999 is a final channel bound and the integer conversion is the last element arithmetic.
- Cause: omitted post-cap physical replacements and post-truncation element state multipliers from the concise overview.
- Evidence: main795EF1..795FA3 physical replacement;7960CD..796133 element factors; captured scalar6331=5/B4FE=2; no second cap at7962F0.
- Disposition: repaired REPORT.md/typed stage map and complete tail reference; actual current-table coefficients preserve integral element values but can raise the earlier cap to9,999,990.
- Reproduction: inspect proof_complete_tail.asm lines98..247, run tail_reference.py caseelement_cap_before_times10.
- Follow-up: native pre-tail/final checkpoint comparison must include the activating gates; ordinary vectors do not establish the exceptional path.

## Third attack-defense channel overview overstated curve scaling

- Objective: recover all attack-defense stage channels.
- Symptom: initial overview grouped XMM11 element-copy with the two curve-scaled channels.
- Cause: compressed three-call summary did not distinguish the missing FBFD40 before the third FBFE00 invocation.
- Evidence: main78E65A..78E67C directly passesXMM11 toFBFE00; compare_runtime_math.py models this route separately.
- Disposition: reports corrected; same-hit19-vector comparison57/57 low32-bit matches, with only zeroXMM11/element controls.
- Reproduction: inspect proof_attack_defense_calls.asm at78E65A onward; run compare_runtime_math.py through project runner.
- Follow-up disposition: the later Water2 corpus supplies12 nonzero element/copy hits;36/36 low32 comparisons match. This closes that narrow input-coverage gap, not all element/tail paths.

## Whole-main SSA construction exceeded preliminary budget

- Objective: retain complete bounded main-function register/stack definitions and branch alternatives.
- Symptom: initial eager stack SSA exceeded the300,000-node preliminary budget; a later alias-collapse attempt left a dangling phi reference.
- Cause: LEA operands were incorrectly included as memory-read slots, and representative rewriting checked membership in the already-pruned node map.
- Evidence: owned build_main_graph.py failure output; current main_graph_verification.json retains157 actual read slots,275,537 collapsed trivial phis and44,990 compact nodes; index verification validates every input reference.
- Disposition: repaired actual-read filtering and representative-map rewriting; bounded single-function pre-collapse ceiling400,000/compact ceiling100,000; completed graph byte checks and49/49 manual placement checks pass.
- Reproduction: run build_main_graph.py then index_main_graph.py with the project runner and stable recorded catalog snapshot.
- Follow-up: preserve explicit partial-store/alias nodes rather than reducing uncertain memory state to a guessed scalar.

## Recursive provenance traversal stalled on graph cycles

- Objective: recover actor/pair/value provenance after conservative native stack alias analysis.
- Symptom: recursive traversal consumed roughly300/273 seconds in two owned analysis helper processes without completion; a subsequent index pass also encountered an uppercase/lowercase RVA lookup mismatch.
- Cause: repeated recursive paths through cyclic phi/alias graphs; inconsistent hex normalization in the manual cross-check.
- Evidence: verified owned helper chains51864->50540->13492 and34916->53184->49280 were terminated; no game/debugger processes were touched. Current verification records167,502 bounded fixed-point provenance updates and all49 manual pointer/field matches.
- Disposition: replaced recursion with worklist sets/bitsets and normalized cross-check RVAs. Latest graph and index completed successfully; valid retained evidence was preserved.
- Reproduction: run the owned graph/index scripts through the project runner, then inspect main_graph_verification.json/main_graph_index_verification.json.
- Follow-up: keep bounded fixed-point propagation and explicit opaque helper/alias contracts; do not reintroduce recursive path enumeration.


## 2026-10-13 Pro r2 reference Windows text decoding

Objective: replay returned Pro reference on the configured Windows project runner.
Symptom:17synthetic test methods passed; test_catalog_coverage failed reading
UTF8effects.json with host defaultGBK. Cause: Path.read_text omitted explicitencoding.
Evidence: damage-pro-return-audit-20261013-r2/local_return_verification.json,
REPLAY_RETURN.ps1 and local-tests.log. Disposition: preserve received ZIP/source;
use scopedPYTHONUTF8=1 in the local wrapper;18/18tests and raw replays pass.
Reproduction: run received tests withoutUTF8setting on GBKdefaultWindows. Follow-up:
future authored JSON/text reads should use explicitencoding='utf-8'; no numerical
formula correction or live write was required.

## 2026-10-13 Triggered action-observer handoff cleanup failure

Objective: avoid consuming the action-capture window while the owner is away.
Symptom: waiter captured a real player-Ki decrease1370->1329.2769775, but four
scheduled cleanup/handoff retries did not prove empty breakpoint state, so the
formal action collector was not armed. Cause: asynchronous CE removal remained
pending during the automatic timer handoff; exact bridge/thread scheduling unknown.
Evidence: damage-native-followup-20261013-r3/live/action_triggered_final.json and
separate action_triggered_cleanup.json. Disposition: preserve the trigger as partial
post-write evidence and reject the following controls as unobserved. Root cleanup
now proves inactive/empty. Replace the handoff with one four-site observer whose
waiting stage transitions on an actual positive player cost, without rearming.
Reproduction: observed in this real owner-triggered run; no fake/synthetic action.
Follow-up: bounded same-observer waiting/capture implementation and a repeat with
larger three-hit cost, preserving exact event/frame/object identity.

## 2026-10-13 - Analysis carrier descriptor-validation filter

- Objective: admit the preserved post-loader Nioh3 sections to REA/Ghidra with an independently checked historical image base.
- Symptom: the first two carrier-builder runs rejected the input before writing a PE because a generic name-prefix filter expected Get_ and then Game::, while the authoritative adjacent pointer descriptors use Collision:: and other namespaces.
- Cause: overly restrictive analysis-helper validation filter, not mismatched source bytes or missing game code.
- Evidence: damage-rea-static-20261013/build_analysis_pe.py, analysis_pe_manifest.json, and the immutable packet evidence/corrected-symbol-exports/descriptors.json.
- Disposition: validate the four exact recorded name/callback pairs and then namespace-bearing ASCII pointer pairs. The corrected run verifies2296 general descriptors and initialized-section byte parity.
- Reproduction: run the builder through tools/run_python_tests.ps1 -ScriptPath using the recorded source section hashes. Corrected current run passes.
- Follow-up: retain exact descriptor checks, separate derived carrier identity, and keep uncaptured global data uninitialized. No live game action or product integration followed.

## 2026-10-13 - Full exception-driven REA auto-analysis scope

- Objective: obtain decompiler evidence for Nioh3 outgoing-damage functions using official REA6.4.0/Ghidra12.1.4.
- Symptom: full captured pdata-driven import created a large unrelated function-analysis workload; logs reported invalid pcode constructors and VarnodeContext out-of-address-spaces during constant propagation outside the selected damage path. No main pseudocode was returned during this trial.
- Cause: full-image default analyzer scope includes protected/unrelated code and derived function-body guesses; exact effect on selected functions remains unknown.
- Evidence: damage-rea-static-20261013/full-autoanalysis-trial/{ghidra.log,script.log,status.json} and ghidra_startup_threads*.txt.
- Disposition: cancel the owned trial, retain original source bytes, and use a separately hash-bound focused carrier with exception directory disabled and113 caller-asserted native entry seeds. Raw original pdata remains intact and is still the boundary-validation input.
- Reproduction: full-carrier REA procedure_pseudo_code78CE8C with default auto-analysis and6G JVM heap. Focused retry changes the import metadata and pre-analysis entry profile; it does not repeat the identical failed input.
- Follow-up: verify native byte/body coverage and decompiler warnings for each selected function; do not treat seed names, tool readiness, or a recovered C signature as proven native ABI or formula closure.

### Follow-up: focused entry import with default recursive analyzers

The focused header/113-seed profile also reached unrelated protected callbacks
through recursive default analysis; no requested pseudocode was returned during
that trial. Logs are retained in focused-default-autoanalysis-trial/. It was
cancelled and its Java provider verified stopped. The task-local REA installation
now has a reversible two-module patch: launch with -noanalysis after applying the
source-backed seeds, and explicitly label the profile
owner-scoped-preseeded-noautoanalysis. Backups and before/after hashes are under
provider-local-patch/. This is a private analysis-tool patch, not an official
REA default preset and not a game/product/client-configuration change. Function
body/signature inference remains subject to raw-instruction verification.

## 2026-10-13 - Native ABI TSV CRLF parsing

- Objective: preserve native XMM0 helper returns in the task-local Ghidra decompiler profile.
- Symptom: ABI pass rejected an entry before reporting committed seeds; REA then returned a fail-closed missing-seed-report error. No ABI-corrected pseudocode was accepted from that trial.
- Cause: Windows text writers produced CRLF in the raw ABI TSV; Java initially split only on LF, leaving a trailing CR in the float token. A direct diagnostics launch also passed raw CRLF seed names, which Ghidra correctly refused as invalid names. REA's ordinary seed snapshot normalization itself was not broken.
- Evidence: damage-rea-static-20261013/abi-script-diagnostics/{failed_crlf_stdout.log,script.log,script_fixed.log,fixed_stdout.log,failed-mcp-pass/}.
- Disposition: strip one trailing CR per line in both private Java parsers without changing source native bytes or ABI input hash. Direct read-only headless replay now reports135/135 functions created,0failures,28bound float returns and the known cookie callfixup.
- Reproduction: the retained pre-fix log fails on Invalid native ABI entry; the corrected same-input run passes. The first diagnostic launch also required creating its explicitly scoped D: user-home directory before JDK startup.
- Follow-up: retain initial untyped outputs and the repaired private-tool source/hash manifest; verify byte coverage again before accepting corrected C output. No game, save, product or global client setting was changed.

## 2026-10-13 - Pro r4 return integrity, decode representation and annotation boundary

- Objective: accept the supplied r4 native analysis against original bytes and replay its local controls, preserving all-affix scope.
- Symptoms: strict manifest-coverage preparation rejected one unlisted generated verification receipt; the first independent decoder rejected a standalone F0 LOCK record; an HP annotation range cut its last five-byte CALL short by one byte.
- Causes: the receipt was generated after the 395-entry manifest; LLVM splits LOCK from CMPXCHG in the exported record representation; the human-readable end-exclusive marker was 32989C instead of 32989D. No original native-byte mismatch was found.
- Evidence: damage-pro-return-audit-20261013-r4/received_manifest_verification.json, independent_native_decode.json, hp-review/corrected_ranges.json, and hp-review/native_verification.json under the D: deliverables root. The unlisted receipt SHA is 2f6ffa6d8d992587cb8953fff34b221114eacc8bf459cf555a84af4b3fe532fe.
- Disposition: preserve the received archive/source; allow only the exact separately hashed generated receipt without using it as proof; independently decode joined prefix bytes and retain explicit unsupported interpreter status; record corrected annotation metadata separately. Full offline replay passes in a working copy and independent decoding reports zero mnemonic/operand mismatches.
- Reproduction: strict coverage comparison, decoding F0 alone, and comparing the 329898 CALL's length against the declared range reproduce the three issues. Native JSON contains the complete original CALL.
- Follow-up: retain the FC0AE0 blocked path, do not infer final post-notification HP, and apply the owner-reaffirmed all-affix contract. These findings do not justify more per-affix gameplay or a complete-formula claim.

## 2026-10-13 - All-effect census sparse CSV projection and populated counter

- Objective: validate every returned effect/group against original native bytes and produce an accurate all-ID coverage matrix for Pro r5.
- Symptoms: initial CSV parity rejected sparse empty-group JSON; an initial passed row replay counted all1152membership dictionary keys as populated alongside392empty groups.
- Causes: fixed-header CSV versus omitted stacking_rule_retained JSON keys; defaultdict lookups inserted empty membership lists before a key-count summary. Neither changed original game data or established full formulas.
- Evidence: damage-pro-return-audit-20261013-r4/all-effects-review/EXPERIMENT_FAILURE_LEDGER.md, census_gaps.json/csv_sparse_JSON_fields, replay_result.json and replay_census.py under the D: deliverables root.
- Disposition: compare exact CSV projections with required empty omitted fields, retain392sparse row identities, count nonempty memberships and assert760+392=1152. Final replay passes19,980real-input checks and679stable input hashes; portable r5 replay also passes.
- Reproduction: pre-correction exact-key comparison fails at group row15; pre-correction len(membership) summary reports1152populated rows. Owned audit ledger preserves the defect descriptions.
- Follow-up: retain separate catalog/local/candidate/source/stacking/output dimensions. 493local records and917candidate IDs are not complete formulas or percentages.

## 2026-10-14 - Equipment-context exclusive end and unavailable prior archives

- Objective: give Pro concrete current equipment-generation/input code for the optimizer while retaining historical evidence boundaries.
- Symptom: three recorded September primary archives and converter FINAL_REPORT were absent at their exact D: locations and bounded alternate roots. The first current-byte export helper also rejected requested55182D as outside the preceding pdata interval.
- Cause: prior artifacts unavailable; cause of absence unknown.55182D is the exclusive end of5515FC, mistakenly included in the helper's requested-entry list.
- Evidence: damage-pro-return-audit-20261014-r5/equipment-native-context/CURRENT_NATIVE_CONTEXT.json, collect_equipment_context.py and equipment-native-context-partial-end-boundary; current pdata proves5515FC..55182D. Two historical Pro-response ZIPs survive with verified SHA/CRC.
- Disposition: preserve the incomplete trial, remove only the exclusive-end request, export six current exact ranges and three checked edges. Label historical Pro proposals and absent old observations explicitly; no legality or native-generation proof is inferred.
- Reproduction: original requested55182D fails containing-interval assertion; corrected same-source run passes. Missing path checks are recorded in CURRENT_NATIVE_CONTEXT.json.
- Follow-up: all current raw sections and selected exports are included in r6 for Pro to derive actual producer/domain/stat contracts; no new gameplay or fabricated old capture.

## 2026-10-14 - R6 nested historical checksum manifest packaging

- Objective: seal the all-input optimizer-foundation handoff with exact root checksum/ZIP coverage.
- Symptom: final validator rejected extra ZIPmember evidence/local-r5-acceptance/shared-review/SHA256SUMS.txt. The failed console run emitted no stdout file, so a later attempt to preserve that absent log also reported missing path.
- Cause: reused checksum validator excludes every SHA256SUMS basename from its payload, while ZIP construction copied all files; the new shared-review manifest had not been renamed. No source/formula/native-byte mismatch occurred.
- Evidence: damage-pro-return-audit-20261014-r5/r6_validation_failure_record.json and r6_archive_receipt_failed_nested_manifest.json; failed ZIP SHA a35db1ddb54ff19068fea0618cee07e95517daca4f16c080cc88503277d98723 preserved with _failed_nested_manifest suffix.
- Disposition: retain exact historical manifest contents as INPUT_SOURCE_SHA256SUMS.txt, update source-copy mapping/builder, regenerate root hashes and repackage without discarding the failed archive. Preserve the console error as an explicit record, not a claimed nonexistent log.
- Reproduction: pre-repair validator reports that one extra member; repaired validator passes5457checksums/5458exactmembers. Relocated numerical/graph/source replay had passed and was unaffected by manifest metadata repair.
- Follow-up: builder now renames nested historical checksum files during copy. Use only the final ZIP with SHA77bd3e3902b670a4529e8ce3db26032754e3a440954cbf49725882a173ced4b3 for Pro. No product or live state changes.

## 2026-10-14 - R6 table availability, helper-domain and independent-checker corrections

- Objective: execute both R6 returns and independently accept native local arithmetic for the constrained equipment optimizer.
- Symptoms: the report requested a missing manager+70 table that was already supplied; extended internal field70(502,0x20) returned25050 instead of native25100. Owned independent helpers initially misread optional rows by eight bytes and mishandled literal XOR when the first operand was zero; schema and one instruction-anchor fixture assumptions were also corrected before acceptance.
- Causes: stale input-availability conclusion; a last-row clamp was generalized beyond certified caller indices; omitted captured-header offset; overbroad same-register XOR-zero specialization; differing variant export schemas and a CALL-interior anchor. No supported public R6 arithmetic mismatch was found.
- Evidence: D:/Nioh3_v080_deliverables/deliverables/damage-pro-return-audit-20261014-r6/upstream-review/{REPORT.md,ACCEPTANCE.json,optional_row_source_diagnostic.json}, rank-equipment-review/{verification.json,AUDIT_FAILURES.md}, execution/replay_summary.json and LOCAL_AUDIT.md. Three table70 captures match SHA15bf34a342ed7fc2d4739c954af8d13c3856988ba6f7518e1227e6db4cd4b9b9.
- Disposition: preserve received R6 code unchanged; accept only exact local caller domains; require a guard or true native extrapolation before general field70 reuse; reuse existing table70 instead of requesting a redundant capture. Repair only the owned audit helpers and rerun their negative controls. Final366 upstream and34/24/18/9/3 rank/equipment/unlock comparisons pass; all36 package commands return expected exits and immutable received bytes remain unchanged.
- Reproduction: index502/field20 contrasts the private helper's row501 value25050 with D21240 extrapolation25100; unlock0/improvement5 exposes XOR ECX,1 and must floor agility index at1/score80; original optional rows require header8+32*index. The retained helper diagnostics and traces record each repair.
- Follow-up: carry domain correction, exact current-table identity and decisive negative controls into R7. Keep provided interpreter replay, independent native arithmetic, final actor/legality/gameplay and optimizer proof separate; no game/CE/Computer Use/native CPU/product/save work occurred.

## 2026-10-14 - R7 reserve null-context source route and static-inventory shape

- Objective: accept the R7 native forward return and preserve exact source admission for future constrained equipment evaluation.
- Symptoms: all delivered numerical tests passed, but a supported high-level body input with null stat+7D8 still emitted reserve source submissions. Nonzero raw attributes paired with that null context were also accepted. Separately, an owned static-table inventory helper rejected the bonus_curve/rows directory before producing any evidence because it assumed a flat rectangular store for every table.
- Causes: the R7 high-level reserve loop omitted native2FB778/2FB77F's null-context jump to2FB928; its explicit context/raw-array inputs lacked the consistency guard proved by2FB9B4's28zero-byte fallback at3976728. The private inventory helper conflated fixed-stride hash tables with bonus_curve's object/pointer/per-row storage.
- Evidence: D:/Nioh3_v080_deliverables/deliverables/damage-pro-return-audit-20261014-r7/item-body-review/{reserve_context_counterexample.json,r7_body_null_context.patch,patch_metadata.json,REPORT.md}, repair-execution/repair_summary.json, static-input-inventory/INVENTORY.json and collect_static_inputs.py. Original modelSHAacd9378311648734c110f4cdb65b9f53380e1c1c124467957222fa390506af78; repairedSHA436188d64191671f4fb3575edcaeb2f2479e77d7ac7f1b2672ce73cf7c3aaf38.
- Disposition: preserve the exact received return; repair only a labelled external derived copy, skipping the whole reserve loop and rejecting contradictory raw attributes. Reproduce both original behaviours and pass the two corrected CLI cases, derived integrity,156body/bonus and18API checks. Correct the private inventory walker to retain indirect curve storage; all14families/715files match original source and the successful record is explicit. The failed inventory attempt created only an empty owned output directory and no successful receipt.
- Reproduction: reserve-only zero-requirement E7AB/group38BApayload1, raw[0]*7, gate1, stat7D0zero and absent attribute context yields an original557BA0submission despite native epilogue selection. The repaired result has no reserve events/routes; nonzero raw attributes in the same state return unsupported with reader2FB9B4. The source numerical totals remain zero, explaining old test coverage's blind spot.
- Follow-up: carry the exact patch/regressions into R8; complete all source ordering/lifetime and13additional body writes before claiming full actor reconstruction. Numerical parity, source-route correctness, original preservation, legal/owned domains and full optimizer proof remain separate. No game/CE/Computer Use/native call/product/save changes occurred.

## 2026-10-14 - LFM stored-length flag and literal-tail decoder correction

- Objective: independently read installed PC2.0.2.0 parameter resources while Pro R8 derives final-stat and prepared-attack code.
- Symptoms: the first archive03 extraction stopped at compressed entry3 after three raw payloads; a corrected zlib-only trial stopped at entry523 after523payloads. Neither partial directory contains an accepted complete extraction receipt.
- Cause: the owned decoder first treated the chunk word as decoded length instead of a high-bit zlib flag and low15-bit stored length. It then omitted the valid final raw-literal chunk variant. These were local helper defects, not Pro or game-data defects.
- Evidence: D:/Nioh3_v080_deliverables/deliverables/nioh3-resource-parameter-intake-pc202-20261014/{INITIAL_LFM_CODEC_DIAGNOSTIC.json,parameters-03/,parameters-03-r2/,parameters-03-r3/EXTRACTION_RECEIPT.json,E2E_RESOURCE_REPLAY.json}. The final replay reads952zlib chunks and one literal chunk from1295real stored parameter slices.
- Disposition: preserve both partial trials; use only the successful r3 corpus. Correct the bounded codec, check LFMB/LFMO/load_inf concordance, re-decode through an independent portable implementation, and reject unsupported word bits, invalid zlib, truncated words, wrong decoded sizes, duplicate keys and out-of-range row indices. Installed source files are read-only and full archive bytes are equal before/after packaging.
- Reproduction: item entry235 begins with chunk word0x8A91, whose stored zlib length is0xA91 rather than0x8A91 decoded bytes; entry523 has a final uncompressed8-byte chunk. The first two assumptions fail these real sources; the corrected complete extraction and independent E2E replay pass.
- Follow-up: preserve14whole-table byte identities, the seven-part2954-row optional table and3362-key definition bridge as data evidence. Do not infer loader assembly, indirect bonus storage, final damage, natural legality or owned inventory from these identities. No game/CE/Computer Use/native call/product/save change occurred.

## 2026-10-14 - Native table-only handoff omitted outer acquisition provenance

- Objective: record the exact origin of the native controls and new disk-parameter bridge so future updates can reuse the method.
- Symptom: immutable table-only handoffs contained715table files but not their original outer table manifest, original run identity/profile, or original capture attempts. A later packaging time/PID could therefore be mistaken for the actual table acquisition.
- Cause: the preserved R2 builder copied SNAP/tables/tables into evidence/native-tables, omitting its parent metadata. No table-byte mismatch was found.
- Evidence: original combat-data-intake-20261012/runtime/run-20261013-pid21384/tables/manifest.json, SHA256dd18be2adae4fab68a6866b2382dfbf01d93ede1adae94fab95165b0e514d357; frozen R8 input-r6/project-source/build_pro_package.py; resource-intake method-record/ACQUISITION_LINEAGE.json and provenance copies.
- Disposition: preserve the original manifest, run/profile/attempts/section manifest and base70/power90 source lineage in a separate method record. All715received table files equal the original files and original manifest hashes;17metadata/tool/doc source copies are independently hashed. Keep S1 unchanged. Label current collector source only as a current snapshot because no exact historical source hash was recorded.
- Reproduction: inspect the builder's table-child copy and compare it with the original parent directory; the old package lacks the outer manifest while every carried table byte still matches. Original capture UTC is2026-10-13T02:25:49.044101+00:00, PID21384; later R2 packaging/PID18464 data is a separate epoch.
- Follow-up: carry outer manifests, collector source/version hashes and source-to-copy mapping into future packages, record source acquisition separately from packaging/extraction, and use the documented reusable offline update method. No new live access, formula closure, save/product mutation or compatibility approval occurred.

## 2026-10-14 - Premature restriction of gradient-based equipment search

- Objective: choose a sound future optimization approach for the constrained equipment evaluator.
- Symptom and cause: the recommendation prematurely restricted gradient methods to an auxiliary role based on discrete choices and thresholds, conflating lack of direct derivatives through integer selections with inability to optimize continuous relaxations or a distribution over discrete loadouts.
- Evidence: primary papers Categorical Reparameterization with Gumbel-Softmax (arxiv.org/abs/1611.01144) and Neural Combinatorial Optimization with Reinforcement Learning (arxiv.org/abs/1611.09940); no solver benchmark for this game's evaluator exists yet.
- Disposition: correct the runbook, current handoff, index and continuation guidance. Keep relaxation and policy-gradient methods as eligible search architectures alongside other methods. Preserve exact final constraint/evaluator checks and distinguish search quality from global certificates. Archived acquisition snapshots and S1 bytes remain unchanged.
- Reproduction and follow-up: parameterize a categorical distribution over a fixed finite feasible set; the expected-reward score-function gradient does not require differentiating the game reward. After evaluator/domain closure, compare methods under equal budgets on representative problems and exact tractable references. Discreteness or nonconvexity alone must not select or exclude a solver.

## 2026-10-14 - R8 audit adapters, owned bytecode residue and cleanup time precision

- Objective: independently accept rebuilt R8 arithmetic, preserve immutable inputs and close a bounded core-table acquisition cleanly.
- Symptoms: initial audit helpers failed on end/end_exclusive metadata, a counterexample keyword collision and groups/effect_groups naming. An early body-audit import emitted13new .pyc files in received inputs. The first normal-close guard rejected the task-owned process before requesting close.
- Causes: heterogeneous export/JSON schemas and a helper parameter-name collision; bytecode was not disabled before initial imports; ConvertFrom-Json already returned DateTime and culture reparsing lost fractional timestamp precision. These are owned audit/cleanup tool defects, not Pro numerical or native-data defects.
- Evidence: damage-pro-return-audit-20261014-r8/body-review/AUDIT_FAILURES.md and INITIAL_METADATA_SCHEMA_FAILURE.log; attack-review/HARNESS_FAILURE.md and ARITHMETIC_AUDIT_FAILED.json; source-domain-review/REPORT.md; OWNED_BYTECODE_CLEANUP.json; CORE_GAME_CLEANUP_DATE_PARSE_FAILURE.json and CORE_GAME_CLEANUP.json.
- Disposition: fix the small adapters, retain failed records, rerun bounded independent comparisons and disable bytecode before imports. Remove only13exact new .pyc paths absent from the original ZIP after resolving each under received; do not recursively delete or alter payloads. All12522original file hashes/coverage match afterward. Compare exact process-creation FILETIME, request normal close only for the task-launched process, and verify exit without force-kill.
- Reproduction: the retained original metadata calls fail before accepted results; the early received tree contains extra __pycache__ files; reparsing a DateTime as a culture string changes fractional precision. Corrected body/rank/source checks pass, and launch/capture/current creation FILETIME134365089691015086 matches exactly before normal close.
- Follow-up: use explicit source roots/schema adapters, bytecode-free replay and exact ownership identity in future acceptance. Preserve the approved read-only core capture and positive reread controls separately; no target-memory write, native game call, CE, Computer Use, save/product mutation or arithmetic repair occurred.

## 2026-10-15 - Loadout lab reference, coverage and pattern-construction boundaries

- Objective: implement formula-independent IR, explicit set collaboration and repeatable synthetic CLI benchmarks while native R9 analysis continues.
- Symptoms: early structural validation applied a candidate lock-narrowed option list to the fixed reference. Review also identified that an unknown constraint metric/rank could hide behind an empty candidate domain and receive an infeasibility certificate without being evaluated. A large set-pattern run exhausted bounded construction work before a first candidate. One E2E fixture initially used undeclared native output_type labels, yielding the correct invalid-input rejection and a failed expectation.
- Causes: reference and candidate constraints were conflated; semantic coverage was deferred until a candidate existed; independent per-set bounds missed jointly impossible demand/capacity; the new test's opaque_scalar/opaque_record labels did not match the parser's explicit f32/bytes/opaque type enum.
- Evidence: loadout-optimizer-lab-20261015/review/{adversarial-review.json,final-acceptance.json}, retained e2e-replay-20261015T004152557, worker joint-capacity-probe.json and final-quick/final-standard report/CSV artifacts under the D: deliverables root. E2E vectors include reference score111/140, empty-domain unknown metric/rank refusal, ordered native unknown retention, multi-tag/alias patterns and standard first-witness limits.
- Disposition: preserve counterfactual reference compatibility/instance checks while exempting candidate locks; reject unknown semantics before search; retain opaque fields/native nodes and original source payloads rather than zero; add conservative joint tag deficit/upper-capacity bounds without assuming one set per item. Correct only the fixture labels and rerun the35-case gate. No guessed dominance/top-k or heuristic optimality/infeasibility claim was introduced.
- Reproduction: the recorded analytic CLI cases distinguish each boundary. Corrected gate passes35E2Ecases/67CLI operations, and the200-item set-pattern run completes512candidate attempts within its declared work cap while keeping quality/gap unknown. These are synthetic rules, not native-game evidence.
- Follow-up: keep exact model/domain/certificate scopes and raw acquisition/algorithm selection separate. Require verified native adapters and genuine bound/domain evidence before any native optimality claim; property sampling alone is not a proof.

## 2026-10-15 - Owned pytest bytecode and narrowed cleanup

- Objective: keep new lab test/build artifacts on D: and freeze bytecode-free reproducible source.
- Symptom: early worker imports/collect-only execution generated seven new package .pyc files and one new test .pyc under the checkout. A compound read/delete/list cleanup command was automatically rejected before execution, with only 'blocked by policy' returned.
- Cause: bytecode suppression was enabled after those initial imports; the rejected command combined cleanup with unrelated reads. No more specific policy reason was supplied, so no claim about that decision's cause is made.
- Evidence: lab review/adversarial-review.json, OWNED_BYTECODE_BEFORE_CLEANUP.json and OWNED_BYTECODE_CLEANUP_RECEIPT.json in the task D: output.
- Disposition: subsequent runs disable bytecode before imports, disable pytest cacheprovider and use explicit D: basetemp/output. Root used standalone literal-path Remove-Item commands limited to the eight newly owned cache files; those narrower operations succeeded without Force, recursive deletion or source removal. No broader cache cleanup or override was attempted. Cache files are excluded from delivery.
- Reproduction and follow-up: initial omitted PYTHONDONTWRITEBYTECODE produces ignored caches; final source/benchmark/E2E replay writes no checkout artifacts. Keep environment controls before test-module imports and separate narrowly scoped cleanup from diagnostic command batches.

## 2026-10-15: loadout-lab v2 integration and reporting repairs

- Objective: add actual discrete learning, integer bounds and repeatable experiment
  acceptance without weakening the native-formula boundary.
- Symptoms: malformed list-valued attack_field caused TypeError; forged result
  scope/status metadata was incompletely checked; stochastic/B&B construction
  limits were initially labeled not_applicable; mixed unsupported pairs could
  abort a suite. A final E2E report comparison also included new wall-seconds fields.
- Causes: missing type/scope validation and historical three-algorithm/timing
  assumptions in a five/six-algorithm laboratory.
- Disposition: input/scope validation hardened, capability-aware supported vs
  unsupported denominators implemented, construction limits generalized, historical
  tests explicitly select their three baselines, and replay excludes only timing
  fields while preserving numerical/counter/trajectory equality.
- Evidence: D:/Nioh3_v080_deliverables/deliverables/loadout-optimizer-lab-v2-20261015/
  review/development-pass-2.xml and finalization/review/e2e-final/pytest.log.
  Frozen final acceptance/replay receipts record the subsequent repair gate.
- Boundary: no native formula, game legality, GPU speed or final solver is inferred
  from these synthetic experiments. No product or game state was modified.

## 2026-10-15: R9 source-collector interpreter prestate precision

- Objective: execute ordinary first-pass source collectors from supplied original
  instructions and compare their actual aggregate-map mutations.
- Symptom: direct-entry VM blocked at0x55769D on LEA EAX,[R12-1] after only R12B
  was assigned; higher incoming callee-saved bits were unspecified in the fixture.
- Cause: the interpreter demanded a full address although the native subsequent
  consumer only uses AL. This was not a missing game-memory capture.
- Disposition: explicit varied incoming R12 canaries were supplied at the isolated
  function ABI boundary, including zero, all-ones and high-bit patterns. Collector
  output invariance and original instruction consumers were checked; no helper
  return, source mutation or unknown result was stubbed.
- Evidence: damage-pro-return-audit-20261015-r9/source-frontier/ controls-01 and
  all-effects-01. All7253 subsequent collector cases passed their scoped controls.
- Follow-up: integrate typed source receiver/owner-core alias identity before
  accepting the parent prepared-stage producer. Side-key calls remain unsupported.

## 2026-10-15: bounded slot15 export and interpreter adapter repairs

- Objective: recover81744C out-of-line active/reserve slot15 behavior and replay
  the whole stage, rather than treating the contiguous parent export as complete.
- Symptoms: the first helper export assumed every code target had a.pdata entry;
  small leaf wrappers did not. The first VM replay rejected uppercase Capstone
  memory-width text; the next exposed the previously documented partial-R12 ABI
  precision limit in55764C.
- Cause: export/adapter assumptions, not absent game code or a demonstrated native
  formula discrepancy. The two out-of-line target addresses share one489-byte
  unwind extent; leaf wrappers are separate bounded terminal blocks.
- Disposition: preserve failed directories; export leaf blocks only with an explicit
  option and a256-byte/first-ret-or-jmp bound. Normalize only operand spelling for
  the existing VM, preserving exact instruction bytes. Supply varied incoming
  callee-saved R12 canaries at the direct function-entry fixture boundary.
- Evidence: damage-pro-return-audit-20261015-r9/slot15-source-frontier/
  parameter-helpers (failed), parameter-helpers-02, controls-01, controls-02 and
  controls-04. Final128cases match records, defense, ordinary source poststates
  and exact ordered numeric writes. The no-sync domain is explicit; no whole
  actor, hardware execution or legal item/slot combination is claimed.

## 2026-10-15: stage2 active-source coverage and input validation repair

- Objective: independently verify body source collection and the ordered
  prepared/body/slot15/set/refresh caller using actual nonzero ordinary effects.
- Symptom: the earlier body controls selected effect0/group0, so their claimed
  ordinary source cases returned before submission. The numerical, reserve and
  side checks remained useful, but those cases did not establish active ordinary
  collection. Independent review also found that slot15 byte fields could accept
  None, bool or out-of-range values and bypass the intended native branch.
- Cause: fixture selection admitted sentinel rows; the local slot15 entry omitted
  strict native u8 validation. A separate null-source assumption was too broad:
  the native collector has only effect-specific early returns, not a universal
  null guard.
- Disposition: preserve and explicitly supersede the inert coverage, select real
  effect5F5E/group4AF4 plus cap/max/wrap controls, add strict byte validation, and
  accept null context only when the actual effect path returns before dereference.
  A demanded-null case is refused and separately reaches the native bad read.
  No body numeric formula change or invented source no-op was introduced.
- Evidence: R9audit stage2-independent-review/body-coverage-repair/ACCEPTANCE.json
  and OLD_COVERAGE_DISPOSITION.md;56CLI cases,104submissions(67activeordinary,
  37reserve),203ordinary-map writes; repair-01/RECEIPT.json;slot15controls-06;
  ordered-rebuild-integration/controls-08and FAILURE_HISTORY.json.
- Reproduction: run the preserved old zero-effect case and observe zero ordinary
  submissions, then corrected nonzero duplicate/cap/full-map/shared cases. The
  relocated second checkpoint replays all56corrected body controls and89ordered
  cases with exact source identities and known-byte comparisons.
- Follow-up: require a semantic submission/mutation witness for each advertised
  source path, preserve missing-dependency vs invalid-input distinctions, and do
  not infer a universal native guard from convenient empty fixtures.

## 2026-10-15: guardian ABI fixtures and flat720/index distinction

- Objective: derive guardian numeric/source consumers and supply real unlock and
  synchronization tables for subsequent actor stages.
- Symptoms: early guardian replays stopped on an unexported stack probe, then an
  unspecified GSstack boundary and missing hash leaf. A twelve-byte guardian
  record guess disagreed with actual916694, which addresses eight-byte records.
  One source8508fixture was mutated after native memory initialization. A separate
  complete-flat720join incorrectly required the physical rows to be in native
  upper-bound order and stopped before producing a report.
- Causes: bounded export/environment/input setup gaps and conflating flat storage
  with its separately constructed pointer index. The curated attack-curve JSON
  omitted A865even though all six rows were present in the original complete
  capture. None of these gaps established missing game data or a zero unlock.
- Disposition: retain guardian prefix-01..06failures, execute the actual exported
  stack-probe instructions with an explicit varied GSboundary, correct the raw
  record stride and fixture setup order, and compare116whole prefixes/2,912helper
  cases. Preserve reconcile_curve720_flat_order_assumption.py, record the order
  drops in COMPLETE_CURVE720_RECONCILIATION.json, and capture the actual complete
  337-pointer index rather than silently sorting the storage or inventing a miss.
- Evidence: R9audit guardian-source-frontier/prefix-07/RECEIPT.json,
  actor-curve-runtime-capture/CAPTURE_RECEIPT.json and acquisition scripts. The
  capture also supplies42actual manager8rows. All reader/EXE/process/item-epoch
  and repeated byte controls pass; no game write/call/hook/save/CUA was used.
- Reproduction and follow-up: original916694scales its index by8; old flat720data
  has A865at indices251..256 but is not the native sorted pointer vector. Replay
  future consumers with the captured exact permutation and declared source/GS
  prestate. Guardian constructor execution and full actor remain separate proofs.

## 2026-10-15: curve row geometry explains the old130-row resource gap

- Objective: complete resource provenance for all718 curve parameters without
  inventing coefficients or requiring per-affix gameplay experiments.
- Symptom: the inherited comparison found532 of662 full88-byte row sequences in
  S1payload01119;130did not occur, so a complete serialized reconstruction remained
  open. Manager720's complete flat body also failed direct whole-range matching.
- Cause: actual718 flat extents and native consumers establish84-byte records.
  The old collector included four bytes beyond each record. Different native and
  resource permutations changed the next row ID. Manager720's full12-byte records
  were also permuted relative to the serialized table.
- Disposition: preserve all old captures/reports. Verify every original blob
  against the original manifest, reconstruct all662consumed84-byte718records from
  payload01119, and prove the exact pointer permutation. Verify all661in-table
  overreads against the next physical row ID; retain the last outside-table bytes
  without treating them as row data. Reconstruct all337720records from00194 and
  independently compare the native comparator order. No coefficient was patched.
- Evidence: R9audit curve718-resource-frontier/{reconstruction-01,strict84-controls-01,
  public-catalog-01}/RECEIPT.json and curve720-resource-reconstruction/replay-01/.
  Strict84-byte sample3972and whole-selector11916controls pass; Stage3 matches
  all11916native values and15public CLI cases. The earlier532/130count is reproduced.
- Reproduction and follow-up: compare the native flat extent divided by662 with
  the collector's read size, then consume only offsets00..53. For updates, validate
  consumer width and row multiplicity before diagnosing a payload absence or
  numeric discrepancy. Parameter identity does not establish complete damage,
  source eligibility or equipment legality.

## 2026-10-15: bounded Stage3 lift and harness corrections

- Objective: extend native actor/source components without promoting inert or
  incorrectly modeled paths to positive formula acceptance.
- Symptoms, causes and retained evidence under the R9 audit root:

| Lane | Symptom and cause | Disposition and reproduction |
| --- | --- | --- |
| passive118 | Negative progression predicted two source calls where actual5543AA admitted none; first lift omitted the zero-threshold gate for slots0/1 | Apply actual gate to all slots; retain probe-02 and FAILURE_NOTE.md; accepted732-case matrix includes the repaired branch |
| passive118 interpreter | LEA EAX after an R12B write demanded irrelevant unknown upper R12 bits | Bounded masked carry-aware LEA preserves unknown bits/width; retain probe-03 and independent masked/concrete controls; frozen VM unchanged |
| synchronization | Leaf export budget too short; missing sub alias, operand case and manager/state-pointer conflation blocked initial harness runs | Native byte-bounded exporter derivative and exact disjoint input graph; controls-07 and portable-02 accepted; FAILURE_HISTORY.json retains failures |
| conditional materializer audit | Root recorder expected write.value but the actual retained event used new_bits | Fix the recorder key only; whole819400 and independent27-positive-group replay pass; numerical source unchanged |
| stateful materializer | One-variant smoke chose only null owners; B148 first used nearby3BD4890 instead of actual3BD4080 | Retain inert probe-01 without positive credit; require nonzero changed native writes for every group and correct exact constant; full1400-case gate passes |
| source loop | Whole ATTR read crossed separately mapped regions; first order fixture rebound values while reversing keys | Snapshot mapped intersections with known masks; reverse the same entry multiset; controls-04 and independent review accepted |
| curve718 review | unpack expected48 bytes but received the full captured context | Use unpack_from; controls-03 accepted; model unchanged; independent-review/FAILURE_HISTORY.json retained |
| constructor | Leaf/CRT ISA/service-size/import/RTTI and metadata assumptions failed during bounded discovery | Retain FAILURE_NOTES.md; byte-matched leaf, independent standard-service controls and267486-anchor PE proof close the accepted domain; no substituted game-helper return |
| guardian raw globals | Discovery varied network mode and7310 together, confounding a claimed mode1 frontier | Isolated raw controls show mode1 alone completes when7310=0; quest-table demand depends on actual7310/7314 path; sealed full-caller receipt states the corrected domain |
| post-materializer closure | Windows receipt source keys used backslashes, causing a POSIX-key lookup KeyError after native/public acceptance | Normalize closure metadata keys; rerun hash closure only; numerical and342-case public evidence unchanged; FAILURE_HISTORY.json retained |

- Reproduction/follow-up: each lane preserves its exploratory output, exact input,
  code identity and accepted replacement receipt. These are bounded corrections,
  not new game validity rules. Complete actor/source lifetime, damage and legality
  remain separate open contracts.

## 2026-10-15: transient worker routing failure during an idle interval

- Objective: resume the three authorized Sol/Max research lanes.
- Symptom/cause: worker turns returned401 during the idle interval; the exact
  upstream authorization/routing cause is unknown.
- Disposition/evidence: after the owner's explicit resume, one retry per existing
  lane recovered. No global provider/model/auth configuration was changed and no
  credentials were inspected. Subsequent native/public receipts identify actual
  completed work; interrupted turns are not counted as formula evidence.
- Reproduction/follow-up: transient service state is not reproduced locally.
  Preserve bounded retry behavior; do not turn an unrelated service failure into
  a game-data or numerical conclusion.

## 2026-10-15: full source-reset export and opaque-write recorder corrections

- Objective: derive the complete64AA1C source reset and verify every object byte,
  known mask and ordered store before composing source lifetime.
- Symptom/cause: the old256-byte leaf export budget could not cover8C8840's296
  bytes. The first opaque-prestate audit then tried to parse old_bits=null; null
  is the inherited VM's intentional unknown-data representation, not zero.
- Disposition: preserve partial native-records and smoke-01. Use an explicitly
  bounded1024-byte exporter derivative, retaining original/derived hashes and
  exact source bytes. Add read-only value/mask witnesses around the unchanged
  memory writer; never fill unknown bits. No mathematical model change was needed.
- Evidence: R9audit source-lifecycle-frontier/{native-records-02,smoke-02,
  controls-01,rep-word-01,final-evidence-01}/; complete287 native/freshCLI cases,
  72 REP STOSW primitive controls,405 executed-byte matches and228896 source
  stores pass. This is the complete reset kernel, not complete556118 construction.
- Reproduction/follow-up: use audit_reset.py and finalize_reset.py with fresh
  D: outputs. Independent review and full source-construction/lifetime remain
  distinct. The frozen Stage3 package excludes this later lane.

## 2026-10-15: later native harness and checkpoint-driver corrections

- Objective: retain complete original-byte/source proofs and actually replay the
  frozen checkpoint after relocation, without relabeling unsupported frontiers.
- Symptoms, causes and dispositions:

| Lane | Failure and cause | Retained evidence / repair scope |
| --- | --- | --- |
| finalizer smoke |Harness expected `return_sentinel`; inherited VM reports actual `return` |actor-rebuild-finalizer-frontier exploratory controls retained; correct acceptance classification;85 private/promoted/public envelopes subsequently pass |
| reset independent review |Decimal Capstone operand offsets were parsed as hexadecimal |source-lifecycle-frontier/independent-review preflight notes; recorder-only repair and eight fresh complete native/CLI vectors |
| source families |Private import collision and expected-refusal classification assumptions; raw map graph aliasing was then rejected explicitly |source-talent-families-frontier/EXPERIMENT_NOTES.md; new named modules, exact refusal reasons and one extra alias refusal;148 envelopes accepted |
| source promotion |Import observer expected the CLI filename in sys.modules; actual script module is __main__ |source-talents/promoted-01 retained; observer-only correction and fresh promoted-02; runtime numerical ASTs unchanged |
| guardian world-map |New expected-data reference narrowed raw level65536 to u16 using a different prefix |world-map-extension keeps old proof immutable; exact accepted u32 dense/attack contract in a new expected namespace;113 whole calls pass |
| guardian overlay replay |Code loader read a byte-proof JSON as an instruction export without operands |Strict native-folder/record filtering; fresh complete24-envelope acceptance retained |
| checkpoint draft |Draft path and fingerprint metadata ordering differed from sealed selection |packaging-stage3/FAILURE_NOTE.md; selection/math freeze kept separate from corrected draft metadata |
| checkpoint v1 |Each outer case rehashed entire package and nested caches, making628 outer invocations impractical |Immutable v1 ZIP and29 completed cases retained; explicit performance-aborted receipt; v2 separates628 fresh inner cases from32 actual outer controls with repeated full hash phases |
| checkpoint v1 expected envelopes |24 guardian reference files wrap response and exit_code rather than storing direct response |v2 checks the wrapper exit and complete response; no expected values or runtime changed |
| checkpoint v2 component3 |Sync adapter intentionally raises T.Unsupported with partial_result; replay driver called it without catching that exact expected refusal |Immutable v2 ZIP retained after628 inner/32 outer controls and first two component jobs; v3 draft authorized for strict partial-state/refusal comparison and all11 preflight jobs before reseal |
| checkpoint v3 preflight |Materializer reader imported native_vm before the verified inherited-path bootstrap; two guardian jobs lacked accepted original spans in the package |Preserve first precheck. Root reviewed exact74 historical export additions/23 original.text matches and bootstrap ordering only; all11 fresh second-precheck jobs pass without new dynamic code. Frozen707 model files and628/32/11 scope are unchanged; final extraction acceptance is separate |

- These are harness/reference/driver failures, except the explicit raw-graph alias
  guard, and do not establish formula correctness or invalidate unrelated native
  proofs. No observed numerical mismatch was hidden or converted to success.
  V1 and v2 are incomplete delivery acceptance, never final accepted ZIPs.
- Reproduction/follow-up: preserve each lane's original exploratory namespace,
  exact source identities and fresh replacement receipts. Full formula, source
  lifetime, legal-domain and real optimizer acceptance remain open. The current
  owner Pro suspension overrides the default handoff routing for this work.

## 2026-10-15: masked source/query ISA and HP-producer fixture corrections

- Objective: close new source/query and whole HP-capacity paths without treating
  unknown input or an inert fixture as successful evidence.
- Source7E0: the frozen instruction interpreter required fully known SHR input
  where the actual556363 `SHR R9,32` only needs the resulting bits. A private
  exact-site adapter preserves byte/bit knowledge; literal and completion-based
  native controls establish its domain. OF/AF remain unknown where undefined.
  Evidence: source-progress-7e0-frontier exploratory notes and accepted auxiliary
  controls. Shared VM and historical proofs are unchanged.
- Canonical query: the same issue occurred at2E9B47 `SHR RAX,40`. A separately
  bounded exact-byte adapter with masked/concrete completion controls repairs
  that instruction only. Evidence: player-query-f3-frontier. This does not prove
  any unknown F3 bit false or a current-player instance binding.
- HP actor fixture: first BE108 smoke failed before execution because the
  existing region merger intentionally merges only adjacent read-only regions;
  the new actor tail was writable. After a nonoverlap check, the existing extent
  helper extends the actor, initially unknown, and the complete declared raw
  prestate is written. Numerical model and shared VM are unchanged.
- HP unknown-input audit: an overstrict expectation demanded a stop atBE122 MOV.
  Native execution propagates unknown low32 bits with known-zero upper32 and
  reaches its first exact numerical demand atBE19A CVTSI2SS. The replacement
  audit records both reader and consumption sites with the actual mask.
- HP evidence: hp-notification-frontier/HP_PRODUCER_FAILURES.md retains first
  smoke source and hp-producer-controls-02/frontier-000.json. Fresh controls-04
  passes184 real returns and11 expected raw frontiers;203 actual public envelopes
  and an independent13-native/nine-CLI review pass. No missing data was filled
  with zero and no numerical mismatch was relabeled as success.
- Reproduction/follow-up: replay each named private exact-site/native harness
  using the project wrapper and fresh D: output. Whole source construction,
  dispatcher/HP application, legality and real optimizer remain separate open
  contracts. Pro handoffs remain owner-suspended.

## 2026-10-15: main frame and source278 verification instrumentation

- Objective: connect original main-call frame effects and the accepted278 source
  prefix to actual CLI/graph artifacts without weakening input provenance.
- Hot-getter first audit expected `requested_stop`; the retained VM correctly
  reports `completed_slice/stop_before` at191C964. All first109 hot controls and
  frame checks already passed. Preserve hot-getter-native-01 and its exact failed
  source. The corrected fresh03 proof pins real unknown demand sites, checks all
  original main/prologue bytes and passes114 controls; shared VM is unchanged.
- Source278 actual public verifier passed the first complete CLI envelope but
  rejected its own output/import_observer/sitecustomize.py as a private import.
  The sealed verifier/1,884-file manifest stay immutable. Root's separate
  stage4-public-integration/verify_source278_public.py permits only that exact
  generated instrument path and records its hash, retaining every other runtime
  containment check. Fresh source278-02 passes all84 actual public envelopes.
- Channel-copy recorder first treated partial known writes' `new_bits=null` as
  zero. Null means an opaque full scalar, not zero known bits. The replacement
  step observer reads the real value/mask immediately after each original store;
  it changes no memory or math. Preserve channel-native-01/failed source; fresh02
  and promoted replay both pass115 real block returns/three unknown frontiers.
- A dictionary-metadata syntax typo in the first conditional graph guard draft
  stopped parsing before any graph output. It was corrected directly; fresh
  graph03 binds a real local domain guard and reproduces all418 old ancestries.
- Evidence: main-stack-frame-frontier/{hot-getter-native-01..03,channel-native-01..02,
  hot-getter-graph-01..03,channel-graph-01}; Stage4 public receipts and the focused
  main-context research record bind final artifacts. No numerical mismatch was
  converted to success. Repeat with the named wrapper scripts/fresh D: outputs.
- Follow-up: conditional graph metrics do not prove whole-main damage, current
  context production, complete source lifetime, legality or optimizer readiness.

## 2026-10-15: independently detected channel demand-site defect

- Objective: expose an exact raw-input frontier for the new main context merge.
- Symptom/cause: v1 labeled both missing float operands with source-MOVSS cluster
  start1FF09F8+15*i. Actual masked MOVSS can transport unknown bits; numeric demand
  is MULSS1FF09FD+15*i. For a missing destination, the old label even named a
  source read. The original audit compared blocked status but omitted exact PC.
- Disposition: preserve old module/audit snapshots and all old receipts. Correct
  dependency.rva to the first exact MULSS, add first_required_reader_rva, and
  require equality with actual native next_rva for all three raw unknown controls.
  No arithmetic, masks, aliases or ordered source/destination store changed.
- Evidence: main-stack-frame-frontier/independent-review diagnostic vectors,
  channel-diagnostic-correction-01 snapshots, fresh channel-native-03,
  Stage4 evidence/damage-channel-context-v2, promoted-native02 and actual public
  damage-channels-02. Both native118 matrices and118 actual public envelopes pass;
  the three exact demand PCs are794142,1FF09FD,1FF0AA2. channel-graph-02 retains
  the numerical graph and replaces only receipt bindings/domain provenance.
- Reproduction/follow-up: replay the v2 native/public plans through the wrapper
  with fresh D: output. New APIs distinguish required readers/bindings from an
  actual first exact consumption site; do not claim those labels interchangeable.

## 2026-10-15: end-to-end research method and memory-flow gap

- Objective: assess the owner's concern that local reverse engineering repeatedly
  hits walls without yielding a complete constrained-optimizer scorer.
- Cause: the queue expanded runtime/container reconstruction before fixing a
  final observable and end-to-end baseline. Shared Python arithmetic also meant
  VM/high-level agreement was not independent hardware arithmetic evidence.
  Existing graph pair counts were local scalar diagnostics, not completeness.
- Concrete defect: main-block stores7962D1/+224 and7962F0/+228 precede
  loads796310/796319 and sum/store796322/+218, but inherited memory-load nodes
  have only address edges. The output graph omits the reaching store values.
  Unknown callee alias effects and the intervening CALL stack slot must be
  handled before forwarding; a same-address scalar label alone is insufficient.
- Disposition/evidence: preserve old graphs/proofs and narrow new expansion.
  Three read-only method assessments, output-triage-01 and a focused guarded
  memory-SSA correction are under method-audit-20261015. An isolated exact-byte
  hardware leaf experiment now independently compares115 contexts/230 concrete
  completions (222 CPU calls/eight copy-only controls). No game was accessed.
- Scope: arithmetic sets MXCSR sticky status bits naturally. The current oracle
  receipt records those separately from preserved control bits; the first
  ambiguous metadata label/source and old results remain history. This does not
  establish a complete hit, valid loaded-player input or gear legality.
- Follow-up: NIOH3_DAMAGE_METHOD_RESET_20261015.md defines one complete fixed-
  scenario scorer, independent endpoint oracle and row-level all-affix coverage
  as the next milestone. Existing source268/container/guardian work is sealed;
  further expansion waits for objective-path justification, not a helper count.

## 2026-10-15: old focused REA seed scope omitted new consumers

- Objective: use the existing REA/Ghidra backend for the objective-path consumers
  FBED80/FBEDA0/FC0360 rather than extending manual instruction tooling first.
- Symptom/cause: the first two analyze_function calls reported no procedure,
  and trace_native_values reported no analyzable seed. The reused October13
  focused profile intentionally defined135 selected entries; these three were
  absent. This was neither an engine installation failure nor missing code.
- Disposition: preserve all failed outputs. A new process-local seed file adds
  the three exact byte-backed entries (138 total) and reopens the same immutable
  carrier. All three analysis calls and native value tracing then succeed.
  No installed provider patch, native byte or global configuration changed.
- Evidence: method-audit-20261015/rea/ initial outputs, query-02 seed provenance,
  full successful REA results and acceptance-01/ACCEPTANCE.json. The independent
  prior consumer inspection confirms the504-byte CFG beyond the first unwind
  span. Seeded function names/entries remain declared rather than discoveries.
- Follow-up: inspect operation results/actual data completeness. A successful
  read_bytes envelope returned0/8 bytes with complete=false for uncaptured.data;
  preserve that unknown instead of zero filling. A separate bounded read-only
  native table capture now supplies the actual two8-row bodies. Reproduce the
  retained checks using verify_query_and_tables.py through the project wrapper.

## 2026-10-15: untyped curve ABI removed real main dependencies

- Objective: use REA to connect the two actual-table curve consumers to their
  caller, rather than treating decompiler text as a complete damage formula.
- Symptom/cause: main-input-03 C displayed FBEDA0()/FBED80() and omitted the two
  live sums. Their floating parameter metadata was absent, so those native
  dependencies did not survive the recovered caller representation. The original
  instructions and earlier byte-backed consumer contract retain the arguments.
- Disposition: add precisely three byte/CPU-backed fastcall signatures in a
  private seed script. A process-local Node loader adapter selects that private
  bridge and commits the extra ABI/seed/adapter hashes into the profile. Installed
  provider/global/native files stay unchanged. The initial Windows --import
  launch used a filesystem path and failed before MCP; a file URL corrects it.
  Original transport/error are preserved. No reinstall or setup retry occurred.
- Evidence: rea/main-typed-04/PLAN.md, signatures, typed/untyped main C, adapter
  source/load receipts and acceptance-01. Both typed calls and both summations
  are restored. Independent actual-table CPU comparison passes142 finite vectors;
 18 explicit refusal cases are excluded rather than completed or zero-filled.
- Remaining limit: the larger actual main value trace reports provider P-code
  truncation at3,000 operations, irrespective of a20,000 outer node budget.
  Keep its unknowns/paging scope; increasing only page size cannot restore the
  omitted P-code. No complete-main/alias/final-hit claim follows from this run.
- Reproduction/follow-up: use the task's fresh typed launcher and profile verifier,
  then original-instruction/CPU evidence for exact semantics. These are guarded
  analysis annotations, not automatic function-type discoveries or source labels.

## 2026-10-15: larger MAIN REA export remains partial

- Objective: recover the late MAIN endpoint dependency through actual REA rather
  than assuming the old scalar graph is complete.
- Symptom/cause: the task-private expanded High P-code serializer reaches its
  explicit65536-operation cap (104741 edges), ending before the late796xxx
  endpoint stores. The normal MCP analysis response also exceeds its10MiB budget.
  Provider truncation and response transport size are separate limits.
- Disposition: retain the partial export, its profile and the resource_constraint
  response. Official export_evidence_bundle recovers the retained observation
  ev_d3f920289aaf8e71f66008b443019092b62114ecc1755317a13f6945372451ea
  to a12031336-byte file. This does not recover omitted operations. Continue with
  focused real-path functions and exact input/output dependencies, not another
  unbounded graph expansion.
- Evidence/reproduction: fixed-scenario-e2e-20261015/rea-full-main contains the
  private bridge, FULL_MAIN_HIGH_PCODE.json and RETAINED_EVIDENCE.json. Full-P-code
  file SHA256 is337837ff9bcb227294025071a75165d40fa8e420ba7b871f1440f46bb79cfac8.
  Reproduce with the task-local public launcher/profile; native target unchanged.

## 2026-10-15: combined trigger timeout and failed post-hit supplement

- Objective: retain a coherent raw-build/MAIN-entry/final/actual-HP observation
  without making the owner repeat separate tests for individual affixes.
- Symptom/cause: the first45-second combined window retained eight temporary
  rebuild entries and no MAIN frame; the menu/equipment actions consumed the
  bounded window. It is not a successful hit recording. A later read-only
  metadata supplement returned WinError31 before any blob/thread read; the
  original game process had exited.
- Disposition: retain both partial/failed artifacts. A fresh ordinary-only run
  did capture one actual MAIN entry/actors/final/HP-after sequence, stopped on
  the first matching HP event, and verified removal of owned breakpoints/timers.
  Do not join temporary rebuild owners to the real player or fill missing
  TLS/CRT state from the failed supplement. The owner has left; no new manual
  action or game restart is requested for this metadata.
- Evidence: coherent-live-01/capture.json (4289230 bytes), capture-ordinary.json
  (1753125 bytes; SHA2562e9b32cc0b3f875b6623146e1848d0e478abda9fc7564df212089271302f5b1d),
  COMMANDS.jsonl, EXIT_CLEANUP.json and post-hit-supplement-01/RECEIPT.json.
  observed-oracle-01/ORACLE.json independently extracts MAIN1311 and HP loss1311;
  this is an oracle, not a completed model or accepted full calculation.
- Follow-up: finish cumulative raw-build and objective-path replay offline with
  explicit machine/context contracts. Any truly necessary future capture must
  be ready before requesting a short owner action; no repeated waiting window.

## 2026-10-15: cumulative core cut double-counts a source contribution

- Objective: couple a real raw build to MAIN without retaining captured derived
  STAT/D fields as candidate output.
- Symptom: both actual composed candidates stop at the unknown maxHP read
  78D088/guard78D08F. Native HP-writer analysis then distinguishes generated base
  3469, actual final sourceB1B6 aggregate668 and actual maxHP4137. The current
  conditional replay has source aggregate996 and would produce4465; the extra328
  is a concrete phase discriminator, not numerical tolerance.
- Cause: inlineSTAT(owner+25A8).source(+7D8) is the same pointer field as
  owner+2D80. A later captured core can already contain inline equipment effects.
  377440 copies its post556118 source to the external source before377394 runs
  inline/external equipment collection. Reusing a post-inline source as that
  earlier copy input can collect a contribution again. The prior conditional
  receipts did not prove an absolute pre-collection source phase.
- Disposition: preserve cumulative02 and its native parity controls with their
  partial/unadmitted flags. Do not substitute captured668 or subtract328 without
  an original aggregation/phase proof. Source-factory output is itself not a
  universal nongear baseline; it has real submissions and gear-sensitive reads.
  A correct temporal cut must name its exact phase, aliases and candidate domain.
- Evidence: fixed-scenario-e2e-20261015/build-forward/
  HP_WRITER_PROJECTION_DIAGNOSTIC_01.json, cumulative-02, cumulative-native-04,
  source-lifecycle-frontier/direct-callers-01 and root coupled-gate-01.
- Follow-up: independent phase-cut design plus HP value/read-set projection.
  No live process, Pro handoff or new owner action is initiated by this finding.

## 2026-10-15: frozen MAIN attack input is not candidate coupling

- Objective: ensure a changed equipment candidate changes the actual attack input
  as well as its STAT/D/source state.
- Symptom/cause: the actual reaching read78D222 obtains the attack coordinate
  directly from MAIN+08. Replacing sourceSTAT alone leaves this packet field
  captured and frozen. A zero counterfactual delta would not establish scoring
  correctness. The entry's fully observed return slot identifies current caller
  return3277EB; root verified its direct call3277E6 to MAIN in actual REA bytes.
- Disposition/evidence: retain the original entry-only scene as an observation
  input. Root attack-input-link uses the prior correlated getter/builder/copy
  witness and new actual4F3550/2B4A3C/326C94 REA queries.2934+148=3082 is a useful
  captured equality, not proof of selected-record/override/copy production.
  Regenerate only proved packet inputs; final218 remains oracle-only.
- Follow-up: resolve original selection, overrides, packet copies and timing.
  The builder itself performs a later376C6C restore after its getter and before
  packet stores, so a post-getter core observation cannot masquerade as its
  pre-getter source. A future bounded design records these phases separately.

## 2026-10-15: packet inventory decoder access bits omit MOVUPS stores

- Objective: cross-check native attack-packet stores against direct MAIN field
  reads so candidate coupling does not stop at MAIN+08 alone.
- Symptom/cause: the first bounded second-decoder inventory omitted the known
  2B59AD MOVUPS memory destination, and therefore omitted MAIN+08 despite the
  independently observed78D222 read. Bundled decoder operand-access metadata
  does not report these memory-destination stores reliably.
- Disposition: preserve contract-01 as partial diagnostic. The corrected private
  inventory records original decoder bits and applies Intel-syntax MOV operand
  position semantics only; no native/provider/shared decoder file changes.
  An independent known store/read control requires2B59AD/78D222 to intersect.
  contract-02 reports14 direct-field intersections, including08,0C,14,18 and24.
- Evidence/reproduction: fixed-scenario-e2e-20261015/attack-input-link/
  derive_packet_contract.py, original REA byte/instruction exports and contract-02.
  This remains a conservative direct-base-register inventory, not a whole-callee
  memory-SSA or complete gear-dependence proof. Indirect aliases remain explicit.

## 2026-10-16: R10 assembler encounters a host-only Lua binding

- Objective: make the current native/source/observation continuation portable
  without bundling installed tools.
- Symptom/cause: the first assembly refused the frozen observer's external
  `C:/Program Files/Cheat Engine/lua53-64.dll` binding as unmapped. That binary
  is the host interpreter for the retained Lua mock, not a model input.
- Disposition: preserve the incomplete assembly. Recheck and retain its exact
  size/SHA as an explicitly excluded host-only identity; do not copy the DLL.
  Portable replay verifies the45 included external bindings and the separately
  recorded interpreter identity, and does not claim fresh Lua execution.
- Evidence/reproduction: R10 preparation `assemble_r10.py`, frozen V2 manifest,
  `HOST_ONLY_BINDINGS.json`, and the next fresh assembly/replay receipt.
- Follow-up: actual extracted-directory CLI and immutability checks remain
  required. This packaging failure does not change native formula evidence.

## 2026-10-16: R10 content review finds an incomplete consumer proof chain

- Objective: provide sufficient independent numerical evidence for continuation
  past the actual B0 type/state input gap, not only executable runtime imports.
- Symptom/cause: the first unpublished R10 ZIP passed CRC, extraction and all
  selected math/refusal CLIs, but selected only10 runtime files for the finite
  FBED80/FBEDA0/FC0360 consumer. Its acceptance referenced160 CLI vectors and142
  original-CPU results that were absent. One evidence-map factory path also had
  an incorrect lane prefix. Runtime closure is insufficient research closure.
- Disposition: preserve that draft; include the entire small sealed consumer
  proof chain, original CPU JOB/image/source and exact recorded outputs. Add160
  portable full-JSON/exit replays with142 independently checked recorded CPU
  matches, correct the path, and rebuild/re-extract before delivery.
- Evidence/reproduction: R10 preparation independent review, the consumer
  `ACCEPTANCE.json`/`PROVENANCE.json`, assembler selection and fresh replay02.
- Follow-up: no new game/hardware execution or full-score acceptance follows
  from this repair. Continue the explicit source/packet/MAIN frontier.

## 2026-10-16: returned diagnostic bypasses a paired domain and overstates knowledge

- Objective: independently verify the R10 return's local2451/2451/2430 contrast
  before using it as a candidate-to-packet bridge.
- Symptom/cause: the diagnostic varied prepared banks after one partial BUILD.
  Fresh raw slot1 input under the old fixed reference is actually refused. A
  separate own-reference yields2430, without admitting that original pair. The
  all-FF record wrapper also upgrades12unwritten padding bytes to known.
- Disposition: retain the return unchanged. The new working research copy runs
  four raw partial BUILDs, preserves the refusal/separate-reference distinction
  and propagates actual seed masks plus ordered stores. Local values stay the
  same where admitted; eight public CLI E2E checks and new independent local
  CPU controls pass. No complete actor/phase/packet/damage admission follows.
- Evidence/reproduction:20261016-r10 local audit, candidate-review/paired-02,
  working-replay-01 and selector-cpu-01. Native helper fixture omission remains
  an explicitly skipped auxiliary control, not independent818248 acceptance.

## 2026-10-16: R11 selection accidentally includes an execution cache

- Objective: keep the next self-contained handoff minimal and evidence-complete.
- Symptom/cause: copying an entire working-smoke directory selected a redundant
  materialized original-input cache. Content review found it before delivery.
- Disposition: select only the86sealed review files and top-level smoke evidence;
  preserve the original generated draft and create a fresh versioned package.
  A proposed generated-draft cleanup was rejected by automatic approval review
  with only `blocked by policy`; it was not executed or retried through another
  tool. Retaining the draft avoids destructive cleanup and does not block work.
- Evidence/reproduction: R11 preparation/source-selection manifests and final
  v2 source root. A later prompt-only123-byte range correction does not alter
  numerical code; final ZIP extraction/replay is required after resealing.

## 2026-10-16: live V3 rotation meets deferred CE breakpoint removal

- Objective: collect a phase-correct core/builder/getter/packet/MAIN/HP chain
  while the owner performs one equipment change and two ordinary attacks.
- Symptom: the first actual-player builder input was captured, but rotating its
  currently executing hardware breakpoint failed with `Owned removal deferred;
  phase cannot advance`. The observer stopped after one callback/event, before
  a complete hit. This was a collector failure, not an owner action failure.
- Cause: the live CE breakpoint remains inventoried until the callback resumes;
  the synthetic successful-removal mock removed it immediately. The old deferred
  control tested refusal, not successful in-callback transition on this backend.
- Evidence: damage-pro-return-audit-20261016-r10/coherent-phase-v3-live-01/
  `capture-partial.json` SHA6045f85c585d6124a500f47bdf2ce94f22d6d319853b9e54561d27684ef51fad.
  Root stop/export and final stdin cleanup prove no remaining owned/global
  breakpoints and a running debugger. No game function, save or target-data write.
- Disposition: keep the frozen V3 and failure intact. A separate static four-site
  runtime retains CORE/MAIN-entry/MAIN-final/HP throughout the run. Normal and
  deferred-removal Lua E2E controls pass. The first60-second static window has
  57callbacks/zero accepted events, no error; action timing/filtering cannot be
  inferred from that absence. V4 adds bounded per-site diagnostics and changes
  only the wall window to300seconds, retaining the old callback/raw/event limits.
- Follow-up/result: V4 captures one positive861hit in12callbacks, with entry,
  final and actual HP checkpoints, observed MXCSR and both STAT/D roles. Final
  cleanup is verified. No core, getter or packet-producer sequence was captured;
  those remain open. A rotating collector is not accepted on this CE backend.

## 2026-10-16: indexed curve equation incorrectly defaults every missing key to zero

- Objective: independently compare the complete552890/D24860/clamp path against
  the retained `Curve720.value` equation, including absent keys in the full index.
- Symptom/cause: all1326valid-key vectors match CPU bits, but internal missing
  A785returns5.0for INT_MIN/0/31/INT_MAX while the equation returns0. Native
  whole-vector search/wrapper control flow can select a successor-group row;
  an independent by-key missing-value default erases that behavior.
- Evidence: `offline-forward-s1/curve-cpu/run-01/{JOB,HARDWARE}.json`, full native
  original bytes and model-comparison/run-02. All1358vectors are retained:
  1354matches/four mismatches. Four direct original-VM controls reproduce the CPU
  bits with known masks; the clamp body is present but not reached on those paths.
- Disposition: do not mutate the sealed model or remove counterexamples. The
  narrow validA784caller bridge remains within its checked domain. R12 asks Pro
  to repair and falsify the exact indexed contract before broader integration;
  no automatic equivalence to the separate FBFD40flat-selector path is claimed.
- Reproduction/follow-up: portable explicit-root comparison script, CPU job,
  classification hashes and native controls are packaged. Full damage/source/
  packet/legality readiness remains false.

## 2026-10-16: R13 status collector's captured-tree shape was not validated

- Objective: validate the proposed read-only status input extension before live
  use. The supplied12graph/256tail tests passed under actual Windows Lua5.3.
- Symptom/cause: six malformed child/parent/first-node controls were marked
  complete. Deduplication of left/parent/right edges hid corruption; valid parent
  backedges must still be supported. Independent result was14/20.
- Disposition: preserve the supplied ZIP/source. Validate the already captured
  child graph, reciprocal parent links, unique root and first/minimum relation
  in an owned copy. Original12+256and independent20/20controls pass with identical
  actual read traces and no extra reads/sites/budgets. Freeze all13runtime hashes.
- Evidence/reproduction: damage-pro-return-audit-r13-20261016/collector-review/
  FINAL_REVIEW_RECEIPT.json, RUNTIME_FREEZE.json and REPRODUCE.ps1. A preliminary
  independent decode at78F158was inside the78F155LEA; its failed draft is retained
  and replaced by23corrected native checks. The final live run captures an empty
  target tree/list21with no prefix unknowns and verified cleanup. Entry snapshots
  still do not prove consumer-time atomicity or raw-equipment production.

## 2026-10-16: protected disk code is not postloader instruction evidence

- Objective: reconstruct the demanded readonly vtable slot38FE810from disk PE
  DIR64 metadata instead of requesting another gameplay capture.
- Symptom: a preliminary guard comparing reader9819F9disk bytes with the runtime
  JMP rejected. The installed executable's protected .text differs from the
  approved postloader .text. A separate raw MAIN78F260window also began inside an
  instruction; it is retained as a rejected inspection draft and not packaged as
  native instruction authority.
- Disposition: keep disk PE headers/raw.rdata/relocation block as static pointer
  sources, and the approved postloader .text/carrier as executed-code authority.
  One uniquely typed DIR64slot resolves to555B00and agrees with a same-process
  postcapture PAGE_READONLY/MEM_IMAGE8byte sample. No arbitrary.rdata fallback,
  heap value, target call or write is introduced.
- Evidence/follow-up: R13 audit/static-rdata-forward/one-cell-02and its final
  source-status binding. Only that cell is admitted. The next source-side
  status-tree payload remains unknown; this is calibration support, not a
  prerequisite for the native candidate evaluator product milestone.

## 2026-10-16: generic handoff validator skipped nested input manifests

- Objective: preserve the R13 input as a flat byte-identical directory in R14.
- Cause: the generic file collector excluded every basenameSHA256SUMS.txt,
  although only the outer manifest is self-referential. A nested input manifest
  must itself be an ordinary hashed payload.
- Disposition: exclude only root/SHA256SUMS.txt. A public-CLI E2E accepts a valid
  embedded manifest and rejects its mutation; all seven existing/added checks
  pass. Actual R14ZIP validation covers all799members/798payloads, including the
  preserved R13manifest. Product code and earlier immutable archives are unchanged.

## 2026-10-17: R14 native metadata typed binding

- Objective: verify native effect-instance raw-slot binding before integration.
- Symptom/cause: equal-valued floats and Booleans were accepted as uint32metadata
  by Python tuple equality. Three independent CLI controls reproduced false
  binding witnesses. Full native queries still refused scoring.
- Disposition: preserve the return and apply an exact non-Boolean uint32guard
  only in an owned provider copy. The original controls give6/9; the patch9/9.
- Evidence/reproduction: damage-pro-return-audit-r14-20261017/registry-review/
  REVIEW.md, REPRODUCE.ps1, return-controls-01and patched-controls-01receipts.
- Follow-up: incorporate the diff with explicit repository provider/evidence
  relocation. Do not copy delivery-only paths unchanged into the repository.

## 2026-10-17: semantic decoder prototype identity joins

- Objective: expose real saved-capture items and profile through a stable
  observation interface, without promoting observations to candidate scores.
- Symptom/cause: the first draft checked top/raw identity but omitted duplicated
  arming/PID identity and the actual current-player global chain. Its snapshot
  sequence also lacked a corresponding event association. A consistent foreign
  owner/backlink or edited sequence could therefore be presented as player epoch.
- Disposition: require lifetime/module/arming agreement, same-snapshot manager,
  player/source/owner reciprocal pointers and unique matching MAIN-entry event.
  Keep separate E8 owner records and pool records; unknown fields remain null.
- Evidence/reproduction: semantic-review/CHECKS.json and FINAL_EPOCH_CHECK.json,
  plus the public CLI negative
  controls in semantic-e2e-03under the R14 audit. Early semantic-state-01and
  semantic-e2e-01/02are development artifacts, not the final accepted source.
- Follow-up: new layout/adapters require the same typed identity/epoch boundary;
  this is partial offline translation, not live or numerical scorer acceptance.

## 2026-10-17: R15 returned result identities depend on host path separators

- Objective: reproduce the returned source's public complete verifier on the
  owner's Windows host after its successful Linux delivery run.
- Symptom: all public component/research/native commands completed, but the
  final RESULTS.core comparison failed. The13core differences were proof/model/
  assignment identities, not metric values. Comparing the two underlying proof
  objects found18native artifact paths per candidate using Windows backslashes.
- Cause: native provenance and implementation identities serialize
  str(path.relative_to(...)) instead of a canonical package-relative path.
- Disposition: keep the original ZIP/extraction unchanged. In an owned working
  copy, use as_posix for protocol path fields, retain original delivered results,
  and regenerate identity-dependent reports from actual repaired execution.
- Evidence: damage-pro-return-audit-r15-20261017/provided-replay-01,
  RESULTS_CORE_DIFFERENCES.json, PROOF_CONTENT_DIFFERENCES.json and repair/.
- Follow-up: the final acceptance receipt and Claude handoff own repaired replay
  status. Do not report the original Windows verifier as passing or erase its
  failed run; code-identity changes require regenerated reference artifacts.
  The repaired full verifier now passes; all numerical/proof/domain fields are
  exact, with only three legitimate changed compiler/assignment identities.

## 2026-10-17: R15 research return aliases shared producer cache

- Objective: verify repeated provider research calls preserve independent
  candidate state and evidence when previous results remain caller-owned.
- Symptom/cause: mutating a returned cache_zero_projection or ordered_submissions
  changed a later candidate's returned source evidence. NativeProduction.run_default
  returned references to its cached nested lists/dictionaries.
- Disposition: an owned-copy deep copy at the return boundary prevents both
  leaks. The same mutation/reuse test passes and numerical outputs remain equal;
  the immutable Pro source and fresh-process published results are preserved.
- Evidence/reproduction: damage-pro-return-audit-r15-20261017/bridge-review/
  RECEIPT.json, defensive-copy.diff and its independent test script.
- Follow-up: include this narrow fix in the accepted working copy before Claude
  continues; this is a reusable API correctness issue, not full damage closure.
  The fix is applied in the owned working copy and its complete public replay
  passes without modifying the sealed Pro source.

## CE launched by the test wrapper lost its scan temp directory (2026-10-17)

- Objective: start Cheat Engine with the ce-mcp plugin for the R16 live hit capture.
- Symptom: CE showed "The temporary scan directory D:\Nioh3_v080_deliverables\tmp\pytest-...
  does not exist" and autosaved .tools/cheat-engine-mcp/ExceptionAutoSave_load_ce_mcp.CT.
- Cause: ensure_ce_mcp.py run through tools/run_python_tests.ps1. CE inherited the wrapper's
  per-run TEMP, which the wrapper deleted on exit.
- Disposition: recreated the exact directory, then deleted the ignored autosave. No game
  memory was touched; the first driver run stopped before attach.
- Reproduction: launch CE through the wrapper, then let the wrapper exit.
- Follow-up: launch CE with a persistent TEMP under the build root, or have
  ensure_ce_mcp.py pin CE's scan directory.

## R16 offline continuation: bounded diagnostics and repairs (2026-10-16)

- Objective: reproduce the helmet from S0 plus its standalone native record,
  harden the next collector, and preserve genuine offline frontiers. Evidence
  root: F:/Nioh3_r16_build/deliverables/codex-r16-offline-20261016/.
- Body-only STAT initially predicted +57C=4 rather than observed 9. The omitted
  later A992/D518/value5 source materializer adds 5 at native81F2B5. The retained
  original comparison is stat-provenance/evidence/COMPARE_HELMET_INITIAL.json;
  the repaired 18-field/70-byte comparison passes. Blind new-item validation
  remains pending. See stat-provenance/FAILURE_LEDGER_NOTE.md.
- The first item E2E reproduced 714/714/720 but detected a producer source change
  during execution and correctly failed its before/after pin. Original
  evidence/item-e2e-01 is retained. Frozen-source E2E02 passes all four native
  and readable-formula vectors with unchanged source/input identities.
- The first new collector E2E confused raw event MAIN_entry with snapshot kind
  MAIN_entry_prestate, and rejected empty Lua tables serialized as {}. No game
  was used. evidence/collector-e2e-01 retains the refusal; repaired E2E02 passes
  the normal hit, timeout and partial-arm cleanup controls. Unknown/absent
  breakpoint inventories remain refused.
- S1 actual-frame native replay first stopped on a static curve-context BSS
  pointer that the old fill metadata classified using on-disk image size.
  Only exact audited initialized-BSS curve/default-D cells were admitted with
  their same-process post-hit caveat. In evidence/swap-replay-02 the readable
  formula matches 793 and720; native replay still stops at78C9A9 reading
  1C62A033D30, a missing dynamic pointer. No other hit's dynamic state was copied.
  Follow-up: capture the exact consumer chain at MAIN entry in a fresh process.
- The retained swap driver STATE ends runner exit1 despite five complete
  frames, then says disarmed. Its EXIT_CLEANUP still has cleanup_pending and
  four breakpoint addresses. buff-provenance/INHERITED_CAPTURE_FAILURE.json
  preserves the inherited proof gap. Current process state was not inspected;
  future live work must prove clean fresh ownership before arming.
- An initial fresh-cache umbrella verification completed all item vectors,
  then the mock collector correctly refused OutputRoot outside its narrower
  configured BuildRoot. The wrapper now validates that boundary up front;
  codex-r16-offline-validation-01 retains command diagnostics. Final verification
  uses one consistent build/output boundary. No build/test temp used C: or D:.
- Focused review found two additional admission gaps outside the retained
  vector: unknown oracle bytes could inflate a comparison count, and the
  standalone STAT CLI did not validate capture identity/single-entry uniqueness.
  Both gates were hardened; CLI negative controls preserve the original inputs
  and refuse unsupported identity, ambiguous entries/roles, extra tail affixes
  and incomplete oracle masks before native replay.

## R16 live passive capture and provenance repairs (2026-10-16)

- Objective: identify checked Common/Samurai/Ninja passives and reusable native/
  version/analytics contracts. Evidence root:
  F:/Nioh3_r16_build/deliverables/r16-live-passives-20261016/.
- The30-second source window recorded zero callbacks while the owner was
  unavailable. It is a zero-event timeout, not an empty effective set. First
  stop proof retained four pending owned addresses despite inactive timer.
  Deferred removal/reconciliation timing was observed; exact subsystem cause
  remains unknown. Independent exact-session/PID-birth recovery found the
  global inventory already empty and reconciled the inactive owner. Original
  receipts remain unchanged; OWNED_ADDRESS_RECOVERY and FINAL_CE_DETACH/
  FINAL_RESOURCE_CLOSURE prove later cleanup. Driver reconciliation is now
  bounded after reconnect. The task-created helper closed; game remained open.
- First source-observer mock harness failed to inject context_failure. Corrected
  harness passes22 actual Lua53 cases; original mock-e2e-01 and its failure note
  are retained. This was not a game/CE failure.
- Legacy text RVAs failed identity. A48-byte old UTF16-copy prefix had zero
  matches in a bounded128KiB neighborhood; one historical allocation hint failed
  its4KiB structured-prefix control. Both routes stopped. Current UI consumers
  instead prove5A4B04/63D9C4/63DDA0 and441 exact native text-ID lookups.
- The first root text wrapper read20 bytes for a21-byte signature and refused
  before dictionary lookup. Width now derives from the signature; fresh active02
  and all01 captures pass without calls/writes.
- Root review found crosswalk04 used source598+84 as cost: F52D incorrectly0
  versus independent5. Names/keys remained correct. Correct source130.f32+84
  now carries197-row byte provenance; source598+84 remains unclassified raw data.
  Crosswalk05 asserts all25 costs against native UI order and screenshots.
  Five CLI controls pass, including four cost/capacity mutations. Old04 and
  inputs remain unchanged; see COST_FIELD_FAILURE_LEDGER_NOTE.md.
- The portable migration wrapper's60-second subprocess limit was too short for
  the full existing relocation control. No game/CE was involved. The repaired
  bounded300-second wrapper reports progress and passes49 identical anchors;
  data roots/product/new-version acceptance remain unapproved. Failed output
  is retained separately from the successful second control.
- Two visual glyph transcriptions were corrected from direct native text
  (F52D叁,20DE冻霞); original images/transcriptions remain intact. An unmanifested
  generated Python cache moved to a bounded F: backup without permanent deletion;
  the old918-file sealed package revalidates all917 payload hashes unchanged.

## R16 owner-return source-arm fatal error (2026-10-16)

- Objective: verify actual passive-source binding before controlled Counterattack
  Deflect /1.1-status duration and refresh collection. Root alone owned the CE
  session. Evidence: F:/Nioh3_r16_build/deliverables/r16-counterattack-live-20261016/.
- Fresh PID35932/birth134366201932746665, executable and loaded.text hashes,
  839-node read-only snapshot and all four native site signatures passed. The
  initial global breakpoint inventory was explicitly empty. The new task-created
  approved CE helper was PID2452/sessionce-2452 onport5566.
- During source observer arm(150,2048,4096), the bridge's Lua request timed out
  after30seconds. No ARM_PROOF or ARMED checkpoint was received. The owner
  supplied the game's fatal0xC0000005 dialog at00000148C85418F9. The game was
  absent at the first subsequent OS process query. No combat/form-change
  instruction had been issued. No game-function call or code/state patch was
  requested by this run; debugger initialization can introduce target-side
  machinery and is not equivalent to OS-only reading.
- Exact fault cause is unknown. The last observed boundary precedes the arm
  call, which includes debugProcess(2), hardware-slot installation and a
  synchronized timer. No individual substage or exception context was returned.
  Prior Lua mocks and a zero-callback live idle phase do not prove callback
  lifetime, VEH attach or GUI-thread reentrancy safety. Suspected hazards stay
  hypotheses in the bounded offline review; no repeated live attempt is allowed
  by this result.
- The original arm and cleanup failures are preserved. Exact-session reconnect
  could query stale attachment35932, but a pure Lua cleanup query also timed out.
  The old target's breakpoint cleanup remains unverified at process exit.
  Root verified the created helper's image/PID/start-versus-connection epoch,
  confirmed the target absent, closed only CE2452 and saved independent closure.
  This does not retroactively turn failed target cleanup into a pass.
- Disposition: suspend this debugger route. Prepare a separate QUERY_LIMITED_
  INFORMATION|VM_READ-only collector with monotonic sampling, bounded tree reads,
  explicit uncertainty and process-epoch validation. Ask the owner to restart
  only after helper closure; preserve source arm as a failed control. No new
  buff causal, duration or refresh conclusion is admitted from this attempt.
- Reproduction: do not repeat the live failure. The pinned observer, contract,
  source hashes, command journal, owner image, CAPTURE_RECEIPT,
  FAILED_ARM_HELPER_RECOVERY and FAILED_ARM_HELPER_CLOSURE are retained for
  isolated review. Follow-up: OS-only controlled transitions and a separate
  debugger lifecycle/exception-context investigation before debugger reuse.
- A90-second refresh window ended with no transition before the owner's two
  successes were confirmed. Actual action/window overlap is unknown, so that
  window is retained as uninformative and excluded from negative/refresh
  controls. A fresh180-second window captured the two-success extension.
  The fragmented protocol consumed too much owner effort. Owner direction is
  now batch native source/class analysis and representative shared-mechanism
  controls, with no further manual actions requested in this study.
- One generated native-review Python cache was moved intact into the explicit
  F: task backup before sealing; source and original capture bytes were not
  changed. Replay uses frozen source pins and records bytecode hygiene rather
  than treating generated cache files as research evidence.

## R16 direct native buff analysis repairs (2026-10-19)

- Objective: actually run official REA/Ghidra on the retained postloader code.
  Symptom: initial doctor/native authority on F: failed Win32 error87. Cause:
  F: is exFAT; Windows REA requires local NTFS target/runtime authority. Evidence:
  F:/Nioh3_r16_build/deliverables/r16-native-buffs-20261019/rea/DOCTOR.json and
  DOCTOR_NTFS.json. Disposition: explicitly stage immutable target/tools/private
  runtime under D:/Nioh3_v080_deliverables/tmp/r16-rea-native-20261019/, preserving
  canonical evidence on F:. Reproduction: do not repeat the unsupported F:
  runtime; actual NTFS sessions decompiled136 unique functions and closed.
  Follow-up: keep filesystem admission in the update runbook; no security
  bypass, global configuration change or game attachment was needed.
- Objective: prove a shared mixed-class damage pool on a retained full MAIN
  entry. Initial type132+20% expectation incorrectly predicted1.3 with type0.
  Native MAIN instead gave1.1/805 because the type132 consumer gate at78F1AE
  was false through the baseline78CE20 branch. Evidence: stacking/
  RESEARCH_CORRECTIONS.json, WHOLE_MAIN_PROOF.json and whole-main/ negative
  controls under the same native evidence root. Disposition: retain negatives;
  use admitted type172 controls,879 alone versus952 with type0. Reproduction:
  stacking/REPLAY.ps1 preserves both results. Follow-up: derive applicability
  from native gates, not a matching aggregate offset alone.
- Objective: execute full original A648C over a synthetic ABI. Prototype wrote
  beyond its mapped stack at the first instruction because entryRSP was too
  near the region top. No game was accessed. Evidence: stacking/
  RESEARCH_CORRECTIONS.json. Disposition: use alignedSTACK+F008 and complete
  caller home space; accepted original-CPU controls pass. Reproduction: repaired
  REPLAY.ps1 no longer fails; follow-up is explicit ABI/known-memory admission.
- Objective: serialize the all-descriptor original-instruction dispatch catalog.
  Initial CLI raised AttributeError: W has no report method. Cause: wrapper
  used a nonexistent inherited VM API. Evidence: catalog/dispatch-batch-01 and
  FAILURE_NOTE_FOR_ROOT.md. Disposition: serialize value/known/width explicitly.
  Reproduction: accepted catalog/e2e-batch-01 executes all positive commands and
  six negative controls with47 unchanged pins. Follow-up: preserve the failed
  folder; no native conclusion depended on its missing output.
- Objective: attribute runtime status sources. Earlier OS collector labeled
  node+22 as source_metadata_u16. Native insertion/poison controls now prove
  node+22..27 is untouched padding on that route; using it as source identity
  was a semantic error. Evidence: stacking/natural-scope/
  NATURAL_SCOPE_AND_PADDING_PROOF.json. Disposition: preserve sealed raw data,
  deprecate the legacy field and use status+74 with native caller/table joins.
  Reproduction: original insertion preserves99ABCCDDEE7F and rebuild reads none
  of those six bytes. Follow-up: new analytics contracts distinguish presence,
  suppression, effective contribution and actual source identity.
- Objective: independently accept actual REA outputs. The first verifier rejected
  composed batch providerrea-workflow; a second rejected raw seed SHA against
  REA's canonical seed commitment. Causes: verifier assumptions, not target
  mutation. Disposition: verify the explicit upstream Ghidra profile and official
  lowercase-address/LF canonical seed representation while pinning raw bytes
  separately. Repaired rea/REA_NATIVE_ACCEPTANCE_01.json passes carrier/original
  section parity,136 exports, two independent instruction comparisons and owned
  process closure. Follow-up: retain both hash meanings in recovery procedure.
- Objective: independently derive STAT1AC from its synchronized attribute/table90
  producer. Initial proof slice continued past the target store into an unrelated
  manager reload and refused unknown RSI after producing the correct value.
  Cause: an unnecessarily broad slice boundary, not missing producer arithmetic.
  Evidence: catalog/stat-1ac-proof-01 and STAT_1AC_PROVENANCE.md under the native
  evidence root. Disposition: end at exact target-store boundary819056; retain
  failed folder without inventing RSI. Reproduction: stat-1ac-proof-02 passes12
  seed/clamp plus36 late-D controls and independently matches selected6CE78B3F.
  Follow-up: full actor reconstruction and core's uncaptured source remain
  separately scoped; no game operation is needed for this selected proof.

## R16 clock continuation: virtual-table boundary and pointer lifetime (2026-10-19)

- Objective: recover the producer of frame_context+10 rather than infer units
  from observed slopes. Earlier analysis treated a contiguous ordinary-pointer
  run beginning400D8F0 as one vtable and assigned an alternate update slot108.
  Cause: contiguous pointers do not prove a C++ table boundary. New native LEA
  setters authenticate normal400D908/constructor1C2CCEC and alternate400D9C8/
  constructor1C2CB8C; their relevant methods are slot30. The old53-function108
  census is candidate-only. Evidence: r16-native-buffs-continue-20261019/clock/
  registration-* under the F: delivery root. Disposition: preserve old sealed
  evidence, correct current routing, trace actual scheduler registration and
  frame writers. No extra gameplay or debugger attempt was required.
- The root's exact scheduler+1A0 pointer capture was stable but its pointee held
  repeated stack bytes, not a plausible active timer record. Native458A4 later
  proved the frame record is stack-allocated and the global can retain its
  pointer after return. Reproduction is the preserved frame-context-os-01 raw
  capture plus the native frame-constructor chain. Disposition: grade it only as
  a raw current-slot observation, never a physical-unit/FPS value; trace the
  stable application inputs and full callback lifetime instead. OS handles
  closed; no target writes or game-function calls occurred.

## R16 continuation: repaired bounded harness and capture failures (2026-10-19)

- Objective: run creation preparation for all406 source definitions. The first
  resumed interpreter retained its deliberate stop latch at3F0324 and reported
  401 unavailable rows without a missing input. Cause: harness phase control,
  not native dispatch. The repaired driver clears only that latch while keeping
  registers, flags, memory and call frames. Evidence: the continuation root's
  dispatch/prepare-all-01 and preparation-e2e-01/E2E_RECEIPT.json. The accepted
  rerun reaches399 actual factories and seven nativeNULL outcomes, with17
  controls and five unknown-input refusals. Constructor assignment initially
  also lacked a declared ABI stack; the accepted12 assignment/getter controls
  now provide it. Neither repair supplied a native helper return.
- Objective: follow an optional short-frame job registry. Root's initial plan
  failed to honor a native empty-vector predicate and followed its sentinel.
  The canonical-address/geometry guard refused before an invalid pointee read.
  Evidence: evidence/short-frame-job-os-01 and PLAN_SHORT_FRAME_JOB.json under
  r16-native-buffs-continue-20261019. The OS handle closed. Disposition: preserve
  the failed capture; newer plan versions enforce nonzero predicates and vector
  bounds. The actual main SHORT frame route was independently closed through
  5AC664 and does not require this empty optional registry.
- Objective: discover the normal-state virtual dispatch. An initial candidate
  scorer applied its decode-byte budget after decoding and inspected1,518,703
  bytes before exporting24,357 useful bytes. Cause: budget placement. The
  repaired scanner caps candidates before decoding; exact downstream evidence
  came from564F40/564F60 and the genuine565170 slot30 dispatch. A separate field
  writer census correctly refused a256KiB candidate budget before full-body
  decoding. Evidence: clock/FAILURE_NOTES.md. No broader census was repeated.
- Objective: execute complex numerical handlers. Unaccepted fixtures initially
  put the type87 bit54 selector at the wrong structure offset and treated a
  type85 native0.5 constant as1. Both were corrected against original bytes.
  A type22/72 registry fixture also ended before the original inline16-byte
  vector at definition+80; this was a fixture boundary, not a missing pointer.
  Accepted pass02/pass03 and pass04 receipts retain exact knownness controls.
  No live game or sealed prior evidence was modified.
- Objective: decompile new REA entries. BATCH_CONTINUE02 explicitly requested
  six functions absent from that session's seed set and received invalid-input
  refusals. Four other entries succeeded. Later explicit seed batches record
  the repaired17/17,28/28 and27/27 requests separately. Cause: analysis metadata,
  not a native failure. Evidence: continuation rea/BATCH_CONTINUE02.json and
  SEED_PLAN_02..05.json. Do not count refused entries as decompiled or numerical
  closure; source-byte controls remain the authority.

- Objective: identify the HP-side callback after type146 effects. Frozen pass03
  described66C188 as event88 dispatch. Original B93F00 and its call ABI instead
  show that88 is a byte count for initializing a descriptor buffer. The later
  376C6C call is broad HP-derived-state/source recomputation, also reached by
  ordinary A56A4/3FC5CC paths. Cause: semantic mislabeling of a literal argument.
  Evidence: continuation handlers/pass-04 native helpers, HP write-prefix
  controls and explicit erratum. Disposition: preserve frozen pass03 bytes and
  supersede the event label; five original clamp/write prefixes and unknown-cap
  refusal stop at the real constructor call. Full recomputation/event identity
  is not inferred from the number88. Current CRT policy byte02 was captured
  read-only so the original memset path could execute without a return stub.

## R16 continuation: contracts and interpretation repairs (2026-10-19)

- Objective: reproduce current passive admission through the actual CLI. The
  first current-source wrapper compared textual byte `01` with `0x01` and
  rejected a numerically valid field. Cause: representation comparison in the
  harness. Disposition: compare the parsed integer; current-e2e-02 passes27
  checks, including394 original gate cases and unknown-input refusals. Evidence:
  continuation passive_sources/current-e2e-01..02 and FAILURE_NOTES_FOR_ROOT.md.
- Objective: exercise V3 collector rejection paths. An intermediate E2E passed
  its functional controls but failed while decoding PowerShell output using
  the host default encoding. Cause: text transport in the test driver. The
  repaired driver saves raw logs and decodes UTF-8 explicitly. The17-case
  linear-e2e-03 receipt is accepted; intermediate outputs remain preserved.
- Objective: collect two native resource30 rows. The first parameter-source
  batch interpreted bare native labels10158/18706 as decimal instead of hex.
  It refused absent keys before reading a selected row and closed its OS
  handle. Correct keys are0x10158=65880 and0x18706=100102. The original failed
  script remains in parameter-source-batch-os-01; batch-os-02 captures exact
  hash-selected rows and final stable brackets. No native value was fitted.
- Objective: follow cached asset buckets through the bounded OS-plan executor.
  An agent-produced plan used unsupported `address` fields. The actual v3
  consumer refused `pointer_from` lookup after roots/header, before any bucket
  read; the handle closed. The repaired producer uses supported vector_row and
  stable pointer chains. It is checked against the actual consumer using
  file-backed transports, then executed read-only in asset-cache-buckets-os-02,
  nodes-os-01 and vectors-os-01. All raw failure and repair artifacts remain in
  handlers/pass-08 and evidence/asset-cache-buckets-os-01..02.
- Objective: prove equal-strength mode0 status insertion from genuine1AE/31
  fields. The first root harness omitted the native7A0 parameter context used
  by2082BB4/2085804/78CC90 and stopped at its missing global. Cause: incomplete
  fixture mapping. The repaired harness maps only hash-verified retained
  context/payload/hash bytes and exact consumed status fields; nine original
  controls pass in equal-strength-insertion-02. No helper return is replaced.
  Heap allocation/destruction remains an explicit separate slice boundary.
- Objective: identify the current action for combat statistics. V3 retained
  the legacy name current_action_key_u32 for entity+A68. The natural trial
  showed only1909/2D3F despite many actions. Native401B34 instead obtains those
  same values from item68.u32+58, and complete555B08 dispatch maps them to
  categories0/6. Cause: semantic overreach from a raw field label. Preserve
  frozen V3 schema and captures; the natural decoder calls it legacy_raw_A68
  and does not assign swing, attack, counter or guard identities. A writer join
  for entity+A68 remains separate. Evidence: passive_sources/
  FIELD_TERMINOLOGY_401B34.md and natural_combined/REPORT.md.

- Objective: authenticate small resources118/260/720. Root's first batch
  required a12-byte stride for context720's Q18..Q20 vector. Cause: confusing
  the sorted pointer vector (337 Q entries) with contiguous12-byte rows at
  Q0..Q8. The first capture refused geometry and closed its handle before any
  vector follow. SOURCE_BEFORE_REPAIR.py and its failed receipt remain in
  small-resource-batch-os-01. Fresh batch-os-02 separately captures both spans,
  validates pointer containment/alignment/count and final stable brackets;
  it also authenticates118/260 and current profile/attributes.
- Objective: compose the migration constructor. The full constructor reaches
  2073100->3F1B18->3F1ED4->12F1DC and refuses an uncaptured runtime registry at
  module4B50348. This is an explicit remaining input, not evidence of a game
  fault. Original timer/type writer blocks are composed across that boundary;
  no return is supplied for the unexecuted initializer. Migration E2E passes14
  controls with declared allocation/topology/post-activation inputs. Evidence:
  form_migration/e2e-01/FAILURE.json, e2e-wrapper-01 and FAILURE_NOTES.md.
- Objective: advance the native backend lookup. The first backend planner
  recognized unknown reads only inside a previously mapped page and refused a
  legitimate missing data page. The repair accepts that frontier only at four
  exact367440 comparison instructions with4-byte key geometry and bounded
  previously authenticated backend offsets. It does not treat arbitrary faults
  as new data. Original traces/refusal and actual v3-consumer file transports
  are retained in handlers/pass-08/BACKEND_PROGRESS_* and planner evidence.
- Objective: attribute late source stages. Exploratory labels incorrectly
  associated554268 with the58C0 bitset and source71 with the guardian index;
  neither was accepted as a numerical dependency. Original instructions show
  554268's actual118/item/attribute gates, and374F58 uses source70 (compiler
  owner-form copy) for the guardian pair; source71 gates numeric addition.
  A separate initial3FAA98 guardian label actually denotes the set-expansion
  stage. Disposition: correct before source acceptance, preserve raw inputs
  and the legacy recovery-anchor name with versioning/SEMANTIC_ERRATA.json.
  The byte signatures/relocations are unchanged. No item value or active index
  is selected by fitting observed output maps.

- Objective: obtain a complete REA/Ghidra decompilation of the70,440-byte
  central dispatcher66FC28. Phase08 accepted233 explicit seeds but the one
  requested decompilation failed with GhidraRemoteError/decompile_failed,
  `Response buffer size exceeded`. Runtime was about586 seconds. This is a
  provider output limit, not a new game fault or an additional decompiled
  function. No unchanged retry was made. Native signed-switch/table/case and
  callback/counter blocks instead pass12 original-instruction controls in
  ninja_trigger; full unrelated preamble remains outside that composition.
  Evidence: rea/BATCH_CONTINUE08.json, REA_PHASE08_OUTCOME.json and
  PROCESS_CLOSURE_04.json. All owned provider/client processes closed; prior
  accepted pins revalidate and the accepted decompilation total remains232.

- Objective: verify per-source D writes after native reset/materialization.
  The old native-loop audit filtered by region-name strings, while its caller
  named the D region differently. Its per-source D audit/mask was empty and the
  corresponding written-byte equality assertion was vacuous. The independent
  full2672 reset-known-byte comparison and predicted D.bits/known hashes were
  unaffected. Preserve reset-cache-e2e-02. The versioned address-based adapter
  in reset-address-audit-e2e-01 passes17 checks/two actualCLIs and requires
  nonzero exact coverage:49 D-writing groups,193 unique written bytes with
  matching native store addresses/widths and accepted recipes. Native bytes
  and predicted binary images are unchanged.
- Objective: validate V4 six-slot capture through the actual wrapper. Two
  intermediate file-backed fixtures used an unsupported typed payload shape
  or an unauthenticated method form and refused safely. The corrected fixture
  retains exact raw field widths and original method identity; native18,
  actualCLI26 and the independent41-endpoint genuine quiet audit pass. Evidence:
  action_identity/FAILURE_NOTES.md, v4-e2e-03 and quiet-audit-01. V3 and earlier
  replay sources remain frozen; no live failure or combat-event claim follows
  from fixture admission.

- Objective: verify masked D under the strict retained MAIN request contract.
  The first portable0xCD poison case was rejected before VM execution because
  unknown bits in value_hex must be canonical zero. Root/user request keys
  remain unchanged; change records stay in sidecars. Preserve consumer-e2e-01.
  The final five actualCLI controls distinguish portable poison followed by
  mandatory canonical serialization (same valid request as masked D) from an
  explicit noncanonical-request rejection. No runtime CD execution is claimed.
  A second test assertion expected last_missing_read for a mapped-but-unknown
  word; actual native dependency is uninitialized_value with known_mask0, so
  last_missing_read correctly stays null. The final consumer-e2e-03 records
  reader78E69D/D+7DC and block78E6A5, rejects the old score, and pins actual
  loaded R10 interpreter sources. No VM/cut/format validation was weakened.

- Objective: read the native HP allocator query delegate required by19EDE0.
  The first plan serialized require_nonzero_ranges as[0,8] instead of[[0,8]].
  The actual v3 reader refused its pair unpack after reading the registry
  pointer, then closed its OS handle. Cause: PowerShell nested-array flattening
  in root's plan producer. Preserve evidence/HP-allocator-query-os-01. The
  corrected explicit JSON is accepted in fresh os-02:8 stable regions,128 read
  bytes, identity matched and handle closed. Allocator+D8's vtable slot98 is
  live RVA12F8E0, supplying actual branch data rather than a native return stub.
- Objective: independently validate four accepted component seals. Root's first
  adapter resolved implementation paths against the manifest directory and then
  treated unhashed preserved_root_paths strings as digest records. Both attempts
  failed before producing an acceptance receipt. Cause: incompatible manifest
  collection schemas. The corrected verifier uses the implementation root and
  distinguishes references from actual digest records. COMPONENT_SEALS_VERIFY_01
  passes all declared pins across571 unique files, including its own source;
  this verifies integrity without expanding the original behavioral claims.
- Objective: correct phase08 REA pin-count metadata during root integration.
  Updating the original outcome after the Ninja component sealed it temporarily
  changed that external dependency. Cause: editing a sealed dependency instead
  of adding a correction sidecar. The corrected metadata is preserved in
  rea/REA_PHASE08_OUTCOME_02.json, and the original is restored byte-for-byte to
  SHA256 fa04283afb07c3fa46a42614b91943a1f3eb42d1ffe12f26db7281865e5a5383.
  The independent four-seal verification now passes. Accepted native results
  and232 distinct decompilations are unchanged;316 prior/322 total evidence-pin
  counts are metadata, not additional successful decompilations.

- Objective: continue exact shared-handler runtime inputs. The first
  PLAN_RUNTIME_FOLLOWUP assumed a previously nonnull world slot9 would remain
  populated. Its current captured pointer isNULL, so the actual v3 reader
  refused the negative-offset follow and closed its handle after12 stable
  regions/682 bytes. Cause: changed world topology between independent reads.
  Preserve runtime-followup-os-01. The versioned plan follows only current
  nonnull slots0/8/12/13 and adds selected-pointer nonzero guards. Its actual
  consumer control refuses a newly null selected slot before following it;
  fresh runtime-followup-os-02 succeeds with22 stable regions/882 read bytes.
  Newly nonnull unplanned slots remain unknown to native iteration.
- Objective: verify root's native pre/post agility slice. All12 prediction
  CLIs completed their declared domains, but e2e-01 incorrectly expected every
  unknown-input refusal to be uninitialized_value. Native form/modifier/unlock
  branches instead refuse as uninitialized_flags at8178DC/817987/817975.
  Cause: overly narrow test metadata, not a native execution failure. Preserve
  e2e-01; e2e-02 checks each exact dependency kind and PC and passes44 checks.
  No VM, prediction source, native bytes or acceptance gate was weakened.
- Objective: compare a readable agility equation against original81789C.
  Root initially assumed threshold scale1000; the actual constant at3BD4DE0 is
  10 and original230 getters return percentage thresholds20/70/30. The first
  equation run rejected that assumption before native score comparisons.
  Cause: conflating the weight ratio's1000 with the threshold parameter scale.
  Preserve score-equation-e2e-01/FAILURE.json and its minimal reconstructed
  reproducer; the original failed source snapshot was not retained. The second
  run saves the actual getter phase and compares256 complete original calls,
  all matching the independent equation. No game access or VM change occurred.

- Objective: prepare the exact final3F9FC0 inventory query plan. An offline
  generator rejected an assumed consecutive slot array13/14/15/16. Original
  rdata3978EF8 actually contains13/9/10/15 followed by banks0/1. Cause: extending
  the first proven slot by analogy. The incorrect description was corrected
  before any live plan was emitted; root used only eight exact selector reads
  in PLAN_FINAL_INVENTORY_SELECTORS.json. The successful capture and
  FINAL_INVENTORY_CONTRACT.json retain actual bytes and guards. No inventory
  rows were read because every selector wasFFFFFFFF.
- Objective: diagnose finalization source-map changes. The first diagnostic
  demanded an exact inactive-form count, which that producer deliberately
  leaves UNKNOWN. Cause: diagnostic knowledge overreach. Preserve probe01;
  later raw bits/masks keep the inactive count null and execute whole3F9FC0
  in1478 instructions. The final24-check/six-CLI receipt verifies both source
  spans unchanged in the current vector. See passive_sources/
  FAILURE_NOTES_NUMERIC_CONTINUATION_20261019.md; no inactive byte was filled.
- Objective: establish the migration restore's inserted-bound dependency.
  First fixture attempt assignedXMM1 directly, but the reused CPU.run(delta)
  ABI overwrote it with default0 before the real setter. Cause: fixture ABI
  mismatch. The repaired control passesdelta550 to original setters and
  verifies both restored550 and unknown-bound refusal. A separate initial
  disassembly selected only the70-byte FFA290 pdata prefix; the explicitly
  named continuation closes the actual tail instead. Both attempts remain in
  migration_refresh/FAILURE_NOTES.md. Main16 controls remain unchanged;
  notification's nine controls keep OS lock calls outside their body domain.

- Objective: verify three late numerical groups with unknown operands. The
  first phase-test assertion expected the original ADD to complete and merely
  propagate an unknown mask. The frozen engine correctly refused exact
  arithmetic at81A33B (and the corresponding other group readers). Cause:
  incorrect new test expectation. Preserve late-group-phase-e2e-01; attempt02
  requires six true positive/wrapping completions and six exact unknown-input
  refusals with no object store, passing53 checks. VM, native instructions and
  previous producers stay unchanged. See FAILURE_NOTE_LATE_GROUP_ASSERTION.md.

- Objective: feed independently predicted STAT and D into retained MAIN.
  The first request erased the raw900-byte STAT window, unintentionally
  erasing the distinct HP aggregate at core+818. Native78D337 refused an
  old-scene HP+2DD0 dependency. Cause: transport extent mistaken for owned
  object bytes. Preserve probe-masked-01 and its exploratory producer. The
  corrected7E0 owned span leaves separate HP context explicit and rejects
  any unclassified tail read. Actual core+7E8 readers classify it as HP+2D90.
- Objective: close the integrated MAIN718 dependency. An initial demand label
  used temporaryR9=1 after lower_bound, although original2FA68F saved incoming
  column10 inEBP. Cause: reading a clobbered volatile register as the entry
  ABI. Request computation was unchanged. Preserve curve-parameters-01 and
  its producer; authoritative02 uses the native register receipt. Same-version
  corpus projection closes the old missing-vector frontier without a supplied
  result. Final five-CLI acceptance keeps old scenario context explicit.
- Objective: continue the native resource request through its real current
  ISA branch. Original4CF7C8 reaches22239 instructions, then Unicorn reports
  UC_ERR_INSN_INVALID atB943CC in an AVX2 memcpy block, with no missing memory.
  Cause: emulator instruction support, not missing game bytes. Preserve the
  pass08 request frontier; no forced ISA selection or code patch is permitted.
  A separate exact-instruction adapter is under verification; this frontier
  does not increase E8 acceptance or establish a game fault.

- Objective: support the reached original CRT copy instructions. An initial
  invalid-instruction-hook repair missed native VZEROUPPER because Unicorn
  silently executed it without clearing upper YMM lanes. Cause: inconsistent
  backend VEX support, separate from the earlier VMOVDQU invalid opcode. The
  repaired preinstruction adapter admits only authenticated original sites and
  verifies all16 upper lanes, complete original copy calls, overlaps and three
  strict refusal controls. VEX_COPY_INSTRUCTION_E2E.json is accepted; prior
  CPU/ISA selection and helper files stay frozen. No whole memcpy return is
  substituted and this does not close an entire effect handler.
- Objective: capture exact component leaves for ordinary insertion. The
  first offline draft contained50 roles, exceeding unchanged reader v3's32
  limit. Root review caught it before a live call. Preserve excluded
  status_factory/PLAN_WORLD_COMPONENT40_50.json. Two32-role A/B plans pass
  actual-consumer size, epoch, instability and cross-packet-phase controls;
  both subsequent live packets are stable and closed. The composition checks
  exact shared ancestry/phase bytes rather than assuming sequential packets
  are atomic. No reader limit was raised and no failed draft entered native
  execution. Reproduce with run_world_plan_e2e.py/run_world_phase_e2e.py.

- Objective: extend the type37 empty-target method proof to one-candidate
  processing. The active TYPE37_GENUINE_CENSUS_E2E.json path and its producer
  were overwritten before versioning, losing the exact earlier19181aac...
  receipt bytes. Cause: treating a reviewed local proof as mutable. Bounded
  snapshot/output recovery failed; no metadata was invented to claim recovery.
  TYPE37_PRESERVATION_AUDIT_01.json records the exact lost SHA256 and scope.
  The legacy path is now a failed/unavailable-proof tombstone and cannot
  support acceptance. No frozen seal referencing that legacy receipt was
  found; the12 canonical allocator/service sources remain hash-identical.
  Versioned _02 independently reruns seven original-method returns and four
  unknown refusals, pinned with its own immutable source snapshots. The
  original19181 proof is not reinstated. Actual marker writes remain unknown,
  and full complex E8 coverage stays79/87. No raw capture or sealed damage
  dependency was changed. Further scope extensions must use fresh filenames.

- Objective: admit predicted numerical files into the two-form SOURCE
  consumer. Read-only review found V2 hashed current STAT/D files but did not
  enforce their four expected output digests from the pinned producer receipt.
  All then-current full binaries independently matched; no714 is contradicted.
  Cause: confusing current input pinning with upstream artifact admission.
  Preserve V2/e2e-01. V3 validates full900/B00 extents and expected digests
  before slicing/program preparation. Six actual CLIs/seven checks in e2e-02
  include changed-digest/truncated-file preexecution refusals and native map
  unknown controls. REVIEW_01.md confirms the bounded fix. No old sealed
  producer or consumer was modified.

- Objective: reuse the genuine world-modifier set for ordinary insertion.
  Unsealed probe09 offered a shared-helper-v1 capture to the explicit-v3
  loader, which refused a missing game_calls_or_writes key before insertion.
  Cause: treating distinct receipt schemas as interchangeable. The versioned
  adapter validates the actual shared-helper schema, profile/epoch and only
  the five consumed roles. Probe10 runs genuine empty-set lookups; formal
  insertion-e2e-01 passes41checks/11CLIs. No generic-loader relaxation, invented
  empty state or live change occurred. See status_factory/INSERTION_FAILURE_NOTES.md.

- Objective: derive migration's actual returned-object bound. Bounded prototypes
  refused a wrong STAT/D caller binding at236C0BD, a serializer instruction
  site mislabeled846657 instead of84665B, and an overbroad late-role requirement
  on the single-read readonly delegate table. A textual second-cell opcode typo
  was caught before live use. Cause: new ABI/site/schema metadata errors, not
  missing native timer values. Fresh versions correct only the authenticated
  bindings; migration_bounds/FAILURE_NOTES.md preserves each attempt. A seal
  check also caught confusion between a historical producer snapshot and the
  evolving canonical path; their version scopes are now pinned separately.
  Final e2e-01 passes50checks/16actualCLIs with real two-object construction,
  replacement and ordinal lookup. OS normal-return and controlled serializer
  entry remain explicit; no unconditional natural callback is claimed.

- Objective: prepare one natural expanded-bound/form-switch observation with
  immediate audio. File-boundary review found that a long snapshot could count
  as15seconds rest, a return to Samurai could be silently accepted, a healthy
  incomplete live run could exit successfully, and a failed second Beep or bad
  final collector receipt could lose wrapper evidence. V2/V3 preserve those
  counterexamples and repair state/completion/durable finalization boundaries.
  V3 review additionally reproduced buffered Ninja data predating the actual
  double-cue end being counted as post-cue rest. Cause: observation/delivery time
  conflation. New state_v3/run_guided_capture_v4 bind the actual second Beep end,
  require the form-field read to start afterward, and measure rest readEND to
  later readSTART with bounded known snapshots. Early/overlapping form reads
  permanently disqualify the trial. Root state-e2e-03/anchor-e2e-01 pass19/7
  actual file CLIs; quiet-04 has41 genuine samples/no sound/closed handle.
  Independent V4 review remains required before owner gameplay. Old sources,
  rejected outputs and raw captures remain preserved under guided_capture and
  status_factory/guided_review; no audible trial has been armed by these tests.

- Objective: read the exact2086D4 startup gate in the restarted process. The
  first root-authored PowerShell plan flattened required[[0,8]] into[0,8].
  Explicit v3 refused with TypeError after one root record/zero gate bytes,
  then closed its read handle. Cause: nested-array serialization, not game
  memory behavior. Preserve new-epoch-cleanup-byte-os-01 and plan01. Fresh
  literal JSON plan02 retains[[0,8]]; new-epoch-cleanup-byte-os-02 passes four
  stable records/36 read-and-reread bytes with raw gate0. Existing Python plan
  builders already preserve nested guards; no generic reader was relaxed.

- Objective: start the new fixed kind0 request-link reader. Its first real
  bootstrap refused FileNotFoundError before identity/target reads (zero bytes,
  zero nodes, no open target handle). Cause: loading the frozen helper from a
  deeper snapshot directory changed its relative ROOT and thus its identity
  utility path. File-boundary tests had mocked that provider. Preserve sealed
  pass09 V1 and new-epoch-kind0-query-links-os-01; a versioned V2 must explicitly
  bind/pin the canonical utility and test its actual import resolution before
  another root-owned live call. This is a bootstrap defect, not whole-request
  acceptance or a game fault. No game code/write/debugger was used.
  Later V2 binds the exact canonical identity utility/source, passes an actual
  unmocked import and original identity-code/file-kernel controls, and its
  fresh root-owned read succeeds with13,834bytes/10nodes/closedhandle.
  V3 adds the exact metadata DWORD only and succeeds with13,855bytes/10nodes,
  keeping all caps. Both queries independently resolve the same raw80000000
  metadata word. These are pointer/metadata observations, not whole-request
  or natural E8 acceptance; the original V1/seal/failure remain preserved.

- Objective: recover the new cleanup singleton from profile07. Same-version
  location tests resolve128 anchors, but roots-e2e-05 rejects the new recipe:
  its migration target signature was24 bytes while the unchanged root decoder
  admits exactly one7-byte RIP instruction. Cause: function-anchor extent used
  as instruction extent. Preserve profile07/roots-e2e-05. Fresh builderV2 and
  profile08 narrow only that anchor/recipe to the authenticated7 bytes; the
  independent opcode/target/relocation controls remain required. Earlier
  accepted profile06 and decoder sources stay unchanged.

- October20 objective: close the actual HP progress lookup after disk3F0
  binding. The minimal36-byte key3 capture suffices for the817740 u32 value
  getter (actual50), but the real HP caller374BD8->374C1A->374B3C reads the
  adjacent byte+24 at374C41, outside that capture. Cause: two callers of the
  same hash helper consume different value-object fields. Preserve the value
  proof and genuine132026-instruction UNKNOWN frontier; never infer the flag
  from level50. Explicit flag0/nonzero alternatives continue separately and
  both reach uncaptured key273/bucket31. Follow-up is code-derived typed node
  extent and actual caller coverage, not another blind one-byte capture.
  Evidence: hp_3f0_projection/progress_projection, frozenV2 and declaredV4
  traces. The game is closed and no new collection is requested.

- October20 objective: extend the creator tail using saved current observer
  topology. The first local wrapper import lacked the handlers module path
  and failed before the accepted prefix. A bounded path repair leaves frozen
  sources unchanged. Actual native execution then reaches A2DD02/15steps with
  a nonempty current head and correctly refuses unknown first-node+18.
  Evidence: status_factory/migration_tail/current_observer_projection and
  its checkpoint01. This is a bootstrap repair plus a genuine missing input,
  not complete callback acceptance or evidence of an empty current list.

- October20 observer coverage verification initially passed a historical
  failed/partial startup receipt to the successful-capture admission verifier.
  It correctly refused the missing identity_after/null actor. Another bounded
  audit expected an unnecessarily full identity_after schema while the approved
  reader emits reduced PID/birth/base. Cause: audit classification/schema
  assumptions, not capture corruption. Final audits retain full before-identity
  hashes, classify failed records as partial raw provenance only, and never
  admit them to replay. See current_observer_projection/AUDIT_REPAIR_RECORD.md
  and hp_3f0_projection/progress_review_sol/HARNESS_REPAIR.md. Final observer
  E2E12checks and independent progress review14checks pass.

- October20 original observer follow-up initially required a pdata entry for
  the23-byte A46B0 leaf and refused before export. Versioned bounded named-leaf
  decoding preserves that refusal and authenticates the leaf/tail bytes. The
  first standalone native block runner lacked the retained Unicorn path and
  failed before CPU execution. Final check_observer_flag_effect_v2 explicitly
  uses/pins22 retained runtime files and passes60 original-instruction cases.
  See offline_continue_20261020/OBSERVER_LOOKUP_SCOPE.md. This establishes the
  selected01 flag's OR1 idempotence, not whole lookup/creator completion.

- October20 Pro package local checks initially compared native binary32
  expressions with binary64 equality and rejected a valid relation. A selected
  raw lookup exporter also correctly refused a missing pdata function extent.
  Package-local fixes explicitly round binary32 and use the existing bounded
  reachable native export. See the package PACKCHECK_REPAIRS record. Finalv2
  directory/ZIP checks and1412 local checks pass in place and after relocation;
  no full native replay, game action or external submission is inferred.

- October20 Pro-return acceptance: original verify_return.py passed its input
  manifest but refused equality with DERIVATION_AUDIT.json on Windows. Fresh
 39-check calculation passed. Cause: native path-separator spelling in three
  relative-path expressions, not changed hashes or native arithmetic. Preserve
  the original return and failed invocation; acceptance/verification ports only
  those expressions to.as_posix(), records original/ported hashes and regenerates
  the separate copy's manifest. All1412/145/39 file/model checks then pass.
  Evidence:r16-pro-return-acceptance-20261020/acceptance/verification/.

- October20 HP native continuation: exporter initially assumed pdata entries
  forA698C/1AB3D28 and refused named leaves. Exact bounded leaf extents repair
  the export. A19EDE0 indirect tail remains an explicit dispatch frontier,
  admitted only with authenticated query bindings. Changed-HP runs initially
  stopped at CRTpolicy B93F40, registry130A98 and query19EDE0. Existing pinned
  HP-specific allocator/query inputs and declared private CRT policy permit
  the standalone suspended-component completion; they do not certify active
  player HP. Evidence:acceptance/hp/ native receipts and recovered ASM.

- October20 progress continuous reproduction initially omitted the existing
  --raw-source-variant348-control and stopped at2105A5C. Adding that declared
  input mode reproduces both original branches to the genuine key273 guard.
  Preserve the first attempt; native-observation-04 is authoritative. This is
  fixture selection, not newly observed progress bytes or fullHP completion.

- October20 actual PE-root admission initially assumed a captured section's
  byte length equals PE VirtualSize. The original dump uses SizeOfRawData,
  which can be slightly larger (text59638784 versus59638411). run-01 refused;
  run-02 validates captured provenance against raw size while retaining strict
  VirtualSize for full-source-instruction and8-byte root-slot ownership.
  Evidence:acceptance/versioning/README.md and run-02/ACCEPTANCE.json. All10
  actual slots and42 refusal controls pass; no changed game is accepted.

- October20 type6 composition falsified an input-deadness hypothesis: H+24
  is read at376C76 BEFORE native3775D9 overwrites it. Initial UNKNOWN refuses
  after the HP store; a poisoned known value cannot stand in for a legal
  admitted prior state. Final linked-positive domains require known prior0,
  derive5 through actual9D81C and return with positive healing. Preserve the
  failed deadness control and after-write refusal grade. Evidence:
  acceptance/resource/FAILURE_NOTES.md and DETACHED_COMPOSITION.json.

- October20 private marker composition first lacked mapped private header
  backing and a required owner component50/mode chain. Those prototype input
  omissions refused before full completion; the final sparse graph explicitly
  declares its fields, leaves unrelated bytes UNKNOWN, preserves bucket/NEXT
  topology and labels changed handles/generations PRIVATE. Ordinary-mode
  follow-up later reused the genuine existing parameter230 asset to cross
  5A05AC, rather than requesting gameplay for an already retained parameter.
  Evidence:acceptance/marker/COMPOSITION_FRONTIER.json and ordinary-03/.

- October20 final sealing initially classified every.pyc as generated cache,
  but the immutable Pro manifest intentionally contains one version_recovery
  bytecode file. All282 original manifest hashes matched. The protective
  manifest-member guard refused quarantine before any move. Final sealing
  preserves manifested original/ported input files and excludes only unaccepted
  worker cache. A source-only verification rerun redirects imports away from
  supplied bytecode; its first wrapper omitted the package root on sys.path,
  then a minimal entry-path repair passes1412/145/39 again. Original return
  files were never changed. Evidence:acceptance/verification/verify_source_only.py
  and RETURN_VERIFICATION_SOURCE_ONLY.json; final manifest records exclusions.

### 2026-10-20: retained hit-observer CLI preparation mismatch

- Objective: prove the retained observer CLI/Lua path is ready to collect a
  candidate-specific MAIN/HP hit before requesting owner actions.
- Symptom: first mock run used an unsupported archived `phased` mode and the
  wrong event key; a second used a phased-cycle event against core_main's four
  armed sites, causing an unarmed mock callback and unverified mock cleanup.
- Cause: root reused stale command assumptions instead of the frozen contract.
  No game or actual debugger was accessed.
- Evidence: `F:/Nioh3_r16_build/deliverables/r16-equipment-execution-20261020/`
  `hit-observer-cli-e2e/` and `hit-observer-cli-e2e-02/` retain both failed runs.
- Disposition: repaired to contract-owned `core_main`, explicit core/entry/final/
  HP events and timer drain. `hit-observer-cli-e2e-03/` completes one mock frame
  with verified cleanup; `HIT_OBSERVER_CLI_E2E.json` records the command sequence.
- Reproduction/follow-up: rerun `verify_hit_observer_cli.py --output <fresh path>`
  through the project Python runner. Natural-hit and live cleanup acceptance
  remain separate, pending a fresh approved game session.

### 2026-10-20: offline equipment-executor binding repairs

- Objective: connect the equipment emitter to the concrete owned Windows
  event/cleanup adapter before any live invocation.
- Symptoms/causes: an initial private fixture used an inadmissible empty
  key/seed/rarity; preexisting-DR refusal bypassed create-process file-handle
  closure; an opener refactor left a stale local at FlushInstructionCache.
- Evidence: `F:/Nioh3_r16_build/deliverables/r16-equipment-execution-20261020/`
  `executor-review/FAILURE_NOTES.json` and the concrete composition receipt.
- Disposition: keep emitter admission unchanged; correct the private fixture;
  close the owned file handle before raising adoption; use self.api.dll.
- Reproduction: project-runner `verify_windows_composition_e2e.py`; final8/8
  concrete adapter/service cases,15/15 lifecycle cases and62 file hashes pass.
  No actual process/game access occurred. Live scheduler/cleanup/insertion
  acceptance remains separate from these repaired fixtures.

### 2026-10-20: merged read-only capture fixture schema repairs

- Objective: execute the actual progression, parameter, level and guard
  callbacks together through the one-session coordinator before live use.
- Symptom: fixture assembly stopped first on a missing `value_hex`, then on
  an integer/string offset addition. No process handle was opened.
- Cause: the718 pointer-vector capture retains some raw bytes in explicit
  vector/row fields instead of every read receipt; role offsets are serialized
  as hexadecimal strings. The integration fixture assumed a uniform schema.
- Evidence: task-owned `merged-capture-e2e-01/` and `-02/` attempts and
  `verify_merged_capture.py`; the final receipt is
  `merged-capture-e2e-03/MERGED_CAPTURE_E2E.json` under
  `F:/Nioh3_r16_build/deliverables/r16-equipment-execution-20261020/`.
- Disposition: reconstruct only explicitly retained known bytes, reject alias
  conflicts, parse serialized offsets, preserve source schemas and seals.
- Reproduction/follow-up: run `verify_merged_capture.py --output <fresh task
  path>` through the project Python runner. Three cases pass, including two
  independently missing-input cases. Actor/insertion stages are fixture stubs;
  this proves callback integration and partial reporting, not live acceptance.

### 2026-10-20: candidate grade718 resource and private replay repairs

- Objective: execute ordinary affix materialization for the six selected armor
  specimens without deriving values from an observed AFTER state.
- Symptoms/causes: the retained model lacked its grade718 binding manifest;
  assuming archive row order failed full-row comparison; an initial private
  original-helper replay omitted its security-cookie global. A negative E2E
  fixture also aliased the two identities it meant to make different.
- Evidence: task-owned `candidate-production/EXPERIMENT_FAILURES.md`,
  `static-binding/evidence/grade-curves/BINDING.json`, `grade-e2e-1/` and
  `e2e-6/` under the equipment-execution delivery root.
- Disposition: authenticate all662 complete rows and recover the native vector
  permutation; explicitly declare the private cookie and execute the original
  check; replace the aliased fixture dictionary before mutating it. No helper
  result, missing row or unknown live field was supplied from a score.
- Reproduction/follow-up: project-runner grade and candidate E2E scripts;
 264 grade cases and15 candidate cases pass. The candidate outputs remain
  conditional intermediate profiles, excluding complete SOURCE, damage and
  wearer acceptance. No game session was needed for these repairs.

### 2026-10-20: portable agility CPU-runtime bootstrap

- Objective: verify a portable post-materializer agility adapter against the
  retained original81789C cases and new complete8192B6..8192F6 gate execution.
- Symptom/cause: the first attempt could not import Unicorn from the project
  interpreter; the retained research runtime lives in a task dependency folder.
- Evidence: `candidate-agility/e2e-01/` and the corrected verifier under
  `F:/Nioh3_r16_build/deliverables/r16-equipment-execution-20261020/`.
- Disposition: reuse the existing retained runtime path exactly as other native
  probes do, while keeping the project Python runner. No package installation
  or global interpreter/configuration change occurred.
- Reproduction/follow-up: project-runner `verify_agility_e2e.py --output <fresh
  path>`; `e2e-04/AGILITY_E2E.json` passes292 scoped checks. Full candidate
  generation and natural six-item acceptance remain separate.

### 2026-10-20: maximum-HP support partial-capture preservation

- Objective: retain useful known/unknown raw fields when a late bounded
  maximum-HP support read fails.
- Symptom/cause: review found the first helper appended a pass only after all
  reads succeeded, discarding prior raw observations on assertion/read failure.
- Evidence: sealed `hp-max-producer/capture_max_hp_support.py`; repaired
  `hp-max-support-v2/` with eleven file-based E2E controls.
- Disposition: preserve the original delivery unchanged; use the separately
  pinned V2 helper. Append passes/records before reads and retain exact received
  masks/short bytes. Observed NULL world roots remain distinct from unknowns.
- Reproduction/follow-up: V2 E2E and
  `merged-capture-e2e-07/MERGED_CAPTURE_E2E.json` exercise late failures without
  reopening the game. No live read or write occurred during this repair.

### 2026-10-20: conditional constructor field-contract corrections

- Objective: connect explicitly generated character images and a full-health
  scenario to the original constructor and downstream MAIN kernel.
- Symptoms/causes: root initially mistook HP subobject base H+38 for current
  HP, and later described the legacy name `packet136` as a136-byte packet.
  Both were instruction-ticket errors caught during source/contract review.
- Correct contracts: S=H+38 is the HP subobject; maximum HP is S+18=H+50 and
  current HP is S+20=H+58. The constructor object is190hex=400 bytes;
  `packet136` names a U16 field at offset136hex, not a buffer length.
- Evidence: original shared-HP contract/fixture, constructor-input-audit report
  and original2B42B4/2B4A3C image. The new kernel must preserve H+38 bytes and
  export the complete400-byte constructor image while keeping caller-owned
  MAIN fields beyond190hex separate.
- Disposition: corrected the new independent kernel/interface before sealing
  or game use; existing sealed evidence remains unchanged. No game memory or
  source save was written. No truncated packet was admitted as a damage input.
- Reproduction/follow-up: require explicit H+38 preservation and full400-byte
  export assertions in the new conditional-kernel E2E receipt. Native caller
  transfer/pointer ownership remains a separate bridge obligation.

### 2026-10-20: multi-piece body fixture binding repairs

- Objective: verify ordered five-piece body arithmetic and the original
  synchronization rewrite without score or AFTER inputs.
- Symptoms/causes: the first native factor-initializer fixture omitted the
  authenticated rdata constant; a later rewrite test assumed selected/core
  STAT10 controls were equal, contrary to the actual BEFORE namespaces.
- Evidence: `candidate-body-rebuild/EXPERIMENT_FAILURE_LEDGER_ENTRY.md`,
  `BODY_E2E.json`, `REWRITE_DOMAIN_E2E.json` and the component manifest.
- Disposition: map original rdata and execute817308; test each namespace's
  actual original sync guard before expecting a rewrite trace. No native return
  value or missing actor byte was fabricated, and no game reopen was requested.
- Reproduction/follow-up: project-runner `body_e2e.py`;22 body controls and
  three restricted-SOURCE rewrite controls pass. Complete candidate SOURCE52
  generation/invariance remains the upstream compiler's responsibility.

## 2026-10-20: Conditional MAIN wrapper assumptions

- Objective: run original MAIN against explicit same-instance conditional
  images without observed score or AFTER inputs.
- Symptoms/causes: an initial wrapper ceiling of2000000 rejected the archived
  reference's larger explicit step bound. A separate private packet+4 control
  incorrectly expected720; original MAIN returned719.
- Evidence: execution delivery `candidate-main-kernel/MAIN_KERNEL_E2E.json`,
  `EXPERIMENT_FAILURE_LEDGER_ENTRY.md`, and manifest70b1f176c32e333c7b65742500907a3884424f75283f31070b2569ef6cfd8fa7.
- Disposition: retain an explicit maximum2500000-step bound; correct the
  test to original719. Independent private+20 returns742. No input was tuned
  to force the old720 calibration. All34 final controls pass; root checked
  all seven sealed file hashes.
- Reproduction/follow-up: run `candidate-main-kernel/main_e2e.py` with the
  project runner. Actual natural same-hit ENTRY/BUILDER integration remains
  separate. No live execution or native HP-store acceptance is claimed.

## 2026-10-20: Full generated SOURCE ownership integration

- Objective: apply actual original-controller write journals to the conditional
  constructor without retaining stale known bits.
- Symptom/cause: the sealed v1 instruction whitelist refused original SOURCE
  reset MOVQ at64AA72 and MOVD at64AA77. This was a missing supported store
  family in the wrapper, not an observed equipment damage mismatch.
- Evidence: `candidate-constructor-kernel-v2/RESET_STORE_E2E.json`, its39-check
  manifest and `candidate-production/public-generation-e2e-1/HEAVY_KERNEL_V2_CHECK.json`.
- Disposition: preserve v1 unchanged; add unique v2 modules admitting only
  those two original byte/RVA/role/width combinations. All31 prior controls
  and eight new integration controls pass. The first comparison test caught
  the old refusal under the wrong dynamically loaded exception class; its
  catch now handles each policy's own refusal class without changing admission.
- Reproduction/follow-up: run v2 verify_e2e.py and verify_reset_stores.py via
  the project runner. Continue full candidate/constructor/MAIN integration;
  no runtime acceptance or game write is claimed.

## 2026-10-20: Pro observer verification on Windows Lua runtimes

- Objective: independently reproduce the returned constructor/observer suite.
- Symptom: stock Windows resolution used lupa.lua54;68 of70 checks passed, while
  two expected refusal tests errored because LuaError is not RuntimeError.
- Cause: test exception-type assumptions differ between the lupa binding and
  the ctypes CE Lua runtime. This does not demonstrate a game observer failure.
- Evidence: acceptance/observer-review receipts and logs under
  F:/Nioh3_r16_build/deliverables/r16-real-gear-pro-acceptance-20261021/.
- Disposition: rerun unchanged returned sources using the actual CE Lua5.3
  library selected explicitly; all70 checks and positive mock CLI/cleanup pass.
  The environment variable is historically named LUA54_LIBRARY; the recorded
  actual interpreter is Lua5.3, and no Lua5.4 or live-CE acceptance is claimed.
- Reproduction/follow-up: use the observer-review report's exact library path,
  project Python runner and fresh owned copy. A generic lupa harness should
  separately normalize expected exception types if that runtime is supported.

## 2026-10-20: Explicit constructor runtime bundle integration

- Objective: execute the repaired returned constructor with authenticated local
  native resources, preserving sealed sources, masks and epoch boundaries.
- Initial symptoms/causes: origin assertions mislabeled exported M/C modules;
  a resource-only epoch was initially joined as dynamic; one test incorrectly
  expected CANDIDATE_MAXIMUM to clamp current HP.
- Disposition: verify the actual indexed_curve/main_forward/native_vm tuple;
  preserve the static-resource identity separately; keep dynamic epoch joins
  strict. Correct the test to preserve H58, without changing the HP policy.
- Evidence: acceptance runtime-binding/EXPERIMENT_FAILURE_LEDGER_ENTRY.md,
  RUNTIME_BINDING_E2E.json and manifestf03614ef91839281fa34ebe0e989b88f749f8dcb82c78fd831447b37961c2b35.
  All22 final controls pass; original initializer returns,492 distinct constructor
  RVAs execute, then actual unknown action.flags+F8 at377F74 refuses completion.
- Reproduction/follow-up: project runner, runtime-binding/verify_runtime_binding.py.
  Producer/MAIN public bundle propagation and shared controller execution remain
  separate. No stubbed helper, game access, save write or full-chain claim.

## 2026-10-21: Joint/caller and owned-context integration controls

- Objective: close fresh HP inputs, original caller execution and installed
  debug-context readiness without a game session.
- Symptoms/causes: an initial HP mutation targeted an unconsumed branch; the
  PRIVATE caller fixture initially chose outer-actor rather than query-entity
  +260; a no-active-slot selector was initially modeled as boolean instead of
  its original domain value3. These were fixture/control defects, not game facts.
- Disposition: use the original consumed230 branch, entity selection and native
  selector domain; preserve old failures. Final metrics and joint controls pass.
- Owned CE readiness failures: venv launcher PID differed from its child;
  deferred cleanup needed an explicit lifecycle retry; a Lua fallback incorrectly
  called type on a zero-return expression. Corrected only the owned-toy harness.
  run-04 passed registers/XMM/MXCSR/TID and cleanup, with all owned processes closed.
- Evidence: deliverables/r16-metric-native-20261021/e2e-04,
  r16-caller-main-20261021/common_graph.py, r16-context-readiness-20261021/run-04,
  and r16-joint-native-20261021/FINAL_COMMON_RUNS.json under the F: build root.
- Remaining frontier: separate exploratory MAIN at78C9A9 reads an absent map
  field before full post-joint state integration. No guessed map/helper return
  was supplied. Integration is assigned to the verified Pro continuation package.
- Reproduction: project Python runner with NIOH3_BUILD_ROOT set to
  F:/Nioh3_r16_build; package TASK_FOR_PRO.md and exact copied scripts. No game
  read/write, save mutation or natural same-call acceptance is implied.

## 2026-10-21: Explicit decoder controls and nested checksum coverage

- Objective: use the unchanged returned v3 direct-CFG planner with an available
  local decoder, preserving pinned bytes and the sealed Pro implementation.
- Symptom/cause: initial10/11 controls; Capstone prints a small direct immediate
  target as5 while the planner's syntax contract requires0x5. It was correctly
  refused/classified as indirect before any real export was produced.
- Repair: normalize only typed immediate branch targets to hexadecimal in the
  explicit owner decoder. Retain the initial receipt, all sealed sources and
  planner rules. Final11 controls pass; three original closures export158 bytes.
- Packaging symptom: a temporary top-level hash regeneration excluded every
  file named SHA256SUMS.txt, including the nested export manifest. The structural
  validator caught the missing coverage before delivery. Repair excludes only
  the exact outer manifest. Final18 hashes/all19 ZIP members pass.
- Evidence/reproduction: F:/Nioh3_r16_build/deliverables/r16-static-v3-acceptance-20261021/
  owner_export.py, DECODER_CONTROLS_INITIAL_FAILURE.json, DECODER_CONTROLS.json,
  closure-export-01/ and BUILD_RECEIPT.json. Use the project Python runner and
  pinned F: build root. No GNU decoder, game execution or224-test replay claimed.

## 2026-10-21: Repeated incremental static exports

- Objective: provide enough immutable code/resources for autonomous Pro work.
- Symptom: v2/v3/v4 repeatedly returned new missing instruction RVAs despite
  successful earlier export checks; the owner requested fewer handoff cycles.
- Cause: the first exporter used pdata-fragment extents; later direct closure
  stopped at authenticated known code without traversing its downstream edges.
  Local coverage success was not whole-program static availability.
- Disposition: replace incremental RVA selection with complete pinned text,
  rdata/pdata, separate disk static/initializer views, complete1295 retained
  parameter payloads and708 checked catalog/runtime pins. Do not relabel the
  bank as fresh runtime state or a whole-chain native execution result.
- Evidence: r16-v4-full-static-20261021/BUILD_RECEIPT.json and the full-static
  Pro package's STATIC_BANK.json, RESOURCE_BOUNDARIES.md and FULL_STATIC_E2E.json.
- Reproduction/follow-up: owner build_full_static.py, project Python runner;
  full archive validation passes2422 hashes/all2423 files. Pro should resolve
  further in-image code locally and continue to the broad end-to-end objective.

## 2026-10-21 — game crash after Studio live addition while CE VEH debugging was active

- Objective: equip six items added through Nioh3Studio's live equipment addition, then capture a validation hit.
- Symptom: re-arming the CE observer after the additions raised a game "Fatal error" dialog
  (0xC0000005 at 0x11000190E, outside the module), and the CE session dropped. The next in-game save crashed.
- Cause: likely the interaction between the two debuggers. CE's VEH debugger had stayed attached since earlier
  observer windows, and Studio's insertion attaches and detaches its own Windows debugger. The addition
  journal shows a clean insertion (verified, allocation freed, original bytes restored, debugger detached).
  The exact faulting component is unknown.
- Evidence: `F:/Nioh3_r16_build/work/studio-state/equipment-add/`, `work/live1021-h2.STATE.txt`, owner screenshot.
- Disposition: the save written before the crash was intact and contained all six items, so nothing was lost.
  CE was restarted on a fresh game process.
- Follow-up: never run Studio live addition while CE's VEH debugger is attached. Add items first (or close
  CE), then attach CE. Also add an observer-side guard that refuses to arm when an external debugger
  attach/detach happened since the last arm.
