# Rebuilding the damage model after a game update

Status: written 2026-11-06 for PC 2.0.2.0. **This procedure has not yet been run on a real update.** Every value below was
verified on 2.0.2.0 only. Update this file in the same commit as any model change. Record how a value was found, not only
the value.

Related documents, which this one links instead of copying:

| Document | Use it for |
| --- | --- |
| `OFFLINE_PARAMETER_ACQUISITION.md` | Extracting parameter tables from the game archives, and classifying what changed |
| `COMBAT_RESEARCH_UPDATE_RUNBOOK.md` | Recovering managers, skill graph, text and buff semantics on a new version |
| `NIOH3_CURVE_RESOURCE_RECONSTRUCTION_20261015.md` | Rebuilding curve tables, with a reproduction section |
| `NIOH3_DAMAGE_FORMULA_ZONES_20261022.md` | Formula zones and gates, and the trace tools that produced them |
| `NIOH3_SPELLS_AND_PROGRESS_SOURCES_20261027.md` | Evidence and results for spells, sources, composition, attributes and soul cores |
| `NIOH3_NATIVE_BUFF_MECHANISMS_20261019.md` | Status buffs, stacking, dilution |
| `NIOH3_SITUATION_OPTIMIZER_20261023.md` | Situation-aware armor search |
| `EXPERIMENT_FAILURE_LEDGER.md` | What went wrong before, and why |
| `docs/knowledge/GAME_VERSION_UPDATE_PIPELINE.md` (main repository) | Product gates. Research recovery does not replace them |

## 1. What the model is

The model runs the game's own logic. No coefficient is fitted. Three kinds of code do the work:

- **Lifted stages:** Python reimplementations of native functions, in `cache/original/nioh3-r10-pc202-20261016/project-source`
  and `deliverables/r16-damage-evaluator-v1-20261021/src`.
- **Native replay:** the original MAIN function runs from executable bytes on a captured memory state (`demand_fill_loop.py`).
- **Static tables:** extracted or dumped game data.

```text
inputs: outfit items, attributes, guardian, soul cores, skills, title blessings, clan blessing, progress
  weapon stage 818248 ──┐
  body stage 2FB2BC ────┤ equipment aggregate on an emptied source
  scroll stage 81744C ──┤
  set expansion 3FAA98 ─┘
  + named layers (SOURCE_LAYERS.json) + guardian table effects + soul core item effects
       = effect aggregate per form (source+0x35D0 form 0, source+0x4728 form 1)
  load factor 819204, materializer 819400  => D page and STAT fields
  attack record = weapon stages + guardian attack + soul core attack
  attack packet  MAIN+8 = rec0+0 + rec0+8, +C = rec0+C, +10 = rec0+10, +14 = rec0+18
  attributes inline at STAT+0x844 (seven i32), read at hit time
  native MAIN 78CE8C on the 10-16 frame  => damage at MAIN+0x218
```

Entry points today:

- `evaluator/src/state_predict.py`: `Predictor.predict(capture, attrs, guardian, cores)`. Use this one for new work.
- `evaluator/src/regress_inputs.py`: every observed number of the current hit, from inputs alone.
- `evaluator/src/search_inputs.py`: search over attributes, cores and guardian.
- `evaluator/src/composed_model.py`: composition of the aggregate.
- `evaluator/src/cross_state.py`: the original experiment script. It duplicates `state_predict.py` and keeps its frozen predictions.
- `deliverables/r16-damage-evaluator-v1-20261021/regress.py`: the 10-21 equipment regression.

## 2. First decision after an update

1. Do not delete or overwrite anything from the old version. Copy the old `Nioh3.exe` and record version, size and SHA-256.
2. Classify the update:

| Finding | Meaning | Work |
| --- | --- | --- |
| Executable hash unchanged | Nothing to do | Run section 7 |
| Executable changed, tables unchanged, anchors found at new addresses | Relocation | Sections 3, 4, 6, 7 |
| Tables changed | Balance patch | Sections 3, 5, 6, 7 |
| Function bodies changed at anchors | Formula or layout may have changed | Everything, starting with the changed function |

3. Fail closed. If an anchor is ambiguous, keep the old version unsupported until it is resolved. Do not apply one RVA delta to
   all addresses; offsets in different sections move differently.

Most of the effort after a small patch should be sections 4 and 6. Rediscovery should be needed only for what actually changed.

## 3. Version-bound values

All of these must be revalidated. RVAs are relative to the `Nioh3.exe` base.

### Global roots

| Value | 2.0.2.0 | Hard-coded in | How it was found, how to re-find it |
| --- | --- | --- | --- |
| Manager root | `Q(base+0x45B9E30)` | `read_skills.py`, `dump_sources.py`, `source_ledger.py`, `read_blessings.py` | Soul core name builder at `+0x1F1730B` reads it. Find by a text ID immediate in the module, then disassemble |
| Progress object | `Q(Q(base+0x4751860))` | the same files | `9D2B74` reads the level at +0x69DC from it |
| Text tree | `Q(base+0x45C8450)+0x1CD0` | `dump_all_texts.py`, `dump_sources.py` | MOV at `5A4B07`, displacement at `5A4B0A`. Chain `5A4B04 -> 63D9C4 -> 63DDA0` |
| Inventory root | `base+0x4751530` | `read_live_inventory.py` | Compare with `docs/knowledge` inventory notes |

### Native functions

| RVA | Role |
| --- | --- |
| `78CE8C` | MAIN damage function. Damage stored at hit context +0x218 |
| `3275AC` | Hit dispatcher, only caller of MAIN. `32843F` loads +0x218; `329890` calls HP apply `787268` |
| `325F4C` | Resolver. Tag-4 source owner at +0x260 |
| `818248` | Prepared weapon stage |
| `2FB2BC`, `2FB4D1` | Body stage, slots 4..12 |
| `81744C` | Slot 15 stage, equipped scroll |
| `3FAA98` | Set expansion |
| `819204` | Load factor |
| `81920E..819251`, `819400` | Materializer |
| `2B30A0`, `78A194` | Attack packet builder and copy |
| `556118` | Source compiler. Blocks in the table of the spells document |
| `557BA0`, `55764C`, `5557DC` | Effect aggregation and collectors |
| `374F58` | Guardian: adds 玩家的强化值 to six records and +0x1C8 |
| `554268` | Passive items: slots 0, 1 add attack and defense to records and +0x1C8 |
| `2FA678`, `D24926` | Grade curve lookup and binary search; heap table is `resource01119` |
| `FBFE00` | Piecewise attack-minus-defense bonus |
| `2FA2F0..2FA3AF` | Effective item parameters (`manager+0x9B0`, stride 0xF8) |
| `624518` | D getter. `demand_fill_loop.py` cuts calls to it after a byte check |
| `6267D0` | Status boost and cut pair |

Also bound to the version: `IMAGE_SIZE` and `STATIC_TABLE_READERS` in `demand_fill_loop.py`, `RVA` values in `static_gates.py`,
`OUTPUT` cut sites in the replay, and all hook sites in the observer.

### Layouts

| Object | Field |
| --- | --- |
| Hit context (MAIN) | +8 attack, +0xC, +0x10 break, +0x14 element, +0x140 form, +0x148 stance (0 high, 1 mid, 2 low, -1 ninja), +0x198 source, +0x1C8 target, +0x218 damage |
| STAT | +0x18 HP, +0x24 records (64 bytes, six of them), +0x12C attack group copy, +0x1C8 defense, +0x218 toughness, +0x7A0 load factor, +0x7B8 weight, +0x7C0 D pointer, +0x7D8 source pointer |
| STAT inline | +0x820 level and +0x824 attributes, +0x840 level and +0x844 attributes. STAT = source - 0x810 |
| Source | +0x14 raw attributes, +0x34 synchronized attributes, +0x10 and +0x30 level, +0x2C form, +0x58 guardian slots (`IHH` key, level, reserved), +0x70 active index, +0x72 u16 compared to `required_u16` (146), +0x80 passive items, 0xE8 each, eight slots |
| Aggregate | source+0x35D0, +0x4728: 12-byte entries (u16 key at 0, u32 value at +4; decode through typed_source), count at +0xC00 |
| Passive item | u16 key at 0, level at +0x36.. from the numeric helpers, seven effects from +0x30, stride 24: u16 group at +4, u32 value at +12 |
| Equipment item | Effects at +0x3C. Bank stride 0xF70, slot stride 0xE8 |
| Current player | Weight is read from `current_player.external_STAT`+0x7B8, a second object |
| Owner | +0x2DC0+0x7C = +0x2E3C class 0 status aggregate |
| Progress | +0x5D98 and +0x5DE8 upgrade maps, +0x5EA8 title ranks, +0x6020 skills, +0x60A0 slotted skills, +0x6340 clan slots, +0x69DC level |
| Manager | +0x118 passive table, +0x130/+0x598 skills, +0x268/+0x278 completion, +0x7C0/+0x7C8/+0x7E0 upgrades and titles, +0x9B0 rarity |
| D | Field map comes from materializer recipes for 1152 native groups. Reflection grades: +0x34 心, +0x38 刚, +0x40 咒; +0x2B4 debuff count; +0x31C weight reflection |

Reflection field-to-attribute mapping beyond 心 and 刚 is not fully written down. The native replay handles it; check it if a
reflection-based number moves.

## 4. Recovering addresses and layouts

1. Dump loaded `.text`, `.rdata` and `.pdata`. Do not assume disk bytes equal loaded code. Use `SizeOfImage`.
2. Run the repository's `prepare_game_version_update.py` flow (see the combat runbook, Recovery sequence). Preserve ambiguous anchors.
3. Rederive data roots from matched code, then validate heap shape and locale independently.
4. For each function in section 3, match by bytes around the old site, and by call graph from a neighbour that matched.
   Reachable anchors: MAIN is the only callee of the dispatcher; the materializer is called from the load factor area;
   `557BA0` is the common callee of the compiler blocks.
5. Search the module for text ID immediates when a data table has no other handle. Capstone is in the project environment.
   Never name a script `dis.py`.
6. Check layouts with the read-only recipe in section 8. Each layout can be confirmed from a live read:
   attributes against the menu, records against 近距离攻击力 minus the +8 field, defense against 防御力.

## 5. Recovering tables and static data

Follow `OFFLINE_PARAMETER_ACQUISITION.md` for archives. This model additionally needs:

| Data | Source today | Refresh |
| --- | --- | --- |
| Passive table (`manager+0x118`, 84 rows, stride 0x54) | Live dump, `work/live-1105/passive118_rows.bin`; hash equals the 10-16 receipt `CAPTURE_RECEIPT.json` | Dump again, compare hash. `key_to_index` lives in the receipt |
| Guardian definitions | Cached, via `guardian_stage.py`; export with `guardian_table.py` to `data/GUARDIANS.json` | Rerun; compare with one menu |
| Set effects | `export_sets.py` to `data/SETS.json` | Rerun |
| Effect text | `dump_all_texts.py`, `data/TEXTS.json`, `GROUP_TEXT_IDS.json` | Rerun with the game open |
| Grade curves | `reconstruct_grade_table.py` from `resource01119` | Rerun; the check is against retained live bytes. `NIOH3_CURVE_RESOURCE_RECONSTRUCTION_20261015.md` has the details |
| Default D page | `inputs/default_d_cells.json` | New capture |
| Blessings, skills, clan | `read_blessings.py`, `read_skills.py`, `dump_sources.py`, `source_ledger.py` | Rerun on the test save |
| Source layers | `build_source_layers.py` to `data/SOURCE_LAYERS.json`, with `check_composed.py` | Rerun after a new capture |

The r10 cache has a sealed membership check and rejects extra files. `grade_curves.ROOT` points outside it on purpose.

## 6. New baseline capture

A changed executable invalidates every captured frame and MAIN replay. The evaluator needs one fresh hit on the new version.

1. Port hook sites in `evaluator/observer/` to the new RVAs. Take observer-v5 `coherent_observer.lua` from
   `deliverables/r16-live-passives-20261016/resources/frozen-observer-v5/` and apply `observer_1027.patch`, or spell hits will be dropped.
   `drive_observer.py` runs it.
2. Keep the game open. Capture one plain hit with the observer: mid stance, first light attack, training samurai, no buff.
   Training grounds rebuild the player every frame for a while, so the callback budget must not count those events.
3. Run `demand_fill_loop.py` while the same process is alive. It fills each missing read from the live PID and labels it as
   module image or post-hit heap. This took about 20 minutes. Dump heap pages before closing the game.
4. Replay must return normally with the exact damage at MAIN+0x218. `--offline` then reproduces it without the game.
5. Update the frame used by `state_predict.py` (`BASE`, `BASE_DEFENSE_SOURCES`, `CORE_SLOTS`, `PLUS8_WEAPON`). `BASE_DEFENSE_SOURCES`
   is the sum of guardian and soul core defense present in the baseline.
6. Rebuild the empty-retained static set and run `check_composed.py`. The residual must be zero on every group.

## 7. Acceptance order

Run these in order. Stop at the first failure and fix that level.

| Step | Check | Pass |
| --- | --- | --- |
| 1 | Replay of the fresh capture | Exact damage |
| 2 | `check_composed.py` | Zero aggregate residual, both forms |
| 3 | `layer_stability.py` | Layer constant across saved outfits of a day |
| 4 | Menu check | Attack, defense, HP and weight equal the prediction |
| 5 | `regress_inputs.py`, updated with fresh game numbers | All equal |
| 6 | `regress.py` | Equipment regression equal |
| 7 | Blind tests, section 9 | Exact |

Current regression, all exact on 2.0.2.0 (`data/REGRESS_INPUTS_20261106.json`):

| Case | Game |
| --- | ---: |
| 10-23 no armor | 793 |
| 10-23 armored, one core | 884 |
| Both cores | 1038 |
| Attributes (5,5,5,36,6,5,140) | 849 |
| Attributes (37,150,27,36,80,15,140) | 1043 |
| Attributes (37,140,27,26,80,15,160) | 1031 |
| Attributes (37,150,37,26,80,15,140) | 1036 |

Bounds of this evidence: one action, one target, samurai. 1038 and 849 were reached after seeing the game's number. Spell attack
against attributes, ninja, other weapons and other enemies are not covered.

## 8. Live recipes

**Read without a debugger.** `live_read.py <pid> <out>`. Finds attribute sources by the attribute pattern; change the pattern to
your attributes. Reads must tolerate partial pages: use `ReadProcessMemory` and accept `got.value`. A failed whole-chunk read
silently skips objects. Confirm with STAT+0x7D8 == source. Each style has its own source with one table filled, and the
inactive style refreshes only after switching to it.

**Find a copy of an unknown value.** `level_scan.py` scans, the owner changes the value once, and a second run intersects.
This narrowed 17125 addresses to 7.

**Write attributes on a test save.** `live_attr_write.py <pid> <old> <new>`, then swap one piece of equipment so the game
recomputes. Level needs a separate write; the game's own level was not separated from two look-alike words. Persistence across a save and reload is
unverified.

Rules:

- Never run Studio live addition while the CE VEH debugger is attached. It crashed the game on 10-21.
- Armed Lua-callback breakpoints froze the game; keep them bounded inside one call.
- Separate runtime data from injected code in any cave.
- Read an aggregate only after visiting that style.

## 9. Blind test protocol

1. Freeze first. Commit the predicted numbers and the exact state, or write them to a file, before the hit.
2. Read the live state before each hit. Confirm attributes, core slots and aggregate count. On 11-06 a hit was almost reported
   against the wrong attributes.
3. Keep the hit fixed: same action, target, stance, no buff, full HP.
4. A number obtained after fixing the model is a reproduction, not a blind pass. Say which it is.
5. Avoid tests that cannot fail. Guardian 凤凰 predicts the same 1038 here, so it says nothing.
6. On a miss, compare live STAT, aggregate and D to the composed ones first. That splits the problem in one step:
   if all match, the fault is in what the replay reads beyond them.

## 10. Traps already paid for

| Trap | Consequence | Rule |
| --- | --- | --- |
| Replay state beyond the materialized STAT range | 994 predicted, 1038 real. Attributes at STAT+0x844 stayed at the baseline's | Anything read at hit time must be written, not just STAT below 0x7C0 |
| Weight read from a second object | 859 predicted, 884 real | Write `current_player.external_STAT`+0x7B8 |
| Packet is a copy | Changing records has no effect | Update MAIN+8, +0xC, +0x10, +0x14 |
| Zero entry is not absence | 4787 vs 4677 | A removed source removes its entry |
| Materializing onto a captured STAT | Fields look doubled | Start from the emptied frame and default D |
| Fit with a neat formula | "154 + column" attack fit was a coincidence | Prefer the table path; withdraw fits when the source appears |
| Filter on source tag | Spell hits looked absent from MAIN | Log dropped calls before concluding |
| Region upgrades counted twice | Wrong layer | Only source7c0 enters; check with zero residual |
| Stale inactive style | False removals | Visit both styles |
| Relative paths in the test wrapper | Hit the wrong drive | Always absolute paths |
| Chunked memory reads | Objects missed | Page-tolerant reads |
| Delta on a captured state | 1188 predicted, 884 real | Compose from sources |
| Character-specific constants | Baseline carries guardian and cores | Subtract them through `BASE_DEFENSE_SOURCES` and rebuild records |

## 11. What must be kept outside git

Git holds code, documents and data summaries. These are not in the repository and cannot be rebuilt without the old game version:

| Path under `F:/Nioh3_r16_build` | Size | Contents |
| --- | ---: | --- |
| `cache/` | 1.2 GB | Sealed tables, lifted stages (`project-source`) |
| `deliverables/r16-damage-evaluator-v1-20261021/` | 108 MB | Evaluator v1, frozen baseline, inputs |
| `work/blind-1023/` | 331 MB | Blind captures used by the regression |
| `work/live-1104/` | 246 MB | Spell captures, retained cells, ledgers, texts |
| `work/live-1105/`, `work/live-1106/` | 6 MB | Passive table, live dumps |
| `deliverables/pro-r2-return/` | | Replay runtime referenced by `demand_fill_loop.py` |
| A copy of the old `Nioh3.exe` | | Needed to diff functions |

The build root is not a cache. It was deleted once by accident on 10-17 under its old location. Back up these paths before a
game update installs. Steam will replace the executable.

## 12. Open items

- The record +8 field: +35 samurai, +36 ninja, source unknown (`PLUS8_WEAPON`).
- Zero-value group 0x3E52 in the live aggregate, source unknown. No effect on this hit.
- Groups 0x52D 36 and 0xF919 35, no text or located source.
- Area completion is a stored total, not recomputed.
- The game's own level word among three candidates.
- Spell attack against attributes; debuff count with several statuses; stacking buffs.
- Passive slots 2 to 7 hold items with no effect in the aggregate; their role is unconfirmed.
- Persistence of live attribute writes.
- `cross_state.py` and `state_predict.py` duplicate logic.
