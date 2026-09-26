# V0.8.1 Source Feature Matrix

Consolidated function-family view of the three supplied v0.8.1 source intakes, so no
supplied feature is silently dropped and none is promoted past its evidence. Static
inventory only: no save, trainer, Cheat Engine object, `pc.exe`, or game was executed,
nothing was attached, and no write path was exercised.

Status vocabulary: **available** (a reachable path exists in the pinned source),
**placeholder** (declared with an unfilled address), **broken** (author-declared
non-working), **absent** (claimed or implied, no code path), **binary_only** (only
inside a bundled third-party binary), **unknown-2.02** (needs current-version
evidence). Record counts are not independent working features.

## Pinned artifacts on D:

| Artifact | Local path (under `D:/Nioh3_v080_deliverables/deliverables/v081-source-intake-20260921/`) | SHA-256 |
| --- | --- | --- |
| CT inventory, 947-entry flat CSV | `ct/features.csv` | `93abdac9d2989c8e6d9b5c6fd08e35a73081cc16329e6c246f06ebbcf1f7230e` |
| CT inventory, full hierarchy JSON | `ct/features.json` | `64ba4960d7350786ab55e9953216021c29a86966bc1869da2413343f4238c3db` |
| CT subfeatures (currency/stats/inventory/equipment) | `ct/subfeatures.json` | `9709ce38d03e8b05edf6438ffc5596b1513c48d9d772a16b0203b774ceab222d` |
| CT report | `ct/INVENTORY.md` | `b20e0b9b41ee233010fe12964b6f97c148dd93c32f51dd336a944839d248842f` |
| CT archive manifest (hashes, members, bounds) | `ct/archive_manifest.json` | `2fb2d2cca70259a848c55017a43b82f4ca2ce068f87f7f13978e14b2ee8efed5` |
| ID-dump catalog counts/schema/overlap | `ct/id_dumps.json` | `a64e5541ad2310ba5f32087428b3236b7ff7f2bda521bddfa4bb0d32bd9795f7` |
| Save-editor upstream feature matrix | `save-editor/feature-matrix.json` (+ `.csv`) | `ff2019fb427a2999f50e5d0b6fc80fa89f63b42e7c9bb2451c58d9909aec68ec` |
| Save-editor inventory report | `save-editor/INVENTORY.md` | `fd4350f639ee7f424313b46b5ed330ce81ac7169ef390ecb603be268eb2b63a6` |
| Save-editor source manifest (per-file blobs) | `save-editor/source-manifest.json` | `9d0671383c7172cde37f4349a1300634e4651d138e8dba42220728f295be22c7` |
| Pro equipment review | `pro-equipment/REVIEW.md` | `cb91960fdcb658d11e0e324aa0846a6c54ab65b153b5fa707b7a69cd031e61f0` |

Pinned upstream identities: CT archive `6fb2af78e64bae4d74ac73b0474b4f8a2e104acac794e8cdfdb3b25188be79c3`,
CT file `1c90f14cff703f5b1c4b495ef4c4b39609e3a2aec01e95bc81486a5163664a4f`;
save-editor `alfizari/Nioh-3-Save-Editor` commit `b5d0789791fe31d06ad325d4012aa0333c60cd8f`
("update offsets"); Pro equipment archive `FD9CBA60CD03C5FEB7A1EFAA791A36CC1D9F06C860751671B89E6AD14D505644`.

## Function families

### A. Save container, integrity, identity

| Family | Precise subfields | Status | 2.02 |
| --- | --- | --- | --- |
| Open/decrypt `SAVEDATA.BIN` | basename must be exactly `SAVEDATA.BIN`; backup `SAVEDATA.BIN.BACKUP[_n]`; decrypt via bundled `pc.exe` | available (`main.py:200-228`) | region offsets unvalidated |
| Save/encrypt | serialises all four inventories, patches checksum, re-encrypts, copies to chosen path | available (`main.py:270-297`) | no post-write verification |
| Checksum | seed `+0x900190`, body `[0x190,0x900190)`, value `+0x900194`, `0x400`-block fold over `0x2400` blocks | available, artifact-corroborated (`checksum.py:3-42`) | matches repo port `codec.rs:124-155` |
| Integrity bypass `-cs` | disables run-time integrity check when decrypting USR saves | binary_only (`pc.exe` strings) | PQ-4 open |
| Steam ID transfer `-sid` | re-encrypts for another SteamID3 | binary_only (`pc.exe` strings) | PQ-5 open |
| Declared region map | Equipment `0x270066/0xF0/2500`, Usable `0x302832/0xE8/1500`, Storage `0x35779E/0xE8/393`, Scrolls `0x176CCE/0xE8/400`, total `0x9001B0` | partial (scroll region matches; others uncorroborated) | PQ-1 open |

### B. Character and equipment records

| Family | Precise subfields | Status | 2.02 |
| --- | --- | --- | --- |
| Character stats edit | Amrita, Gold, Constitution, Heart, Stamina, Strength, Skill, Intellect, Magic (nine fields; README's "Courage"/"Dexterity"/Level do not exist) | partial (`main.py:1897-1907`) | PQ-2 (+9 stat shift) |
| Equipment core | `+0x00` id u16, `+0x02` appearance u16, `+0x04` quantity u16, `+0x06` level u16, `+0x08` level pre-forge u16, `+0x0A` plus u16, `+0x14` familiarity u32, `+0x1C` inventory key u16, `+0x30` rarity u8 | available (`main.py:300-332`, `main.py:428-456`) | PQ-1 open |
| Equipment flags | `+0x18` flags1 u8, `+0x1A` flags2 u8, `+0xE8`/`+0xEC` equipped u8 (equipment only) | available (`main.py:436-452`) | semantics PQ-3 |
| Equipment effect slots | seven `0x18`-byte slots from `+0x38`: id u16 `+0x00`, value u32 `+0x04`, category icon u8 `+0x09`, extra u8 `+0x0A` | available (`main.py:442-448`) | PQ-3 |
| UI object indices | `+0x28`/`+0x29` u8 editable but never written | dead_code (`main.py:869-882`) | PQ-3 |
| Equipment spawn | canned 512-byte template + catalog name, `random.shuffle` unseeded, counter bumped at `0x33F41E` | available (`main.py:493-498`, `main.py:582-689`) | PQ-3; not legal generation |

### C. Inventories, catalogs, import/export

| Family | Precise subfields | Status | 2.02 |
| --- | --- | --- | --- |
| Usables/materials | same `0xE8` parse, no equipped/flag writes | available (`main.py:335-361`) | PQ-1 open |
| Usable max quantity | sets every stack to 9999 | available (`main.py:1722-1730`) | no range validation |
| Storage box | list/edit only; no spawn code path | available / spawn absent (`main.py:29-31`) | PQ-1 open |
| Scrolls | `+0x00` id, `+0x06` level, `+0x08` level_again, `+0x0A..0x27` opaque, `+0x28/+0x29/+0x2A` UI, `+0x30/+0x31` rarity, seven `0x18` slots from `+0x34` (value `+0x08`, category `+0x0D`, extra `+0x0E`) | available (`main.py:363-393`, `main.py:470-490`) | PQ-1 open |
| Import character | keeps destination `[0,0x15F)`, copies the rest; placement ids not remapped | available (`main.py:252-267`) | PQ-5 |
| Import inventory sections | raw block replace plus `+0x1C`/UI index remapping | available (`main.py:1275-1513`) | PQ-1 |
| Remove item | no delete/clear/zero routine exists | absent | PQ-3 |
| Export to spreadsheet | no `xlsx`/`csv`/serializer anywhere | absent | none |
| Catalogs | `items_little_endian.json`, `effects_big_endian.json` loaded; `items.json`, `effects.json` dead; all 791 effect entries have `type: null` so the type filter is inert | available / stale (`main.py:56-111`) | portable_data_only |
| Search/filter/per-type sub-tabs | Tk UI only | available (`main.py:1545-1566`) | reuse_blocked |

### D. Supplied Cheat Table (v2.00.02, CheatEngineTableVersion 52)

947 entries = 791 address + 147 group + 6 dropdown-list definitions + 3 script. 794
address/script records; 658 declared non-placeholder addresses; 131 `+` placeholders;
2 address records missing an address (`356228`, `356231`); the 3 script entries have no
address by design and are read through their AA/Lua bodies.

| Family | Precise subfields | Status | 2.02 |
| --- | --- | --- | --- |
| Activation script `355462` | pins `BaseAddress Nioh3.exe+04749820`, `ClanAddress +4B97B54`, `BSAddress +045CEBD8`, `SPAddress +047498A0`, `NJAddress +045BEEF8`, `STGAddress +4B768E0`, `VisAddress +45B2EF0`, `UPSAddress +45B2FB8` | available, runtime-unvalidated | offsets version-locked |
| Compact / Fullview UI `392` | inline Lua (`LuaCall`) building a Compact/Fullview menu; no top-level `LuaScript` node | available | UI-only |
| Equipment editor `355920` | 36 direct fields (ITEM ID, ITEM TRANSMOG, QUANTITY, LEVEL, LEVEL (BEF. FORGE), ITEM (+VALUE), CRUCIBLE ART, FAMILIARITY, FAMILIARITY MAX, ITEM SEEN, ITEM LOCKED, ITEM FAVORITE, CRUCIBLE ART (YES/NO), INVENTORY ID, RARITY, SPECIAL EFFECT #1..#7, SPECIAL EFFECT VALUE #2..#7, SP EF #1..#7 CAT FULL BYTE ARRAY) plus 7 `SPECIAL EFFECT ... VALUE` sub-groups (???, CATEGORY, EXTRA PERK EFFECT); AOB `66 39 30 0F 84 8D 01 00 00` at `Nioh3.exe+22AC437`, symbol `grab_item` | available, runtime-unvalidated | AOB uniqueness PQ |
| Player stats | BATTLE STATS (health/current/max Ki, guardian spirit fill/active, spirit force max/current), CURRENT STATS and BASE STATS (CONSTITUTION, HEART, STAMINA, STRENGTH, SKILL, INTELLECT, MAGIC, LEVEL), SKILL CAPACITY (COMMON/SAMURAI/NINJA base+addition), SKILL POINTS (SAMURAI/NINJA total-max, spent), CLANS/CURRENT GLORY + CLAN FAVOR slots 1-8, CLAN PROTECTION (SAMURAI/NINJA STYLE 1-2), NINJUTSU slot max/current | available | offsets unvalidated |
| Player currencies | AMRITA, SILABAR INGOT AMRITA/AMRITA, GOLD, SOUL FRAGMENTS, GLORY | available | unvalidated |
| Equipment/inventory CT | EQUIPMENT EDITOR, PENANCE STONE (EARLY WORK), SUMMONING SEALS / ONMYO MAGIC slots 1-8 with AMOUNT | available / partial | mixed |
| Settings mirrors (282 records) | display toggles (headgear, melee/ranged weapon, HUD), ultrawide HUD adjustment, compass orientation, damage display, controls/accessibility/camera/audio/visual/online/language pages | available | mirrors, not presets |
| Gameplay counters (325 records) | damage/action/rift/other statistics; 131 of them are `+` placeholders, many in `(NEXT TBL UPDATE)` groups | available / placeholder | not features |
| Titles | PRESTIGE (SAMURAI/NINJA/TACTICS/SUBJUGATION/FORMIDABLE ENEMIES reputation and points) | **broken** (author: "BROKEN FOR NOW SINCE I NEED TO ADJUST OFFSETS") | retained backlog |
| Dropdown definitions | `SPECIAL_EFFECT_DROPDOWN`, `EQUIPMENTDROPDOWN`, `CATEGORY_DROPDOWN`, `CRUCIBLE_ARTS_DROPDOWN`, `FOOTER_TWO_DROPDOWN`, plus one attribution entry | available (lookup tables) | not standalone features |
| ID dump catalogs | equipment/items 1527 rows (13 sections, 2- and 4-byte ids), special effect 744 rows / 743 distinct ids (Uncommon 310, Common 235, Divine 81, Rare 75, Exotic 19) | catalogs, not functionality | v1.05/v2.00.02 labelled |

### E. Pro trainer-side equipment research (closed sample, static)

Trainer-side only: add path zero-fills 512-byte buffers; basic fields at
`0x00/0x02/0x04/0x06/0x08/0x0A/0x10/0x14/0x1A/0x28/0x30`; edit stores at
`+0x06,+0x08,+0x0A,+0x14(u32),+0x30(u8),+0x10(u32),+0x1A(u8)`; cache stride `0x10`;
native add call shape `(manager, out512, in512, 0, 7)`; snapshot magic `0x4E494F33`
version 1 stride `0xA0`. Status: available as *trainer-side* facts only;
**unknown-2.02** for game module identity, live item stride (`live_item_stride: null`),
`mode_7` meaning, `unknown_28`, RNG/legal roll ranges, persistence, and game-side
insertion ABI. `AddItemToBag` (`0x140410`) is a log-only false lead.

## Settings and counters are not new combat presets

Everything the CT exposes under SETTINGS and GAMEPLAY RECORDS is a mirror of an in-game
setting, menu preference, or progress counter. None of it is a new combat preset, and no
statistic field is a disguised invincibility or speed feature. The 131 unfilled counters
and the broken Titles section stay in the backlog and are not claimed as working.

## Proposed root sequence

1. Read-only: equipment/items/catalog browsing plus selected-record identity.
2. Currencies, core stats, and existing-item quantity/basic/effect editing through the
   already accepted Rust save/runtime contracts.
3. Create/import and temporary runtime controls only once insertion, ownership, and
   version contracts are validated.
4. Broken Titles and unfilled counters stay retained backlog; natural/legal PRNG
   generation remains separate unresolved research and is not "create item".

## Coverage and non-removal

Every supplied feature appears above and in the linked matrices, including the
binary-only, dead, and absent ones; nothing is silently removed. No claim is made that
all of them ship in 0.8.1, and the sequence above does not promote anything to
available beyond the static evidence listed.

## Provenance checks (static, not runtime)

- Archive integrity by `7z t`, member path/size bounds, and compression-free expansion
  limits before extraction (`ct/archive_manifest.json`).
- SHA-256 pinning of every source archive and artifact; CT parsed as inert XML with
  DOCTYPE/ENTITY rejected before parsing.
- Source provenance by static reading with line anchors, static PE optional-header and
  string inspection, and an artifact-level recomputation of the checksum fold on public
  samples.
- Not performed anywhere in these intakes: executing `main.py`, `checksum.py`, `pc.exe`,
  the trainer, or the CT; installing dependencies; attaching to the game; or reading or
  writing any owner save.

## Open 2.02 unknowns

Non-scroll save regions (PQ-1), the "+9" stat shift (PQ-2), spawn/flag semantics (PQ-3),
`-cs` effect (PQ-4), `-sid` mutation (PQ-5), sample staleness (PQ-6), CT module offsets
and AOB stability, and the game-side equipment insertion ABI all need version-matched
runtime evidence before any product write.
