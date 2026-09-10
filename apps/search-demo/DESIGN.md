# Scroll search interaction demo

Open `index.html` directly in a desktop browser. No server, Node installation,
game or CE is required to review the packaged demo. Clipboard access depends on
the browser; if unavailable the visible ID can be selected manually.

Developer commands from the repository root:

```powershell
node apps/search-demo/build.mjs
python -m http.server 4177 --bind 127.0.0.1 --directory deliverables/frontend-v2/search-ui-demo
node apps/search-demo/verify.mjs
```

Scope: a reviewable browser demo based on the user's 2026-09-09 wireframe.
This is not the final Figma design or a replacement for the engineering workbench.

## Information architecture

- A narrow navigation rail; only Scroll Search is implemented.
- General instructions replace the wireframe's exclusive-choice heading.
- A shared selected-condition tray makes the complete query visible and removable.
- Two filter columns: effects (primary and secondary together) and grace on the
  left; enemies, nested special rules, terrain and challenge capacity on the right.
- Each module has a distinct, restrained background and a textual heading.
- Search settings and the primary action sit below the filter columns.
- Results use a numbered selector and an ordered scroll inspector, never a table.
- The game screenshot is a separately labelled reference, not a generated result.

## Interaction boundary

The demo filters explicitly illustrative local fixtures. It does not generate
legal scrolls, connect to a game, probe a GPU, edit saves, or invoke live addition.
Fixture seeds and affix combinations must not be treated as accepted game data.
Every search setting participates in fixture filtering. Recommended level is an
output metadata preference for this demo, not a seed-filter assertion. Changes
remain a draft until Search is pressed. Dirty results are labelled.

Primary effects, secondary effects, enemies and special rules allow multiple
selections. This prototype uses AND matching for selected conditions; it does
not assert the final native query semantics for duplicate effect occurrences.
The product integration must reuse SearchController and the existing catalog,
finalized-record and installation contracts after layout review.

## Diagnosis intake

The supplied report identifies v0.6.10 / v0.6.9, not v0.9.10. Its commands and
machine-maintenance recommendations are evidence, not execution instructions.
Current `app.py` still synchronously calls `cuda_seed_acceleration_available`
during UI construction; `seed_accelerator.py` calls the native function directly.
Thus the old Tk startup issue is not established as fixed. No attempt to reproduce
a driver hang or modify system settings was made for this visual demo.
Follow-up: isolated bounded accelerator probing, startup responsiveness, and a
supported disable/safe-mode path, with packaged fault-injection acceptance.

## Backlog intake

Preserve the latest screenshot's abandoned R3 unfinished-masterpiece request;
do not reopen it during UI work. Duplicate-name/value semantics, clean terrain
classification, transfer wrap behavior, CE-free execution, language coverage,
support-log copying and descending numeric sorting remain separate integration
items. Screenshot status/priority cells are not proof of implementation.

## Implementation boundaries

React and the repository's existing esbuild toolchain are reused. The output is
a self-contained HTML file with local assets embedded, placed in
`deliverables/frontend-v2/search-ui-demo`. No Electron/Tauri migration is implied.
English application copy follows the workspace instruction; Chinese game terms
are retained as sample content. A localization-ready product pass is separate.

## Visual system and review

The generated `concept.png` is a provisional implementation reference, not a
user-approved final Figma design. Canvas: #f5f3ec; navigation: #202624; action:
#315a48. Modules: sage #e6ebdf, sand #eee7d8, lavender #e9e5ec, rose #ece5e7,
blue #e2e9eb. Headings use Georgia/serif, controls Segoe UI with Chinese fallback.
Spacing uses 6/12/16/24px; borders are 1px and radii 4–6px. Shared Panel,
ConditionList, Choice, Icon and ScrollPreview components own repeated UI.

The concept and latest Chromium screenshot were both inspected with view_image.
Compared: navigation hierarchy, shared selection tray, two-column module order,
module color separation, bottom search settings, numbered result selector,
ordered affix rows, gold dividers and game-reference image framing. The native
1440x1000 desktop view and 1120x800 / 390x844 responsive views were checked.
The initial in-app-browser full-page capture duplicated a vertical strip; its
viewport capture was blurred. Playwright Chromium was used for clean visual QA,
after the in-app browser had already verified the core selection/search flow.

Intentional concept deviations follow the user's requirements and real data
boundaries: no invented enemy-subtype/boss filters, no unverified clean-terrain
claim, challenge capacity 4–7 instead of 0–5, no fake estimated match count,
actual local game icon instead of generated scroll art, no decorative forest,
and explicit demo-data labels. Above-fold title and general instruction match
the concept. Additional teaching, draft-state and fixture-boundary copy is
intentional. This is a faithful structural implementation of the user's
wireframe, not a claim of pixel identity with a final approved design.

Verification: strict TypeScript compile and 18 browser checks passed, with no
page runtime errors. See `verification.json`. Fixed during QA: stray game-icon
content, wrapped sidebar footer, small inspector text, and off-screen desktop
search controls. Production catalog coverage, numeric-value filtering,
multi-language chrome and native search acceptance are outside this demo.
