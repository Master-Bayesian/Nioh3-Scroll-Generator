---
name: nioh3-ui-acceptance
description: Change or review the Nioh 3 Studio desktop UI with feature-parity, localization, native-window geometry, visual evidence, and packaged-app acceptance.
---

# Nioh 3 UI acceptance

Use the real desktop product and its existing verification harnesses. A DOM that renders or a passing typecheck is not visual acceptance.

## Define the affected contract

1. Read `docs/knowledge/CURRENT_HANDOFF.md` and the newest applicable UI handoff or review linked by `docs/knowledge/INDEX.md`.
2. Identify the affected entries in `docs/product/FEATURES.md`. Record whether each behavior is preserved, intentionally changed, deferred, or removed.
3. Inspect the shared component and final effective CSS rules. For text or layout changes, inspect all three locales; for interactive states, inspect the relevant backend/worker lifecycle.
4. Preserve entry points, defaults, terminology, keyboard behavior, cancellation, recovery, and error states unless the owner approves a product change.

## Implement narrowly

- Prefer shared components and design tokens over page-specific overrides. Reconcile or remove superseded rules when safe instead of accumulating another override layer.
- Keep typography roles explicit: application text, labels, values, metadata, actions, and headings should use a small shared scale. Paired labels and values must align intentionally and remain readable in every locale.
- Treat Chinese, English, and Japanese as first-class layouts. Update `apps/workshop/ui-translations.tsv` and regenerate derived locale data through the repository tooling.
- A close, outside click, Escape, navigation, or late response must preserve ownership rules for prepared live-add plans and uncertain writes.
- Do not weaken save, backup, recovery, version, or confirmation boundaries to make a UI flow easier to test.

## Verify in layers

Read [acceptance layers](references/acceptance-layers.md) and run the smallest complete set for the affected behavior. Geometry-sensitive, native-window, and packaged UI changes normally require a real native WebView2 window, native maximize/restore, recorded CSS viewport and DPR, screenshots, and the relevant focused harness.

Update only the affected feature/version records with the strongest completed evidence and explicitly name missing visual, packaged, persistence, propagation, or live-game acceptance. A draft helper, progress message, stale package, inner application EXE, or installer is not final acceptance evidence; only the reviewed outer one-file EXE from the matching candidate SHA can be the current install-free delivery.
