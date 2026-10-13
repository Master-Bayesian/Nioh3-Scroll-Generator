# Documentation guide

`docs/` holds the repository's public documentation.

- [Product feature catalog](product/FEATURES.md) records shipped user-visible
  behavior, safety boundaries, and implementation anchors. Version records live
  under `product/releases/`.
- `knowledge/versions/` holds reverse-engineering records scoped to one game
  executable: [PC v2.00.02](knowledge/versions/pc-v2.00.02/README.md) and
  [PC v2.01](knowledge/versions/pc-v2.01/README.md). Each version gets its own
  directory; an older version's records are never rewritten into a newer
  version's truth.

## Maintenance rules

- Keep conclusions version-, evidence-, and scope-qualified. Automated and
  synthetic checks are bounded evidence, not a claim of live-game or player
  acceptance.
- Run `python tools/audit_documentation.py` after documentation link changes.
  The audit is offline and checks local Markdown targets.
