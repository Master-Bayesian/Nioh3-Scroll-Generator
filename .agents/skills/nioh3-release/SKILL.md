---
name: nioh3-release
description: Prepare, diagnose, or publish the repository's Windows Tauri 2 one-file GitHub releases while preserving exact source and artifact identity. Use for release readiness, hosted release workflow failures, or authorized publication; do not use for withdrawn Electron or legacy Tk packaging.
---

# Nioh 3 Tauri release

Route the request before doing release work:

- **Prepare** builds release readiness or a locally reviewable candidate. Hosted artifact preparation is optional and requires a separate authorization checkpoint before its push or workflow dispatch. Read [prepare mode](references/prepare.md).
- **Diagnose** investigates a local or hosted release failure. Read [diagnose mode](references/diagnose.md).
- **Publish** promotes already verified bytes to `main`, an annotated tag, and a GitHub release. Read [publish mode](references/publish.md).

If the request is ambiguous, use diagnose mode for an existing failure and prepare mode otherwise. Do not infer publish permission from a request to prepare, build, test, review, or diagnose.

## Load current authority

Always read the repository `AGENTS.md`, [current handoff](../../../docs/knowledge/CURRENT_HANDOFF.md), and [release runbook](../../../docs/knowledge/RELEASE_RUNBOOK.md). The runbook owns the procedure and commands; this skill routes work and preserves its stopping conditions.

For the current default one-file product, also read [the one-file delivery contract](../../../docs/knowledge/TAURI_ONEFILE_DELIVERY_20260912.md). Load [hosted build fixes](../../../docs/knowledge/V070_HOSTED_BUILD_FIXES_20260909.md) only when diagnosing or when a matching gate fails. Use `$nioh3-product-stewardship` and the active `docs/product/releases/v<version>.md` record for version readiness.

Query live GitHub release state before choosing or describing the latest version. Publication records are historical evidence and can be superseded by a draft or withdrawal.

## Shared invariants

- Treat the candidate commit as immutable. Any product-code change creates a new candidate SHA and invalidates prior build evidence.
- Build the portable directory once and derive the internal update ZIP and outer one-file EXE from that same verified directory. Never substitute the inner app EXE, the archived NSIS installer, or a rebuilt publication artifact.
- Preserve the complete identity chain: candidate SHA -> clean `build-manifest.json` -> every ZIP member size and SHA-256 -> ZIP sidecar and CRC -> embedded outer-EXE ZIP/footer/stub identity -> outer-EXE sidecar -> production Ed25519 update manifest -> official tag URL and filename.
- Under the current one-file delivery contract, a payload digest detects corruption and the production Ed25519 signature authenticates updates. Recheck that contract before making claims about Authenticode or Windows publisher identity.
- Keep signing material, saves, captures, private dumps, local build state, and unrelated research outside source commits and delivery packages. Stage only an explicit whitelist; never use `git add .` in this checkout.
- Use an explicit Python executable through `NIOH3_PYTHON`. Treat tests, static checks, synthetic saves, and packaged startup as bounded evidence, not unperformed live-game, visual, persistence, or propagation acceptance.
- Store task delivery artifacts under `deliverables/` and preserve unrelated tracked and untracked work.
- A source freeze or backend checkpoint is not a release, and a release is not player or live-game acceptance. Name the completed boundary accurately.

Run the deterministic local source preflight before expensive work:

```powershell
& $env:NIOH3_PYTHON tools/preflight_tauri_release.py
```

Use `--require-clean --expected-sha <full-sha>` in the isolated candidate checkout immediately before release packaging or exact-artifact verification. This preflight is read-only and does not replace the runbook's generated-file, native identity, test, package, signature, or runtime gates.

## Remote mutation boundary

Pushes, workflow dispatches, tag creation or movement, release creation, asset upload, feed replacement, draft/public state changes, and release announcements are remote mutations. Immediately before any such mutation:

1. Finish the applicable read-only checks.
2. Show the owner the exact version, full candidate SHA, target branch/tag, workflow run or artifact identity, intended commands/actions, and whether rollback is possible.
3. Obtain explicit owner authorization for that concrete mutation tranche.

An earlier general request or approval for local preparation does not satisfy this checkpoint. Authorization expires if the SHA, version, tag, artifact bytes, hashes, target, or action sequence changes. Stop for renewed authorization rather than broadening the approved action.

If a hosted run fails, its SHA and artifacts are non-promotable. Diagnose the evidence, fix the cause, and use a new commit SHA; never dispatch the same known-bad SHA again or turn a timeout into acceptance through retries.
