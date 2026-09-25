# v0.8.0 regression fixes — 2026-09-25

Status: **source fixes with bounded evidence; development only**. This record
maps the v0.8.0 user reports and the 2026-09-22 static audit to the changes on
branch `claude/v081-regression-fixes`. It does not authorize a release, a tag,
or a live-game write, and it claims no live-game acceptance.

## User reports

| Report (v0.8.0) | Root cause | Fix |
| --- | --- | --- |
| Grace filter: `Grace filtering needs the rarity-specific Grace output map, which this development worker does not load yet` | `query.rs::check_grace_filter` refused every non-empty `grace_effect_ids`; the UI sends the single choice in that list too. The job layer also lacked the final `c.grace` acceptance. | Payload checks only in the parser; membership checked against the context-bound tables before compilation (R4 final Grace ids, the R5 measured map); `jobs::accepts` applies `search_jobs._run`'s final Grace filter; several Graces pivot on the union of their draw-1 preimages. |
| Terrain filter: `terrain option ids have no shipped reference implementation ...` | `QueryCompiler::compile` refused every `terrain_selection_ids`. | `crate::terrain` ports `terrain_choices` / `resolve_terrain_selections`; the catalog publishes and the search resolves the same ids; the row union feeds the native mask, `has_terrain_constraint` and the composed final acceptance on every route. |
| Temporary override: `...\data\pc_v2_02.json: file not found` | `runtime_app::identity()` passed the data root, not `data/game_versions`, to the profile loader. | One shared `profile_dir_for` rule for every runtime identity. |
| (after the path fix) `PC v2.02 runtime profile is not approved for product use` | The overrides resolved under the blanket native-write purpose, which PC v2.02 does not have. | New purpose `ProfilePurpose::TemporaryOverride`, approved for PC v2.02 only (`TEMPORARY_OVERRIDE_APPROVED_VERSIONS`); the challenge getter is version-selected. See the evidence below. |
| Editor "核对修改" does nothing | The catch used `(e as Error).message`; Tauri rejects with strings, so the status went blank. A successful check cleared the status and opened the confirmation off screen. | `errorText` normalizes every rejection; phase messages; the confirmation scrolls into view; an untouched recommended level skips the worker conversion. |

## Other audit items fixed

- **P0-01 resource binding.** The search compiler loaded the legacy v2.00.02
  tables while the materializer composed from the selected version. The
  factory now receives `Materializer::resource_version()`.
- **P1-01 grouped rolls.** Thresholds for any-of members are split out before
  the request invariants, as the reference does, instead of being refused.
- **P1-02 unconstrained R5.** Takes the full-family replay.
- **P1-03 level 0.** `fromSample` uses `??`.
- **P1-04 untouched header bytes.** `save.prepare_edit` keeps the stored header
  (including the `+0x08`/`+0x12`/`+0x31` mirrors) when every header field is
  unchanged, in Rust and Python.
- **P1-05 pending history.** Receipts are newest first and unresolved ones are
  never cut by the 128-entry window (Rust and Python).
- **P1-06 warnings.** `committed_with_warning` is shown as written-with-warning.
- **P2-01 backup time.** `save.backups` publishes the backup directory name,
  as the Python reference does, instead of an always-empty mtime.
- **Count editor.** It always used the PC v2.01 inventory addresses; it now
  selects the layout of the exact running version (PC v2.02 uses the natively
  accepted live-add inventory binding) and refuses any other build.
- **Live-add prepare (Pro review 2026-09-24).** Prepare records the actual disk
  checkpoint `D0` (`disk_persistence_baseline`) and no longer requires it to
  equal the live-before inventory; batch and legacy-parent rules follow the
  reviewed admission table. Execute/readback are unchanged.

## PC v2.02 temporary-override evidence

Static, from the retained section dumps (`Nioh3_v2.0.1.0.text.bin`, SHA-256
`F8799B5D…48023`; `Nioh3_v2.0.2.0.text.bin`, SHA-256 `4CEC8FB6…E6C29`), with
`tools/compare_text_sites.mjs`:

- `descriptor_complete` (v2.01 `0x20E195C` → v2.02 `0x20E50B8`): 0x500 bytes
  around the hook differ only in 31 rel32 call / RIP-relative displacement
  runs.
- Challenge-capacity getter (v2.01 `0x1028E30` → v2.02 `0x102AD60`): 0x400
  bytes of the body differ only in 28 displacement runs, and the 0x26-byte
  relocation-free prefix occurs exactly once in the v2.02 `.text`.

So register and stack use at both hooks is unchanged. Each session still
re-reads the exact hook bytes in the live process before writing. **Not yet
done:** one live PC v2.02 run of apply → reopen scroll → stop, with a save
backup, before this is published.

## Not changed

- The selected-save ↔ active-character binding flagged by the Pro review.
- `approval_status` / `product_enablement_allowed` in `pc_v2_02.json`; the
  blanket native-write gate (native generation/search) still refuses PC v2.02.
- The direct-save install lifecycle word (`0x06800082`, 2026-09-20) that makes
  a save-path addition unrevealed; it is already in v0.8.0.
