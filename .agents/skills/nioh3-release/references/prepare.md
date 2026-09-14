# Prepare mode

Prepare mode normally ends with a local, inspectable readiness result or exact candidate package. It does not itself authorize a push, hosted workflow dispatch, tag, release, asset upload, update-feed change, or announcement.

## Read only what applies

- Follow sections 1-3 and 6 of the canonical [release runbook](../../../../docs/knowledge/RELEASE_RUNBOOK.md). Use its commands rather than maintaining another checklist here.
- Read the current version record under `docs/product/releases/` and compare its accepted, deferred, and missing evidence with the handoff, player documentation, localization, tests, and packaged UI.
- When preparing the outer EXE, use the acceptance contract in [install-free one-file delivery](../../../../docs/knowledge/TAURI_ONEFILE_DELIVERY_20260912.md).
- Load a previous publication record only to understand the preceding exact release identity. Verify current GitHub state separately before selecting a version.

## Preparation checkpoints

1. Inspect status and the complete intended diff. Separate release files from user-owned research, generated output, credentials, saves, and prior delivery artifacts.
2. Run `tools/preflight_tauri_release.py` early. A dirty-tree observation is expected during development; all other identity failures are blockers.
3. Synchronize version-bearing files and update only the affected product/version records. Regenerate the canonical catalogs, contracts, locales, and test inventory using the runbook, then require no tracked generated diff.
4. Run checks in the runbook's failure-cost order. Report unique tracked test counts, untracked developer-test differences, and hardware skips separately.
5. Freeze one reviewed commit. In a clean isolated checkout, run the preflight with `--require-clean --expected-sha <full-sha>` and then the canonical native-identity gate.
6. Build at most the requested candidate. Derive the ZIP and outer EXE from the same portable manifest, use new output paths, and preserve exact bytes for subsequent acceptance.
7. Record what passed, exact SHA/hashes/sizes, and remaining live or visual acceptance. Deliver the concrete package for owner review and stop before any remote mutation.

If product code changes after the candidate is built, discard its readiness status and restart from a new candidate SHA. Documentation-only interpretation does not let old bytes claim new product behavior.

## Optional hosted artifact preparation

Only when the owner explicitly requests hosted preparation, follow section 4 of the runbook. Complete the local gates first, then show the exact candidate SHA, source ref, push target, workflow file, dispatch ref, and planned commands. Obtain explicit authorization immediately before the push or dispatch.

Inspect the run by exact SHA. A failed run makes that SHA and its artifacts non-promotable: enter diagnose mode and do not dispatch the same known-bad SHA again. A successful run prepares signed artifacts but is not publication permission. Download them into a new directory, verify their exact identity, and stop for owner review before any tag, `main` promotion, release, asset upload, feed change, or announcement.
