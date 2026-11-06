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
slot empty. Confirmed by restoration: with the core re-equipped, form 1 recovered at once and form 0 only after switching to samurai, ending identical to the original snapshot in all 153 rows. So one core serves both styles, and the table of the inactive style refreshes only when that style becomes active. Read an aggregate only after visiting the style.

Original guardians: 草薙 (samurai), 虎御前 (ninja). Most entries display value / 10 as percent; 火属性伤害降低 displays the raw 46, and 造成水属性伤害时增加灵力 value 9 displays grade A-.

## Composed aggregate: zero residual (11-05, offline)

The aggregate can now be built from its sources instead of carried forward from a capture (`evaluator/src/composed_model.py`,
`build_source_layers.py`, `data/SOURCE_LAYERS.json`, `check_composed.py`):

aggregate(form) = equipment (native weapon, body, slot 15 and set stages on an emptied source) + title blessings + skills +
clan blessing + guardian of that style + soul core + elixir upgrades + area completion + unnamed progress.

Checked on the 11-04 captures, with no game running:

| Check | Result |
| --- | --- |
| Composed vs captured aggregate | 87 groups form 0, 66 groups form 1, 0 wrong |
| D rebuilt from the aggregate alone (819400 over the default page) | 60 effect words on the ninjutsu frame, 78 on the onmyo frame; each has one wrong word, 0x3F0 and 0x880, which look like a constant and an uninitialised word; variants keep the captured value of both, so their effect is untested |
| Skill 祸实之法 removed | 4677, equal to the damage the game showed with no debuff on the target |

The replay itself starts from the captured state: a variant writes only the words that differ from the composed baseline. The 祸实之法 row is the only one with an independent game value. The other source removals below are predictions.

| Removed from the lightning hit (4824) | Predicted |
| --- | ---: |
| Title blessings | 4625 |
| All skills | 4309 |
| Clan blessing | 4394 |
| Soul core, guardian, elixir upgrades, unnamed progress | 4824 (none enters this hit; the core still matters through the blind it applies) |
| 忍术伤害 +10% (test value) | 5192 |

For the onmyo hit (1034), only title blessings changed the result (955) among the sources removed.

Corrections to earlier notes:

- **A zero entry is not a removed source.** Group 0xF1A8 with value 0 still materializes (4787). Removing a source must remove the entry; `add()` does so for negative deltas.
- **The debuff count is carried by D+0x2B4 alone.** Zeroing any other effect word keeps the 147 gap (`debuff_scan.py`).
- **The "half" field difference was a test artifact.** Accumulating STAT fields were materialized on a captured frame that already held them, doubling them. Attributes do not change D at all for a fixed aggregate.

Open in the composition: 0x52D 36 and 0xF919 35 are equal in both forms but have no text or located source. Area completion is a stored total
(攻击力 88, 防御力 88, 幸运 178), not recomputed from the tables. Soul core effects are measured for one core only.

## Attack, attributes and the cross-state test (11-05, offline)

**Same layer across outfits.** `layer_stability.py` subtracts the modelled equipment from the aggregate of every saved capture that has an entry snapshot (21 distinct gear and aggregate states, with armor, three weapons, sets and scrolls). The remainder is the same layer on each day: identical to the 11-04 layer on all states from the 10-22 imbue session on, and one other constant layer before it (16 groups differ). Group 0x6A7C 攻击力 is 113 in the old layer and 120 in the new one, and that is STAT +0x12C.

**Attack record.** record = weapon stages (weapon, coreX scaling, scroll) + guardian attack + soul core attack.

- Guardian attack and defense are the menu's 玩家的强化值, from guardian table `numeric(key, level)` (374F58 adds them to all six records and +0x1C8). All four guardians match at level 50: 草薙 71/66, 虎御前 68/73, 凤凰 70/70, 猫又 69/69, and all their five effect entries. `data/GUARDIANS.json` has every definition at level 50.
- Soul core attack and defense are added by 554268 for passive item slots 0 and 1 (attribute source +0x80, 0xE8 each). `soul_core_numeric.py` derives them from the passive table (manager+0x118, 84 rows, dumped live, hash equal to the 10-16 receipt): 0x7BD9 165/156 in both slots gives 330/312, 0xBB71 164/156 in slot 0, 0x3336 116/111 in slot 1. All match the game; live records rose 116 and defense 111 when 0x3336 was equipped on 11-06.
- The record +8 field is the 攻击力 group + 35 (samurai records 0 and 1, three weapons; 36 on the ninja frame, source not located).

**Correction.** The earlier fit of a base attack from attribute 2 was a coincidence and is withdrawn. The −183 between the two attribute sets is −17 coreX and −166 soul core.

**What attributes reach.** Only lifted, unfitted paths: coreX scaling, seeds (+0x1A8, +0x1AC, +0x1BC to +0x1C4, +0x7A4, +0x7A8, +0x7B4), the load factor +0x7A0 through capacity, HP seed, and equipment requirement gates. All seeds and the load factor match on every capture.

**Cross-state prediction** (`cross_state.py`). From the 10-16 baseline, with no STAT or D from the target capture, only its inventory and attributes plus layers read on 11-04:

| Target | Delta model before | Composed | Game |
| --- | ---: | ---: | ---: |
| 10-23 armored hit (h1) | 1188 | 884 | 884 |
| 10-23 no armor (base, h2) | n/a | 793 | 793 |

One measured constant still enters: the +35 field; plus one zero-value entry 0x3E52 of the 10-23 state. STAT words that still differ are records 1, 2, 3 and 5 and +0x798/+0x79C, which this hit does not depend on (writing the target's whole STAT and D gave the same damage).

**Weight is read from a second object.** The formula reads +0x7B8 from `current_player.external_STAT`, not the source STAT. Before writing it, the prediction was 859 and 792.

**Withdrawn predictions.** The 11-05 attribute predictions (892, 884, 873) and the core prediction (989) are withdrawn. The attribute ones never wrote the formula's attribute words; the core one assumed attack 164 and no effects. See below.

**Second soul core and attribute reflection (11-06, live).** Soul core 0x3336 in slot 1 raised the attack records by 116 and defense by 111, equal to the table. Its item record lists four effects, 0x8C1 19, 0xECC1 15, 0x5E5D 11, 0x83FD 5, and the live aggregate grew by exactly those; no other group changed. Live D differs from the composed D on no effect word.

The frozen prediction was 994 and the game showed 1038. The cause: the formula reads attributes inline at STAT+0x844 (seven i32) through reflection grades in D, for example D+0x40 from group 0x83FD 对伤害的反映（咒）. `cross_state.py` wrote STAT only below +0x7C0, so the replay kept the 10-16 attributes (attribute 7 at 50 instead of 150). Earlier hits had no grade on an attribute that differed, so 884 and 793 did not show it. With the attributes written, the same inputs give 1038. This is an explanation found after seeing the number, not a blind result. The pieces that give 884 to 1038: attack and defense 884 to 962, grade 0x83FD 962 to 1038.

**Frozen predictions** for the outfit with both soul cores and the same hit (1038), in `data/CROSS_STATE_20261106.json`. Blind results are marked; the rest are untested. Attributes were written to live memory for these tests and the game recomputed after an equipment change; its level word stayed at 167, HP followed it, and this hit did not depend on either:

| Change | Predicted |
| --- | ---: |
| Attributes (37,150,27,36,80,15,140) | 1043, blind pass 11-06 |
| Attributes (37,150,37,26,80,15,140) | 1036 |
| Attributes (37,140,27,26,80,15,160) | 1031, blind pass 11-06 (also loses a 150-gated entry, group 0xECC5) |
| Attributes (37,150,27,26,90,15,140) | 1036 |
| Attributes (37,150,27,26,70,15,160) | 1041 |
| Guardian 凤凰 instead of 草薙 | 1038 |

**Low-attribute state (11-06, live).** A respec mistake left (5,5,5,36,6,5,140), far from any captured set, and the game showed 849. The model, with no change, gives 849; all 73 aggregate groups, record 0 (2618) and the load factor equal the live values. The state was read and the damage known before the run, so this is not blind, but nothing was tuned for it. Requirement gates and attribute-dependent effects therefore hold over a wide range.

Still open: spell packet attack against attributes (seen at one attribute set only), soul core effects taken from item records inside the optimizer, and the +35 and ninja +36 fields.
