# Spell damage path and progress-object sources (PC 2.0.2.0, 2026-10-27)

## Onmyo and ninjutsu hits go through MAIN too

On 10-25 the observer recorded no frame for an onmyo hit (水弹符, 1034) or a ninjutsu/illusion hit (lightning). An
opt-in recorder at the HP store site (`observer/unmatched_hp_stacks.patch`) kept each distinct caller chain of HP
stores that had no MAIN frame. On 10-27 the training samurai's HP went 75138 → 74104 (−1034, 水弹符) and
74104 → 69280 (−4824, lightning). Both chains pass `0x7872AD` (HP apply 787268, called at `0x329890`).

`0x329890` is inside the hit dispatcher 3275AC, the only caller of MAIN (78CE8C). The dispatcher loads the applied
damage from the hit context +0x218 (`0x32843F: movss xmm15, [r13+0x218]`), the field MAIN stores, for every hit.
Spells compute damage in MAIN, confirmed on 11-04. A diagnostic of dropped MAIN calls showed the two spell hits with a tag-4 source object; the entry filter required tag 0, and relaxing only the player check on 10-27 was not enough. Resolver 325F4C reads a tag-4 source's owning actor at +0x260. `observer/observer_1027.patch` now keeps tag-4 sources owned by the player, records the owner as the source actor and captures the projectile (`source.projectile`, 0x4000 bytes), because it dies with the hit. Captured spell frames on 11-04 match the game: onmyo 水弹符 1034 (packet attack 2014), lightning 4824 (2442). An earlier lightning hit showed 4677; the cause is still open.

## Progress object maps (not the skill tree)

`read_skills.py` reads the progress object `[[Nioh3.exe+0x4751860]]`. Its maps at +0x5D98 and +0x5DE8 rank the
rows of parameter tables source7c0 (6 rows) and source7c8 (32 rows); `source_talents.py` reads the rank's value
at +0x20/+0x28 + 8 × rank. Decoded on 10-27 (TALENTS.json):

- source7c0, rank 9 of 9: elixir upgrades (仙药掉落率 250, 药物恢复体力效果 150, 使用仙药时增加灵力 15, two
  elixir buffs, 幸运 15).
- source7c8, ranks 0..5, one block of four rows per mode value (region): 仙药 erosion recovery, 仙药掉落率（地狱）,
  守护灵技的侵蚀恢复量, and one damage-relevant row per block: 近距离攻击伤害（地狱） 40 (two regions at rank 5),
  灵力增加量（地狱）, 近距离攻击精力伤害（地狱）, 追击和近身刺击伤害 100 (rank 5).

These are a further damage source (region/elixir upgrades), not the learned skill tree or the title blessings.
Both still have to be located.

## Spell formula inputs (11-04)

Both spell frames replay exactly after a live fill: 水弹符 1034 (146 rounds), lightning 4824 (107 rounds). Effect fields
that enter the formula, by probing every zero f32 effect field with 0.01 (names from the game's text pool):

| Attack | Fields |
| --- | --- |
| 水弹符 (onmyo) | D+0x78C 阴阳术伤害, D+0x288 属性攻击伤害/水属性伤害, D+0x518 阴阳术道具的伤害（无伤）, D+0x77C 属性攻击伤害, D+0x31C/STAT+0x7B8 weight reflection, D+0x100 刺击伤害, D+0x268, D+0x134, D+0x7C4 |
| Lightning (ninjutsu) | D+0x790 and D+0x76C 忍术伤害, D+0x28C 属性攻击伤害/雷属性伤害, D+0xA28 幻术伤害, D+0x2B4 视敌方负面效果数影响术伤害, D+0x77C, D+0x988 {}造成的伤害, D+0x31C/STAT+0x7B8, D+0x134, D+0x7C4 |

Three fields carry melee names (D+0x134 近距离攻击伤害（背后奖励无效）, D+0x268, D+0x7C4); they are formula leaves on this
path, and whether they add damage was not separated, so they stay unverified for spells.

**Debuff count is a real input.** D+0x2B4 is 10 on this character. In the 4824 hit the target's status map holds one
node, key 0x117 盲目, applied by the soul core's "onmyo hit applies blind" when 水弹符 hit first. Replaying that hit with
the status map emptied gives 4677, the exact damage the game showed when lightning landed first (spell3 capture). So
one debuff is worth ×1.0314 on this hit, although 盲目 itself has no damage text.

## All non-equipment sources of the source compiler (11-04)

`556118` compiles the source object (STAT+0x7D8). Every effect goes through `557BA0(source, form, group, value)`, which
sums into per-form tables at source+0x35D0 (form 0) and +0x4728 (form 1): 12-byte entries (u16 group, u32 value), count at
+0xC00. Reading those tables gives the final aggregate of each group with no breakpoint.

| Block | Table / state | Meaning |
| --- | --- | --- |
| 5561xx | manager+0x7C0 ranked by progress+0x5D98 | elixir upgrades |
| 5562xx | manager+0x7C8 ranked by progress+0x5DE8, filtered by mode | region upgrades |
| 556337 | progress+0x6340, two 16-byte slots per form | 武家的庇佑 (clan blessing, set per style), confirmed by the owner's menu screenshot on 11-05: 服部, rank 宿老, 背后攻击伤害 +9.8%(+2.8%) and 忍术伤害 +11.7%(+4.2%) in both styles. Slot: u16 group, u32 value, u32 clan id (0x2F = 服部), u32 slot index. The live aggregate is 98/117, so the bracketed serving bonus is already inside the total |
| 556378 | manager+0x7E0 (stride 0x110) ranked by progress+0x5EA8 | **title blessings** (恩惠点数): key +0x80, group +0x88, values u32 +4 + 4×rank |
| 5563e9 | manager+0x278 (0xD8), +0x268 (0x38) | per-area and per-region completion: 攻击力/防御力/幸运 |
| 5565e2 | manager+0x3F8/+0x400 | presence-only unlocks |
| 5566ad | manager+0x130 with progress+0x6020 (skill node: key +0x10, rank +0x1A, enabled +0x1C), manager+0x598 | passive skills, form-gated by category 0x15/0x16/0x17 |
| 556792 | progress+0x60A0 + 8×index | one slotted skill per weapon category and form |

Weapons, armor and other items reach `557BA0` from the rebuild `818248` and `2FB4D1` through `55764C`/`5557DC`, each item
with 7 effect records at +0x3C; the off-form weapon counts only for groups that pass its gate.

Text lookup is now direct: the tree at Q(module+0x45C8450)+0x1CD0 (`dump_all_texts.py`, 30506 strings in the current locale).

**Reconciliation.** `source_ledger.py` subtracts known sources from the live aggregate. Groups fed only by blessings,
skills or slots match exactly, including F4C5 220 = blessing 100 + 灭气追击之法 120, which closes the 10-16 question about
the extra 100. The rest of the residuals come from equipment, soul core and guardian, not yet attributed per item.

Damage-relevant blessings at the captured ranks (`data/BLESSINGS.json`): 武技伤害 50, 属性攻击伤害 50, 对精力耗尽敌人的近距离
伤害 100, 追击和近身刺击伤害 100, 地狱武器伤害 36, 近距离攻击伤害（无伤）18, 伤害反映（装备重量）10.

Skill 祸实之法 (0x22DA, ninja, cost 3) supplies group 0xF1A8 value 10, which is D+0x2B4 视敌方负面效果数影响术伤害, so the
debuff-count effect above comes from this skill.

## Soul core and guardian by live differential (11-05)

Reading the aggregate before and after a swap isolates a source without breakpoints (`ledger_diff.py`).

Guardian entries are per style, five each, and enter the form's aggregate as displayed percent × 10. The two swapped-in
guardians match their menu text exactly (凤凰 in form 0: 146 阴阳术伤害, 114, 208; 猫又 in form 1: 258 追击和近身刺击伤害, 168,
107, 107, 81). The guardians removed gave form 0: 武技伤害 97, 琢技伤害倍率 108, 武技灵力增加量 208, 火属性伤害降低 46, 残心精力恢复量
186; form 1: 背后攻击伤害 134, 被攻击时承受的伤害降低 62, 9, 9, 11. The guardian's 玩家的强化值 (70/70) did not appear in this
aggregate, so that path is still open.

Removing the soul core dropped four entries: 武技精力伤害 78, 格挡成功赋予 11, 残心初始量 50, 阴阳术命中时施加 DEBUFF (0x5DC3). Form 1
changed at the first read and form 0 lost the same four entries by the second read, with the owner reporting the samurai
slot empty. This suggests one shared core and a lazily rebuilt table for the inactive form, not yet proven.
