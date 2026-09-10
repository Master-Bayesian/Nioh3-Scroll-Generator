# Legacy UI parity and preliminary acceptance — 2026-09-09

Historical checkpoint: see [v0.7.0 pre-push completion](V070_PREPUSH_COMPLETION_20260909.md) for the current status. Pending lists below describe their original snapshot.

The implementation-gap table below is retained as the original comparison.
It is superseded by [V2_PARITY_IMPLEMENTATION_20260909.md](V2_PARITY_IMPLEMENTATION_20260909.md),
which records the implemented parity work, newer runtime evidence and remaining
acceptance boundaries.

## Scope and evidence

Reviewed the current Tk implementation in `nioh3_scroll_editor/app.py`, its
catalog/native/save adapters, and the connected React screen in
`apps/search-demo-v2`. This is a working-tree comparison, not a release claim.
No user save or live game process was modified during this iteration.

Current verification:
- 43 Python tests across review integration, cart batching, protected operations
  and search contracts; includes injected NG1/NG2 native routing and group value
  thresholds. Native routing tests use a fake oracle, not a running game.
- 27 desktop Node tests, including real Python IPC and synthetic encrypted saves.
- 15 Electron surface checks: initial state, 25 results, resolved buff labels,
  menu removal, full-width 1600/2560 layouts and native-search prerequisites.
- 15 Electron save workflow checks: automatic discovery, reviewed edit, selected
  cart installation, inventory readback, backup listing/restoration/recycling,
  and renderer isolation. All files belong to an isolated synthetic fixture.
- 26 browser interaction checks: hover/layout (8), grouping/tints (14), and
  body-to-handle dragging, editable grouped values and one-line navigation (4).
- Strict TypeScript checks for both entry points; current development builds.

Evidence JSON/PNG files are in `deliverables/frontend-v2/search-ui-demo-v2`.

## This feedback round

- Removed the inset dark frame and maximum content width; the workspace fills
  its window. Removed the default Electron menu. Windows remain resizable and
  maximizable, with minimum dimensions to protect the two-column layout.
- No slider, result pager or no-match warning appears before a first search.
  The empty-state border is removed. Progress uses an indeterminate indicator
  plus actual status; the stale demo `0 / 200` counter is gone.
- Result count is 1–25, default 25; full batches highlight Next Batch. The compact
  result rail uses shorter aligned rows so all 25 remain accessible.
- The search catalog uses resolved effect labels instead of buff format strings.
- Conditions can be dragged by their label/body; controls retain their normal
  behavior. Dropping on another handle also groups. Each selected group member
  can retain its own value threshold. The worker accepts a group when at least
  one eligible member meets that member's threshold, without rewriting RNG.
- Saves are discovered through the existing save service. A sole result is
  selected automatically; multiple results require choosing an account and a
  one-based game slot. Manual location is a fallback. Search save addition has
  an inline selector, shared with the editor and backup manager.
- Backup management is in the sidebar: list, restore with review, move selected
  backups to the recycle bin, and open the backup folder. Its label is one line.
- QQ, GitHub and update actions have button styling.
- NG1/NG2 search and known-seed generation use the existing protected native
  scanner, with save-bound authentic templates and title-screen confirmation.
  Their results can use the existing save installation path. NG1/NG2 R4 remain
  custom-only and are explicitly labelled as not natural drops.

## Functional differences requiring a product decision

| Area | Legacy Tk | Connected review UI | Assessment |
| --- | --- | --- | --- |
| Update delivery | Signed manifest/hash validation, stable/Beta selection, download, idle transaction checks and managed replacement | Update button opens the release page | Significant parity gap. Restore the managed update workflow before presenting this as automatic updating. |
| NG1/NG2 search acceleration | Save/category-scoped draw-1 and primary maps, cache reuse and inverse candidate traversal | Protected sequential native scan with cancellation and bounded continuation | Functional path connected, but performance is not equivalent. Recommend migrating cache/inverse orchestration before promoting complex early-playthrough searches. |
| Native percentile information | Exact percentages are primarily available on certified offline paths; native records expose raw values | NG1/NG2 show raw values; percentile thresholds and score sorting are unavailable there | Explicit capability limit. Do not invent a percentile from a serialized raw value. NG3 grouped thresholds now work. |
| Existing-inventory multi-selection | Multiple occupied slots can be selected/deleted | Editor selects/deletes one scroll at a time | Optional batch productivity feature; backend already accepts multiple slots. |
| Backup/data directory administration | Open save folder, change data root, reset default data root; backup actions | Backup list/restore/recycle/open backup folder are connected; save-folder shortcut and data-root controls are absent | Optional management parity. Existing configured data root is still used. |
| Early-playthrough temporary editor content | Auxiliary preview depends on the draft seed/category and supports those contexts | Current editor enrichment uses the NG3 preview path | Non-NG3 temporary editing still needs an auxiliary-preview adapter. NG1/NG2 search auxiliary results are present. |
| Uncommitted draft auxiliary preview | Explicit preview from the edited draft seed | Current editor enriches the selected saved record and requires saving a changed seed before temporary application | Consider adding an explicit draft-seed preview action. |
| Temporary override detail/status | Native slot constraints, optional advanced fields, hook status/hits and lifecycle controls | Catalog choices for supported enemy slots/rules/terrain plus start/stop | Core operations connected; richer diagnostics and advanced controls are absent. Challenge capacity has no supported override and stays read-only. |
| Previous search batches | Appends candidates and preserves current selection | Replaces the active result page; cart retains explicitly saved candidates | Product workflow change. Decide whether a previous-batch history is needed. |
| Preview multi-delete/clear | Delete selected previews and clear all | Remove the current preview | Optional; does not delete save records. |
| Research/cache workflow | NG4/NG5 map capture and subsequent mapped offline preview | Native preview route exposed under research labels; cache capture/binding not surfaced in this screen | Do not promote to normal gameplay support. Existing install policy still blocks NG4/NG5. |
| Locale activation | Catalog resource coverage is separate from complete UI translation | Chinese screen; English/Japanese activation still unavailable | Remaining localization work, not evidence that translations disappeared in this round. |

Intentional changes already requested by the user: scroll cards replace the result
table; the cart replaces the comparison workflow; only value-oriented descending
sort options remain; manual mathematical cursors are hidden; the page limit is
25. These should not be reverted merely to match the legacy UI.

Additional release readiness items, separate from legacy parity: the review entry
still uses a development launcher; packaged default entry and new asset integrity
must be checked. Current cart and some visual preferences are session-local.
The copied UI log is not yet a complete support bundle. The live-add executor
still uses the existing configured adapter and must not be advertised as CE-free.

## Recommended order of user testing

Stop the sequence at a failed write/readback check. Preserve the operation receipt
and backup; inspect the result before repeating an uncertain write.

1. Close the previous app normally. Start `start-backend-ui.cmd`. Keep the game
   closed. Confirm empty initial state, 25 default, full-window/maximized layout,
   one-line navigation, resize behavior and the enlarged scroll text.
2. Search NG3 with 1 result, then 25; test R3/R4/R5, next batch, wheel/arrows/slider,
   sorting and a known seed. Check 0 and 26 are rejected. Add only to the cart.
3. Build two alternative effect groups with different thresholds. Drag by the
   label onto a handle, drag out, and verify matching results. Exercise long names,
   grace alternatives, enemies, rule variants and terrain/capacity filters.
4. Verify every discovered account/slot against the intended character. Multiple
   saves must not be confused. Inspect backup lists without restoring anything.
5. With the game at the title screen, add one NG3 scroll to the selected save.
   Refresh inventory: exactly +1. Load the game and verify the seed and effects.
   Return to title. Select 2 of 3 cart items, add them, and verify exactly +2.
6. Edit one existing test scroll using a legal value. Exercise Undo and Redo
   before committing. Verify the review and chosen account. Commit, restart/load
   and confirm persistence. Restore a known pre-test backup from title and confirm
   the original inventory. Recycle only a disposable test backup.
7. Test NG1 then NG2 with a matching authentic template in the selected save and
   the game at title. Start with one simple R3 candidate and a known seed. Test
   cancellation, then a small next batch. Test R4 only as an explicitly custom
   configuration. Missing templates must fail clearly without changing a save.
   Check native search speed before increasing constraints or count.
8. Enter a safe area. Test NG3 live addition: one scroll first, then 2 selected
   from a 3-item cart. Verify exact inventory changes, normal shrine save, then
   title-screen reload. All added seeds must persist, with no unselected item.
9. Test temporary enemy/rule/terrain overrides on an existing test scroll. Verify
   start, stop, normal reload restoration, and safe application exit. Do not
   challenge unrelated scrolls while checking the selected seed's override.
10. After feature decisions are settled, build the actual distribution. Repeat
    startup/search/save readback on its intended dependency setup and a second
    save slot. Test missing/unsupported game and unavailable GPU handling, plus
    package integrity and the chosen update path. A source-tree launch is not
    packaged-release acceptance.

Release sign-off requires exact save targeting, exact selected counts, correct
R4 revealed results, backup restoration and verified runtime cleanup. The real
game portions of steps 5–9 and packaged checks in step 10 remain unaccepted for
this screen. The automated checks above do not substitute for those outcomes.
