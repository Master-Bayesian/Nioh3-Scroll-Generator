---
name: nioh3-ci-quality
description: Maintain or optimize CI quality gates, static checking, test selection, and sharding for the Nioh 3 Studio repository. Use for this project's CI or tooling changes and gate reviews, rather than unrelated repositories or ordinary gameplay research.
---

# Nioh3 CI Quality

Deliver measured CI improvements while preserving the project's static, state-model, native and E2E contracts.

## Establish the task

1. Discover the active Git root, branch, status and origin. Apply this project profile to `Master-Bayesian/Nioh3-Scroll-Generator` and its authorized worktrees/forks; confirm an unfamiliar repository identity before changing its policy.
2. Read the active worktree's `AGENTS.md` and [the maintained CI policy](references/ci-policy.md). Follow the sections relevant to the requested change. A bundled policy can guide an older worktree, but commands, versions and feature coverage must come from that checkout.
3. Distinguish a policy/documentation task, a CI implementation task and a product bug fix. Do not turn permission to document a workflow into permission to migrate dependencies or publish a release.

## Choose the work

- Inspect the actual workflows and scripts before proposing replacements. Measure step and test durations on an identified source/profile, separating execution, setup/cache and queue time. Prioritize the longest path and duplicate event work.
- Keep Node/tsx and Playwright. The frontend migration candidate is Oxlint's combined type-aware lint/type-check mode; Bun check is a reference/alternative. Preserve existing checks until the policy's migration acceptance passes. Rust continues to use its compiler, Clippy and rustfmt.
- Expand applicable lint coverage and verify the semantic rules enabled by this checkout with their corresponding fault controls. Do not equate a fast clean exit with equivalent file/rule coverage.
- For stateful changes, use explicit states, events, guards and outcomes; map changed TLA+ invariants/actions to code and exercise named faults. Follow the policy's evidence boundaries when native or game observations are involved.

## Implement CI changes

- Follow the policy's rollout order. Generate/build shared inputs before consumers; parallelize independent gates and use isolated mutable state/output paths. Use the repository's external build root, prepared Python runner and appropriate Cargo cache ownership.
- Treat historical test durations and dependency reachability as different datasets. Balance shards using duration estimates without dropping unknown or slow tests.
- Represent selection as explicit `Full`, `Affected` or `Blocked` decisions. Use explicit base/head/tested identities, transitive dependencies and non-imported inputs. Ambiguous reachability selects the complete profile; missing inventory blocks validation.
- Validate affected selection in shadow mode against full runs and deliberate faults before it can prune required PR tests. Merge queue, main and formal release run complete declared profiles.
- Make the final gate check source/profile identity, selected-case coverage, unique shard assignments and complete successful reports. It must reject missing, failed, cancelled or stale reports.

## Finish with evidence

For an implementation change, retain the decision and input identities, shard manifest, actual durations, diagnostics, formal controls, E2E outputs and repeat commands. Report measured improvement, required checks and remaining limitations; do not call a planned capability implemented.

For a documentation/skill-only task, validate skill structure, referenced paths, realistic decision scenarios and installed/source package identity. Do not run unrelated native/game experiments or claim product E2E acceptance from document checks.

Use the repository's signed preparation and exact-artifact promotion for authorized releases. This skill adds no publication or real-write permission; continue within the owner's existing task authorization.
