# Documentation guide

`docs/` is the repository's human-readable documentation surface. Start here
to choose the document that owns the question before relying on an older
report.

## Entry points

- [Knowledge-base index](knowledge/INDEX.md) is the complete, classified list
  of top-level knowledge documents. It labels current authority, reusable
  workflow/reference material, active handoffs, and historical snapshots.
- [Current project handoff](knowledge/CURRENT_HANDOFF.md) owns the current
  product, release, and acceptance position. It supersedes a historical report
  when the two disagree.
- [Release runbook](knowledge/RELEASE_RUNBOOK.md) owns the current packaging
  and publication procedure. Publication still requires explicit owner
  authorization.
- [Research handoff workflow](knowledge/RESEARCH_HANDOFF_WORKFLOW.md) owns the
  required boundary between bounded evidence, a Pro research handoff, and
  product integration.
- [Game-version update pipeline](knowledge/GAME_VERSION_UPDATE_PIPELINE.md)
  owns the fail-closed procedure for a new game executable.
- [Product feature catalog](product/FEATURES.md) records shipped user-visible
  behavior, safety boundaries, and implementation anchors. Active version
  records live under `product/releases/`.
- [Experiment failure ledger](research/EXPERIMENT_FAILURE_LEDGER.md) is a
  non-canonical notebook for abandoned research approaches. It is not a source
  of current conclusions or default Codex instructions. For a substantive
  research, build, packaging, or release failure, append the objective,
  symptom, root cause (or `unknown`), evidence paths, disposition,
  reproduction status, and follow-up state; ordinary expected test failures do
  not require an entry.

## Ownership and maintenance map

| Documentation area | Owning document | Maintain when |
| --- | --- | --- |
| Product, release, and accepted evidence position | [CURRENT_HANDOFF.md](knowledge/CURRENT_HANDOFF.md) | A product decision, release position, verified acceptance boundary, or active blocker changes. |
| Release packaging and publication | [RELEASE_RUNBOOK.md](knowledge/RELEASE_RUNBOOK.md) | The supported package/update procedure or publication gate changes. |
| Version compatibility | [GAME_VERSION_UPDATE_PIPELINE.md](knowledge/GAME_VERSION_UPDATE_PIPELINE.md) and `knowledge/versions/` | A game build is investigated, approved, superseded, or retired. |
| Research and reverse engineering | [RESEARCH_HANDOFF_WORKFLOW.md](knowledge/RESEARCH_HANDOFF_WORKFLOW.md) | A research task is started, handed off, verified, integrated, frozen, or reopened. |
| Shipped features and active version scope | [FEATURES.md](product/FEATURES.md) and `product/releases/` | User-visible behavior, entry points, acceptance evidence, or version scope changes. |
| UI and engineering transfers | [UI_FOLLOWUP_HANDOFF_20260912.md](knowledge/UI_FOLLOWUP_HANDOFF_20260912.md) and the classified index | A live transfer changes; preserve completed checkpoints as history instead of rewriting them. |
| Historical evidence | [Knowledge-base index](knowledge/INDEX.md) | A newer authority supersedes a snapshot; add the supersession context without changing the historical record. |
| Failed research approaches | [EXPERIMENT_FAILURE_LEDGER.md](research/EXPERIMENT_FAILURE_LEDGER.md) | A substantive failed approach has future diagnostic value; keep it separate from current conclusions and skills. |

## Maintenance rules

- Keep conclusions version-, evidence-, and scope-qualified. Automated and
  synthetic checks are bounded evidence, not a claim of live-game or player
  acceptance.
- Do not silently rewrite a historical snapshot into current status. Add a
  newer current document or an explicit supersession note instead.
- Put new top-level `docs/knowledge/*.md` documents in
  [the knowledge-base index](knowledge/INDEX.md), with one of its four
  classifications.
- Run `python tools/audit_documentation.py` after documentation link or index
  changes. The audit is offline and checks local Markdown targets plus complete
  top-level knowledge indexing.
