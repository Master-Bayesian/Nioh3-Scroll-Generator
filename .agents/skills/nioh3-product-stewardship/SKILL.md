---
name: nioh3-product-stewardship
description: Preserve feature parity and maintain version records when changing user-visible behavior, migrating the Nioh 3 Studio UI, or preparing a version.
---

# Nioh 3 product stewardship

Keep product behavior traceable without turning routine work into a documentation ceremony.

## Route the task

- For a user-visible change or UI migration, read [feature records](references/feature-records.md).
- For version planning, readiness review, or packaging, also read [version records](references/version-records.md).
- Load only the references relevant to the current task.

## Working rules

- Repository product records describe shipped behavior and acceptance evidence. External planning tools are not product specifications.
- Update only the affected product records. Do not require a full catalog audit for an unrelated or cosmetic change.
- If an existing record is incomplete, verify and improve the affected scope, then leave its broader reconciliation status explicit.
- Preserve existing behavior by default. Ask the owner only for intentional product changes, removals, priority decisions, or acceptance that requires the player or game.
- Use `confirmed`, `observed`, `inferred`, `unknown`, `partial`, and `deferred` consistently. Never promote partial work or historical summaries to completed current evidence.
- Link the strongest current evidence instead of copying historical narratives into product records.
- Finish with implementation, records, and evidence in agreement, including explicit partial or deferred states. State any live or visual acceptance still missing.
