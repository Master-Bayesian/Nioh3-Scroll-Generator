# Live equipment effect search — 2026-11-15

Status: local implementation and bounded UI acceptance complete; unpublished.
Base: `a9c20ef` (published v0.8.9 source), branch
`codex/live-equipment-effect-search`. The owner authorized a local-only source commit and a one-file acceptance package. The exact source SHA is recorded in the package manifest; no push, tag or public release is authorized.

## Behavior

Equipment & items -> Add new equipment -> Live editing now offers wanted
effects, conflict filtering, seed search and a selectable result list. Selecting
a result fills the existing seed field. Manual seeds and the original native
preview / confirmation / cancellation / durable receipt recovery remain available.

The runtime character snapshot does not expose generation difficulty/progress.
Each search therefore re-reads `save.character` for the selected target save and
uses that save's current difficulty and progress with the existing read-only
`runtime.equipment_seeds` item-grant search. The UI names this scope explicitly:
it excludes enemy/region bonuses and does not claim an exhaustive search of the
current live character's state. No save selection or missing progress is guessed.
Save and live character identity equivalence is not established by this change.

The subsequent native preview is authoritative. Every selected effect group
must have an exact-ID member in the preview before confirmation and execute are
enabled. The requirement groups are persisted alongside the operation UUID and
restored when reading a prepared receipt after remount/restart. A mismatch leaves
the existing cancellation and recovery actions reachable; it never retries or
automatically inserts. No runtime write or IPC schema was added or weakened.

| Contract | Outcome |
| --- | --- |
| Entry point, default fields and manual seed input | preserved |
| Wanted effects, mutually exclusive options, seed search/list | changed: added to live addition as requested |
| Save addition and its existing generation behavior | preserved; candidate grouping/exclusion extracted unchanged into a shared helper |
| Native preview, explicit confirmation, backup and execute | preserved; selected effects additionally gate confirmation |
| Unknown results, status, cancellation, recovery, no replay | preserved |
| Chinese, English and Japanese; light/dark; scroll access | preserved with new strings and focused acceptance |

## Implementation

- `apps/workshop/LiveEquipmentSeedSearch.tsx`: read-only search, save state,
  candidate picker, result selection, loading/errors and stale-response fencing.
- `apps/workshop/equipment-seed-options.ts`: shared grouping/exclusion rules
  and exact preview matching; reused by `CharacterEditor.tsx`.
- `apps/workshop/LiveEquipmentAdd.tsx`: search integration, persisted required
  effect groups and preview confirmation/execute guard.
- `apps/workshop/style.css`, `ui-translations.tsv`, generated `ui-locales.json`:
  narrow layout and localization changes.

Changing item/rarity/level resets wanted effects and rules. Changing any search
input, target save or observed save source digest invalidates results. Late
replies do not replace a newer selection. Searching performs no native preview
or insertion; read/search failures remain retryable without creating a receipt.

## Evidence

Evidence root: `D:/Nioh3_v080_deliverables/deliverables/live-equipment-effect-search`.

- `npm run typecheck`: passed.
- `npm test`: 113 passed, zero failed, one existing packaged-worker skip.
  Executed through `tools/run_python_tests.ps1` with the prepared project
  Python and D-backed temp; complete output is `frontend-tests.log`.
- `node tools/audit_v2_ui_locales.mjs`: 1,589 messages passed.
  Locale data regenerated with `tools/export_v2_ui_locales.py` through the
  project Python runner.
- `apps/workshop/verify-live-equipment-add.mjs`: 123 checks passed in Edge,
  production component and job observers with scripted replies. Covers the
  original recovery cases plus selection, read-only search, no-match/errors,
  stale save results, exact wanted constraints, native-preview mismatch and
  persistence across remount/reload. Reports and screenshots are in timestamped
  `equipment-browser-*` folders; early failures are retained separately.
- Current-source debug Tauri build passed using the shared
  `D:/Nioh3_v080_deliverables/build-cache/tauri-target`; frontend output was
  redirected to D via a worktree-local dist junction.
- `apps/tauri/verify-live-equipment-seed-search.mjs`: 93 checks passed;
  `native-r2/native-ui.json` records the debug executable SHA-256, geometry
  and screenshots. Real WebView2 native default/maximize/restore were exercised
  on the host at DPR 1.5 (observed CSS default 1283x729, maximized 1280x672).
  CDP-emulated 1100x740/DPR1, 1280x720/DPR1.25 and 1067x600/DPR1.5 cover
  all three locales and both themes. Emulated sizes are not system-DPI changes.
  Screenshots for Chinese, English, Japanese and preview confirmation were
  inspected. All game/save replies in this native presentation test are fixtures;
  no native builder/insertion or save write was requested.
- `git diff --check`: passed.

The initial broad frontend run lacked the prepared Python configuration; the
corrected project-runner execution passed. Initial browser/native harness runs
also exposed environment/fixture setup failures (missing bundled Chromium,
reload clearing the fixture UUID, fixture save history/discovery). These were
repaired in the harness and their original JSON reports remain in the evidence
root. They do not establish game failures.

Not performed: actual live-game search-to-builder agreement, inventory insertion,
shrine save/reload or multiplayer propagation.
The local-only one-file acceptance package is ready. It is not a public release or publication approval; its final artifact identity and packaged checks are in `D:/Nioh3_v080_deliverables/deliverables/live-equipment-effect-search/acceptance-package/delivery-report.json`.

## Reproduction

Use the prepared dependency junction and project Python/runtime configuration.
Build/test temp and browser profiles must remain under the resolved D build root.

```powershell
npm run typecheck
node tools/audit_v2_ui_locales.mjs
node apps/workshop/verify-live-equipment-add.mjs
node apps/tauri/verify-live-equipment-seed-search.mjs
```

The native verifier defaults to the shared debug shell/worker locations and
uses an isolated test profile. `NIOH3_UI_NATIVE_EXE` overrides the shell,
`NIOH3_UI_EVIDENCE_ROOT` overrides the report directory. The browser verifier
uses installed Edge by default; `NIOH3_UI_BROWSER_CHANNEL` can select a prepared
alternative. Set `NIOH3_UI_ONEFILE=1` with `NIOH3_UI_NATIVE_EXE` to accept the final outer EXE; this verifies its package manifest and records its payload hash. Keep other Studio instances closed while testing the native shell.

## Final local one-file acceptance

The owner authorized local commits and one-file preparation. The frozen product
commit is `f151c7a786b0c76857a974deb9a4bc8868985587`, with a clean package
manifest. The final outer EXE is
`Nioh3Studio-0.8.9-live-equipment-test-f151c7a.exe` (11,704,497 bytes), SHA-256
`e202410431f8fbb89e55ed0eb310d7269ac2f86ba2f09940c2bb552d70887496`.
No push, tag, signing-feed replacement or public release occurred. The UI version
remains 0.8.9; the filename identifies this local test.

The existing build pipeline ran from an ASCII D-backed clone of that exact
commit after Windows RC rejected the original Unicode source path. The complete
failure and successful build logs remain in the evidence root. All 765 archive
members passed size/hash/CRC validation, and the outer payload was verified.
The final packaged Rust graph passed three real framed-IPC handshakes/read-only
representative requests and two exhaustive 16-bit seed searches under synthetic
progress, including an exact requested-effect filter. This does not establish
agreement with the current live character.

`outer-acceptance-r2/native-ui.json` passes 95 checks against these exact outer
bytes, including package/source identity, explicit profile isolation, all three
locales, light/dark, native maximize/restore and constrained emulated viewports.
Game/save replies in this UI test are fixtures. Live insertion and shrine
save/reload remain owner acceptance.

The first outer verifier run omitted `--user-data-dir`: the release shell ignores
the debug-only profile environment fallback, and the verifier cleared the real
profile's localStorage. It performed no game insertion or save write. The seven
removed keys (theme, locale, remembered save, equipment favorites, loadout drafts,
scroll edit mode and effect scope) were recovered exactly from the retained
pre-clear LevelDB state in an isolated profile, restored and read back equal.
The raw database copy and recovery report remain private in the delivery evidence
root. Broker favorites were not cleared. Harness-only commit
`39c317f89a19421c5dc922352197c08f7d8fc2aa` adds explicit profile arguments,
asserts isolation before clearing, and settles startup reads before fixture
replacement. The same final product EXE was retested without rebuilding.