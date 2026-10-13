# Arc UI preview focused acceptance (2026-10-12)

Status: final browser verification passed; matching packaged/native candidate
verification remains pending.

The reviewed source is branch claude/arc-ui-preview at commit
2bde1e3c37815afe3978e226d995b70e6b014f96 before the follow-up CSS work. The
current source work is intentionally limited to four narrow UI fixes:

1. Catalog tab labels may shrink and wrap inside their catalog card. The tab
   labels, order, icon, count badge, entry point, and localization are
   preserved.
2. Compact save-mode installation keeps every SavePicker lifecycle state
   content-sized and gives the install card its own bounded scroll owner,
   including an unresolved receipt and the resolved success/error notice. Save
   selection, receipt recovery, confirmation, no-replay, and protected-write
   ownership remain component behavior.
3. The short-DPI primary-controls hint can shrink and wrap inside the selected
   conditions card under text pressure; labels and condition behavior remain
   unchanged.
4. The short-DPI equipment save plan lets its details area shrink/scroll while
   keeping the title-screen confirmation and write/discard footer reachable;
   plan ownership and discard behavior remain unchanged.

No version bump, release, publication, update-feed change, or real save/game
write is part of this record.

## Preserved product contracts

| Feature ID | Preserved Arc-preview contract | Focused UI change |
| --- | --- | --- |
| SCROLL-SEARCH | Search entry point, condition labels, locale strings, selected-condition behavior, and result controls remain unchanged. | Long catalog-tab labels wrap within the card instead of escaping it; the primary-controls hint wraps inside the selected-conditions card under short-DPI text pressure. |
| SCROLL-ADD | Direct/cart addition still requires the existing confirmation, compatibility, backup, exact-readback, and no-replay boundaries. | Compact save-mode recovery and result notices remain reachable through bounded install-card scrolling. |
| SAVE-BACKUP | Save selection, receipt recovery, restore/re-sign boundaries, cancellation, and no-commit behavior remain unchanged. | The search install SavePicker shares the same bounded lifecycle sizing; backup workspace styling/behavior is not rewritten. |
| SCROLL-COLLECTIONS | Favorites, cart, history, candidate identity, and action labels remain unchanged. | No collection logic or card redesign is included in this focused pass. |
| EQUIPMENT-EDIT | Existing/add equipment entry points, 1,500-row list ownership, plan confirmation, discard, and save ownership remain unchanged. | At short height, plan details use the internal scroll area while the title-screen confirmation and write/discard footer remain reachable. |
| APPEARANCE / SUPPORT-DIAGNOSTICS | Settings, theme choices, feedback controls, and diagnostic boundaries remain unchanged. | Popup behavior is recorded as evidence; no speculative popup/shell change is included here. |

## Evidence boundaries

The [native UI report](../../deliverables/arc-ui-debug-20261012/native/REPORT.md)
and [native readiness handoff](../../deliverables/arc-ui-debug-20261012/native/READINESS.md)
describe a downloaded unsigned preview EXE at the base candidate SHA. They
cover bounded native window, maximize/restore, locale/theme, popup, and empty
no-game states. Matching-candidate verification is still required for the
follow-up source CSS; the base package is retained as bounded evidence only.

The browser evidence uses a synthetic runtime/save/review bridge. It does not
open a game, read a user save, or call a protected commit. The initial
reproduction and follow-up state artifacts are linked from the focused failure
cases in
[FAILURE_CASES.md](../../deliverables/arc-ui-debug-20261012/css/FAILURE_CASES.md).
The authoritative final browser record is the
[web REPORT](../../deliverables/arc-ui-debug-20261012/web/REPORT.md), with the
matching [final-matrix artifact](../../deliverables/arc-ui-debug-20261012/web/final-full-matrix/verify-arc-expanded.json).
It covers three locales, two themes, 18 synthetic bridge runs, and 1,062/1,062
checks passed with zero failures. The bridge observed `commitCalled=false` and
zero forbidden write methods. The focused
[text-fit gate](../../deliverables/arc-ui-debug-20261012/web/focused/verify-text-fit-final.log)
passed, and the focused [character-draft evidence](../../deliverables/arc-ui-debug-20261012/web/focused/character-draft-final-css/verify-character-draft-final.json)
passed 58/58. The final source CSS SHA-256 is
`e3b19ff4ca35a7fe39774c91ac843621a9f57dea0bc3dc38785f757e447fde95`.
Earlier post-CSS reports remain diagnostic history and are not the final record.

A future matching candidate must be built from the reviewed source commit,
exercise the browser matrix and the relevant native harness against the same
candidate identity, and keep game/save/package/publication acceptance separate.
A matching outer one-file EXE, populated synthetic-save route, and packaged
artifact identity remain pending verification. Real-game persistence remains
pending explicit game/save acceptance.

## Evidence disposition

| Layer | Current disposition |
| --- | --- |
| Source/CSS review | Four narrow source changes described above; source CSS identity is recorded in the final browser report. |
| Browser fixture | 1,062/1,062 final matrix checks passed; focused text-fit passed and character-draft passed 58/58. |
| Native base EXE | Bounded popup/window evidence exists for the base preview package; matching-candidate verification pending. |
| Packaged final candidate | Pending clean source-coupled build and exact artifact identity verification. |
| Game/save acceptance | Explicit game/save acceptance remains pending and is outside this focused browser/CSS record. |
| Publication | Not requested and not performed. |

This note preserves feature parity claims without promoting browser fixtures,
base-package observations, or source checks to release or live-game acceptance.

The durable maintenance procedure for these layout boundaries is recorded in
[ARC_UI_LAYOUT_WORKFLOW.md](ARC_UI_LAYOUT_WORKFLOW.md). It does not add a
source refactor or claim automatic screenshot comparison.
