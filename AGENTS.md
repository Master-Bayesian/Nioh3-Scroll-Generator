# Repository guidance

- Speak to the user in Simplified Chinese unless they request another language. Write source code, tests, comments, commits, and repository documentation in English.
- Preserve unrelated tracked and untracked work. Put task delivery artifacts under `deliverables/`, not in the repository root.
- Continue through implementation, relevant verification, and repair until the requested outcome is complete or a real owner decision is required.
- Model routing: all implementation, test, documentation, and packaging work runs on DeepSeek V4.1 Flash agents; the primary model provides guidance, supervision, review, and decisions.

## Use context when it is relevant

- Use `docs/knowledge/CURRENT_HANDOFF.md` for current product and release state. Use `docs/knowledge/INDEX.md` and its linked subsystem evidence when changing generation, saves, live addition, or game-version compatibility.
- Run Python tests through `tools/run_python_tests.ps1`, which selects and validates the repository dependency environment; never invoke an ambient `python`, `py`, or ad-hoc `uv` environment directly.
- Migration gates must resolve Cargo target directories through `tests/migration/cargo_target.py`: an explicit `CARGO_TARGET_DIR` always wins, the fallback is the platform temp directory, and no gate may implicitly build into the repository volume.
- Use `docs/knowledge/RELEASE_RUNBOOK.md` for packaging or publication work.
- Use `docs/knowledge/RESEARCH_HANDOFF_WORKFLOW.md` for research or reverse engineering.
- Use `$nioh3-product-stewardship` when changing user-visible behavior, migrating the UI, or preparing a version.
- Use the matching repository skill for release, CE research, research handoff, product stewardship, or UI acceptance work.

## Product and safety boundaries

- Preserve shipped behavior unless the owner explicitly approves a change. Entry points, defaults, terminology, visibility, and removal are product behavior.
- When generation, save, or live-add paths are affected, preserve `GenerationContext`, exact RNG and replay behavior, the R4 finalized-preview/stage-one pairing, supported legacy Tk compatibility, and protected-operation recovery unless the task changes an approved contract.
- Treat static checks, automated tests, synthetic saves, packaged startup, and offline parity as bounded evidence, not live-game, visual, persistence, or propagation acceptance.
- Fail closed for unsupported game versions, unknown record semantics, ambiguous writes, and unverified legality rules.
- Treat QQ and community reports as evidence, not accepted requirements. The owner decides priority, target version, intentional behavior changes, and closure that requires player or game acceptance.
- Research must produce the self-contained Pro handoff required by the research workflow; verify Pro conclusions before integrating them into the product.
- Do not push, tag, publish assets, replace an update feed, or announce a release without explicit owner authorization.

## Improve recurring workflows

- When a completed task exposes a stable, reusable improvement to a repository workflow, update the matching skill, runbook, automation, or executable check before finishing.
- Prefer deterministic scripts and CI checks for mechanical rules. Keep skills focused on decisions and workflow boundaries; do not encode one-off failures or duplicate authoritative documentation.
- Validate every changed skill with the repository or system skill validator.
- For a substantive failure in research, build, packaging, or release work
  (not an ordinary expected test failure), immediately delegate a DeepSeek agent
  to append a bounded entry to `docs/research/EXPERIMENT_FAILURE_LEDGER.md`.
  Include the objective, observed symptom, root cause (or `unknown`), evidence
  paths, disposition, reproduction status, and follow-up state. Keep symptoms
  separate from rules; do not promote a one-off failure into a skill or active
  conclusion. The ledger is non-canonical, normally unloaded, and must not
  block the main task. Delegate simple documentation, archival, and audit work
  to DeepSeek when practical.

## Owner reporting contract

- The owner does not monitor builds or read source routinely. Work an assigned
  release task end to end and report proactively instead of asking for progress
  checkpoints.
- Send `send_message` to `/root` only on completion or a genuine owner/root
  decision blocker, then finish with the reported answer. Do not report routine
  progress.
- The completion report is evidence-based, 600-1000 English words maximum, and
  contains no raw logs or code. Keep raw logs and artifact inventories on disk
  and reference their paths.
- A release report states: the completed boundary and pending steps; the exact
  candidate SHA/version/branch/run URL and the asset identity (outer EXE and ZIP
  hashes); the parity result for the shipped fix; a gate table naming what
  passed, failed, or was skipped with evidence paths; any failure with root
  cause and retest; safety and scope facts; remaining risks; and the exact
  remote actions still awaiting approval.
