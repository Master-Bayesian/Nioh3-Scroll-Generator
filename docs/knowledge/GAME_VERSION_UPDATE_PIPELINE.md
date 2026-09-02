# Game-version update pipeline

This pipeline converts a new Nioh 3 executable into an evidence-backed product
compatibility profile. It is fail-closed: no discovery or comparison step can
enable a new game version by itself.

## Inputs

- A reviewed baseline research profile in
  `nioh3_scroll_editor/data/game_versions/`.
- The installed game executable and one running process.
- A loaded save for each playthrough context that the product claims to
  support.
- Existing pointer-free baseline resources and native parity templates.

## Stages

### 1. Capture executable identity and live sections

Run `research/dump_live_pe_sections.py` for `.text`, `.rdata`, and `.pdata`.
The live dump is required because the on-disk `.text` image may differ after
loader transformations. The manifest records the four-part file version,
executable size and SHA-256, section RVAs, sizes, and hashes.

### 2. Relocate known sites

Run `tools/prepare_game_version_update.py` against the baseline profile. The
tool uses unique unchanged context anchors and emits both a migration report
and an explicitly unapproved candidate profile.

Ambiguous repeated constants must be resolved through independent evidence.
Use `research/find_runtime_data_xrefs.py` to compare RIP-relative reference
counts and callers, then pass the selected RVA as an explicit override. The
report preserves that override status instead of presenting it as automatic
discovery.

### 3. Validate live pointers and recapture tables

Run `research/dump_r4_finalizer_tables.py --profile <candidate>`. It verifies:

- the running executable file version and SHA-256;
- every profiled live code signature;
- the parameter manager and table structures;
- playthrough and mode globals;
- all deterministic effect and auxiliary tables.

The capture uses process read access only. Run it after loading the intended
save so the active playthrough vector is meaningful.

### 4. Build and compare pointer-free resources

Build a temporary R4 resource with
`research/build_r4_finalizer_resource.py`, then run
`tools/compare_game_version_resources.py` against the production baseline.

The comparison reports static resource equality separately from save-dependent
playthrough progress. The effective playthrough must match; unrelated progress
vectors may differ between saves and are not version identity.

### 5. Run isolated native parity

At the title screen, run the rarity-3, rarity-4, and rarity-5 native parity
tools with `--runtime-profile <candidate>`. They use temporary remote buffers,
never inventory records or saves. Run the auxiliary descriptor parity tool as a
separate gate because enemy, terrain, and special-rule construction has its own
native functions.

Any native mismatch blocks product support even when every parameter table is
unchanged.

### 6. Validate save layout read-only

Decrypt a copied current save into a temporary directory and load it through
`SaveInventory`. Confirm the `0xE8` record region, 400 physical slots, mapped
record types, generation serials, and existing records without writing or
installing a file.

### 7. Promote and integrate

Only after all prior gates pass:

1. Promote the candidate to an approved product profile.
2. Add the four-part file version to the compatibility registry.
3. Route native calls and the temporary auxiliary hook through the approved
   profile rather than module-level RVAs.
4. Reuse old resources only when their deterministic payloads are byte-equal;
   otherwise create a new versioned resource directory.
5. Regenerate player catalogs when any source table or localization key changes.

### 8. Regression and acceptance

Run the complete unit suite, route matrix, GPU parity tests, packaging smoke,
and updater checks. Produce an unpublished acceptance build. Product support is
not complete until a player verifies generation, save insertion, detail display,
and one real challenge on the new game version.

## Rust migration boundary

Research profiles are JSON so Python, Rust, CUDA host code, and D3D11 host code
can share one version identity. A v0.7 Rust core should consume the same profile
and own deterministic generation, legality checks, CPU exact replay, batching,
cancellation, and progress reporting. GPU bulk search remains mandatory;
neither Rust nor Python may silently scan the complete Seed space on CPU.
