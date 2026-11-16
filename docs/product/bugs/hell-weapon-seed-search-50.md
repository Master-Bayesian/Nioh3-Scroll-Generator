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

The first real-character copy rehearsal exposed a separate old commit guard:
save-based equipment addition still rejected keys at the u16 boundary, although
the native 0x54C380 setter/live path had already established u32 equipment keys
in v0.8.7. The save path now applies that existing contract: keys `1..u32::MAX-1`
retain their exact four bytes and advance the counter once. Zero and overflow
remain rejected. No wrapping, renumbering or scroll-key policy is inferred.
The host E2E uses a synthetic 255761 counter to cover the actual refusal range.

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

## Owner-assisted game acceptance (2026-11-16)

The owner authorized one Bizen katana for the level-450 character after returning
to the title screen: item 19447, seed 6533, level 170, +22, rarity 4, hell enabled,
and independently selected skill 49345. Four fresh source backups were hashed
and read back. Private package v2 from clean commit `53cd37d` performed exactly
one protected save commit. Equipment count increased from 583 to 584; all old
equipment, currencies, items and generation inputs were unchanged. The complete
new record matched the rehearsed candidate after allocating its current key and
serial. The plaintext write frame contained only the new record, the two
counters and checksum (77 changed bytes). No game-memory access was used.

The owner then confirmed that the weapon, hell skill and effects all matched in
game. A read-only snapshot after the game saved verified the same identity,
seed, level, plus, rarity, hell status, skill and complete effect list through a
fresh packaged reader. Of the 240 record bytes, only `+0x18` changed from `0x82`
to `0x80`; this observation does not assign a meaning to that flag. The owner did
not separately confirm a second game entry. Game display, a subsequent game
save and fresh disk readback are the observed acceptance scope; a second game
load must not be claimed from these records.

The exact plan, source and backup hashes, one commit receipt, byte comparisons,
owner observation and post-game-save snapshot are retained under
`private-live-acceptance/live-20261116T223712Z/` in the local evidence directory.
The copy-rehearsal failure, obsolete private build and the postcommit comparison
diagnostic are retained separately. The latter compared read-time classification
and audit metadata with a preview; independent full-record and write-frame
verification passed without another commit or restore. This acceptance covers
the selected weapon and skill, with existing automated tests providing the
broader input coverage.
