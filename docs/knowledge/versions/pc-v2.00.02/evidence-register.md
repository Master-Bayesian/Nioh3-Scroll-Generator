# Evidence and supersession register

This register prevents a later summary, test name, or stale report from being
mistaken for stronger evidence than the files actually contain.

## Current evidence

| Claim | Evidence | Strength and limitation |
| --- | --- | --- |
| R3 stable-record parity | `audit/parity/ng3-r3-live-parity-20260829.json` | 10,000 deterministic Seeds, zero stable-record mismatches. Aggregate report only; runtime header byte `+0x1B` differed and is explicitly excluded. |
| R4 stage/final parity | `audit/parity/ng3-r4-live-parity-20260829.json` | 10,000 deterministic Seeds, zero stage/final/accepted-index mismatches. Aggregate report only. |
| R4 retained native pairs | `test_fixtures/r4_native_corpus/base/` and `distributed/` | Ten stage/final pairs, nine unique Seeds. Sanitized tracked copies differ from private captures only in the eight origin-account bytes. |
| R5 record parity | `audit/ng3-r5-native-parity-live-10000-20260829.json` | 10,000 deterministic Seeds, zero full-record mismatches. Aggregate report only; technically verified while product access is hidden by policy. |
| Complete auxiliary parity | `audit/p1_static/COMPLETE_AUXILIARY_PARITY_20260829.md` | All three class branches and 22 native vectors; strongest for third playthrough and `caller_option = 0`. |
| Enemy role structure | versioned `enemy-roles.json`, native table SHA, class control flow, and `test_effect_seed_solver.py` | Complete 487-row table; two process captures produced the same row hash. |
| Recommended display curve | `recommended_level_curve.json` plus native callers | Exact internal-to-display conversion. No closed consumer from challenge descriptor to AI combat level. |
| Exchange tuple | `emaki_exchange.py` plus receive/send disassembly | Effect slots absent; receiver rebuilds canonical effects. Final propagation still needs a second account. |
| Scroll inventory-instance key | `audit/save/inventory-key-collision-20260830.json` plus `test_beta_editor.py` | Two private support saves had no native duplicate `+0x1C` keys; only raw-appended records collided with their donor. A two-key-only repair restored both hidden records in game. |

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
- Released algorithms and protocol acceptance for playthroughs 4 and 5.
- Nonzero `caller_option` auxiliary paths.
- A code/data-flow closure from challenge descriptor `+0x2C` to enemy/Boss AI
  combat level.
- A complete confidence-annotated schema for every byte in the `0xE8` record.
- Signed, independently replayable raw 10,000-pair parity corpora.
