# CPU loadout regression corpus

`katana-light.zip` freezes the existing PC2.0.2.0 katana-light CPU evaluator bundle used to reproduce the review's constrained-search failures. It contains table/frame/character/formula data, not the GPU library or a user save file. It is test input only and is not added to the product bundle.

The regression modifies captured armor records by +N, familiarity, and remodel weight, then compares actual Engine outcomes with Search results. The constraints must reject the first candidate and admit the second before Search is tested. The test also preserves unknown-defense set membership and checks that the unsupported candidate reaches the evaluator. Missing corpus data is an error, never a skipped success.

CI expands this archive under NIOH3_BUILD_ROOT and sets NIOH3_LOADOUT_TEST_BUNDLE before running `cargo test --test constrained_search`. The full public test uses the real evaluator; no constant-damage replacement or mocked search result is used. Its bounded game-data corpus does not establish every game's formula or live-game correctness.

## Legacy source dependency guard

`legacy_source_characters.bin` is the retained kusarigama-reaper-held character input (SHA256 `478ea1f838f42dba318d87248face8ec2790780954e8e21aaf472e16e530544d`). `legacy_source_current.bin` is its current inventory, SHA256 `e9e96450c54cd1ecd92e06d2adcb863a6f0f6fd623bb19c8fb5a640f86dedcbe`.

`legacy_source_set5.bin` (`e19ca49d3246a50a3dc3c2da21fd2ec738670e6f0f2a67dfde3f93fc3ab7c1c0`) replaces only five armor positions with explicit native-table and requirement-valid rarity5/level180 CD36 records. `legacy_source_set5_reduction.bin` (`54904d5344b35d6c14140d1cfb3f77defab2f239d79eeab2a87deb7f9fff5a76`) additionally uses accessory AF66 with zero-valued effect53D4/group38BA. These controlled bodies do not assert actual ownership or natural generation. The weapon bytes are unchanged. Native set production emits zero-valued8508 at five pieces and CB4B120 at the reduced sixth tier.

The source guard tests execute actual native table production and Engine/Search refusal paths with a deliberately constant score oracle, so they are source-admission and coverage evidence, not original MAIN/GPU damage validation. The separate production pipeline counterexample is retained under `evidence/cloud-v4/source-rebuild/legacy-source-dependency-audit` in the full candidate, including raw input/output hashes and the fresh-vs-frozen +135 attack difference.

## Physical assignment oracle

The `physical_oracle` unit tests enumerate a fixed point spread with one CPU evaluator. The fixtures pin distinct owned copies, swaps from each spare weapon slot, the ordered accessory domain, and allowed empty and retained slots. The receipt for each fixture records equipped words for both banks, the dispatched native set rows, outcomes and canonical score bits, the coverage partition, and ordered Top-K row indices.

`audit_tables.bin` is byte-identical to `tables.bin` in the PC 2.02 `katana-light.zip` bundle. Side 41164, row 0 dispatches reward 37872 at two pieces with flag 2, including when both qualified pieces are in the reserve bank. The synthetic flag 1 control changes only that row's flags byte and checks zero, one, and two special pieces. The tiny controls use the native equipment pipeline and a constant score formula. The `packaged_formula_scores_both_owned_copies` opt-in test uses the recorded katana-light formula and pins 1,038 damage for each copy.

Run `cargo test --locked --offline --manifest-path crates/nioh3-loadout/Cargo.toml --lib physical_oracle -- --test-threads=1` with the project build root and resolved Cargo target. Set `NIOH3_PHYSICAL_ORACLE_RECEIPTS` to an existing directory under the build root to retain JSON. For the opt-in formula test, expand `katana-light.zip` under the build root, point `NIOH3_LOADOUT_TEST_BUNDLE` at it, and add `--ignored` to the test arguments.
