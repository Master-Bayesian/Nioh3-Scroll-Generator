# Rebuilding the damage model: a guide

Use this when a game update has broken the model and it must be rebuilt, possibly from nothing. It explains what to build, in
what order, how each part was found, how to tell it works, and which attempts to skip. Values are in the appendices, because
they will change. The methods should not.

Written 2026-11-06 for PC 2.0.2.0. **It has not yet been followed on a real update.** Expect to correct it the first time, and
do so in the same commit as the work. When a model fact changes, record how it was found, not only its value.

Claude and Codex did this work over about two months. Most of that time went on approaches this guide tells you to avoid.

## Part 1. Ideas that made it work

Read these first. Each cost weeks to learn.

**Run the game's code; do not derive its formula.** The early work reconstructed runtime machinery by hand and hit wall after
wall. What worked is a VM that executes MAIN's original instruction bytes on a captured memory state. The replay needs no
understanding of the formula, so it can be exact on the first run. Understanding came afterwards, from tracing the replay.
A neat fitted formula is a warning sign: the "154 + column" attack fit matched and was wrong.

**Drive everything from one observable number.** Pick one hit with one damage value, and make that number come out. Helper
counts, instruction counts and table coverage are not progress (`NIOH3_DAMAGE_METHOD_RESET_20261015.md`). If a piece of work
cannot change the number, or be checked against it, leave it for later.

**Let the process tell you what is missing.** Do not try to predict what to capture. Run the replay until it reads memory you
do not have, fetch exactly that from the same live process, and continue. This `demand_fill_loop.py` idea ended the frontier
problem of R10 to R15, where each round stopped at another uncaptured heap read.

**Compare at the narrowest point.** Every stage has an intermediate you can check against the game: aggregate tables, STAT
words, D words, packet fields, menu values. On a miss, find the first wrong intermediate. Do not reason from the final number.

**Change one thing and diff.** Reading before and after one swap isolates a source without breakpoints. Soul core, guardian and
clan effects were all found this way. Use it before disassembly.

**Compose from sources; do not carry deltas.** "Baseline plus difference" works for armor swaps, then fails for anything that
lives in the baseline: 1188 predicted against 884 real. Build the aggregate from named sources, and treat a nonzero residual as
an unfinished job.

**Prefer reading memory to debugging.** Almost everything can be read with plain `ReadProcessMemory`. Debugger breakpoints froze
or crashed the game several times. Attach a debugger only for the hit capture.

**Write the prediction down before the hit.** A number that matches after the model was changed proves little. Part 4 has the
protocol.

## Part 2. Build order

```text
 0 Preparation
 1 Find the damage ──> 2 Capture one hit ──> 3 Replay MAIN exactly
                                               │
                  4 Read the formula <─────────┤   (explains which inputs matter)
                                               │
                  5 Effect aggregate and D <───┘
                         │
          6 Equipment stages ──> 7 Non-equipment sources ──> 8 Attack and defense sources
                                                                   │
                                                 9 Attributes <────┘
                                                       │
                                     10 Predict from inputs ──> 11 Blind tests ──> 12 Optimizer
                                                                     │
                                                       13 Spells, buffs, other attacks
```

Each stage below gives the goal, the method, the finish line and what to avoid. Do not start a stage before the previous finish
line is met. If only part of the game changed, Part 5 says which stages can be skipped.

### Stage 0. Preparation

Goal: a safe bench, with nothing lost.

1. Back up everything in Appendix C and the old `Nioh3.exe` before the update installs. Stop Steam from updating automatically.
2. Record version, size and SHA-256 of the executable. Dump loaded `.text`, `.rdata`, `.pdata`. Disk bytes are not loaded code.
3. Use a test save. Set up a training samurai with a fixed character.
4. Keep the executable's old version beside the new one. Most old knowledge transfers by matching functions between the two.

Environment rules are in `AGENTS.md`. Python goes through `tools/run_python_tests.ps1` with absolute paths. Build files stay on
the build root.

### Stage 1. Find the damage

Goal: know which function computes damage, and where it stores it.

Method:

1. Hit the training samurai and read the HP drop. Find the target's HP in memory by value, hit again, and intersect.
2. Find what writes it. In 2.0.2.0 the store is in HP apply `787268`, called at `329890` inside hit dispatcher `3275AC`.
3. The dispatcher loads applied damage from the hit context at +0x218. Its only caller relation leads to MAIN `78CE8C`, which
   stores that field.

Finish line: at MAIN's return, context +0x218 equals the HP drop on several different hits.

Avoid: judging whether an attack "uses MAIN" from a filtered hook. Spells looked absent because the filter required source tag 0.
Log every dropped call before concluding (`observer/unmatched_hp_stacks.patch`).

### Stage 2. Capture one hit

Goal: a coherent snapshot of everything MAIN reads, plus the true answer.

The observer (`coherent_observer.lua`, frozen v5 in Appendix C, with `observer/observer_1027.patch`) hooks MAIN entry, filters to
the player's hits, and saves named memory regions: hit context, source actor, STAT, D, attribute source, target. It records the
exit value as the oracle. `drive_observer.py` does preflight, arm, wait, export, stop.

Rules that came from failures:

- Choose a plain hit: mid stance, first light attack, training samurai, no buff, full HP. Every conditional effect you avoid now
  is one you can add deliberately later.
- Training grounds rebuild the player every frame for a while. Do not let those events use up the callback budget.
- Keep regions of unknown bytes marked unknown, using known masks. Never fill them with zeros.
- Keep the game open after the capture. Stage 3 needs that same process.
- Close Cheat Engine when finished, and keep it away from Studio live addition. Together they crashed the game.

Finish line: an exported capture, with the oracle damage equal to what the game showed.

### Stage 3. Replay MAIN exactly

Goal: the original code, run offline on the capture, returns the oracle damage.

The engine is `r13-main/replay.py` with its original-byte VM (Appendix C). `demand_fill_loop.py` wraps it:

1. Start the VM at MAIN entry on the captured state.
2. When it reads a missing cell, read that cell from the live process, label it as module image or post-hit heap, and continue.
3. Stop on normal return, on a non-memory dependency, or on the budget.

Expect these frontiers:

| Stop | Meaning | Answer |
| --- | --- | --- |
| Read in a static table | Startup data on the heap | Safe to fill; list in `STATIC_TABLE_READERS` |
| D getter `624518` | Returns the player's D page | Cut after a byte check that the caller is a direct call; run the rest |
| Curve lookup `2FA678` | Heap copy of `resource01119` | Rebuild statically with `reconstruct_grade_table.py` |
| A read inside a captured object | Your capture is missing a part | Fix the capture; do not fill from later state |
| Volatile state | Timers, frame counters | Identify it; do not assume |

The first success took 12,363 steps and returned the exact 1105. Then run `--offline`, with every cell retained, so no game is
needed.

Finish line: offline replay returns the exact damage on two or more hits, with score ablation (erase the stored score, it must
be recomputed). `fast_eval.py` then keeps a warm evaluator for search.

Avoid: heap reads from after the hit are an assumption. Only trust them for state that did not change.

### Stage 4. Read the formula

Goal: know which fields enter this hit. Prediction does not need this, but debugging and every later decision do.

- `formula_trace.py` runs the replay with shadow dataflow and gives the exact expression behind +0x218.
- `zone_table.py` walks it from damage to motion value and names each multiplier zone.
- `field_scan.py` perturbs each effect field the replay read and reports the damage change.
- `gate_scan.py` flips each compared state value and shows which effects switch. With `--probe`, zero fields get 0.01, so
  effects the character does not wear still appear.

The result on 2.0.2.0 is in `NIOH3_DAMAGE_FORMULA_ZONES_20261022.md`. Compare the new version to it. Unchanged zones mean the
formula survived the patch.

Each table is exact only for the traced path: one action, one target.

### Stage 5. Effect aggregate and D

Goal: understand how "an effect on an item" becomes "a number MAIN reads".

Structure:

- Every effect has a group key. `556118` compiles sources, and each one reaches `557BA0(source, form, group, value)`.
- It sums into one table per form at source+0x35D0 and +0x4728. **These tables can be read live with no breakpoint.** This is
  the most useful fact in the project.
- The materializer `819400` turns the table into the D page and into some STAT fields. Each group has a recipe; there are 1152
  groups. Recipes are extracted, not guessed.

Method: read the table, run the lifted materializer over the default D page, and compare with the captured D.

Finish line: D rebuilt from the aggregate alone matches on all effect words. 0x3F0 and 0x880 remained different on 2.0.2.0 and do
not matter for these hits.

Avoid: materializing onto a captured STAT. It already holds its own sums, so fields look doubled. Always start from the emptied
frame and the default D.

### Stage 6. Equipment stages

Goal: from item bytes, reproduce STAT and the equipment part of the aggregate.

The stages run in native order, and order matters:

1. Weapon stage `818248`: records at STAT+0x24 and weapon effects.
2. Body stage `2FB2BC`, slots 4 to 12: defense, toughness, weight, floats, effects through `55764C`/`5557DC`, and set counting.
3. Slot 15 stage `81744C`: the equipped scroll, a flat attack on all records, defense, effects.
4. Set expansion `3FAA98`.
5. Load factor `819204`, then the materializer.

How a lift is made:

1. Disassemble the function and list what it reads and writes.
2. Translate it to Python, keeping widths, rounding and order. Do not simplify.
3. Run it on captured inputs and compare the output word by word with the capture.
4. Where the state is uncertain, compare with original-instruction runs.

The existing lifts are in `cache/.../project-source` and `deliverables/r16-damage-evaluator-v1-20261021/src`. On a patch, diff the
function bytes. Reuse a lift when the bytes are unchanged, and port only the changed lines otherwise.

Facts that took time to find:

- Weapon type is in the item row; samurai and ninja equipment are separate.
- Weight is item u16 +0xA0, using the redirect row at +0x168 when rarity is 4 or more.
- Saved backpack records are byte-identical to equipped E8 records.
- The packet is a copy. Changing weapon records does nothing until MAIN+8 onward follow.

Finish line: `regress.py` equipment regression passes, and blind armor and weapon swaps match in game. v1 passed 1018, 1120,
1018, 1114.

### Stage 7. Non-equipment sources

Goal: name every source of the aggregate, with zero residual.

Method:

1. Walk `556118`. Each block corresponds to a source family (table in `NIOH3_SPELLS_AND_PROGRESS_SOURCES_20261027.md`). Disassemble
   only enough to find which manager table and progress field each block reads.
2. Decode each family from those tables: title blessings, skills, clan blessing, elixir and region upgrades, area completion.
3. Subtract with `source_ledger.py`: live aggregate minus known sources. Every group fed only by known sources must match exactly.
4. For the rest, change one thing in game and diff (`ledger_diff.py`): remove the soul core, swap the guardian.
5. Store the result with `build_source_layers.py`, and check with `check_composed.py`.

Practical points:

- Each style has its own attribute source and table. The inactive one refreshes only after switching to it. Read both only after
  visiting both.
- A removed source removes its entry. A zero-value entry still materializes: 4787 against 4677.
- Do not count a family twice. Region upgrades were; zero residual exposed it.
- Old deliverables already answered some questions (`r16-live-passives` had skills). Search them before researching.
- Text comes from the tree at Q(module+0x45C8450)+0x1CD0 (`dump_all_texts.py`). Name groups early; it makes the rest easier.

Finish line: `check_composed.py` shows zero wrong groups in both forms, and `layer_stability.py` shows the layer constant across
saved outfits of one day.

### Stage 8. Attack and defense sources

Goal: account for each point of the attack record and defense.

Method: compute weapon stages alone, and subtract from the live record. The remainder is what other systems add. Then look for
functions that write all records.

- Guardian: `374F58` adds 玩家的强化值 to six records and +0x1C8. Values come from `guardian_stage.numeric(key, level)`.
- Soul core: `554268` adds attack and defense from passive slots 0 and 1. Values come from the passive table through the item's
  key and level. Effects are in the item record.
- Record +8 is the 攻击力 group, plus 35 on samurai records 0 and 1. The +35 is still unexplained.

Soul core check: the table gave 165, 164 and 116, equal to the live differences.

Avoid: fitting the remainder. It looked like an attribute curve until the true sources appeared.

### Stage 9. Attributes

Goal: know every path from attribute points to damage.

There are five paths. Attributes do not change D for a fixed aggregate.

1. Weapon scaling (coreX), computed from a coefficient, table90 column 0x1A and the attribute.
2. Stat seeds (`stat_seeds`), load factor and HP seed.
3. Equipment requirement gates. Below a threshold, effects leave the aggregate. Stage 6 handles this when given attributes.
4. **Reflection at hit time.** MAIN reads attributes inline at STAT+0x844 through grades in D. This was missed until 11-06.
5. Attribute-gated entries, such as group 0xECC5 at 150.

Finish line: the blind tests of Stage 11 pass at several attribute sets, including a low one.

### Stage 10. Predict from inputs

Goal: one function: inputs in, damage out, no target capture state used.

`state_predict.py` does this. It rebuilds the state and writes it into the warm baseline replay. **The replay only sees what you
write**, so this is the list that must stay complete:

| Written | Why |
| --- | --- |
| STAT words below 0x7A0, as differences | Materialized fields |
| Records, record +8, +0x12C | Attack, from stages plus guardian and cores |
| +0x7A0, +0x7B8 | Load factor, weight |
| Seeds, HP, +0x1C8 | Attribute and defense paths |
| D effect words | Aggregate effects |
| MAIN +8, +0xC, +0x10, +0x14 | Packet copy |
| `current_player.external_STAT`+0x7B8 | Formula reads weight from a second object |
| STAT+0x844 | Formula reads attributes inline |

How to find a missing entry: copy the target's whole STAT and D into the replay. If the damage becomes correct, the fault is in
your writes, and bisecting finds the word. If it stays wrong, the fault is outside those objects, or in the packet.

Finish line: `regress_inputs.py` reproduces every recorded game number.

### Stage 11. Blind tests

Follow Part 4. Test each new input class once at least: attributes, core, guardian, armor, weapon, scroll.

### Stage 12. Optimizer

Goal: search over real items with exact calls.

`fast_eval.py` memoizes on read sets, so most candidates cost little. `search_inputs.py` searches attributes, cores and guardian;
`search_outfit.py` searches items; `situations.py` handles conditional effects. See `NIOH3_SITUATION_OPTIMIZER_20261023.md`.

Results are best-known, not proven optimal, and they apply to the action and target of the capture.

### Stage 13. Other attacks and buffs

Each new attack type needs its own capture and Stages 2 to 4.

- Spells go through MAIN with a tag-4 source. The owner is at source+0x260, and the projectile must be captured because it dies
  with the hit.
- Debuff count enters through D+0x2B4. Set up order deliberately; the first spell's blind changed the second one's damage.
- Status buffs are in `NIOH3_NATIVE_BUFF_MECHANISMS_20261019.md`.

## Part 3. When a number is wrong

Work down this list. Stop at the first failure.

1. **Is the state what you think?** Read attributes, cores, guardian and form live. On 11-06 a miss was simply the wrong attributes.
2. **Is the aggregate right?** Compare the composed table with the live one, group by group. A wrong group is a source problem.
   Go to Stage 6 or 7.
3. **Is D right?** Compare with live D on effect words. A difference with a correct aggregate is a materializer problem.
4. **Is STAT right?** Compare with live STAT. Records wrong points to Stage 8. Seeds wrong points to Stage 9.
5. **All of them right and the damage still wrong?** MAIN reads something else. This is how 0x844 was found. Copy the target's
   STAT and D wholesale, bisect, or use `formula_trace.py` to list every input.
6. **Is the replay itself right on this hit?** Capture this exact hit and replay it. If that fails, the problem is the path, not
   the model: a new gate, target state or action.
7. **Is it conditional?** Use `gate_scan.py`, and check HP, stance, target status and hit order.

Do not add a constant to close a gap. A gap has a cause.

## Part 4. Blind test protocol

1. Commit or save the predicted number and exact state before the hit.
2. Read the live state before accepting a result.
3. Keep the hit fixed: action, stance, target, no buff, full HP.
4. A number obtained after fixing the model is a reproduction. Label it so, then make a new prediction.
5. Avoid tests that cannot fail. Guardian 凤凰 predicted the same 1038 here, so it would prove nothing.
6. Choose tests that separate paths. Moving 10 points from the 7th attribute to the 4th tests reflection and scaling together
   with a small, exact change.
7. Keep results in the regression, so later changes cannot silently break them.

Attributes can be set directly on a test save, which saves the owner respec time. See Appendix B.

## Part 5. Which stages a given update needs

| What changed | Do |
| --- | --- |
| Executable identical | Run the regression only |
| Code moved, tables and function bytes same | Stage 0, update Appendix A with Stage 1 and 2 methods, new capture, Stages 3, 10, 11 |
| Tables changed, code same | Refresh tables (`OFFLINE_PARAMETER_ACQUISITION.md`), rebuild layers (Stage 7), new capture, Stages 10, 11 |
| Some functions changed | Diff bytes. Reporting which stage owns each function tells you which lift to port. Rerun from that stage |
| Formula zones differ in Stage 4 | Treat as a new formula. The replay still works; lifted stages may not |
| Layouts changed | Fix readers first; check each field with a live read before trusting it |
| New items or sets only | Refresh tables, one blind test |

A new capture is always needed when the executable changes, because the replay runs on its bytes and addresses.

How to move addresses:

1. Run the repository's `tools/prepare_game_version_update.py` and `compare_game_version_resources.py`, following the
   Recovery sequence in `COMBAT_RESEARCH_UPDATE_RUNBOOK.md`.
2. Do not apply one offset to everything. Keep ambiguous anchors ambiguous.
3. Derive data roots from matched code, then validate the heap shape.
4. Match functions by bytes around the old site, then by call graph from a matched neighbour.
5. Where a table has no handle, search the module for a text ID immediate.
6. Confirm each layout with a live read against the menu.

Fail closed. Keep a version unsupported until its anchors are resolved. Product gates are separate
(`docs/knowledge/GAME_VERSION_UPDATE_PIPELINE.md`).

## Part 6. Trap list

| Trap | What happened | Rule |
| --- | --- | --- |
| Hit-time reads beyond materialized STAT | 994 predicted, 1038 real | Everything MAIN reads must be written |
| Second STAT object | 859 predicted, 884 real | Write `current_player.external_STAT`+0x7B8 |
| Packet copy | Changing records had no effect | Update MAIN+8 onward |
| Zero entry | 4787 against 4677 | Removing a source removes the entry |
| Captured STAT frame | Fields doubled | Use the emptied frame and default D |
| Neat fit | False attack formula | Prefer the source; withdraw fits |
| Tag filter | Spells looked absent | Log dropped calls |
| Family counted twice | Wrong layer | Zero residual decides |
| Stale style | False removals | Visit both styles |
| Delta on captured state | 1188 against 884 | Compose |
| Baseline-specific constants | Guardian and cores stay inside | Rebuild records; subtract baseline sources |
| Chunked reads | Objects missed | Page-tolerant reads, accept partial results |
| Relative paths in the wrapper | Touched the wrong drive | Absolute paths |
| Correlated checks | Same primitives on both sides "agreed" | Use the game as the oracle |
| Counts as progress | Helper counts rose, damage did not | Follow the number |

Longer accounts are in `EXPERIMENT_FAILURE_LEDGER.md`.

## Appendix A. Version-bound values (2.0.2.0)

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

### Data and refresh

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

### Code entry points

Entry points today:

- `evaluator/src/state_predict.py`: `Predictor.predict(capture, attrs, guardian, cores)`. Use this one for new work.
- `evaluator/src/regress_inputs.py`: every observed number of the current hit, from inputs alone.
- `evaluator/src/search_inputs.py`: search over attributes, cores and guardian.
- `evaluator/src/composed_model.py`: composition of the aggregate.
- `evaluator/src/cross_state.py`: the original experiment script. It duplicates `state_predict.py` and keeps its frozen predictions.
- `deliverables/r16-damage-evaluator-v1-20261021/regress.py`: the 10-21 equipment regression.

## Appendix B. Live recipes

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

## Appendix C. What must be kept outside git

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

Other paths this guide relies on: the replay is `deliverables/pro-r2-return/.../runtime/project-source/r13-main/replay.py`, and
frozen observer v5 is `deliverables/r16-live-passives-20261016/resources/frozen-observer-v5/`.

## Appendix D. Current evidence and limits

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
| Attributes (5,200,15,75,5,35,150), found by search, menu attack 3045 and HP 3198 also predicted | 1105 |

Bounds of this evidence: one action, one target, samurai. 1038 and 849 were reached after seeing the game's number. Spell attack
against attributes, ninja, other weapons and other enemies are not covered.

## Appendix E. Open items


- The record +8 field: +35 samurai, +36 ninja, source unknown (`PLUS8_WEAPON`).
- Zero-value group 0x3E52 in the live aggregate, source unknown. No effect on this hit.
- Groups 0x52D 36 and 0xF919 35, no text or located source.
- Area completion is a stored total, not recomputed.
- The game's own level word among three candidates.
- Spell attack against attributes; debuff count with several statuses; stacking buffs.
- Passive slots 2 to 7 hold items with no effect in the aggregate; their role is unconfirmed.
- Persistence of live attribute writes.
- `cross_state.py` and `state_predict.py` duplicate logic.

## Appendix F. Related documents

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
