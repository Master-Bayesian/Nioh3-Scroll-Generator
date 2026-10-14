# Loadout persistence and effect text projection

Portable inputs, current-PR reproduction commands and evidence scope: [maintainer handoff](../research/LOADOUT_PLANNER_HANDOFF.md). Historical capture names below identify the accepted archive evidence.

## Local planner snapshots (2026-10-13)

`apps/workshop/loadout-storage.ts` stores a version-1 workspace under
`nioh3-loadout-drafts-v1` in the application's WebView local storage. The normal
desktop profile lives in application data and is shared across local candidate
updates. A web preview uses its own origin/profile; it is not the desktop data.

Every committed planner edit saves all named drafts and the current active ID.
The active draft is overlaid with its latest style, seven attributes and full
selection before serialization, rather than relying on the last switch snapshot.
Both styles, shared soul cores, scrolls, clan ranks/raw values, guardian levels,
skills, capacities, saved extras, equipment origin, native flags, remodel and
read-only audit/source metadata survive. Optional armorRemodels stores planner-only
six-mode overrides independently per style and armor slot; legacy snapshots omit
it without changing their source behavior. Replacement/clear resets the affected
override. Session broker references are omitted.
This is local planner data, not a game/save mutation or a portable broker token.

The precompiled Ajv validator validates the nested snapshot without runtime
compilation/eval under the desktop CSP. Regenerate it with
`node tools/export_loadout_storage_validator.mjs`; its schema is
`tools/loadout-storage-schema.mjs`. Planner identity/attribute invariants are
checked at the same storage boundary. Unsupported versions or corrupt data remain untouched;
automatic writes stay blocked. Explicit recovery first saves and verifies a
byte-exact recovery copy under a separate timestamp/UUID key, then saves current
planner data. Quota/read/write failures are visible and retryable. The existing
explicit Reset UI operation still clears local data. There is no new export,
cross-device sync or deletion feature in this change.

## Native Chinese effect text

`tools/export_loadout_effect_catalog.py` now uses the complete retained PC v2.02
Chinese text tree and the same verified weapon/item argument contract as the set
projection. Native group rows must match the shipped pointer-free resource;
text, weapon-store and runtime-code hashes must match the set evidence. Capture
blobs are checked against their size/hash manifest. No game attachment is needed.

The projection covers 1,152 groups and 3,609 equipment effect identities, with
13 distinct native unit text IDs. It fills native labels/descriptions and the
remaining unit templates. Equipment/scroll summary and soul-core details share
the formatter; unresolved template subjects are not replaced by generic buffs.
Existing resolved effect-specific names remain preferred over group templates.

The previously excluded Spirit group is not a time formatter. Reviewed native
`0x1BC1A08` tests row `+0xC == 0x54FE`, formats the raw integer and literal zero
using text `0x035D4DAF` (`{}<dot>{}`), then inserts this string into the outer
unit. The typed integer formatting helper is `0x1B28CE0`; a raw 42 displays
`+42.0`, not a scaled value. This branch is projected separately from generic
decimal modes and checked against the captured runtime text bytes.

Reproduce through `tools/run_python_tests.ps1`, passing the exporter:

- `--guardian-catalog`: retained `guardian_spirits_native.json`.
- `--clan-catalog`: retained `clan_protections_native.json`.
- `--native-text`: clan capture `native-extraction/native-text-01.json`.
- `--capture`: skill capture `native-extraction/discovery-02/capture.json`.
- `--runtime-sections`: retained `load-capacity-research/disassembly/live-sections`.

All 3,609 mapped effects resolve a label and supported numeric display for valid
test raw values. Unknown IDs, invalid raw values and unverified activation rules
remain explicit; formatting coverage is not proof of combat stacking/formulas
or game legality. Selected skill effects remain excluded from the summary.

## Acceptance boundary

101 focused loadout tests include full metadata round-trip, stale active-draft
prevention, version/corruption recovery, quota failure and every effect identity.
The existing 13,110 guardian/clan display observations remain unchanged. Browser
checks cover refresh, recovered style/configuration, save failure/retry, recovery
and 1366/1100/800 widths. Exact outer-EXE close/relaunch/profile receipts follow
under the local persistence-effects delivery. No live-game acceptance is claimed.
