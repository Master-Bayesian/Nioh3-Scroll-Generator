# Nioh 3 damage formula and multiplier zones (PC 2.0.2.0, 2026-10-22)

## Method

`evaluator/src/formula_trace.py` runs the accepted native MAIN replay with a shadow dataflow layer. Alongside
every original instruction it records which state bytes each value came from and which operation produced it.
Module constants, stack scratch and static tables count as constants. The damage stored at MAIN+0x218 is the root
of an exact expression DAG for that execution path. The shadow does not change the native run: baseline 714, 1114
and 1120 reproduce with identical step counts.

Three tools build on the trace:

- **`zone_table.py`** walks the spine from the damage down to the action's motion value. Every operand off the
  spine is a zone. It names each field from the versioned materializer recipes for all 1152 native effect groups,
  the effect-name catalog and the skill-source catalog.
- **`field_scan.py`** perturbs every effect field the replay read and reports its damage delta.
- **`gate_scan.py`** flips every state value MAIN compares, plus the MAIN hit-context bytes +0x280..0x2B0, and
  diffs the effect fields in the formula. With `--probe`, every zero f32 effect field is first set to 0.01, so
  effects the candidate does not wear still enter the formula.

Each table is exact for the traced path, one action and one target. Here that is mid stance, first light katana
attack, against the training samurai.

## Formula of this hit

```text
base      = motion% (action modifier +0x10, here 26) * 0.01 * 500 * AttackCurve(attack)
            AttackCurve: piecewise linear table (parameter720), e.g. 2300 -> 4.1, 9300 -> 11.1
bonus     = attack-minus-defense tier sum (parameter230 rows), only when attack > target defense
pre       = base + bonus
damage    = trunc( min(999999, pre * F_param * F_action * F_status * F_equipment * F_target...) )
```

Observed values:

| Case | Attack | AttackCurve | base | bonus | F_equipment | Damage |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| baseline (no armor) | 2632 | 4.432 | 576.16 | -4 (attack below defense 2636) | 1.2483 | 714 |
| outfit + scroll | 2994 | 4.794 | 623.22 | +210.26 | 1.3369 | 1114 |

### Zones in spine order (outermost last)

1. **AttackCurve(attack)**: multiplies the motion value. The attack is the packet attack, i.e. weapon record 0
   plus the flat scroll attack.
2. **Attack-minus-defense bonus**: additive on base. It has 5 tiers; coefficients come from the parameter230 rows.
3. **F_param**: parameter230 row (1.0 here).
4. **F_action**: action descriptor +0x316 / 100.
5. **F_status**: owner aggregate pool, the native 6267D0 boost/cut pair over inline STAT +0x88C, +0x89C and
   +0x894. Class0 status buffs land here, e.g. Counterattack Deflect, 1.1. Same-key statuses compete; the
   strongest wins.
6. **F_equipment**: one additive pool, `1 + sum(terms)`. In the 1114 case:

   | Term | Value | Fields (effect names) |
   | --- | ---: | --- |
   | Melee damage group | 0.054 | D+0x108 近距离攻击伤害, D+0x110 中段架式攻击伤害 (stance), D+0x120 速攻击伤害, D+0xCC |
   | Equip-weight reflection | 0.0346 | STAT+0x7B8 weight through grade D+0x31C 伤害反映（装备重量） |
   | Attribute reflection 心 | 0.1151 | STAT+0x848 (attribute 150) through grade D+0x34 对伤害的反映（心） |
   | Attribute reflection 刚 | 0.1151 | STAT+0x84C through grade D+0x38 对伤害的反映（刚） |
   | Unhurt melee damage | 0.018 | D+0x264 近距离攻击伤害（无伤） |

   All terms add, so they dilute each other: +0.1 in any f32 term gives +83 damage here, 1114 × 0.1 / 1.337.
   Weapon attribute scaling (心/刚 grades) and equipment weight sit in the same pool as equipment damage
   percentages.
7. **F_target**: target STAT +0x1CC, +0x1DC (percent), descriptor +0x31A, `1 - STAT+0x21C * 0.001`,
   STAT +0x854 with HP_owner +0x3658, and two MAIN packet slots (+0x154, +0x24).
8. Cap at 999999, then truncation to an integer.

## Gates (what switches an effect into the formula)

These come from `gate_scan.py` on the 1114 outfit. A flip of a MAIN hit-context field switches the listed fields
on (+) or off (-):

| State | Meaning (from the switched fields) | Fields |
| --- | --- | --- |
| MAIN +0x148 (1 = mid) | stance | 0: + D+0x10C 上段架式, - D+0x110 中段架式 |
| MAIN +0x13C bit | quick-attack action flag | - D+0x120 速攻击伤害 when cleared |
| MAIN +0x14 (-1 = none) | element of the hit | 0: + D+0x284 属性攻击伤害/火属性伤害, + D+0x3C0 武器造成的火属性伤害 |
| MAIN +0x288 (1 now) | conditional-melee context: off removes the unhurt and attacking-enemy terms | 0: - D+0x264 无伤, - D+0x360 对攻击中敌人, - D+0x7C8, - D+0xCC |
| HP_owner +0x58 (current HP), +0x48 | player unhurt (HP full) | HP below full: - D+0x264 近距离攻击伤害（无伤） |
| MAIN +0x289 (1 now) | melee hit | 0: - D+0x108 近距离, - D+0x110 stance |
| MAIN +0x296 | guardian spirit skill hit | + D+0x684 守护灵技的伤害, - melee terms |
| MAIN +0x297 | non-melee hit | - melee terms |
| MAIN +0x29A | hell weapon hit | + D+0x130 地狱武器伤害 |
| MAIN +0x299 | familiar/servant strike | + D+0x20 (skill 使役强击之法) |
| MAIN +0x144 | action id class | other values: - D+0x7C8 |
| HP_owner +0x3138 | weapon imbued with an element | + D+0x6B0 属性附武伤害 |
| target descriptor +0x88 | target is a 尸狂 | + D+0x25C 对尸狂造成的伤害 |
| action row +0x93 | thrust | + D+0x100 刺击伤害 |
| MAIN +0x28B, +0x2AC | no damage | damage 0 |

The probed scan (`--probe`) set 374 zero f32 effect fields to 0.01. Three fields left the captured state and were
excluded by bisection: D+0x104 (暗杀伤害), D+0x530 and D+0x714. With the probe, these further terms enter the
equipment pool on this path: D+0x268 对无伤敌人造成的近距离伤害, D+0x360 对攻击中敌人的近距离伤害, D+0x7C4
近距离攻击伤害（地狱）, D+0x134 and D+0x7C8. All of them are plain additive terms, like the worn ones.

### Not covered yet

A single flip of a value MAIN compares on this path does not reach every conditional effect. Some effects need a
state that this path never compares, or two states at once. Known examples are back attacks, ki-exhausted or downed
targets, status-ailment targets, low HP and stance-specific finishers. Earlier work showed the ki-exhausted target
as target entity +0xAD 04 -> 06 (x1.29). These need either a capture in that situation or a static search for the
MAIN code that reads each field.

## Consequences for the optimizer

- The objective has to name the hit situation: stance, quick or strong, element, unhurt, enemy state, status
  buffs. Most conditional effects are ordinary terms of the equipment pool, switched by a MAIN hit-context
  field. A triggered evaluation sets those fields and replays.
- Diminishing returns are exact and simple inside a pool. A new term's gain is `value / pool`, and the pool
  already holds weight and attribute reflections. Terms in different zones multiply.
