---
name: nioh3-research-handoff
description: Build and verify a self-contained Pro-model handoff for Nioh 3 reverse engineering, PRNG derivation, binary analysis, or unresolved causal research.
---

# Nioh 3 research handoff

Turn bounded repository and runtime evidence into a reproducible research package. Do not use chat history as part of the deliverable.

## Start from the contract

1. Read `docs/knowledge/RESEARCH_HANDOFF_WORKFLOW.md`.
2. Write the exact research question, acceptance criteria, known facts, observations, inferences, and unknowns before selecting evidence.
3. Inventory existing probes, captures, source, tests, and prior conclusions. Prefer extending an existing collector or topic-specific builder over creating another parallel workflow.

## Build the package

- Create one descriptive, versioned directory under `deliverables/` and a ZIP containing that directory as its single top-level root.
- Include `README.md`, `TASK_FOR_PRO.md`, `ENVIRONMENT.json`, `SHA256SUMS.txt`, `evidence/`, and `project-source/`.
- Keep the package minimal but sufficient. Include negative results and abandoned leads only when they constrain the analysis, and label them clearly.
- Sanitize private saves, account data, local secrets, signing material, and non-redistributable binaries. Record identities and hashes instead of copying full binaries when possible.
- Make `TASK_FOR_PRO.md` directly usable as the Pro model's initial prompt. It must define required outputs and acceptance criteria without referring to this task, prior messages, or unstated context.
- Record the exact repository state, game/application version, binary or resource identities, controls, locale, evidence grade, and whether each capture read or wrote target memory or save data. Missing classification is an incomplete handoff.

Use [package review](references/package-review.md) when selecting contents or reviewing a finished package.

## Verify before delivery

1. Generate `SHA256SUMS.txt` only after the final package contents are in place.
2. Run `python tools/validate_research_handoff.py <package-directory> --zip <archive>`.
3. Inspect the final `TASK_FOR_PRO.md` and ZIP member list yourself; structural validation cannot prove that the research question is complete.
4. Report the package path, ZIP path, byte size, ZIP SHA-256, validation result, evidence limits, and remaining product boundary. Keep the final ZIP hash outside the archive to avoid a self-referential checksum.
5. Add the package status to `docs/knowledge/CURRENT_HANDOFF.md` and make it discoverable from the knowledge index when it becomes durable project evidence.

Treat the Pro response as a proposal. Reproduce its decisive claims against packaged controls and obtain applicable live acceptance before changing product contracts.
