# Nioh 3 damage evaluator v1 (PC 2.0.2.0) — accepted 2026-10-21

Given a captured baseline hit and the item bytes of an equipment set, this predicts the in-game damage of that
hit. It runs the game's own code, lifted or replayed natively, and fits no coefficients. The prediction is
written before the owner strikes. Every number below equals what the game showed.

| Case | Change from the 10-16 baseline (鬼丸国纲, no armor, mid-stance first light, training samurai) | Predicted | In game | Kind |
| --- | --- | ---: | ---: | --- |
| baseline | none | 714 | 714 | natural, 10-16 and 10-21 |
| helmet | 东国无双之铠 头盔 | 720 | 720 | natural (10-16) |
| helmet + deflect buff | class0 aggregate 1.1 | 793 | 793 | natural (10-16) |
| outfit + weapon | 甲斐之虎 头盔, 邪马台国 胸甲/下衣, 木曾紫 臂甲, 田村后裔 腿甲, 备前长船 180+30 | 1018 | 1018 | **blind** |
| same, after a deflect | + aggregate 1.1 | 1120 | 1120 | **blind** |
| + accessories | 叶二 (slot 9), 八咫镜 (slot 10) | 1018 | 1018 | **blind** |
| + scroll | 百境百怪绘卷·顿悟 (slot 15) | 1097 | 1097 | reproduction |
| other scroll copy | same item id, different effects | 1114 | 1114 | **blind** |

`regress.py` re-runs all of them offline: all 8 match. Latest run: `work/regress-20261021-215038/REGRESSION.json`.

## How it works

```
item E8 records ──> src/outfit_model.py  (native lifts, rebuild order)
   818248 weapon stage: weapon records at STAT+0x24, weapon effect collection
   2FB2BC body stage, slots 4..12 (armor 4..8, accessories 9..): defense, toughness, weight 7B8,
          body floats, effect collection 55764C/5557DC with set-side counting
   81744C slot-15 stage (equipped scroll): flat attack on every weapon record, defense, scroll effects
   3FAA98 set expansion   ->   819204 load factor   ->   81920E..819251 materializer (D and STAT)
   result = captured baseline + (candidate model - baseline model), per 4-byte word
──> src/rank_outfits.py: changed STAT/D words, plus the attack packet from weapon record 0
       (MAIN+8 = rec0+0 + rec0+8, MAIN+C = rec0+C, MAIN+14 = rec0+18; equal in all captured hits)
──> native MAIN 0x78CE8C replay (src/demand_fill_loop.py --offline), optional buff aggregate owner+0x2E3C
```

Static data: the 2FA678 curve table is rebuilt from the static `resource01119` in its native row order
(`src/reconstruct_grade_table.py`). It is checked against 794 retained live bytes with 0 mismatches.

## Use

All Python runs through `F:/Nioh3_ScrollEditor/tools/run_python_tests.ps1` with
`NIOH3_BUILD_ROOT=F:/Nioh3_r16_build`.

- **Regression:** `regress.py`.
- **Rank the backpack:** `src/rank_outfits.py <cache> <baseline capture> <baseline request> <inventory dir> <out> <retained cells> --extra-cells inputs/grade_table_cells.json [--outfit name=off,off@slot ...] [--weapon-kind 1 --weapon-type 刀] [--buff 1.1]`.
- **Best-known outfit:** `src/search_outfit.py`.
- **Current backpack:** `src/read_live_inventory.py`.
- **Currently worn items:** `src/read_live_bank.py`. Both readers are read-only CE.
- **Add exact items through the installed Nioh3Studio:** `src/studio_client.py` and `src/studio_add_clones.py`.
  Close Cheat Engine first; see the limits below.

## Limits (v1 scope)

- **One action and one target.** Results cover the captured action and target only. Other moves of the same
  weapon can use Pro r2 action substitution. A new weapon type or a new enemy needs one capture each.
- **Baseline.** The baseline is one actor state, captured 10-16 and still identical on 10-21. Level, skill or
  attribute changes need a new baseline capture plus the live demand fill (about 20 minutes).
- **Assumed byte.** The config byte CF8 is assumed to be 0 (no item-sync rewrite). This is supported by every
  match above, but the byte itself has not been read.
- **Weapon-swap rounding.** A few D words carry the old weapon's effect and are corrected by an f32 delta, not
  by native-order addition. This can cost ±1 damage in principle; it did not in any case above.
- **Conditional effects.** Conditional effects need the matching runtime state at the hit: buff aggregate, HP
  state, target state.
- **Search.** The backpack search is a best-known local search, not a certified optimum. It optimizes one hit.
- **Live writes.** Never run Studio live addition while CE's VEH debugger is attached. On 10-21 the two
  debuggers together crashed the game (0xC0000005 at 0x11000190E). The save survived.

## Layout

- `src/`: code.
- `inputs/`: frozen inputs: the 10-16 baseline, inventories and bank records, the curve table, item names.
- `evidence/`: the 10-21 natural captures; not in git, hashes are in `MANIFEST.json`.
- `regress.py`: the regression.
- `MANIFEST.json`: hashes of every file, plus the external dependencies.
