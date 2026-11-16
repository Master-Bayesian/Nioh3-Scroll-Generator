# Hell-weapon effect seed search (#50)

## Existing work reused

The original PC v2.02 effect generator, hell first-innate lottery, record builder,
search and seed replay are reused from `f93c8188de19f100526e925dd4dde26543f96bfd`
(`claude/hell-legal-generation`), cherry-picked as `f52ac8e`. That implementation
already retained twelve natural-weapon effect/roll vectors and the native Bizen
katana conversion. It was not rewritten or inferred from an effect pool.

Existing equipment rules provide hell eligibility, effect pools, martial skills,
conflict checks and slot roles. The existing save plan, confirmation, backup,
allocation, commit and recovery pipeline performs addition. The separate live
equipment-add prototype/workflow is not extended by this change.

The native evidence source is retained in Git at
`d21ee3ac:docs/knowledge/V082_LIVE_CHARACTER_EQUIPMENT_RESEARCH_20260926.md`, and
the public raw records are in
`crates/nioh3-data/tests/fixtures/equipment_natural_v202.json`. Only `hell3`,
`hell-conversion-trace` and `inserted-hell` have the documented zero converter
flags for strict parity in this supported context: five observations, three
distinct item/seed pairs and three weapon types. Earlier captures omit inputs;
their successful or unsuccessful replay is diagnostic, not a fixed-context
oracle. The source records were not changed to fit the implementation.

## Product behavior and supported inputs

In the existing save-based legal equipment-add flow, an eligible weapon can
enable hell generation and search the existing 65,536 effect seeds. Wanted
choices include applicable ordinary, star, hell, grace and fixed-set effects.
No-match statements refer to the chosen difficulty, progress and supported
generation route, not every enemy/region/drop modifier.

The supported contexts are `reward_zero_flags` and
`hell_conversion_zero_flags`. The selected final rarity, generation level,
stored level, plus, difficulty and four progress words are retained. Raw
generation level is distinct from the level cap in the stored record. Ordinary
legacy callers without a review binding remain compatible.

The effect seed does not select the hell martial skill. The user independently
chooses an eligible skill before queueing; it does not block effect-only search
or change its results. The waiting-list entry freezes both that choice and the
selected effect result. A later draft, mode or source change cannot rewrite it.

Each search result carries a SHA-256 binding over a versioned domain, supported
context, difficulty/progress and generated record before skill/serial/key
allocation. At save preparation, the host re-reads the target save's progress,
regenerates the selected seed, checks normalized stored fields and the binding,
then validates and installs the explicit skill. A reviewed request cannot
replace generated effects with renderer-supplied entries. The existing final
save plan binds the completed record and target source before confirmation.

## Explicit state and validation

The search reducer has six named states (`idle`, `searching`, `empty`,
`results`, `selected`, `error`) and an exhaustive action switch without a
default. Responses must match both request ID and full search identity, including
mode and save/snapshot identity. A cached character provides generation inputs
only while it matches the selected save and current source digest. Failed
refresh keeps search disabled until the current character has been read.

The backend uses `EquipmentGenerationContext` and `CandidateBinding` enums.
The TLA+ model in `spec/equipment_seed_selection` maps its guards to these
implementations and to the save preparation boundary; it is not a native RNG
proof. Existing save/commit ownership models remain authoritative.

## Repeatable acceptance

- `tools/ui-audit/verify-hell-equipment-seeds.mjs` builds the production UI and
  drives ordinary/hell search, independent skill choice, immutable queueing,
  late replies, changed modes/sources and failed-source recovery in all three
  locales. It retains the bundle, source snapshots, screenshots and payloads.
  This uses a synthetic bridge and never dispatches a real save write.
- `host_character_edit::hell_seed_search_review_commit_and_reload_preserve_the_native_candidate`
  drives the real framed runtime search and protected save host against a
  synthetic encrypted save. It rejects tampered/incomplete candidates without
  writing, commits the selected native-observed weapon, creates a fresh save
  host and verifies disk reload. It retains requests, responses, record bytes,
  source hashes and a replay command under the configured build root.
- The existing ordinary native-generator goldens, twelve natural hell vectors,
  native Bizen example and the scoped full native-record comparison are reused.
- The full lint/typecheck, actual-rule controls, all shipped Rust crate
  tests/Clippy/rustfmt, full TLA+ suite and packaging checks remain required.
  CI retains the new browser and host evidence instead of discarding it.

Local evidence is under
`D:/Nioh3_v080_deliverables/deliverables/hell-weapon-seed-search-20261116`.
Game-process or real-save writes have not been performed for this implementation.
A fresh installed-product/game display and game save/reload check remains a
separate acceptance step; owned-copy, browser and historical evidence must not
be labeled as that new live acceptance.
