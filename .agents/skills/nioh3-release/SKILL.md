---
name: nioh3-release
description: Prepare, diagnose, or publish Nioh 3 Studio Windows Tauri 2 one-file releases using the bounded release pipeline and verified immutable artifacts.
---

# Nioh 3 release

Use the pipeline for mechanical work and evidence reports for decisions. Read AGENTS.md, the current section of [CURRENT_HANDOFF](../../../docs/knowledge/CURRENT_HANDOFF.md), and the relevant mode below. The [runbook](../../../docs/knowledge/RELEASE_RUNBOOK.md) owns commands; publication records own historical results.

- **Prepare:** produce a reviewable candidate without publishing. Read [prepare](references/prepare.md).
- **Diagnose:** identify and repair a concrete failed stage. Read [diagnose](references/diagnose.md).
- **Publish:** promote a successful run's exact signed artifacts, then verify public downloads. Read [publish](references/publish.md).

## Decisions that matter

- Use bounded release E2E by default: known seeds go directly through generation/preview; small searches exercise filtering and continuation. Request extended search only for a solver change or an explicit performance investigation. A frozen algorithm does not need a 158-million-trial rediscovery at every release.
- Keep broad unit, migration, reference-backend and research suites in development lanes. Run focused extra checks when changed behavior requires them; name the risk before adding a release gate.
- Check source/version/resource identity and runner prerequisites before compiling. The hosted runner has no game or GPU: use the existing version-resource fixture for discovery and bounded CPU-capable acceptance, with its synthetic scope recorded.
- Freeze one candidate and build its portable package once. Derive the update ZIP and outer EXE from it; consume the same bytes for acceptance and promotion. Use the shared external Cargo cache.
- Read the exact failed stage and its artifacts. Reuse valid evidence; invalidate only evidence affected by a repair. Check an existing run before starting another. Fix deterministic failures instead of retrying until green.
- Preserve source SHA, clean manifest, member hashes/CRC, outer launcher/payload identity and production Ed25519 signature. Product-code changes require a new candidate; test success alone does not authorize native game writes.

## Authority and coordination

Preparation and diagnosis do not imply remote-write permission. Obtain explicit owner authorization before pushes, dispatches or publication. Once the owner authorizes finishing a release, carry the scoped workflow through without repeatedly asking for the same approval; ask again for a materially different target, version, destructive replacement or safety boundary. The publication helper defaults to a read-only plan; `-Publish` is an explicit mutation boundary.

Use the repository orchestration skill for bounded worker execution. Give a worker the exact SHA/run, owned paths, completion artifact and escalation condition. Let it report completion or a concrete failure; keep the root out of repeated log polling and full-code rereads. Failure details belong in the failure ledger, not this skill.

## Completion

Publication means the public stable release, six exact assets, signed update feed and `releases/latest` all agree, verified from public downloads. Report published/not published first, then EXE and release links. Keep hashes, gate results, provenance and remaining acceptance limits in the machine-readable report and publication record. Close the release wakeup after verified completion.
