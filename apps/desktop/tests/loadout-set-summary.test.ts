import assert from "node:assert/strict";
import { test } from "node:test";
import catalog from "../../workshop/loadout-sets.json";
import effectCatalog from "../../workshop/loadout-effect-catalog.json";
import { loadoutSetSummary, nativeSetTierValue, setForEffect } from "../../workshop/loadout-set-summary";
import { loadoutEffectSummary, nativeGroupForEffect } from "../../workshop/loadout-effect-summary";
import { initialLoadoutSelection, setGuardian, setDestroyedSpikeCount, guardians } from "../../workshop/loadout-state";
import { data, type Sample } from "../../workshop/model";
import type { LoadoutPiece } from "../../workshop/loadout-code";

const piece = (item: number, marker: number, level = 180): LoadoutPiece => ({ item_id: item, level, level_before_forge: level, plus: 0, rarity: 4, effects: [{ effect_id: marker, value: 0 }] });
const moon = setForEffect(47805)!;
const taiping = setForEffect(29274)!;
const fixture = () => {
  const state = initialLoadoutSelection();
  state.equipment.samurai.melee0 = piece(424, 47805);
  state.equipment.samurai.accessory0 = piece(18823, 47805);
  state.equipment.samurai.head = piece(42340, 29274);
  state.equipment.samurai.body = piece(7557, 29274);
  state.equipment.samurai.arms = piece(18743, 29274);
  return state;
};

test("native projection covers all keyed rows, Chinese tiers and the captured growth knots", () => {
  assert.equal(Object.keys(catalog.sets).length, 91);
  assert.equal(Object.keys(catalog.effect_to_set).length, 90);
  assert.deepEqual(catalog.growth_by_level[180], [1000, 2500, 3600, 2000, 1800]);
  assert.equal(moon.key, 5859); assert.equal(moon.name_zh, "月读的恩宠");
  assert.equal(taiping.key, 52534); assert.equal(taiping.name_zh, "太平先驱");
  assert.equal(taiping.tiers.find(tier => tier.required === 5)?.group_id, 0x8508);
  assert.equal(Object.values(catalog.sets).flatMap(row => row.tiers).filter(row => !row.label_resolved).length, 0);
  assert.ok(Object.values(catalog.sets).flatMap(row => row.tiers).some(row => row.label_zh === "攻防强化（枪）"));
  assert.equal(moon.tiers[1].label_zh, "阴阳术命中时施加承受伤害增加");
});

test("Moon and Taiping native tier values include integer equipment-level growth", () => {
  assert.equal(nativeSetTierValue(moon.tiers[0], 180), 428);
  assert.equal(nativeSetTierValue(moon.tiers[0], 50), 350);
  assert.equal(nativeSetTierValue(taiping.tiers[0], 180), 65);
  assert.equal(nativeSetTierValue(taiping.tiers[1], 180), 5);
  assert.equal(nativeSetTierValue(moon.tiers[5], 180), 10);
  assert.equal(nativeSetTierValue(moon.tiers[0], NaN), null);
});

test("actual set counts and tier effects update on removal and remain style-specific", () => {
  const state = fixture();
  const rows = loadoutSetSummary(state, "samurai").sets;
  assert.equal(rows.find(row => row.key === moon.key)?.count, 2);
  assert.equal(rows.find(row => row.key === taiping.key)?.count, 3);
  assert.equal(loadoutEffectSummary(state, "samurai").effects.find(row => row.groupId === 0xB1B6)?.raw, 428);
  assert.equal(loadoutEffectSummary(state, "samurai").effects.find(row => row.groupId === 0xD201)?.raw, 65);
  assert.equal(loadoutSetSummary(state, "ninja").sets.find(row => row.key === taiping.key)?.tiers.filter(row => row.active).length, 0);
  delete state.equipment.samurai.accessory0;
  assert.ok(!loadoutEffectSummary(state, "samurai").effects.some(row => row.groupId === 0xB1B6));
  assert.equal(loadoutSetSummary(state, "samurai").sets.find(row => row.key === moon.key)?.count, 1);
});

test("requirement reduction is presence-based, zero-valued, nonstacking and does not inflate displayed count", () => {
  const state = fixture();
  // Native group 0x38BA's recorded effect identities are independent of the raw value.
  const id = Number(Object.entries(effectCatalog.effect_to_group).find(([, group]) => group === 0x38BA)![0]);
  state.equipment.samurai.melee0.effects.push({ effect_id: id, value: 0 });
  state.equipment.samurai.accessory0.effects.push({ effect_id: id, value: 100 });
  const summary = loadoutSetSummary(state, "samurai");
  const set = summary.sets.find(row => row.key === taiping.key)!;
  assert.equal(set.count, 3); assert.equal(set.reduction, 1);
  assert.ok(set.tiers.find(row => row.required === 4)?.active);
  assert.ok(!set.tiers.find(row => row.required === 5)?.active);
  state.equipment.samurai.head.effects.push({ effect_id: id, value: 0 });
  assert.equal(loadoutSetSummary(state, "samurai").sets.find(row => row.key === taiping.key)?.reduction, 1);
});

test("fixed sets deduplicate identical members and retain their higher equipment level", () => {
  const state = fixture();
  state.equipment.samurai.melee0 = piece(taiping.members[0], 29274, 70);
  const set = loadoutSetSummary(state, "samurai").sets.find(row => row.key === taiping.key)!;
  assert.equal(set.count, 3); assert.equal(set.pieces.find(row => row.itemId === taiping.members[0])?.level, 180);
  assert.equal(set.tiers[0].raw, 65);
});

test("tier level averages use highest contributing levels and ignore forge plus values", () => {
  const state = fixture();
  state.equipment.samurai.melee0.level = 50;
  state.equipment.samurai.melee0.plus = 999;
  state.equipment.samurai.accessory0.level = 100;
  const tier = loadoutSetSummary(state, "samurai").sets.find(row => row.key === moon.key)!.tiers[0];
  assert.equal(tier.averageLevel, 75); assert.equal(tier.raw, 365);
  const locked = loadoutSetSummary(state, "samurai").sets.find(row => row.key === moon.key)!.tiers[5];
  assert.equal(locked.active, false); assert.equal(locked.averageLevel, 75);
  assert.ok(!loadoutEffectSummary(state, "samurai").effects.some(row => row.groupId === locked.group_id));
  state.equipment.samurai.melee0.level = 400;
  assert.equal(loadoutSetSummary(state, "samurai").sets.find(row => row.key === moon.key)!.tiers[0].averageLevel, 140);
});

test("missing or conflicting marker records do not fabricate membership or bonuses", () => {
  const state = fixture();
  state.equipment.samurai.head.effects = [];
  state.equipment.samurai.body.effects.push({ effect_id: 47805, value: 0 });
  state.equipment.samurai.arms = piece(18743, 47805);
  const summary = loadoutSetSummary(state, "samurai");
  assert.equal(summary.pending.length, 3);
  assert.ok(!summary.sets.some(row => row.key === taiping.key));
  assert.equal(summary.sets.find(row => row.key === moon.key)?.count, 2);
  assert.equal(loadoutEffectSummary(state, "samurai").pending.length, 3);
});

test("scroll grace contributes a piece; missing legacy level excludes only dependent numeric tiers", () => {
  const state = initialLoadoutSelection();
  state.equipment.samurai.melee0 = piece(424, 47805);
  state.scrolls.samurai = { ...data.samples[0], level: 180, effects: [{ id: "47805", name: moon.name_zh!, raw: 0, roll: 0, role: "恩宠" }] } as Sample;
  assert.equal(loadoutSetSummary(state, "samurai").sets[0].count, 2);
  assert.equal(loadoutEffectSummary(state, "samurai").effects[0].raw, 428);
  delete state.scrolls.samurai.level;
  assert.equal(loadoutSetSummary(state, "samurai").sets[0].count, 2);
  assert.ok(loadoutEffectSummary(state, "samurai").pending.some(row => row.reason.includes("绘卷等级")));
  state.equipment.samurai.accessory0 = piece(18823, 47805);
  assert.ok(loadoutEffectSummary(state, "samurai").effects.some(row => row.groupId === moon.tiers[1].group_id));
});

test("set and ordinary equipment contributions merge with separately traceable sources", () => {
  const state = fixture();
  // 42813 is the ordinary life effect in the same group as Moon's two-piece bonus.
  assert.equal(nativeGroupForEffect(42813), 0xB1B6);
  state.equipment.samurai.head.effects.push({ effect_id: 42813, value: 100 });
  const total = loadoutEffectSummary(state, "samurai").effects.find(row => row.groupId === 0xB1B6)!;
  assert.equal(total.raw, 528); assert.equal(total.sources.length, 2);
  assert.ok(total.sources.some(row => row.source.includes("月读的恩宠")));
});

test("Double Guardian activates both configured guardians only at the actual five-piece tier", () => {
  let state = setDestroyedSpikeCount(setGuardian(setGuardian(fixture(), "samurai", { guardianId: 292, level: 40 }), "ninja", { guardianId: 197, level: 10 }), 13);
  assert.ok(loadoutEffectSummary(state, "samurai").effects.every(row => row.sources.every(source => !source.source.startsWith("双灵庇佑"))));
  state.equipment.samurai.legs = piece(taiping.members[3], 29274);
  state.equipment.samurai.feet = piece(taiping.members[4], 29274);
  const result = loadoutEffectSummary(state, "samurai");
  assert.ok(result.effects.some(row => row.groupId === 0x8508 && row.raw === 0));
  assert.equal(result.effects.flatMap(row => row.sources).filter(row => row.source.startsWith("双灵庇佑")).length, guardians.find(row => row.guardian_id === 197)!.slots.length);
  delete state.equipment.samurai.feet;
  assert.ok(!loadoutEffectSummary(state, "samurai").effects.some(row => row.groupId === 0x8508));
});

test("the six-piece cross-style tier applies without leaking ordinary other-style effects", () => {
  const state = initialLoadoutSelection();
  taiping.members.forEach((id, index) => { state.equipment.samurai[["head", "body", "arms", "legs", "feet", "accessory0"][index]] = piece(id, 29274); });
  const result = loadoutEffectSummary(state, "ninja");
  assert.equal(result.effects.length, 1); assert.equal(result.effects[0].groupId, 0xCB4B); assert.equal(result.effects[0].raw, 120);
  assert.ok(!result.effects.some(row => row.groupId === 0x8508));
});

test("overfull mixed-level grace values and invalid levels fail closed without mutating state", () => {
  const state = initialLoadoutSelection();
  ["melee0", "melee1", "ranged", "head", "body", "arms", "legs", "feet"].forEach((slot, index) => { state.equipment.samurai[slot] = piece(424, 47805, 50+index); });
  const before = JSON.stringify(state);
  const result = loadoutSetSummary(state, "samurai");
  assert.equal(result.sets[0].count, 8); assert.equal(result.sets[0].tiers[0].raw, null);
  assert.equal(result.sets[0].tiers[2].raw, 70);
  assert.equal(JSON.stringify(state), before);
  state.equipment.samurai.melee0.level = 0;
  assert.ok(loadoutSetSummary(state, "samurai").pending.some(row => row.reason.includes("装备等级")));
});
