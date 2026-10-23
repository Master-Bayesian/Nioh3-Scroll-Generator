# Situation-aware armor optimizer (PC 2.0.2.0, 2026-10-23)

## What it does

`evaluator/src/optimize.py` searches armor for the damage of named, triggered hit situations, not only the plain
hit. A scenario is a `+`-joined list of situations from `evaluator/src/situations.py`; the objective is a weighted
sum of scenario damages. Every damage is a full native MAIN replay of the 10-16 baseline hit (mid-stance first light
katana attack, training samurai) with the candidate's STAT/D and the scenario's state writes, so set rewards, the
weight curve and pool dilution are exact for this hit.

Situations (each confirmed by replay; see `NIOH3_DAMAGE_FORMULA_ZONES_20261022.md`): `back`, `strong`,
`high_stance`, `low_stance`, `desperate`, `pursuit`, `tsukumo`, `ki_exhausted`, `after_deflect` (status
aggregate 1.1, equal to the accepted buff path), and one target status from `status_poison`, `status_paralysis`,
`status_burn`, `status_water`, `status_shock`, `status_wind`.

A situation switches the conditional effects of this hit only. Motion value, attack and target stay those of the
captured light attack; `strong` is the strong-attack effect gate on this hit, not a strong attack's own motion.
Element hits are not offered on this weapon (not a real state); martial art, airborne and imbue situations were
confirmed on their own 10-22 captures and are not available on this baseline.

## Checks (`check_situations.py`)

On the accepted 10-21 loadout (bank + scroll):

- `plain` with the default D cells added gives the observed 1114.
- `after_deflect` by state write equals the accepted buff path (1225).
- Every situation evaluates. `back` 1435, `ki_exhausted` 1409, `pursuit` 1197, any target status 1197 (the
  顺势之法 term), `back+ki_exhausted+status_poison` 1905; stance, desperate, tsukumo and strong stay 1114 because
  this loadout wears none of those effects.

## Search

Start: the current bank loadout (weapon, accessories, scroll and armor stay; armor slots 4-8 are searched).
Stage 1 swaps every backpack armor piece of the hit's form alone into the start. Samurai and ninja forms wear
separate armor, so only samurai armor is a candidate for this samurai hit. Stage 2 runs coordinate ascent over
the top 6 singles per slot from two seeds. The result is best-known, not certified optimal. Runtime: about 2 min
for one scenario and 5 min for two (113-151 loadouts, read-set memo on).

## First results (backpack of 10-21)

| Objective | Best loadout change | plain | after_deflect+status_poison | back+ki_exhausted |
| --- | --- | ---: | ---: | ---: |
| current loadout | none | 1114 | 1317 | 1797 |
| plain = 1 | arm: another 木曾紫之铠 臂甲 copy | 1189 | 1399 | 1895 |
| triggered mix (both scenarios, weight 1) | head 蜷曲蛇形盔, chest 怪童大铠, the arm copy | 1188 | 1417 | 1893 |

The triggered objective picks a different outfit than the plain one: it gives up 1 plain damage for +18 after a
deflect on a poisoned enemy. The 10-21 recommendation (823 / 1244 class) included ninja pieces (黑狐面,
御庭番忍者装束) and is withdrawn.

## Limits

- One action and one target. Other moves need Pro r2 action substitution or their own capture.
- Item data is the 10-21 backpack; changes since then are not reflected.
- Armor only; weapon, accessories and scroll are fixed. Equip load and defense are not objectives.
