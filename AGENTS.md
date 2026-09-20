# Repository guidance

A short router: state what changes a decision, and leave procedure to the linked skill or runbook.

## Conventions

- Talk to the user in Simplified Chinese unless they ask otherwise; write code, tests, comments, commits, and repository docs in English.
- Use PowerShell 7; repository files may be CRLF.
- Run Python through `tools/run_python_tests.ps1`, never an ambient `python`, `py`, or ad-hoc `uv`.
- Build and test through the project build root only: `tools/run_python_tests.ps1` (Python/pytest temp) and `tests/migration/cargo_target.py` (Cargo targets) resolve `D:\Nioh3_v080_deliverables` on this host, or `NIOH3_BUILD_ROOT` / `CARGO_TARGET_DIR` when set; never write build or test temp into the checkout volume or the `C:` system temp.
- Reuse the resolved shared Cargo target for routine gates; an isolated target is disposable and must be removed with `cargo clean --target-dir` when its task ends.
- Preserve unrelated tracked and untracked work; put delivery artifacts under `deliverables/`, not the repository root.

## Routing

- The Astra root owns outcome, ticket design, prioritization, risk decisions, and acceptance; official DeepSeek V4.1 Flash
  workers execute implementation, tests, docs, packaging, and bounded evidence collection.
- Use `$nioh3-agent-orchestration` (`.agents/skills/nioh3-agent-orchestration/SKILL.md`) for role and model IDs,
  ticket shape, parallelism, context, review, and reporting; workers may be a different model, so keep tickets self-contained.

## Context

- `docs/knowledge/CURRENT_HANDOFF.md` for product and release state; `docs/knowledge/INDEX.md` for subsystem evidence on generation, saves, live addition, or game-version compatibility.
- `docs/knowledge/RELEASE_RUNBOOK.md` for packaging and publication; `docs/knowledge/RESEARCH_HANDOFF_WORKFLOW.md` for research and reverse engineering.
- `$nioh3-product-stewardship` for user-visible behavior, UI migration, or version prep; the matching repository skill for release, CE research, research handoff, or UI acceptance.

## Boundaries

- Preserve shipped behavior unless the owner approves a change; entry points, defaults, terminology, visibility, and removal are product behavior.
- For generation, save, or live-add changes, preserve `GenerationContext`, exact RNG and replay, the R4
  finalized-preview/stage-one pairing, legacy Tk compatibility, and protected-operation recovery.
- Tests, synthetic saves, packaged startup, and offline parity are bounded evidence, not live-game or visual acceptance;
  fail closed on unsupported versions, unknown record semantics, ambiguous writes, and unverified legality.
- Community reports are evidence, not requirements; the owner decides priority, target version, and closure needing player or game acceptance.
- Research produces the self-contained Pro handoff; verify Pro conclusions before integrating them.
- Do not push, tag, publish assets, replace an update feed, or announce a release without explicit owner authorization; release reporting detail is in `$nioh3-release`.
- A substantive research, build, packaging, or release failure gets a bounded `docs/research/EXPERIMENT_FAILURE_LEDGER.md`
  entry, delegated to a DeepSeek agent; fields are in `$nioh3-agent-orchestration`, and it never blocks the main task.
- Work the task through implementation, verification, and repair until done or a real owner decision is needed; define completion up front.
- Report on completion or a genuine blocker with a proportionate summary (conclusion, files, evidence, checks, remaining decisions; no padding or raw logs).
- Workers in the shared worktree must not run `git stash`, `git clean`, reset, or checkout/restore another owner's paths, or make any other worktree-wide state change; when isolation is needed, ask root for a managed worktree, or limit edits to explicitly owned paths.
