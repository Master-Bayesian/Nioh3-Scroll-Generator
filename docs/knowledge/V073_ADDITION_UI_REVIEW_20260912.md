# v0.7.3 addition and navigation review

## Product behavior

- Search results can add the current scroll directly, without cart membership.
  The review dialog freezes and retains that exact candidate and its addition
  settings. Preparation is automatic; the existing explicit write confirmation
  and automatic backup remain in place.
- Add to cart and View cart share a row. Subset/batch insertion stays inside the
  cart dialog. Save discovery, location and reload controls share the final row
  with the direct-add action, underneath the save selector.
- Sidebar expansion changes horizontal space and label visibility, while keeping
  brand space, navigation icons and footer controls at the same vertical positions.
- All five filter-section help buttons use an anchored floating layer. Expanding
  or collapsing a section dismisses its old explanation. Help opened while a
  section is collapsed is fully visible. Outside click and Escape dismiss it.
- Closing an unconfirmed live-add review cancels only its owned prepared batch.
  Execution transfers ownership to the existing receipt/recovery workflow. A late
  prepare response is also cleaned up; ambiguous cancellation preserves recovery
  evidence instead of replaying a write.

## Bounded validation

Run with the project's explicit Python environment:

```powershell
$env:NIOH3_PYTHON = '<python-environment>/Scripts/python.exe'
$env:PYTHONUTF8 = '1'
npm run typecheck
npm test
node tools/audit_v2_ui_locales.mjs
node apps/tauri/verify-add-layout.mjs
```

The focused WebView2 test checks untouched default window geometry (including
the machine's fractional DPI), 1600x1000 and 1280x800 viewports, all sidebar
positions, five help explanations, and direct/cart-subset insertion into an
isolated encrypted synthetic save. Each append verifies its exact backup and
inventory contents; preparation must leave save and backup bytes unchanged.
It also changes background search/form state while the direct review is open
to verify candidate retention and frozen settings. No game-memory operations
are permitted by this test. Set `NIOH3_TAURI_EXE` to validate a packaged app and
`NIOH3_UI_OUTPUT` for a dedicated evidence directory.

The six owned-plan regressions test confirmed cancellation, late preparation,
execution ownership, failure/mismatched receipts, and repeated cleanup. The
existing desktop suite and Tauri synthetic-save/clipboard acceptance remain
applicable; the UI work does not establish new in-game live-add acceptance.

## Delivery boundary

Keep this candidate at unpublished v0.7.3. Build an install-free one-file EXE
under `deliverables/releases/0.7.3-add-ui-review-20260912/` from a clean, exact
source revision. Record hashes and packaged acceptance there. The older local
one-file candidate remains historical. Do not push, upload, publish, or modify
an update feed without the owner's review and authorization.
