# Arc UI debug acceptance (2026-10-13)

Status: source implementation and browser checks complete; the next clean
candidate must pass the matching packaged/native checks before delivery.

Base: `claude/arc-ui-preview` at
`2bde1e3c37815afe3978e226d995b70e6b014f96`. Local work:
`codex/arc-ui-debug-20261012`. UI-028 remains owner-confirmed: third
playthrough, level 1, playtime 163:43. No save-summary parser change is included.

## Changes and preserved behavior

1. Catalog tabs allow long labels to shrink and wrap within their card.
2. Compact save installation owns bounded scrolling and keeps uncertain,
   resolved, error, hint, and SavePicker states content-sized.
3. Short-height search controls allow the primary hint to wrap and scroll.
4. Short-height equipment plans keep confirmation/write/discard reachable,
   with separate scrolling for long plan details.
5. The owner-requested brand header uses one font role and baseline for author
   and version, removes the visible build SHA, and preserves compact author
   hiding. Full source identity remains available through diagnostics.
6. Initial automatic save selection waits for inventory initialization before
   starting character refresh. This prevents a deterministic BUSY refusal
   between SaveSession.select and CharacterEditor.load.

Labels, entry points, defaults, localization, selected-save ownership, review,
confirmation, recovery, discard, and no-replay rules remain component behavior.
The header SHA removal is an explicit owner-approved visibility change.

## Repeatable evidence

- Header matrix before the initial-selection fix: 780/780 minimum/wide checks
  and 390/390 short-height checks passed across three locales and two themes.
  Normal screenshots are separate from 50% pseudo-text stress screenshots.
- Initial-selection failure was recorded before the product fix in
  `deliverables/arc-ui-debug-20261012/web/initial-save-selection-baseline/`.
  The existing plan-footer E2E then passed 46/46 browser assertions, including
  initial character load, three locales, long plans, small windows, discard,
  proven pre-write refusal, and unknown-result recovery. Commits are scripted;
  no real save/game write is performed.
- The expanded matrix is integrated into `.github/workflows/frontend-v2.yml`
  after the existing text-fit gate, with artifacts retained on failure. No
  remote run has been dispatched for this local branch.
- The current exact source, hashes, final matrix, EXE and native results are
  recorded in [FINAL_REPORT.md](../../deliverables/arc-ui-debug-20261012/FINAL_REPORT.md)
  and [STATUS.json](../../deliverables/arc-ui-debug-20261012/STATUS.json).

The earlier d5bfb533 outer is immutable intermediate evidence and does not
contain the later header or initial-selection fixes. Its obsolete plan-footer
selector failed before any product assertion. The helper now uses current
navigation, embedded fonts, serialized discovery, current fixture responses,
and a diagnostic source-identity check.

## Acceptance boundary

Browser fixtures cover synthetic 1,500-row data, review/cancel flows, reachable
controls, text overflow, locales, themes and scale equivalents. Native runs
must use the matching clean-source outer and record actual CSS viewport/DPR,
normal/maximize/restore/minimum geometry, package hashes and safe shutdown.
CDP scale emulation does not establish real system-scale acceptance.

No version bump, publication, signing, push, tag, update feed, real-save write,
or live-game persistence acceptance is part of this task. Native discovery may
read real save summaries; synthetic character/plan tests do not use their
inventory or mutate their files. See
[ARC_UI_LAYOUT_WORKFLOW.md](ARC_UI_LAYOUT_WORKFLOW.md) for the durable process.
