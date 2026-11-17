# CI quality and performance

The project adopts [the CI quality policy](../../skills/nioh3-ci-quality/references/ci-policy.md). This is the maintained policy source; it is bundled with the [Nioh3 CI Quality skill](../../skills/nioh3-ci-quality/SKILL.md) so it also works in older worktrees.

## Adoption status

| Area | Status |
| --- | --- |
| Quality requirements, tool roles, selection safety, evidence and rollout policy | Adopted in the policy and local `AGENTS.md` routers |
| Existing npm/ESLint/TypeScript, Node/tsx, Playwright and Cargo commands | Remain the executable baseline |
| Oxlint combined type-aware lint/type checking | Selected migration candidate; coverage and configuration acceptance still required |
| Bun check | Compatibility/reference checker; its read-only probe is not a completed toolchain migration |
| Vitest | No migration planned for this optimization |
| Duration-balanced shards, affected-test planner, final aggregate gate, event deduplication | Implementation and acceptance still required |
| Merge-queue/main full-run enforcement | Required target; the current workflows have path filters and no `merge_group` trigger |

Policy adoption does not claim that the workflows already implement these targets. Until a replacement is accepted, use the checked-in commands and retain their required coverage.

## Use and maintenance

Invoke `$nioh3-ci-quality` for work on these gates. Read `package.json`, the applicable workflows, Rust manifests/features, lint configuration and model runner in the actual worktree before choosing commands.

The skill package under `skills/nioh3-ci-quality/` is versioned. Deploy the same package to the user's discoverable Codex skill directory and verify byte identity. The installed reference is a deployment copy of the maintained policy, not a separately edited rulebook.

Root `AGENTS.md` and `.agents/` remain local under the existing repository ignore policy. Preserve existing worktree guidance and add a short skill/policy route; do not publish private agent files or force-add them to Git as part of this adoption.

The policy defines the rollout evidence and acceptance conditions. Workflow/tool implementation belongs in an isolated change with its own measurements, checks and review. This documentation change does not authorize a release or a real game/save operation.
