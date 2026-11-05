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
