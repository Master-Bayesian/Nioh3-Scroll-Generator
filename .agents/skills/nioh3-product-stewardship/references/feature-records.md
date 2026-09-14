# Feature records

Use `docs/product/FEATURES.md` as the stable catalog of shipped and intentionally unavailable product capabilities.

## For a product change

1. Identify the affected feature entries before editing.
2. Preserve the recorded outcome and essential workflow unless the owner approves a change.
3. Update the affected entries, tests, and player documentation when behavior changes.
4. Record the strongest completed evidence and name any visual, live-game, persistence, or propagation acceptance that remains open.

Keep each entry compact:

- stable feature ID and name;
- player outcome and entry point;
- essential behavior, defaults, and safety boundary;
- primary implementation and regression anchors;
- strongest acceptance evidence and known limits.

Do not copy implementation detail that is already clear from linked code or knowledge documents.

## For a UI or architecture migration

Make a parity table for the affected features only. Use `preserved`, `changed`, `deferred`, or `removed`. A non-preserved state needs the owner's decision and a short reason.

Check behavior, not just backend availability. Entry point, discoverability, terminology, defaults, intermediate states, cancellation, and error recovery can all affect parity.

If the catalog is not fully reconciled, label it accurately. Bootstrap the affected slice instead of blocking a small unrelated fix or pretending the inventory is complete.
