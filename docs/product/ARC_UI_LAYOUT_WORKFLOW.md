# Arc UI layout workflow (2026-10-13)

This workflow records the focused lessons from the Arc preview review. It is a
maintenance guide, not a redesign plan and not a claim of WCAG compliance.
Source identities and current acceptance results are recorded in
[ARC_UI_DEBUG_ACCEPTANCE_20261012.md](ARC_UI_DEBUG_ACCEPTANCE_20261012.md) and
the linked task evidence. Keep earlier candidate evidence immutable.

## Define layout roles before changing a rule

Every affected surface should identify three roles:

- Header: title, navigation, status, and controls that must remain visible.
- Content: text or rows that may wrap or scroll.
- Footer: confirmation, cancel, recovery, and destructive-action controls that
  must remain reachable and must never be clipped.

Record the rectangle, computed display, overflow, min/max size, flex basis,
flex grow/shrink, and scrollHeight/clientHeight for each role at the failing
state. A geometry pass is partial evidence: it does not replace screenshot
inspection or behavior/ownership checks.

For inline metadata, use a shared font role and baseline alignment. Reset
inherited margins and line heights within that component. Measure the actual
text baseline with a temporary zero-height inline marker; mixed-script glyph
bounds do not reliably represent it. Keep technical build identity in
diagnostics when the owner excludes it from the normal product header.

Give exactly one element ownership of each scroll region. An inner text/row
region should use min-height:0 and overflow:auto when its parent is a bounded
flex or grid column. A footer should be flex-shrink:0. The outer card or side
pane may scroll when the complete card cannot fit, but an overflow:hidden card
must not clip an action area.

Intrinsic text sizing and explicit scrolling solve different problems.
min-width:auto or min-width:max-content can preserve an unbreakable intrinsic
width and push a card outside its parent. Use min-width:0 plus an explicit
wrap/overflow policy for a flex child that must yield width. Use an explicit
scroll owner with min-height:0 for content that must yield height. Do not use a
global shrink rule to conceal an unknown minimum.

## Flex and display:contents hazards

Flex basis is direction-specific. A basis intended to span a horizontal
SavePicker row can become a large vertical height when its child is promoted
into a flex column. Recheck every basis after changing flex-direction.

display:contents removes the wrapper's box while leaving its children in the
parent's layout. Recompute selectors, flex bases, grid placement, and scroll
ownership for those children. In the Arc preview, compact SavePicker children
became direct install-column items; unresolved-operation, Notice, hint, and
label bases therefore required a scoped content-sized reset and a bounded
install-card scroll owner. A grid-column fix was rejected after the final
container was shown to be flex.

Keep rules scoped to the affected direction and lifecycle. Horizontal SavePicker
surfaces must retain their existing row behavior. Prepared, uncertain,
resolved, cancelled, and error states must be measured separately. Clearing an
uncertain marker must not remove the visual recovery boundary before the
resolved/error message and final actions are reachable.

## Required matrix

For a changed surface, inspect all Simplified Chinese, English, and Japanese
locales, light and dark themes, the 1100px minimum-width boundary, and a
short-height/DPI-equivalent viewport. The current browser matrix uses 1100x740
DPR 1, 1600x1000 DPR 1, and 1170x620 DPR 1.25. A real native run must also
record CSS viewport and DPR, then exercise normal, maximize, restore, minimum
resize, and 100/125/150% scaling equivalents.

Exercise 1,500-row equipment data and long real catalog/plan text, not just a
four-row fixture. Include long labels in cards, headers, notices, selects, and
action rows. Verify first/last row reachability, filter/header persistence,
details scrolling, footer visibility, focus-visible controls, Escape/backdrop
dismissal, and no horizontal page overflow.

The state matrix must include:

- normal content with ordinary text;
- explicitly labelled pseudo-locale 50% text-stress pressure;
- prepared plan;
- uncertain result;
- resolved success or not-committed result;
- cancelled state;
- error/recovery state.

Normal screenshots and pseudo-stress screenshots are different evidence and
must be labelled as such. A duplicated-looking navbar in a pressure screenshot
may be the intentional text mutation; inspect the normal screenshot before
calling it a product duplication defect.

## Evidence and acceptance layers

The existing expanded E2E artifacts are the primary browser evidence. Preserve
the JSON report, screenshots, source revision, script hash, bundle hash, bridge
boundary, and forbidden-write result under the task deliverables directory.
A green CI run proves the covered assertions only. It does not prove native
window geometry, real DPI, package identity, save persistence, or live-game
behavior.

For native acceptance, use the exact outer EXE from the matching candidate,
record its hash and source identity, and inspect screenshots from normal,
maximized, restored, minimum-size, and scaling-equivalent states. Screenshot
comparison is meaningful only when OS, browser/WebView2, fonts, theme, CSS
viewport, DPR, and the reviewed source are held consistent. This workflow does
not claim that automatic screenshot comparison is implemented.

Engineering references:

- [MDN: min-width](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/min-width)
- [Playwright visual comparisons](https://playwright.dev/docs/test-snapshots)
- [W3C reflow understanding](https://www.w3.org/WAI/WCAG22/Understanding/reflow.html)

These references guide engineering choices; they do not establish WCAG
conformance for this product.

## Maintenance boundary

Future cleanup should gradually scope shared components and prune superseded
legacy CSS only when feature-parity and geometry evidence covers the affected
states. Preserve entry points, labels, locales, write confirmation, recovery,
and cancellation behavior while doing so. A broad stylesheet refactor is not a
substitute for a reproduced failure.

This workflow update does not perform legacy pruning, component refactoring, source CSS changes, harness changes, package work, version changes, or publication.
