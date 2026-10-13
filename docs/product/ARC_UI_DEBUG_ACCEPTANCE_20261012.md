# Arc UI debug acceptance (2026-10-13)

Status: local preview accepted at frozen source
`3ba77597686913edb1f182c2cb8471c25d02aeaa`. Browser and matching exact-outer
native checks passed. This is not live-game persistence or public-release
acceptance.

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

- Final frozen-source browser matrix: 1,170/1,170 checks passed across three
  locales, two themes and three viewports. Independent text-fit passed. Normal
  screenshots are separate from 50% pseudo-text stress screenshots. Fourteen
  native-select truncations and two version-metadata/navigation overlaps under
  synthetic text pressure remain recorded as explicit pressure exemptions;
  ordinary native content has zero text-fit findings.
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
- The exact outer passed three standalone cold-cache/repair launches, the
  native character-draft fixture (58/58), and native plan-footer fixture
  (72/72). Package hashes and the clean frozen source were verified.
- Native window/header evidence contains 25 screenshots, zero text-fit
  findings and 25/25 baseline checks. Actual CSS viewport/DPR was 1455x910
  at 1.1, maximized 2328x1266, minimum 1100x743; both restores matched normal.
- The separate supported-width WebView2 scale check passed 12/12 across all
  locales/themes at 1100x740, DPR 1.25 and 1.5. This is CDP emulation, not a
  change to Windows scaling.

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

The older plan-footer script also forces a 935px CSS width, below the product's
1100px minimum. Its footer assertions passed but the Japanese title stacks
and overlaps navigation there. Those screenshots do not establish whole-page
acceptance; the unsupported-width issue is retained as a boundary. The extra
1100px scaling matrix covers the actual supported minimum without hiding it.

No version bump, publication, signing, push, tag, update feed, real-save write,
or live-game persistence acceptance is part of this task. Native discovery may
read real save summaries; synthetic character/plan tests do not use their
inventory or mutate their files. See
[ARC_UI_LAYOUT_WORKFLOW.md](ARC_UI_LAYOUT_WORKFLOW.md) for the durable process.
