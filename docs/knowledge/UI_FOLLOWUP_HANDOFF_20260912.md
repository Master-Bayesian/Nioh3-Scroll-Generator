# UI follow-up transfer: 2026-09-12

## Latest owner instruction

Transfer all remaining known UI issues and release precautions to a new task
using **gpt-5.6-sol, xhigh**. There are **three** newest requests, not two:

1. At certain maximized/fullscreen resolutions and display scale factors, content
   goes outside the screen and no scrollbar becomes available. Windowed mode works.
2. Clicking outside a modal (inside the application), including Favorites, should
   dismiss it instead of requiring its close button.
3. Settings checkboxes should look like iOS switches: label left, switch right.

Continue implementation and bounded verification in that new task. Produce a
local **install-free, directly runnable single EXE**, then describe the changes
to the owner. **Do not push commits, tags, release assets, or update feeds.**
The owner has repeatedly rejected premature publication and installer EXEs.

## Exact checkpoint

- Original checkout: `F:\Nioh3_ScrollEditor`.
- Branch: `codex/tauri2-migration`.
- Product commit: `bca6ed2049ac595d138131c68f1f738946bb0ca6`.
- Version: unpublished `0.7.3`; do not bump merely for local revisions.
- At transfer, tracked product files are clean. The only new follow-up code is
  `apps/workshop/verify-collection-layout.mjs`: syntax checked, **not run or wired
  into acceptance yet**. Treat it as a draft helper, not passing evidence.
- The newer card-spacing/equal-height/action-alignment implementation has **not
  been made**. Delegated work stopped before those product edits; do not infer
  completion from earlier progress commentary.
- No app, compiler, or acceptance process remained at the final process check.
  The owner's previous Studio window was normally closed to release its
  single-instance lock. The game was not operated.

## Completed and verified at the product checkpoint

- Add current scroll works without cart membership. The dialog retains and
  freezes the candidate, mode and settings. It prepares automatically; actual
  writes still require explicit confirmation and keep automatic backups.
- Add to cart and View cart share a row. Cart subset addition stays inside View
  cart. Save selector and utility controls are compact; the primary direct-add
  button shares the utility row. Shorter English/Japanese preview button labels
  prevent an extra row.
- Sidebar brand space and all navigation/footer row heights stay fixed across
  collapse/expand; eight controls have equal vertical coordinates.
- Five filter-section explanations float outside clipped accordion containers.
  Expanding/collapsing closes old help. Opening help on a collapsed section works;
  outside click and Escape dismiss it.
- An owned, unexecuted live-add plan is cancelled when its review closes, including
  a late preparation response. Execution transfers ownership to recovery. Only
  a matching cancelled, zero-write receipt clears the matching recovery marker;
  ambiguity preserves it. Six new regressions cover this lifecycle.

Verification completed:

- TypeScript check and localization audit: passed, 563 UI strings.
- Desktop test suite: **57/57 passed**.
- Real WebView2 plus encrypted synthetic-save focused acceptance: passed on the
  packaged `bca6ed2` executable, including direct insertion with empty cart,
  frozen candidate/settings under a background search change, cart subset across
  page replacement, exact inventory (three records), and two exact backups.
- Default native window on this machine means approximately **1455x909 CSS pixels
  at DPR 1.1**, despite a 1600x1000 physical screenshot. That default plus emulated
  1600x1000 and 1280x800 fitted without result-pane scrolling. All three locales
  fitted the default addition controls. This does **not** prove the newly reported
  maximized/DPI combinations work.
- Five section-help popovers were hit-tested for clipping and dismissal.
- An earlier debug run of `apps/tauri/verify.mjs` passed search, favorites,
  synthetic inventory/restore and automatic clipboard replacement. It predates
  the final compact translated labels; keep that evidence scoped.

## Remaining required fixes

### A. Collection cards and preview spacing (owner's preceding report)

- In the cart, favorite-star and Remove from cart controls are not aligned.
  Put them in a consistent horizontal action row with aligned button boxes.
- Preview effect/rule line spacing is too tight. Improve readability while
  preserving fixed dimensions across results and the compact default layout.
- Cart/history cards currently change height when a larger enemy list wraps.
  Reserve consistent space, retain every enemy label, and keep cards equal in
  size. Include Favorites and comparison cards when they share the same renderer.
- The last large cards' bottom actions can be clipped. Ensure the relevant
  dialog/collection scroll container includes the full card and its actions.
- Cause observed in source: only `.result-scroll .scroll` has explicit fixed
  rows; dialog cards still use older natural-flow CSS. Cart star/remove buttons
  also lack the existing `.collection-actions` wrapper used elsewhere.
- Main preview currently has 528px regular / 441px compact card heights. After
  the compact footer fix there is only about 17px spare at the default viewport
  and 23px at 1280x800. Do not increase card height casually and regress fitting.
- Seed hints from the checked-in catalog: **12008, 12011, 12019**, with varied
  enemy counts. Regenerate through the real known-ID preview and record the
  actual counts; catalog hints are not a new generation oracle.

### B. Maximize/fullscreen and DPI overflow

Test real native maximize/restore in addition to emulated CSS viewports. Inspect
the existing `minmax()` column minimums, fixed row heights, nested flex/grid
minimum sizes and `overflow:hidden` rules. Provide a reachable scroll fallback
at genuinely constrained sizes, without reintroducing unnecessary scrolling at
the default size. Test useful 100/125/150% scaling equivalents and both shorter
and wider viewports; record actual CSS dimensions and DPR rather than assuming
physical pixels equal layout pixels. Do not change system-wide scaling settings
without a reason; use app/window-scoped tests first.

### C. Modal backdrop dismissal

Apply consistently to the application's actual dialogs. An outside click should
dismiss; interacting with content must not. Do not let a press begun inside and
released outside unexpectedly commit or dismiss. Preserve owned prepared-plan
cleanup and execution/uncertain-operation receipt handling; closing a view must
never replay a write. Section-help dismissal is already implemented separately.

### D. Settings switches

Style only Settings boolean controls as accessible switches with text on the
left and a switch on the right. Preserve labels, keyboard/Space behavior, focus
visibility, state persistence and the actual backend preference values. Keep
cart selection and write-confirmation checkboxes as their existing controls.

### E. Additional directly observed layout defect

The English scroll title can wrap onto a second line and collide with the
challenge-limit/recommended-level region. This was seen in the packaged English
acceptance screenshot. Review it as part of the shared card layout work. It is
not marked fixed. Preserve the complete title accessibly if abbreviating or
clamping its visual presentation.

## Files and testing entry points

- `apps/workshop/main.tsx`: result controls, dialogs, sidebar, settings, history.
- `apps/workshop/style.css`: accumulated overrides; inspect final effective rules.
- `apps/workshop/ScrollCard.tsx`: shared card content.
- `apps/workshop/CartActions.tsx`, `prepared-live-batch.ts`: protected addition UI.
- `apps/workshop/SectionHelp.tsx`: completed anchored explanation popovers.
- `apps/tauri/verify-add-layout.mjs`: verified native/synthetic-save harness.
- `apps/workshop/verify-sidebar-alignment.mjs`, `verify-section-help.mjs`:
  verified geometric helpers.
- `apps/workshop/verify-collection-layout.mjs`: **draft** helper for the new work.
  Exports `addKnownEnemyVariantsToCart`, `verifyCollectionLayout`; read its exact
  arguments before use. It uses ordinary preview/cart UI, no game writes.
- `apps/workshop/ui-translations.tsv`: authoritative localized labels;
  regenerate `ui-locales.json` with `tools/export_v2_ui_locales.py`.

Use the explicit Python environment:
`F:\Nioh3_ScrollEditor\.codex_tmp\v2-build-env\Scripts\python.exe`.
Node/npm: `C:\nvm4w\nodejs`; Cargo: `C:\Users\oudeb\.cargo\bin\cargo.exe`.
Set `NIOH3_PYTHON` and `PYTHONUTF8=1` per shell. The Studio is single-instance:
an existing player window causes an isolated test app to exit 0. Normally close
only Studio before testing; do not force-kill it or close the game.

The sidebar helper originally took element screenshots between measurements.
At fractional DPI, that can resize the viewport by one physical pixel and
produce a false 0.9px movement. It now measures all states first and takes full
window screenshots afterward. Preserve this ordering.

## Packages and build precautions

The clean cached build checkout is
`F:\Nioh3_ScrollEditor\.codex_tmp\v073-review-source`, detached at `bca6ed2`.
Its release build completed, and the output is:

`F:\Nioh3_ScrollEditor\deliverables\releases\0.7.3-add-ui-review-20260912\portable\Nioh3Studio.exe`

That is the inner app, **not a standalone one-file deliverable**. The folder's
`evidence/add-layout/verification.json` is passing packaged evidence. No new
outer EXE or update ZIP has been made for this checkpoint because the owner
added the card/fullscreen/modal/switch work before delivery.

The last complete outer EXE remains the older `ca75739` artifact under
`deliverables/releases/0.7.3-onefile-review-20260912/`, 31,746,127 bytes, SHA-256
`7067203816c605b32683103234fa0fb7c6fda60999a5e1ec971b64b351cba72c`.
It does not contain the new UI changes. Do not present either old file as a
finished package for all current requests.

Build the final candidate once after the UI is verified, from an exact clean
revision. Reuse cached dependencies/build directories; F: does not support
junction/hardlink assumptions. Cargo's incremental copy warning is expected.
Use `tools/build_tauri.ps1`, `tools/archive_frontend_v2.py`, and then
`tools/build_tauri_onefile.py`. Builders reject overwriting existing outputs.
Use a fresh task-specific delivery directory or preserve previous intermediates
under diagnostics with verified absolute paths. Never publish an installer in
place of the install-free EXE. Keep the source commit and byte hashes explicit.

For final acceptance, point `NIOH3_TAURI_EXE` at the produced **outer EXE** and
`NIOH3_UI_OUTPUT` at a fresh evidence directory, then run the focused harness.
Include new collection/fullscreen/backdrop/switch checks. Existing single-file
update/cleanup behavior was verified at `ca75739`; no updater source changed in
this UI checkpoint. Do not repeat unrelated game/reverse-engineering tests.

## Other project status: preserve scope

Read `CURRENT_HANDOFF.md`, `INDEX.md`, `V073_PRO_REVIEW_CLOSURE_20260912.md`,
`TAURI_ONEFILE_DELIVERY_20260912.md`, and `RELEASE_RUNBOOK.md` as applicable.
The earlier Pro review patches and transaction/backup/recovery fixes are
integrated; do not restart them as missing solely from old chat excerpts.

- Detailed bounded logs, automatic clipboard replacement, and prominent orange
  cart action are implemented. Preserve them and their settings behavior.
- Favorites and cart limits remain 50; history remains the latest three pages.
- Save insertion/edit/delete/restore remain usable at the title screen. Do not
  reintroduce a requirement to close the game.
- The owner closed unreproduced title-save corruption and intermittent live-add
  reports pending a fresh affected save and log. This is their issue disposition,
  not proof of a repaired root cause or of user fault.
- NG1/NG2 **player progression** real-time insertion acceptance was deferred by
  the owner; it is different from adding NG1/NG2 scroll records.
- Possessed-enemy research is frozen: seed 86872488 was a multiplayer-only
  positive, not a confirmed solo positive. No more CE, ETL or native research.
- Website work and usage statistics remain out of scope.
- Many unrelated untracked research files remain in the original checkout.
  Never stage all files or sweep/delete them. Stage only intended changes.
- Speak Simplified Chinese; source, tests, comments, commits and project docs
  are English. UI locale resources naturally contain Chinese/Japanese.

The complete transfer folder is
`F:\Nioh3_ScrollEditor\deliverables\handoffs\ui-followup-20260912\`.
It contains this document, the ready task prompt, exact status/diff metadata,
the draft helper, and selected screenshots/evidence. Its ZIP has a SHA-256.
