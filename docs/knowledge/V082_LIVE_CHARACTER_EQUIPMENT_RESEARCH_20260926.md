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
  2. re-initializes the 7 effect entries;
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

## Natural-drop legality check against the static exports (2026-09-26)

`validate_drops.py` (in `deliverables/v082-ce-research/`) checked the 87 captured drop
records (341 effects, levels 167–173) against the 2026-09-02 pool enumeration
(`equipment-effect-pools-v2.json`) and the level-180 value export:

- Values: all 341 fall inside the exported raw range (normal 291/291, star
  10/10, hell 4/4); most ranges are a single raw value.
- Pool: every star and hell effect and 228 of 291 ordinary effects are listed in the pool
  for the item's type class (item row `+0x15C`). The remainder are structural:
  slot 0 is the item's innate effect from item row `+0x158`; the last slot
  is the set effect from row `+0x154`, or, on rarity-4 items without a set,
  a divine-blessing set effect (惠比寿/月读/毘沙门天的恩宠 …) that needs its own table.
- The per-slot byte at slot `+0xF` (the earlier "category" byte, and the
  "templates" `92 D1 80 D1 ...` / `8F 8F D5 ...`) is uninitialized stack: the entry
  constructor +0x551314 never writes it and the values are fragments of
  `0x7FF7D1...` pointers. It carries no meaning and must not be validated.

## Native generate-and-build preview (live, 2026-09-26)

The game's own item-grant routine +0x2188610 (loops a reward list; the chain the
2026-09-21 Pro upstream response located) builds each item as:

1. compact descriptor `C` (stack, ~0xCC bytes): `+0` u16 item id, `+4` u32
   level, `+8` u32 plus, `+0xC` u8 rarity, `+0xD..+0x12` trait flags, `+0x13`
   u8 *no-serial* flag, `+0x14` u16 hell skill, `+0x18`/`+0x1C` bytes,
   `+0x20` u32 flag/seed word, `+0x24` seven 0x18 effect entries built by
   +0x551314;
2. `init_generation_context(ctx, id, rarity, seed_word)` (+0x5513C8), where
   `seed_word` is the record's `+0x20` u32 (low u16 flag `1`, high u16 seed);
   the drop path then stores drop-source fields at `ctx+8` (qword),
   `ctx+0x10` and `ctx+0x18` (observed `{1|5|0, 0x2710|0x7D0}`,
   `0x07D0 | area<<16` with area 0xA6 normal map / 0x116 hell scroll, `0`);
3. `generate_effects(&C+0x20, &C+0x24, ctx, 0)` (+0x557F34);
4. `build_record(record, &C)` (+0x5515FC) fills the 0xF0 record, applies level
   scaling, sets the star flag (`slot+0xE` bit 0x04 from effect row `+0x20`
   bit 0x08) and, only when `C+0x13 == 0`, takes a serial from
   `[[Nioh3.exe+0x4751530]]+8` into record `+0x28`;
5. `insert(manager, out, record, &slot, 0)` (+0x54D324), the same insertion
   the v2.02 scroll live-add candidate uses.

A remote-thread preview of steps 1–4 into a private buffer
(`deliverables/v082-ce-research/equip_preview.lua`, `run_preview.py`,
`preview-reproduction.json`) reproduced five captured natural drops whose
drop context had been traced (忍者手斧, 念珠丸恒次, 扇子 with 惠比寿的恩宠,
足轻中铠 膝甲, 勾玉): every effect id, value, star/set flag and roll is
byte-identical. The only differing bytes are insertion-owned (`+0x18` flags,
`+0x1C` key, `+0x28` serial), the meaningless slot `+0xF` bytes and `+0xE8/+0xEC`.
The three drops without a traced context did not match, which shows the
drop-source fields take part in the effect draw. The first previews ran with
`C+0x13 = 0` and advanced the live serial counter by 27 (0x2677C1 → 0x2677DC); with
`C+0x13 = 1` the record keeps serial `UINT64_MAX` and the counter is untouched.
No inventory, save or other global state was written.

Open for product use: game-thread dispatch of step 5 (reuse the scroll
live-add scheduler hook), the meaning of the drop-source fields (the reward
routine leaves them zero), and hell conversion (+0x2287870 needs its drop
context `+0x1C/+0x23/+0x25/+0xDB`).

## First native equipment insertion (live, owner present, 2026-09-26)

`equip_insert_once.lua` (in `deliverables/v082-ce-research/`) reuses the scroll
live-add dispatch point: a debug-register breakpoint on the pickup dispatch
`+0x12E9E50` (return `+0x20BB1C`, empty pickup queue, scheduler `+0x1408 == 0`
and `+0x1629 == 1`) redirects RIP once into a cave. The cave runs steps 1–4
above with serial allocation on, requires `build_record` to return the record
with `+0x28 == planned serial` and the counter at `serial + 1`, then calls
`insert(manager=[+0x4751530], out, record, &slot, 0)`, replays the original
prologue `40 53 57 48 83 EC 38` and resumes. No code bytes are patched.

Sample: the natural 扇子 (`0x7B72`, Lv170 +18, rarity 4, seed word
`0xAF640001`, 惠比寿的恩宠) re-generated in PID 19820. Result: status 3, slot
1400 (the first empty slot, as planned), registers preserved. A container
diff shows exactly one changed slot; against the natural record, the only
differing bytes are insertion-owned (`+0x18` flags `0x82`, `+0x1C` key,
`+0x28` serial `0x267CC7`) or uninitialized entry bytes (`+2/+3`,
`+0xF`, `+0x12/+0x13` of the entries). The owner saw the item in game and saved.
The decrypted `SAVEDATA00` (new example `crates/nioh3-save/examples/equipment_slot.rs`)
holds the identical record at slot 1400. The only key difference (`0xC82B` at
insertion, `0xC82C` in the save) comes from the owner dropping the item and
picking it up again, which re-keys it; memory and save agree afterwards.

## Hell conversion context and a crash (live, 2026-09-26)

A probe on `try_hell_convert`/`convert_to_hell` in a hell scroll (9 kills, 2
conversions) showed that both take one persistent drop-manager object whose only
fields the converter reads were `+0x1C = 0x0249` and `+0x23/+0x25/+0xDB = 0`; the level
argument was 175. Conversion re-runs `build_record` with serial allocation on,
so a converted item takes a second serial, and sets record flag `+0x18` bit
`0x100000` (the `+0x1A = 0x10` byte). The two conversions drew skills
`0x3FB9` and `0x8D12` and raised level 160 → 163 and rarity 2 → 4.

The first hell insertion (备前传太刀, fake converter context with only `+0x1C`
set) inserted correctly (status 3, slot 1423, record serial = planned + 1,
counter = planned + 2), but the game then crashed with `0xC0000005` at
`0x7F000002302A`. Cause: the cave had grown past `0x300`, where the script also
kept its runtime result cells; the builder-result write overwrote the cave's
own resume pointer, so the dispatch thread jumped to a garbage address. The
item was never saved; the last save (21:04:44, before the crash) decrypts and
still holds the earlier inserted 扇子. The script now keeps every runtime-written
cell at `+0x2C00`, separate from code, and refuses to arm when the assembled
cave ends past `+0x1000`.
With the fixed layout the retry (PID 45248, 备前传太刀 `0x4BF7`, Lv170, rarity 3,
random seed word `0x698C0001`, hell-scroll drop context, converter context
`+0x1C = 0x0249`, level 175) completed and was acknowledged: slot 1401 (planned),
registers preserved, one container slot changed, serial `0x268364` (planned + 1).
The record is a natural-shaped hell weapon: hell skill `0x3435` (one of the
three katana rows), `+0x1A = 0x10`, flags `0x00100082`, rarity 4, plus 22, slot 0 the
hell-only effect `0x8641` 赋予雷属性 (effect flags `0x50`), then 强攻击精力消耗降低,
地狱武器掉落率, 中段武技精力伤害 and 布袋尊的恩宠. The owner confirmed the weapon in game.

## Equipment values use the scroll value formula (offline, 2026-09-26)

`value_parity.py` evaluated `EffectGenerationTableIndex.resolved_effect_value`
(the recovered normalization at RVA `0x571478`, PC v2.02 tables) with the
entry's roll byte at slot `+0xC` and the record level `+6` for every nonzero
effect in the captured records: 408 of 408 match the stored value. `roll_ranges.py`
shows every observed roll inside the scroll rarity roll table
(r0 0–30, r1 30–50, r2 50–80, r3 60–100, r4 80–100, r5 90–100), including star and
hell-weapon effects. The legal value set of an effect on an item of level L and
rarity r is therefore `{f(effect, roll, L) : roll in roll_range(r)}`, and a
value's quantile is its roll position, exactly as for scrolls. Rarity 5 adds
the param `0x98FE` level bonus inside `build_record` (not yet sampled live).

## Following the in-game selection (live, owner present, 2026-09-26/27)

Goal: the editor selects whichever equipment the player's cursor is on in the
in-game inventory menu ("持有物品").

- Chain (read-only): `menu = [Nioh3.exe+45C91A0]` (vtable `+4011278`), item
  detail widget at `menu+0x6850`, displayed item pointer at `widget+0x1B0`.
  `+45C91A0` is the first entry of a static table of UI singletons created at
  startup, so the chain exists whether or not the menu is on screen.
- The widget refresh `+22AEB34` reads `mov rax,[rcx+0x1B0]` at `+22AEB47`; its
  seven bytes are checked before every read. The CT "Equipment Editor" hooks the
  next instruction (`cmp [rax],si`, CT offset `+22AC437` was an older build).
- Verification: a temporary logging cave at `+22AEB47` (installed with the
  process suspended and every thread's RIP checked, removed the same way)
  followed 26 cursor moves across weapons, armour, soul cores and consumables.
  The widget address never changed; for equipment the pointer was the record in
  the owned-equipment array, so `slot = (item - container) / 0xF0`; other items
  point into neighbouring containers. The cave was removed and the original
  bytes confirmed.
- Open/closed: the pointer keeps the last item after the menu closes. Two open
  snapshots and one closed snapshot of the menu object agree on
  `menu+0x1C == 0 && menu+0x60F8 == 1` while open (`1`/`0` when closed). `+0x1C`
  is not a generic visibility flag for the other menus in the table.
- Failed attempt (do not repeat): CE Lua-callback hardware read breakpoints on a
  record froze the game twice; the second attempt also crashed CE (exception
  0xE0465043) while a pumping Lua loop ran inside the plugin call.
- FLiNG trainer (for reference): it installs all 23 hooks at attach time as
  absolute-jump caves (`FF 25`) in private RWX pages, so option toggles change no
  game code. "Edit item quantity on click" hooks the menu item-command handler
  `+1EA0E00` (`rdx` = the clicked item record, `r8d` = command 1/0x2A/0x2B/0x2C)
  and the quantity getter `+2FA554`, gated by the caller `+1F52759`. It follows
  clicks, not the cursor. The on-disk `.text` is encrypted (SteamStub), so
  comparing live code with the file is meaningless.
- Product: `runtime.menu_selection` (`nioh3_runtime::character::read_menu_selection`)
  returns `{menu_open, slot_index, item_id}`; the character page polls it every
  300 ms when "跟随游戏内选中" is on. Only the inventory menu is covered; the
  equip-slot screen may use a different menu object.

## Held and stored item quantities (offline + live cursor evidence, 2026-09-27)

- Save layout: every record array is preceded by `tag u32 | size + 4 u32 | size u32`.
  Held items ("持有") start at `0x302832` (equipment end + 0xC), 1500 records of
  0xE8 bytes; storage ("仓库") starts at `0x35779E`, 400 records (the header says
  `0x16A80`; the third-party editor's 393 is short). The header words are checked
  before any read.
- Live layout: each array is followed by a u64 count, so held items start at
  `player + 0x370E0 + 0x927C8`. The cursor pointers captured while following the
  selection (火男面具 slot 161, 高贵粪球 179, 黏胶 539) land exactly on held
  records. The live storage array is NOT where the save layout suggests: live
  order is equipment (2500 x 0xF0), held items (1500 x 0xE8), storehouse
  equipment (4000 x 0xE8, flags like 0x100084), stored items (400 x 0xE8) at
  `player + 0x201118`; each array is followed by a u64 equal to its capacity.
  The first build derived storage from the save and its capacity check refused
  the storehouse equipment it landed on (owner report 2026-09-27); a bounded
  read-only search near the player found the 36 stored stacks byte-identical
  to the save. The reader now requires the exact capacity words around both
  item arrays.
- Count: the game's getter `+2FA554` returns 1 for flag `0x800000`, the u32 at
  `+4` for flag `0x200000` (all observed records; materials reach 47,213,196), and
  the u16 at `+4` otherwise. The same item id appears in both arrays as separate
  records (36 in the owner's save). Books ("书籍与指南") are learned recipes with
  count 0 and are not offered for editing.
- Product: `save.character` / `runtime.character_snapshot` list the stacks;
  `save.prepare_character_edit` and `runtime.character_edit` accept
  `items: [{container, slot_index, quantity}]` and refuse any change outside
  bytes `+4..+8`. Existing records only; nothing is created.

## Native generation oracle on PC v2.02 (live, owner present, 2026-09-27)

- Trigger: a v0.8.1 user on PC v2.02 could not use native search / known-seed
  view ("PC v2.02 runtime profile is not approved for product use"). NG1/NG2
  scrolls are only generated natively, so the whole early-playthrough search was
  unavailable on v2.02.
- Evidence for `profile::NATIVE_ORACLE_APPROVED_VERSIONS` (game at the title
  screen, pid 3404, no save access): the repository parity gates
  `research/validate_ng3_rarity{3,4,5}_native_parity_live.py` with
  `--runtime-profile nioh3_scroll_editor/data/game_versions/pc_v2_02.json`,
  10,000 seeds each. R3: 0 full-record mismatches. R4: 0 stage, final and
  accepted-index mismatches. R5: 0 effect-slot mismatches; all 10,000 records
  differ only in the known v2.01 two-offset rarity header cap (0 unexpected), so
  semantic parity passes. Reports:
  `deliverables/v082-ce-research/v202-native-parity/r{3,4,5}-10000.json`.
- Owner acceptance with the test8 build: NG1 native search and known-ID view
  both return scrolls.
- Bug found in that run (all versions): every scan started at seed 0 and the
  known-ID view accepted 0. For a seed the game can never hand out it draws a
  replacement id and skips effect generation, so the first "candidate" had an id
  (0x0050C7CE, 134118000) but no effects and was marked installable. The scan now
  skips unnatural seeds and any record whose seed differs from the requested one,
  and `runtime.generate` refuses an unnatural id (`INVALID_SCROLL_ID`).
- Native search without constraints costs about 0.5 s per result because the UI
  starts one job per candidate and each job re-identifies the game; unchanged
  and not version-specific.

## Hell martial-skill names (live, read-only, 2026-09-27)

- The parameter manager `[Nioh3.exe+0x45B9E30]` holds table contexts; each
  store is `u32 tag 0x20042200 | u32 count | rows`. `+0x5A8` is the hell-skill
  table (`HELL_SKILLS_V202`); `+0x598` is the martial-skill table with 0x94-byte
  rows: `u32 skill id | u32 1 | u32 0 | u32 name text ID | ...`.
- Name text IDs resolve in the zh-CN localization pool
  (`u32 text id | u32 UTF-16 units | text`). This run found the pool in the
  6.4 GB private region about 47 MB past `LOCALIZATION_POOL_RELATIVE_HINT`, by
  searching a ±128 MB window for the known entry 无想剑 (`0x02FF5EEC`, which is
  also item `0x9047`'s text ID). The old v2.00.02 text-ID anchors did not match.
- All 39 natural hell skills resolved. The 33 also in the trainer table
  `Hell/Hell.json` match it exactly; the six shield-spear skills (weapon types
  4866 and 13257) are 粉碎刺击 `0x5841`, 震霆 `0x99AF`, 地裂挑击 `0x490D`,
  锚击 `0x60A3`, 乱舞落 `0xC67A`, 龟翔 `0x35C5`.
- Bundled as `apps/workshop/hell-skill-names.json`. Helper scripts:
  `tmp/claude-ce/find_pool.py`, `resolve_skill_names.py`.

## Effect entry markers and replaced effects (live, owner present, 2026-09-27)

- Slot normalization (v2.02 `+0x5518A8`, v2.00.02 `+0x5712D8`) writes entry
  `+0x00` = the effect row's group (`+0x02`) and the low six bits of `+0x0D` =
  that group row's category (`+0x24`). Its two optional additions are
  `+0x550484` (curve-scaled, effect fields `+0x14..+0x1A`) and the flagged one
  (`+0x0E..+0x12`, sets entry `+0x0E` bit `0x10`); star rows such as `0xD4F0`
  have both zero, so their only value is the base formula.
- Owner's equipment (1406 records): 5474 entries follow the marker rule; 19
  carry another effect's group (8 plain, 2 blacksmith-processed and 2 unique
  records, 12 records in all). Their groups name the original effects (八咫镜
  slot 3 was 火抗性, 八尺琼勾玉 was 水抗性), and every value above the base
  formula sits in one of them.
- A live blacksmith replacement (种子岛枪, 灵力 → 火枪伤害 `0xB3DB`) wrote group
  `0x90D7`, category `0x1B`, roll 99 and value 33, and set record `+0x18` bit
  `0x04` and `+0x1A` bit `0x02`; it audits natural. Accessory effects cannot
  be replaced in game, and graces can only be replaced away, never onto.
- So a stale group marker means only the id was written by a tool:
  `Finding::ReplacedEffect` is unnatural even for unique or blacksmith records.
  The editor now writes the marker and category with each effect, which also
  makes the game show the new effect's icon (it kept the old one before).

## Equipped markers and removing equipment (live, owner present, 2026-09-28)

- Worn state lives in the record itself: `+0xE8` and `+0xEC` (u32) are the
  item's position in each of the two equipment sets, `0x11` when that set does
  not wear it. Swapping one helmet in each set (two read-only dumps of the
  player object, 2 MiB each) changed only these words on the four helmets,
  plus `+0x18` bit `0x02` cleared on the newly worn ones. Each set had exactly
  ten worn positions (0, 1, 4-10, 13), one item each. The player object holds
  no pointer into the equipment array and only three stray key matches, so
  there is no separate loadout list to keep in step.
- Record `+0x18` bit `0x01` (24 records in the owner's save, most with
  `0x20000`) is not the worn marker; it is probably a lock/favourite flag.
- A free slot: item id 0, header zero except `+0x0F = 0x40`, `+0x18 = 0x02`
  and `+0x28..=+0x30 = 0xFF`; each effect entry has id `0xFFFFFFFF` and zero
  fields except entry bytes `+0x2/+0x3/+0xE/+0xF/+0x12/+0x13`, which keep the
  last item's bytes; both set words are `0x11`. This rule
  (`nioh3_save::character::free_equipment_slot`) reproduces all 1,094 free
  slots of the live inventory byte for byte.
- Live removal (`runtime.character_edit` `remove`) writes that free slot with
  the usual compare-and-swap and read-back, refusing worn items. Removing the
  item under the open inventory menu's cursor worked: the item disappears when
  the menu page is switched. Community saves showed why it is needed: CE edits
  had turned two equipment records into book ids (制作指南 石块 `0x8DB5`,
  锻造书 石动 `0x1834`) that the game can no longer open or discard.
- Soul-core re-roll (a player's save, 2026-09-28): re-rolling a random effect
  replaces its id and value but keeps the slot's star marker (`+0xE` bit
  `0x04`), so a star row can show unstarred and an ordinary row can keep a
  star; a core still carries at most one marker.

## Game-side add and discard in the save (owner present, 2026-09-29)

Two decrypted saves around one blacksmith purchase and one discard (dropped on
the ground), same session, compared byte for byte:

- The discarded record (slot 1461) became exactly the free-slot layout that
  `free_equipment_slot` writes. Nothing else in the save names it: its key,
  serial and effect bytes occur nowhere after the save, so an item dropped on
  the ground is not persisted.
- The purchase landed in the first free slot after the occupied tail
  (slot 1464) with flags `+0x18 = 0x180`, key `+0x1C = 0xC86F`, `+0x20 = 1`,
  seed `+0x22`, serial `+0x28 = 0x26B007`, set words `0x11`/`0x11`.
- Two save-wide counters live next to each other: `+0x36E226` (u16 in a u32)
  is the next inventory key and advanced `0xC86F -> 0xC870`; `+0x36E232`
  (u32) is the next generation serial and advanced `0x26A79F -> 0x26B228`,
  because opening the shop generates its whole stock. Neither counter is the
  maximum of the stored records (the highest stored serial was `0x26A447`).
- Unrelated diffs: another record lost its new-item bit `+0x18 & 0x02`, plus
  world and play-time state.

So a save-file insertion must take the key and serial from these counters and
advance them; a save-file removal needs nothing beyond the free slot.

## Buff and ailment template arguments (live, read-only, 2026-09-29)

- 620 effect names are templates (`…赋予^09~BUFF~{}^09~~`,
  `使敌人陷入^09~DEBUFF~{}^09~~状态时…`); the editor showed them as the generic
  增益效果/异常状态, so the 45 variants of 吸收精华后赋予 were indistinguishable.
- The argument is the effect's group, not its value: every templated effect's
  `effect_group` row (`+0x0C` key) carries the buff or ailment name text ID at
  `+0x38` and its description at `+0x48`. All 145 such groups resolved in the
  live zh-CN localization pool to 26 names (承受伤害减少, 昂灵, 毒, 麻痹, …).
- Bundled as `apps/workshop/effect-arguments.json` (effect ID -> zh-CN name);
  `model.ts` writes each name into its template slot and the Chinese UI shows
  it (吸收精华后赋予昂灵, 使敌人陷入毒状态时增加灵力). English and Japanese keep the
  generic wording until their pools are captured the same way. Helper scripts:
  `tmp/claude-ce/resolve_buff_text.py`, `make_effect_arguments.py`.
- Bare `{}` templates (`{}计量槽增加量`, `{}造成的伤害`, `{}的持有上限`,
  `武技成功时恢复精力（{}）`, `武技的持续时间（{}）`; 207 effects) name a
  ninjutsu, onmyo magic or martial skill instead: the group row's `+0x14` is a
  subject key, and a parameter row `0, 0x100, 0x100, 0, key, 1, 0, name text,
  ?, 0, description text` carries its name. 175 resolved (怪风 for `0x23AF`,
  checked against the owner's 凶王耳饰); 32 groups carry no subject
  (奥义两立（{}）, 不消耗{}, five `{}造成的伤害`) and keep the generic
  （特定对象）. Merged into `effect-arguments.json`; helper
  `tmp/claude-ce/resolve_bare_subjects.py`. In-game checks so far: 0xF437
  幸运增加 (按司盾矛), 0x27F9 水 (凶王耳饰), 0x23AF 怪风 (凶王耳饰).
- 奥义两立（{}） (22 effects, 11 groups): the group's `+0x20` is a weapon-type
  key that the fully named `攻击力（刀）`-style groups share, which gives the
  weapon (刀, 双刀, 枪, 斧, 大太刀, 锁镰, 旋棍, 手甲, 薙刀镰, 机关棍, 手斧).
  不消耗{} names item `0x382A` (御神水, a consumable) at group `+0x18`. The
  eight v2.02 ninjutsu effects in groups 1127 and 1138-1144 carry no subject
  anywhere in their rows; an in-game look is pending.

## Forge-material markers and adding equipment (2026-09-29)

- Entry `+0x0D` high bits are the forge-material markers the game draws left
  of an effect: `0x40` a filled hexagon, `0x80` an outline one (the tooltip:
  changing the effect at the blacksmith needs 灵石炭). On the owner's 1552
  records every set entry has `0x40`, every grace `0`, and random entries all
  three about equally; the same effect varies between records, so it is not a
  table property.
- Adding equipment to a save (`save.prepare_character_edit` `add`) builds the
  record over the free slot with the purchase layout above, takes the key and
  serial from the two counters and advances them. Built from the purchase's
  free slot and inputs, it matches the game's record except the purchase flag
  (`0x180`; a new record gets a fresh drop's `0x82`), the seed and per-entry
  bytes the game fills from state not modelled (second group word, forge
  markers on random entries, `+0x0F`, `+0x10..+0x17`). New set entries carry
  `0x40`; innate and random entries carry no marker.
