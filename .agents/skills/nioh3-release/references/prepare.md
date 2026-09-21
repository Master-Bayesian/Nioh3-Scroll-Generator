# Prepare

Use sections 1-3 of the [runbook](../../../../docs/knowledge/RELEASE_RUNBOOK.md).

1. Establish the intended version, changes and acceptance boundary from current project state. Preserve unrelated work and use an explicit commit whitelist.
2. Run deterministic source preflight and cheap environment checks. Choose the normal bounded profile unless a concrete change requires extended search.
3. Freeze one commit. Use the existing release workflow for one clean build, E2E acceptance and signing, after authorization to push/dispatch. A local-only request stays local.
4. Retain the six exact signed outputs and acceptance report. If a stage fails, route to diagnosis with the run and evidence; earlier success is not a substitute for the failed check.
5. Deliver the outer EXE and a concise statement of what was actually accepted. A successful preparation run is not a public release.

The hosted workflow is the command source of truth. Avoid copying its full build/test sequence into an agent ticket or inventing a second local build just to repeat compilation.
