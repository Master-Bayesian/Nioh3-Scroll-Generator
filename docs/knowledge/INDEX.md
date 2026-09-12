# Nioh 3 scroll research knowledge base

This directory is the durable, versioned entry point for conclusions that must
survive chat loss, handoffs, and future game updates. It records what is known,
how it was learned, where the machine-readable source lives, and what remains
unproven.

## Storage model

- `docs/knowledge/`: human-readable current conclusions and capability status.
- `nioh3_scroll_editor/data/`: pointer-free product resources and localization
  catalogs consumed by the application.
- `audit/`: append-only raw captures, disassembly, parity corpora, screenshots,
  and experiment evidence. Private saves and account identifiers must never be
  promoted into public documentation.
- `research/`: reproducible capture and reverse-engineering methods.
- tests: executable claims that prevent a later implementation from silently
  contradicting the recorded knowledge.

The knowledge documents do not replace raw evidence. They link a conclusion to
its resource hash, native RVA, captured vectors, and regression tests whenever
those are available.

## Evidence grades

| Grade | Meaning |
| --- | --- |
| `native-byte-parity` | Offline output was compared with native output byte-for-byte on a declared corpus. |
| `native-control-flow` | Native table consumers and branch behavior were recovered and supported by controlled vectors. |
| `native-table` | A runtime parameter table was captured read-only and verified by hash, but not every consumer is closed. |
| `observed` | Reproducible in-game observation without a complete code path. |
| `inferred` | Best current explanation; must not be used as a product safety gate. |
| `unknown` | The available evidence does not justify a conclusion. |

Every claim is scoped by game version, record type/playthrough, rarity, normal
or exceptional caller path, and generation stage. A numeric ID alone is never a
global semantic key.

## Version index

- [Current development handoff](CURRENT_HANDOFF.md)
- [v0.7.3 direct addition and navigation review](V073_ADDITION_UI_REVIEW_20260912.md)
- [Install-free one-file delivery](TAURI_ONEFILE_DELIVERY_20260912.md)
- [Research handoff workflow](RESEARCH_HANDOFF_WORKFLOW.md)
- [PC v2.01 Crucible possessed-enemy research freeze](CRUCIBLE_POSSESSED_RESEARCH_FREEZE_20260911.md)
- [v0.7.2 follow-up fixes prepared for review](V072_FOLLOWUP_FIXES_20260911.md)
- [v0.7.2 Pro review patch integration](V072_PRO_REVIEW_INTEGRATION_20260911.md)
- [v0.7.3 item-by-item Pro review closure](V073_PRO_REVIEW_CLOSURE_20260912.md)
- [PC v2.01 title-save ownership research plan](TITLE_SAVE_OWNERSHIP_RESEARCH_PLAN_20260911.md)
- [PC v2.01 title-save static recovery integration](V072_TITLE_SAVE_STATIC_INTEGRATION_20260911.md)
- [Title-save investigation approach reset](TITLE_SAVE_APPROACH_RESET_20260912.md)
- [Game-version update pipeline](GAME_VERSION_UPDATE_PIPELINE.md)
- [PC v2.00.02](versions/pc-v2.00.02/README.md)
- [PC v2.00.02 player enemy-combination guide](versions/pc-v2.00.02/catalogs/enemy-combinations.md)
- [PC v2.01 approved compatibility profile](versions/pc-v2.01/README.md)

## Update procedure

For a new executable version:

1. Create a new version directory; do not overwrite the old one.
2. Capture the runtime tables again and record executable identity, RVA/AOB,
   locale, capture manifest hash, and table hashes.
3. Regenerate versioned catalogs from raw keys. Never copy names by assumption.
4. Re-run native parity corpora and structural regression tests.
5. Record changed, unchanged, and unknown subsystems in the capability matrix.
6. Mark old documents with `superseded_by`; never silently edit history into a
   different version's truth.
