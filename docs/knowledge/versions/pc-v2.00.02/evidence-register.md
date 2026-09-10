# Evidence and supersession register

This register prevents a later summary, test name, or stale report from being
mistaken for stronger evidence than the files actually contain.

## Current evidence

| Claim | Evidence | Strength and limitation |
| --- | --- | --- |
| R3 stable-record parity | `audit/parity/ng3-r3-live-parity-20260829.json` | 10,000 deterministic Seeds, zero stable-record mismatches. Aggregate report only; runtime header byte `+0x1B` differed and is explicitly excluded. |
| R4 stage/final parity | `audit/parity/ng3-r4-live-parity-20260829.json` | 10,000 deterministic Seeds, zero stage/final/accepted-index mismatches. Aggregate report only. |
| R4 retained native pairs | `test_fixtures/r4_native_corpus/base/` and `distributed/` | Ten stage/final pairs, nine unique Seeds. Sanitized tracked copies differ from private captures only in the eight origin-account bytes. |
| Earlier-playthrough R4 Grace | `audit/scroll-type-selector-matrix-20260827.json` plus the PC v2.01 live validation reports | A stored P2 Seed `212942103` retains final slot-5 Grace `0x5012`; a second private P2 save retains `0x4FE4`. These records do not prove legal drops. PC v2.01 native scratch validation closes the P1/P2 R4 constructor/finalizer path; both configurations remain custom-only. |
| R5 record parity | `audit/ng3-r5-native-parity-live-10000-20260829.json` | 10,000 deterministic Seeds, zero full-record mismatches on PC v2.00.02. Aggregate report only. Rarity 5 is exposed by the product; on PC v2.01 the native header cap and product preservation policy are documented separately. |
| Complete auxiliary parity | `audit/p1_static/COMPLETE_AUXILIARY_PARITY_20260829.md` | All three class branches and 22 native vectors; strongest for third playthrough and `caller_option = 0`. |
| Enemy role structure | versioned `enemy-roles.json`, native table SHA, class control flow, and `tests/test_effect_seed_solver.py` | Complete 487-row table; two process captures produced the same row hash. |
| Recommended display curve | `recommended_level_curve.json` plus native callers | Exact internal-to-display conversion. No closed consumer from challenge descriptor to AI combat level. |
| Save-wide generation serial | `deliverables/fb014-fb016-20260831/FB016_SAVE_AUDIT.json` plus the later live field-isolation test | Controlled save and live evidence: scroll `+0x28` collided with a structured non-scroll item serial and caused equipment rendering. Editing `+0x1C` alone did not fix the item identity. |
| Exchange tuple | `emaki_exchange.py` plus receive/send disassembly | Effect slots absent; receiver rebuilds canonical effects. Final propagation still needs a second account. |
| Scroll-local `+0x1C` key | `audit/save/inventory-key-collision-20260830.json`, `tests/test_beta_editor.py`, and the later FB-016 field-isolation test | Duplicate donor keys can hide raw-appended records, but `+0x1C` was disproved as the cause of the equipment-rendering FB-016 case. Its broader semantics remain unresolved. |
| Reroll candidate builder | `reroll_effect_helper_callers_20260831.json`, `reroll_state_functions_20260831.json`, `reroll.py`, and `tests/test_reroll.py` | Static control flow closes Seed+counter RNG, legal pool, five draws, and counter mutations. No retained live candidate vector yet; one per-save group eligibility set remains to be inferred by controlled capture. |
| Runtime enemy descriptor fields | `runtime_auxiliary_override.py`, captured native candidate rows, and `tests/test_beta_editor.py` | Inner entry `+0x04` is the enemy lookup key and `+0x08` is its exact native role. The working tree writes both. This is source/static evidence only until a new hit-backed challenge pass. |
| Special-rule item key `0x3011` | bundled v2.00.02 `item.bin` SHA-256 `1CDAEF2A...FEC21938`, unique row 1771, and `probe_scroll_auxiliary_text_catalog.py` | Exact native row and candidate localization IDs are closed; the displayed item name remains unresolved until the current-locale runtime pool is queried. |
| Stable v0.6.9 publication | GitHub Actions run `33648790533`; downloaded `latest.json` and executable under `deliverables/v0.6.9/` | 413 tests, one-file build, Ed25519 signature, 17,927,879-byte size, SHA-256 `E520B92C5A70462399D5898B1E85E41420D7E82AB17745274DFE6C2832EDFD7A`, startup, cleanup, and public-update lookup passed. No game or save write. |
| Stable v0.6.10 publication | GitHub Actions run `34080366708`; downloaded `latest.json` and executable under `deliverables/v0.6.10/` | 423 tests, one-file build, Ed25519 signature, 17,933,766-byte size, SHA-256 `E2ABD12562C615155E0B97F146B66BBB7FD6209863ACEE9391321D12AEC9621E`, startup, cleanup, and public-update lookup passed. The reported R4 Seeds also have separate live-native and in-game reveal evidence. |

The 10,000-Seed parity files are real native/offline aggregate results, not
10,000 retained pairs of raw records. Do not describe them as a forensic raw
corpus. The smaller R4 fixture set is the portable byte corpus.

## Current authority and supersession

| Topic | Current authority | Earlier material that must not be used alone |
| --- | --- | --- |
| Auxiliary generation | `COMPLETE_AUXILIARY_PARITY_20260829.md`, resource v3, current implementation/tests | Early class-1-only or generator-root reports. |
| Final effect names | `effect_names_multilingual.json` and its native text IDs | CT lists, hand-maintained effect pools, and slot-global raw-ID mappings. |
| R4 slot 5 | finalizer engine + final record | Historical 65,536 stage-one map interpreted as final names. |
| `0xBABD` | context-aware final/stage distinction | The old global `0xBABD = 技之深奥` label. |
| Primary inversion | exact replay after certified prefilter | The conditioned draw-2 representative map as an exact partition. |
| Enemy feasibility | native role rows plus recovered class paths | Name-only role assumptions or finite scan failure. |

## Missing evidence

- Large native parity corpora for playthroughs 1 and 2.
- Authentic first-playthrough rarity-4 acquisition evidence remains absent;
  product support instead rests on the complete PC v2.01 draw-1 map and
  repeatable native finalizer result, and does not claim a natural P1 drop.
- Released algorithms and protocol acceptance for playthroughs 4 and 5.
- Nonzero `caller_option` auxiliary paths.
- A code/data-flow closure from challenge descriptor `+0x2C` to enemy/Boss AI
  combat level.
- A complete confidence-annotated schema for every byte in the `0xE8` record.
- Signed, independently replayable raw 10,000-pair parity corpora.
- Live native reroll candidate vectors covering initial display, manual refresh,
  accepted choice, multiple rarities, and the save-scoped group eligibility gate.
