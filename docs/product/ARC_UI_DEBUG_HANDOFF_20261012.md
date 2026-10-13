# Arc UI preview: debug handoff (2026-10-12)

Branch `claude/arc-ui-preview`, merged with `main` at v0.8.8. Every push builds an
unsigned preview one-file EXE (`.github/workflows/ui-preview.yml`, artifact
`nioh3-ui-preview-exe`). Nothing here is released; do not tag or publish.

What the branch changes against `main` is in the owner-facing patch note
(`deliverables/PATCH_NOTE_claude-arc-ui-preview.md`, Chinese). This file is for
whoever debugs the branch next.

## Where the layout bugs came from

`apps/workshop/style.css` is the accumulated stylesheet of several earlier
designs (about 1,600 rules). The Arc redesign is a layer on top
(`apps/workshop/arc.css`, every rule prefixed with `html` to win on specificity)
that overrode the old rules page by page. Text broke wherever an old rule was not
overridden:

- fixed pixel `height` and `grid-template-rows` on boxes that hold text;
- `white-space:nowrap` + ellipsis on names;
- structural rules inside media queries whose breakpoint is 1100/1101px, while
  the window's minimum width is exactly 1100px (`tauri.conf.json`), so the
  "narrow" variants apply at the minimum window;
- `overflow:hidden` containers that the app scrolls with `scrollIntoView`
  (following the game cursor), which pushes headers out of sight. Use
  `overflow:clip` for boxes that must not scroll.

`tools/relax_fixed_text_boxes.mjs` rewrote the fixed heights into minimums
(141 declarations; buttons, inputs, icons, fixed-size boxes and scroll
containers keep their size). Before and after, every element box in the zh-CN
pages and states measured the same (`tools/ui-audit/web-measure.mjs`).

## Automated gates (CI: `frontend-v2.yml`)

- `apps/workshop/verify-text-fit.mjs`: web build; search (empty and with enemy
  and rule conditions), every editor section, favourites; zh/en/ja; 1100x740 and
  1600x1000. Fails on any finding of `apps/workshop/text-fit-detect.mjs`.
- `apps/workshop/verify-character-draft.mjs`: now also runs the detector on the
  equipment page at 1100x740 in every locale (fixture data, 4 items).

The detector (`text-fit-detect.mjs`, runs in the page) reports:
`clipX`/`clipY` (text clipped by its own box, unless the element has a `title`
holding the full text: a deliberate one-line summary), `escape` (text beyond a
clipping ancestor), `overlap` (text drawn over other text on the same layer;
popups and dialogs are a separate layer), `outside-card` (text or a control
outside the bordered card that holds it, ignoring scrolled content), `select`
(a select's chosen option wider than the select), `pageX` (page scrolls
sideways).

## Not covered by the gates

- Save management, "add new equipment", the settings popup, the cart and
  history dialogs, the backup restore flow, the re-sign flow.
- Real data volume: the equipment fixture has 4 items; the real list has ~1,500.
- Dark theme in CI (the live tools below cover it).
- Window heights below 740 and DPI scaling other than 100%.
- Native selects cannot wrap; a translation longer than its select still
  truncates (see the pseudo locale in `web-stress.mjs`).

## Tools for a running desktop app (`tools/ui-audit/`)

Start the preview EXE with WebView2 remote debugging, then attach Playwright:

```powershell
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9333'
Start-Process <path>\Nioh3Studio-<version>-ui-preview-<sha>-win-x64.exe
```

| Script | What it does |
| --- | --- |
| `live-pages.mjs <out> [locales] [themes] [sizes]` | Every nav page in each locale/theme/size: screenshot + detector report |
| `live-states.mjs <out> <locale>` | Every editor section open, equipment page in save mode |
| `live-equipment-plan.mjs [WxH]` | Equipment save mode: toggles a favourite and prepares a plan (never commits), reports the plan panel |
| `live-inject-css.mjs <png> <locale> <WxH> [steps]` | Replaces the running app's inline styles with the working-tree `style.css` + `arc.css` and audits. Linked stylesheets stay loaded, so rules removed in the tree can still apply; confirm in a rebuilt EXE |
| `live-eval.mjs <WxH or -> <js>` | Evaluates an expression in the app page |
| `web-stress.mjs <out> [sizes]` | Web build: all states in zh/en/ja plus a pseudo locale that lengthens every text by half |
| `web-measure.mjs <out.json>` | Records every element box in zh-CN states, for before/after diffs of a CSS change |

The live scripts switch the UI language through `localStorage`
(`nioh3-ui-locale`) and restore it at the end; if one is interrupted, the app may
stay in another language. They use `Emulation.setDeviceMetricsOverride` for the
window size; the real window is not resized.

Viewport emulation does not show OS scaling. Testers run at 125-175%; a
2048-pixel screenshot of a "small" window is about 1170 CSS pixels.

## Open items

- UI-028 is fixed in `654069c`: the summary block was recognized by the word at
  `+0x10`, which is per account, not a format tag. The tester's save now reads
  playthrough 3, level 1, 163:43, which the owner confirmed. The save is not in
  the repository.
- The equipment page at 740px height gives the header area (mode tiles, mode
  row, save picker, notice) about 290px; the list keeps about 240px.
- `relax_fixed_text_boxes.mjs` turned some legacy rules into duplicate
  `min-height` declarations (for example `.effects-list{min-height:190px;
  min-height:150px}`); they are overridden by `arc.css` and change nothing
  measured, but the legacy sheet should eventually be pruned instead of patched.
