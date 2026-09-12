# Project instructions

- Write all source code, tests, comments, commits, and project documentation in English.
- Speak to the user in Simplified Chinese unless the user requests another language.
- Read `docs/knowledge/CURRENT_HANDOFF.md` and `docs/knowledge/INDEX.md` before changing generation, save, live-add, release, or reverse-engineering code.

## Research tasks

- Every research or reverse-engineering task must produce a self-contained handoff package for a Pro model. A research task is not complete when its evidence exists only in chat history, `.codex_tmp`, Cheat Engine state, or an unindexed scratch file.
- Use Codex for bounded evidence collection, reproducible capture tooling, controlled experiments, source inventory, and package verification. Package open-ended binary analysis, mathematical derivation, solver design, or unresolved causal analysis for the Pro model instead of spending an unbounded session on it.
- Follow `docs/knowledge/RESEARCH_HANDOFF_WORKFLOW.md` for required package contents, evidence grading, privacy boundaries, validation, and delivery paths.
- Put every package in a dedicated directory under `deliverables/` and also create a ZIP with a SHA-256 digest. Include a ready-to-run Pro task prompt and explicit acceptance criteria.
- Preserve confirmed facts, observations, inferences, and unknowns as separate categories. Never turn an intermediate runtime state or a plausible interpretation into a product claim.
- After the Pro result returns, verify its claims against the package evidence and applicable live acceptance before integrating it into production.
