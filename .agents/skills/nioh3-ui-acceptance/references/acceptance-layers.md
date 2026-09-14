# UI acceptance layers

Choose layers according to the risk. Do not report a higher layer from lower-layer evidence.

## 1. Static and contract checks

- TypeScript/type checks and focused unit tests.
- Localization export/audit and missing-key checks.
- Source review of shared rendering, final CSS cascade, worker messages, cancellation, and error recovery.

These checks can prove code and contract consistency, not appearance.

## 2. Focused interaction and geometry

Reuse the focused helpers under `apps/workshop/` and `apps/tauri/` when they cover the behavior. Assert complete cards, reachable controls, stable alignment, focus/keyboard behavior, dismissal boundaries, and scroll reachability. Record exact viewport dimensions and DPR with the result.

Emulated viewports are regression evidence, not native maximize/DPI acceptance.

## 3. Native visual acceptance

Run the actual WebView2 desktop shell. Check default, maximize, restore, and constrained window states. Cover useful 100%, 125%, and 150% scaling equivalents without changing system-wide settings when app-scoped testing suffices. Capture screenshots in all affected locales and inspect them; pixel coordinates alone can miss wrapping, clipping, hierarchy, and inconsistent typography.

Respect the single-instance product. Close only the Studio instance needed for the test; do not force-close the game.

## 4. Packaged-product acceptance

Build from an exact clean revision only after source-level UI acceptance. Point the focused harness at the final install-free outer EXE, store evidence under a task-specific `deliverables/` directory, and record the source revision and artifact hash. An inner executable, dev server, or older package is not evidence for the final candidate.

## 5. Game/save acceptance

Use isolated synthetic saves for save-bound UI flows when possible. Real-game writes require explicit authorization and the normal backup, confirmation, receipt, and recovery contracts. A successful UI action is not proof of persistence or multiplayer propagation.

## Typical commands

Select only applicable commands and use the explicit project runtimes documented in the current handoff:

```powershell
npm run typecheck
npm test
node tools/audit_v2_ui_locales.mjs
```

Use the current UI handoff for focused WebView2 harness names and environment variables; those entry points can change as the product evolves.
