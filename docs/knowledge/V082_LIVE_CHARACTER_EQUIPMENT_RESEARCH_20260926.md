# v0.8.2 live character and equipment research (PC v2.0.2.0, 2026-09-26)

One game session (module base 0x7FF7B9F10000, SizeOfImage 87,011,328). Reads
used PROCESS_VM_READ; the owner-authorized writes are listed under "Writes".
Breakpoints used the CE MCP bridge with the VEH debugger and were removed at the
end. Owner UI screenshots confirm every value marked (UI). This is live research
evidence for one save and one session, not product acceptance.

## Player struct (CT "BaseAddress")

CT v2.00.02 `[Nioh3.exe+4749820]` -> v2.02 `[Nioh3.exe+4751850]` (shift +0x8030).
Struct begins with vtable `Nioh3.exe+402DA20`. CT relative offsets are unchanged:

| Field | Offset | Width | Observed |
| --- | --- | --- | --- |
| Amrita (精华) | +0x10 | u64 | 0 (UI) |
| Gold (持有金钱) | +0x18 | u64 | 19,072,714 (UI) |
| Soul fragments | +0x20 | u64 | 9,096,009 |
| Silabar ingot amrita | +0x30 | u64 | 0 |
| Allocated stats 体心刚武技智咒 | +0x94..+0xAC | 7 x u32 | 22 150 9 108 20 50 40 (UI) |
| Initial stats | +0xB0..+0xC8 | 7 x u32 | 5 5 5 6 5 5 5 |
| Skill capacity base/add (common, samurai, ninja) | +0x5F88..+0x5F9C | 6 x u32 | 10+32, 10+31, 10+31 = 42/41/41 (UI) |
| Level (等级) | +0x69DC | u32 | 364 (UI) |

## Other bases

| CT symbol | v2.00.02 | v2.02 | Check |
| --- | --- | --- | --- |
| ClanAddress (glory, direct) | +4B97B54 | +4B9FCA4 (+0x8150) | value 2,000,000,000 = 持有武功 (UI); unique in module |
| SPAddress chain `[[[+X]+4D0]+240]+14` | +47498A0 | +47518D0 (+0x8030) | samurai total/spent 209/168, ninja (-0x20) 203/154 — unconfirmed |
| BSAddress chain `[[+X]+50]+198` | +45CEBD8 | +45D6C18 (+0x8040) | max/current HP 3706/3706, max/current Ki 1370/1370 — unconfirmed |
| NJAddress chain | +45BEEF8 | not found near +0x8040 | open |

## Save file (SAVEDATA00, decrypted copy)

The v2.02 character block is a tagged stream `hash u32 | size u32 | value[size]`,
not fixed offsets; the third-party editor's constants (`0x3DDBB3+9` etc.) land
one byte off. Observed keys:

| Hash (LE bytes) | Size | Value | Meaning |
| --- | --- | --- | --- |
| `52 30 b4 13` | 8 | 0 | Amrita (UI) |
| `df 54 ad 75` | 8 | 19,072,714 | Gold (UI) |
| `59 44 1e 57` | 4 | 9,096,009 | Soul fragments (matches memory +0x20) |
| `ff 11 7e 60` | 0x20 | count 7 + 22 150 9 108 20 50 40 | allocated stats (UI) |
| `7e 18 4d 61` | 0x20 | count 7 + 5 5 5 6 5 5 5 | initial stats |

## Writes (owner-authorized, 2026-09-26)

- Gold +1 at player+0x18: UI updated immediately; after shrine save and reload the
  save file's gold key held 19,072,715. Live edits persist through the game's own save.
- Allocated stats: writing +0x94 does not recompute level (+0x69DC) or HP; HP is
  recomputed when the level-up page opens; level is independent and also stored under
  save key `1bd2815f` plus the header copy at 0x28. Level = sum(allocated) - sum(initial) + 1.
- Restored the pre-respec build (22 150 9 108 20 50 40, level 364, amrita 0).

## Equipment (live trace)

- Equipment container = `[[Nioh3.exe+4751530]] + 0x10` = player struct + 0x370E0;
  0xF0 records, 2500 slots. Memory slots 0..2 are byte-identical to the decrypted save
  at 0x270066 + i*0xF0, so live and save edits share one record format.
- Record: +0 item id, +2 transmog id, +4 qty, +6 level, +8 level before forge, +0xA plus,
  +0x1C inventory key, +0x20 u16 flag, +0x22 u16 seed, +0x30 rarity, slots from +0x30
  stride 0x18 with effect id at slot+8, value at slot+0xC, u16 rolls at slot+4 and
  slot+0x10, pool/category byte at slot+0x13.
- Forge (blacksmith) generation goes through `generate_effects` (+0x557F34) from caller
  +0x20C82C5 (chain 2FA42A, 20C8949); same caller also produces ~1,861 one-slot previews
  when menus open. Enemy drop: caller +0x625136/+0x625164; blacksmith shop list:
  +0x22825C7/+0x22825DD (deterministic per visit).
- Forged 甲斐国江 (0x8D5B, Lv175, rarity 3, seed 0xDCC3): 风林火山 0xD524,
  近距离攻击精力伤害（地狱） 0xA166 value 15 (+1.5%), 陷入水状态时精华槽增加 0x02E3 value 2 (D+),
  赋予造成伤害增加 0x3576 value 58 — matches owner screenshot.
- Blacksmith effect re-roll does not call `generate_effects`. A write watch on the slot
  hit once at +0x1025249 (stack 2656A5, 557E09, 557DC3, 22EDDD0, ...), writing only the
  persistent container copy: id 0xA166 -> 0x8D2B (承受伤害增加灵力), value 15 -> 4 (C),
  both u16 rolls re-drawn. The player chooses the effect from a candidate list; the value
  is re-rolled by the game.

## Offline: the forged item's resource row

`nioh3_scroll_editor/data/r4_finalizer/pc_v2_02/resource_v1/tables/item.bin`
(8-byte header, 3,362 rows of 0x1A0) holds every item, not only scrolls; the
domain index currently keeps just the scroll rows. 甲斐国江 is row 42 with
`+0x152 = 0x8D5B` (the same record-type key the scroll rows use) and
`+0x154 = 0xD524` (风林火山), which is the fixed first effect the forge call
produced. So `ScrollItemDefinition::field_154` is the item's fixed/set effect.
The per-entry bytes the forge caller pre-set at slot `+0x13`
(`54 3F BE D1 54 BA 2D`) do not occur in the row; they are presumably drawn by
the caller before `generate_effects`, which a live trace of the caller
(`+0x20C8949` -> `+0x20C82C5`) has to confirm.

## Offline: soul cores share the equipment array

Classifying the 1,318 occupied owned-equipment records of the owner's save by the
CT's item-ID dump (`Equipment_Items_v2.00.02CE_RawDump.txt`) gives 369 weapons,
265 + 41 samurai and 181 + 18 ninja armour pieces, 204 accessories, 164 + 20
soul cores (base game + Hell Rising) and 56 ids the v2.00.02 dump does not name.
Soul cores therefore live in the same 2,500-slot array with the same `0xF0`
record layout (for example slot 1, Bloodedge Demon Soul Core `0x6D36`: effect
ids at `+0x38/+0x50/+0x68` with values at `+4`), so the equipment edit path
already reaches them.

## Offline: the forged values match the native value table

The 2026-09-02 catalog export (`effect_value_ranges_level_180.csv` in
`deliverables/catalogs/Nioh3_PC_v2.01_catalog_resume_20260902.zip`) lists a
single raw value for each of the forged/re-rolled effects at rarity 3–5:
`0xA166` 15, `0x02E3` 2, `0x3576` 58, `0x8D2B` 4 — exactly the values the game
wrote. The same export's equipment effect-pool enumeration (effect flags
0x40/0x80, item flags 0x0800/0x1000, slot weight by item row `+0x15C`) is the
starting rule set for the deferred equipment legality audit.

## Bundled item names

`apps/workshop/item-names.json` carries the 2,727 Simplified Chinese item names
and native groups of `仁王3_PC_v2.01_全物品列表_简体中文_20260902.xlsx`, captured from
the game's runtime localization pool like the shipped effect names. Items added
after PC v2.01 fall back to the item-type group and the hex id; the optional
local catalog import remains for supplementing names.

## Drop generation, star effects and hell weapons (live, 2026-09-26)

Owner session with a trainer one-hit-kill: normal map, then a hell-environment
katana scroll. The drop probe (entry breakpoints on `generate_effects` and
`init_generation_context`, plus the drop return +0x625164) and before/after
snapshots of the owned-equipment array captured about 90 new records. Traces and decoded
diffs are in `deliverables/v082-ce-research/` (`trace-drop*.tsv`,
`container-diffs.jsonl`, `dis-hell*.txt`, `hell-skill-table.json`).

- Every enemy drop runs `init_generation_context` from +0x625136 and
  `generate_effects` from +0x625164 (mode 0, 7 slot templates). Set items take a
  different route into +0x625136 (stack via +0x2849ED) and carry `0x5B` in the
  pre-context.
- Star (green ✦) effects are separate effect ids, not a flag on the ordinary
  effect: group 0x9AE9 (武技精力伤害) has the star row `0x31D0` (effect-row
  `+0x20 = 0x1B`, rarity weights 0/0/0/0.5/0.75/1…, so no star below rarity 3)
  and the hell row `0x3790`. In the record the star slot's byte `slot+0xE` (entry `+0xA`)
  is `0x04`, and the record flag byte `+0x18` gains `0x04` (`0x80` → `0x84`,
  `0x82` → `0x86`). Weapons and armour both follow this rule (for example 宫司净衣 头冠
  速攻击精力消耗降低 value 180 = −18.0%).
- Hell weapons: record `+0x10` u16 is the hell martial skill id and record
  `+0x1A` is `0x10`; all other records have both zero. Hell effects proper are
  effect rows with effect flags `+0x1C = 0x50` (bit 0x10) and weight 1 on all weapon
  columns (for example `0x3790` 武技精力伤害, `0x89C2` 地狱武器掉落率). Effects
  whose names end in “（地狱）” (`0xA166`, `0x36E6`, `0x044D`, …) are ordinary
  `0x40` rows with weight 200 and also occur on non-hell drops.
- Hell conversion is a second pass, not a separate drop generator.
  `try_hell_convert` (+0x2285E58, only caller of the converter at +0x2285FFD)
  runs after the normal drops of one kill:
  1. gate on a param-table lookup (id 0x2A05);
  2. chance = `[arg2+0x18]` + (context `+0xD8` ? param 0x9F1 × const : 0) +
     the player's hell-drop-rate stat (`+0x1D0` × const), out of 10000;
  3. candidates are the just-dropped records whose item row `+0x182` is 1 or 2,
     item flags `+0xB0 & 0x02` set, and record `+0x18 & 0x400000` clear;
     one is picked uniformly and converted.
- The converter `convert_to_hell` (+0x2287870, rcx = drop context, rdx = record,
  r8d = level):
  1. raises rarity via +0x9F070C(4), adjusts the pre-forge level and plus;
  2. rebuilds the 7 slot templates with categories `8F 8F D5 00 80 8F 50`;
  3. calls `init_generation_context` (+0x2287A55) and `generate_effects`
     (+0x2287A91) with the **same seed** as the normal drop, mode flags from
     context `+0x23/+0x25` and a type of 3;
  4. picks the hell martial skill at +0x2286D6C and stores it at record `+0x10`.
  The three converted katanas re-used the seeds `0xDF4A`, `0x3F40` and `0xB049` of their normal
  generation.
- Hell martial skill table (`[[Nioh3.exe+0x45B9E30]+0x5A8]`, 39 rows of 0x20):
  `+0x18` skill id, `+0x1A` weapon-type key (item row `+0x58`), `+0x1C` minimum
  level, `+0x1E` weight 10, plus six float multipliers (0.2/1.0) selected by a
  per-player byte. Katana (key 6409) has `0xC0C1`, `0x3435`, `0xAC06`; all three
  appeared on this session's hell katanas.
