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
