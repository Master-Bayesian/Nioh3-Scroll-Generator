# Current project handoff — 2026-09-02

This file is the durable entry point for the next development agent. It is a
working-tree handoff, not a release note and not evidence that unfinished code
is ready to ship.

## Repository state

- Repository: `F:\Nioh3_ScrollEditor`
- Branch: `codex/todo-321`
- Public baseline before this migration: tag `v0.6.6`.
- Release target: stable `v0.6.7`, explicitly approved by the user on
  2026-09-02 for immediate PC v2.01 compatibility publication.
- The earlier local `v0.6.7-beta.1` acceptance build remains historical and is
  not part of either update channel.
- Preserve unrelated untracked research, captures, packages, and user files.

## Most recent completed delivery

The complete effect-name catalog and the currently verifiable value ranges were
exported before resuming editor work:

- `deliverables/catalogs/Nioh3_PC_v2.00.02_Trilingual_Effect_Ranges_20260831.zip`
- `outputs/20260831-effect-catalog/Nioh3_PC_v2.00.02_Trilingual_Effect_Catalog.xlsx`
- 3,609 native effect IDs with Simplified Chinese, Japanese, and English names.
- Exact level 1–180, rarity 3/4/5 discrete raw values for the 51 effects that
  are reachable by captured scroll-generation contexts.
- Level-180 range attempts for all 3,609 effects; 182 contextual definitions
  are explicitly unresolved rather than guessed.

## Current working-tree changes (not released)

The working tree includes the following accumulated post-v0.6.6 work:

- exact PC v2.01 executable detection and a separately validated native
  runtime profile, while retaining PC v2.00.02 support;
- byte-identical v2.01 effect, enemy, terrain, and special-rule resources;
- product handling for the v2.01 feature-flag-9 rarity-5 header cap without
  changing the verified effect-slot algorithm;
- exact same-name enemy-form choices backed by a shared trilingual registry;
- regenerated enemy-role and legal-combination catalogs with 148 player-facing
  entries while preserving 142 native localized identities;
- a hybrid R4 DirectCompute matcher: one or two independent ordinary-effect
  groups run the exact GPU completion finalizer, while three or more retain the
  cheaper lossless `N-1` stage filter; both paths independently replay GPU
  survivors through the exact CPU finalizer;
- removal of the incomplete stage-one R4 inverse from product final-record
  routing and enforcement of a single advertised batch-trial budget;

The following pre-existing editor/runtime work is retained from the
v0.6.5/v0.6.6 development line:

- `nioh3_scroll_editor/savegame.py`
  - Adds exact read/patch helpers for the mapped scroll header: playthrough,
    mirrored level, mirrored recommended level, Seed, mirrored rarity, and
    transfer count.
- `nioh3_scroll_editor/app.py`
  - Adds Seed/playthrough-derived terrain, grouped-enemy, and ordered-rule
    preview.
  - Imports the selected legal-search candidate Seed into the local editor.
  - Edits header fields and all seven unrestricted effect slots in one existing
    backup-gated transaction.
  - Shows the verifiable native raw-value set as guidance while retaining the
    full uint32 local input domain.
- `nioh3_scroll_editor/runtime_auxiliary_override.py`
  - Installs an exact-signature-gated rel32 trampoline at v2.00.02 RVA
    `0x20DD558`.
  - Matches one displayed Seed and reuses native enemy-vector capacity; it does
    not call an unverified game allocator.
  - Supports repeated enemy keys, one terrain enum, and three ordered rule keys.
  - Writes both the enemy lookup key and its exact native role into each inner
    descriptor. The earlier lookup-key-only overwrite could leave a name that
    looked correct in details but was internally inconsistent for challenge
    consumption.
  - Restores the original instruction on stop/app close and intentionally
    retires the 4 KiB trampoline until game exit to avoid an unload race.
- `test_beta_editor.py`
  - Adds exact byte-boundary, validation, auxiliary-formatting, raw-value-hint,
    runtime-profile, trampoline, and rel32 regression coverage.

Validation completed from source without touching a real save:

- `py_compile` passed for the modified modules and test file.
- 79 editor tests passed.
- 401 release-reproducible repository tests passed after rebuilding the DirectCompute DLL and
  enabling the approved PC v2.01 runtime profile.
- At a live PC v2.01 title screen, 10,000 rarity-3 and rarity-4 native Seeds
  passed their complete parity gates, while 10,000 rarity-5 Seeds matched in
  every effect slot and differed only by the documented feature-flag-9 header
  cap. A final 64-Seed release-gate replay for each rarity also passed.
- Repeated native auxiliary descriptors matched the v2.00.02 control vectors,
  and read-only save decryption retained the 400-slot record layout.
- A 100,000-trial NVIDIA route smoke used CUDA for primary-only, Grace-only,
  rule-only, enemy-only, and Grace-plus-enemy searches, and DirectCompute for
  unrestricted ordinary-effect and ordinary-plus-auxiliary searches. No case
  entered the disabled CPU/Python bulk fallback.
- A withdrawn Tk application constructed and completed idle layout with
  `UI_SMOKE_OK`; the runtime pickers contain 487 legal enemy keys, 20 terrain
  enums, and 277 enabled rule keys including `None`.
- A running v2.00.02 process accepted and safely removed the application-owned
  trampoline. A three-Ichimokuren override was previously observed in details
  and a challenge, but the later exact-role write has source tests only and
  needs a new hit-backed challenge acceptance pass.

The user approved publishing the PC v2.01 compatibility release before a new
player-side challenge acceptance pass. The application offers deliberately
impossible layouts such as three copies of Ichimokuren as temporary runtime
state. It does not misrepresent enemy, terrain, or rule edits as save
persistence: they revert when the hook stops, the game regenerates the
descriptor, or the process restarts. Direct save-only persistence remains
unsupported.

## User decisions that remain authoritative

- Support rarities 3, 4, and 5. Rarity 5 is lower priority, not removed.
- The local editor must allow unrestricted edits even when the result is not
  native-legal or propagation-safe.
- Full header/effect editing is implemented in the working tree. Enemy, terrain,
  and special-rule customization is intentionally temporary runtime state, not
  a falsely persistent save edit.
- Show native raw-value ranges as guidance, but continue allowing any `uint32`
  raw value for local editing.
- Keep all source code and internal developer documentation in English. Speak
  to the user in Simplified Chinese.
- Preserve authorship as `MasterBayesian & Saber_Li`, the QQ group, and GitHub
  links in product-facing materials.
- Challenge-completion reroll prediction was abandoned as a product TODO on
  2026-09-01. Preserve the frozen evidence in `reroll-generation.md`, but do
  not resume implementation or request more captures.

## Remaining order

1. Finish the stable v0.6.7 build, signed update manifest, GitHub release, and
   public update-channel verification.
2. Post-release, live-check the exact-role runtime enemy overwrite after the application hit
   counter reaches at least one. A zero-hit profile still means the game reused
   a cached descriptor and no overwrite occurred.
3. Resolve native item key `0x3011` through its exact item-row localization text
   IDs; do not infer the item name from neighboring rows.
4. Collect player-side v2.01 generation, installation, detail-display, and live
   challenge evidence without representing release-gate automation as gameplay
   acceptance.
5. In the release after the PC v2.01 compatibility release, research the native
   mechanism behind purple or empowered enemy appearances and whether possessed
   Underworld forms of ordinary enemies can be represented as independent
   selectable identities. Do not add guessed forms before live validation.
6. After the current release, replace the Tk frontend in v0.7. Evaluate a Rust
   core with a Tauri frontend against Electron; GPU bulk search remains
   mandatory regardless of the UI host.
7. Validate and tune D3D11 compute on an AMD discrete GPU only when suitable
   hardware becomes available. Integrated-GPU parity is not a discrete-GPU
   performance result.

## Authoritative references

- `docs/knowledge/versions/pc-v2.00.02/project-status.md`
- `docs/knowledge/versions/pc-v2.00.02/effect-and-seed-solving.md`
- `docs/knowledge/versions/pc-v2.00.02/enemy-generation.md`
- `docs/knowledge/versions/pc-v2.00.02/special-rule-feasibility.md`
- `docs/knowledge/versions/pc-v2.00.02/scroll-legality.md`
- `docs/knowledge/versions/pc-v2.00.02/save-and-propagation.md`
- `docs/knowledge/versions/pc-v2.00.02/reroll-generation.md`
- `docs/knowledge/versions/pc-v2.00.02/evidence-register.md`

Before changing code, read this handoff and `project-status.md`, inspect the
current diff, and verify every claimed capability against the listed evidence
boundary. Do not infer live-game parity from compilation, tests, or static
analysis alone.
