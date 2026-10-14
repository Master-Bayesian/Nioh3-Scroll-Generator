import assert from "node:assert/strict";
import { test } from "node:test";
import catalog from "../../workshop/loadout-effect-catalog.json";
import { clans, guardians, initialLoadoutSelection, setGuardian, setDestroyedSpikeCount, skills } from "../../workshop/loadout-state";
import { equipmentEffectDetail, isSpecialEffect, formatNativeEffectValue, loadoutEffectSummary, mergeNativeEffectValues, nativeEffectGroup, nativeGroupForEffect } from "../../workshop/loadout-effect-summary";
import type { LoadoutPiece } from "../../workshop/loadout-code";
import { data, type Sample } from "../../workshop/model";

const piece = (id: number, value: number): LoadoutPiece => ({ item_id: 424, level: 180, level_before_forge: 180, plus: 20, rarity: 4, effects: [{ effect_id: id, value }] });
const idsByGroup = new Map<number, number[]>();
for (const [id, group] of Object.entries(catalog.effect_to_group)) idsByGroup.set(group, [...(idsByGroup.get(group) ?? []), Number(id)]);
const luckIds = idsByGroup.get(15118)!;

test("complete native text projection resolves every shipped equipment effect and unit", () => {
  for (const id of Object.keys(catalog.effect_to_group).map(Number)) {
    const detail = equipmentEffectDetail(id, 10);
    assert.notEqual(detail.display, null, String(id));
    assert.ok(!detail.label.includes("{}"), `${id}: ${detail.label}`);
  }
  assert.equal(formatNativeEffectValue(nativeEffectGroup(22065)!, 42), "+42");
  assert.equal(formatNativeEffectValue(nativeEffectGroup(7480)!, 42), "+4.2");
  assert.equal(formatNativeEffectValue(nativeEffectGroup(46078)!, 42), "+4.2%");
  assert.equal(formatNativeEffectValue(nativeEffectGroup(0x54fe)!, 0), "+0.0");
  assert.equal(formatNativeEffectValue(nativeEffectGroup(0x54fe)!, 42), "+42.0");
});
test("unresolved scroll templates use the verified native subject rather than a generic replacement", () => {
  const id = idsByGroup.get(57709)![0], state = initialLoadoutSelection();
  state.scrolls.samurai = { ...data.samples[0], effects: [{ id: String(id), raw: 42, roll: 0, role: "副词条", name: "奥义两立（{}）" }] };
  const result = loadoutEffectSummary(state, "samurai");
  assert.equal(result.pending.length, 0); assert.equal(result.effects[0].label, "奥义两立（刀）");
  assert.equal(result.effects[0].display, "");
});

test("empty configurations have no sample effects or pending entries", () => {
  for (const style of ["samurai", "ninja"] as const) assert.deepEqual(loadoutEffectSummary(initialLoadoutSelection(), style), { effects: [], pending: [] });
});
test("formatter matches every native guardian and clan value across all captured indices", () => {
  let count = 0;
  for (const guardian of guardians) for (const effect of guardian.slots) {
    const group = nativeEffectGroup(effect.effect_group_id)!;
    effect.raw_values_by_level_index.forEach((raw, index) => {
      assert.equal(formatNativeEffectValue(group, raw), effect.display_values_by_level_index[index], `${guardian.name_zh}/${effect.label_zh}/${index}`); count++;
    });
  }
  for (const clan of clans) for (const effect of clan.slots) for (const relation of ["current_clan", "other_clan"] as const) {
    const group = nativeEffectGroup(effect.effect_group_id)!;
    effect.raw_values_by_rank_index[relation].forEach((raw, index) => {
      assert.equal(formatNativeEffectValue(group, raw), effect.display_values_by_rank_index[relation][index], `${clan.name_zh}/${effect.label_zh}/${relation}/${index}`); count++;
    });
  }
  assert.equal(count, 13110);
});
test("native formatting preserves percent precision, signed values, grades and hidden presence", () => {
  const groups = Object.values(catalog.groups);
  const tenths = groups.find(group => group.format_supported && !group.grade && group.decimal_mode === 1 && group.unit_template === "+{}<dot>{}%")!;
  assert.equal(formatNativeEffectValue(tenths, 123), "+12.3%");
  assert.equal(formatNativeEffectValue(tenths, 0), "+0.0%");
  assert.equal(formatNativeEffectValue(tenths, 0xffffffff), null);
  assert.equal(formatNativeEffectValue(nativeEffectGroup(15118)!, 0xffffffff), "-1");
  const grade = groups.find(group => group.grade)!;
  assert.equal(formatNativeEffectValue(grade, 0), "D-");
  assert.equal(formatNativeEffectValue(grade, 10), "A");
  assert.equal(formatNativeEffectValue(grade, 0xffffffff), "AAA+");
  const hidden = groups.find(group => group.format_supported && !group.grade && group.unit_template === "")!;
  assert.equal(formatNativeEffectValue(hidden, 123), "");
  assert.equal(formatNativeEffectValue(nativeEffectGroup(0x54fe)!, 42), "+42.0");
  for (const raw of [NaN, Infinity, 0.5, -0x80000001, 0x100000000]) assert.equal(formatNativeEffectValue(tenths, raw), null);
});
test("group accumulator retains native integer, cap, grade duplicate and max rules", () => {
  const luck = nativeEffectGroup(15118)!;
  assert.equal(mergeNativeEffectValues(luck, [10, 20]).raw, 30);
  assert.equal(mergeNativeEffectValues({ ...luck, raw_cap_i32: 25 }, [10, 20]).raw, 25);
  assert.equal(mergeNativeEffectValues({ ...luck, raw_cap_i32: 25 }, [30]).raw, 30);
  assert.equal(mergeNativeEffectValues({ ...luck, raw_cap_i32: -1 }, [0xffffffff, 2]).raw, 1);
  const grade = Object.values(catalog.groups).find(group => group.grade)!;
  assert.equal(mergeNativeEffectValues(grade, [8, 10, 4]).raw, 12);
  assert.equal(mergeNativeEffectValues(nativeEffectGroup(0x38ba)!, [20, 10, 30]).raw, 30);
});
test("equipment variants merge by native group and retain each source", () => {
  const state = initialLoadoutSelection();
  assert.ok(luckIds.length > 1);
  state.equipment.samurai.melee0 = piece(luckIds[0], 10);
  state.equipment.samurai.head = piece(luckIds[1], 20);
  state.equipment.shared.soul0 = piece(luckIds[0], 5);
  state.equipment.ninja.head = piece(luckIds[0], 100);
  const total = loadoutEffectSummary(state, "samurai").effects.find(row => row.groupId === 15118)!;
  assert.equal(total.display, "+35"); assert.equal(total.sources.length, 3);
  assert.equal(loadoutEffectSummary(state, "ninja").effects.find(row => row.groupId === 15118)?.display, "+105");
  delete state.equipment.samurai.head;
  assert.equal(loadoutEffectSummary(state, "samurai").effects.find(row => row.groupId === 15118)?.display, "+15");
});
test("scroll uses final effect identity and raw value; grace and sentinel slots are excluded", () => {
  const state = initialLoadoutSelection();
  state.scrolls.samurai = { ...data.samples[0], effects: [
    { id: String(luckIds[0]), name: "幸运", raw: 4, roll: 0, role: "副词条" },
    { id: String(luckIds[0]), name: "套装标记", raw: 100, roll: 0, role: "恩宠" },
    { id: "4294967295", name: "空槽", raw: 100, roll: 0, role: "副词条" },
  ] } as Sample;
  assert.equal(loadoutEffectSummary(state, "samurai").effects[0].display, "+4");
  assert.equal(loadoutEffectSummary(state, "ninja").effects.length, 0);
  state.scrolls.samurai = null; assert.equal(loadoutEffectSummary(state, "samurai").effects.length, 0);
});
test("guardian level and spike gate determine included contributions; other style is isolated", () => {
  let state = setGuardian(initialLoadoutSelection(), "samurai", { guardianId: 292, level: 40 });
  state = setGuardian(state, "ninja", { guardianId: 197, level: 10 });
  assert.equal(loadoutEffectSummary(state, "samurai").effects.length, 3);
  assert.equal(loadoutEffectSummary(state, "samurai").pending.length, 2);
  state = setDestroyedSpikeCount(state, 13);
  assert.equal(loadoutEffectSummary(state, "samurai").effects.length, 5);
  const group = guardians.find(row => row.guardian_id === 292)!.slots[2].effect_group_id;
  assert.equal(loadoutEffectSummary(state, "samurai").effects.find(row => row.groupId === group)?.display, "+39");
  state = setGuardian(state, "samurai", { guardianId: 292, level: 1 });
  assert.equal(loadoutEffectSummary(state, "samurai").effects.find(row => row.groupId === group)?.display, "+13");
});
test("clan rank and joined-clan multiplier select actual values and cross-source merging", () => {
  const clan = clans.find(row => row.slots.some(slot => slot.effect_group_id === 15118))!;
  const slot = clan.slots.find(slot => slot.effect_group_id === 15118)!;
  const state = initialLoadoutSelection();
  state.clans.samurai[0] = { clanId: clan.clan_id, slotIndex: slot.slot_index, rankIndex: 9 };
  state.equipment.samurai.head = piece(luckIds[0], 10);
  let effect = loadoutEffectSummary(state, "samurai").effects.find(row => row.groupId === 15118)!;
  assert.equal(effect.raw, slot.raw_values_by_rank_index.other_clan[9] + 10);
  state.joinedClan = clan.clan_id;
  effect = loadoutEffectSummary(state, "samurai").effects.find(row => row.groupId === 15118)!;
  assert.equal(effect.raw, slot.raw_values_by_rank_index.current_clan[9] + 10);
  state.clans.samurai[0] = null;
  assert.equal(loadoutEffectSummary(state, "samurai").effects[0].display, "+10");
});
test("unknown values remain pending while skills are excluded from both summary lists", () => {
  const state = initialLoadoutSelection();
  state.equipment.samurai.head = piece(0x12345678, 10);
  state.equipment.samurai.head.hell = true;
  state.equipment.samurai.head.hell_skill = 0xa148;
  state.equipment.samurai.arms = piece(luckIds[0], NaN);
  state.equipment.samurai.body = piece(luckIds[0], 0.5);
  state.skills.common = [skills.find(skill => skill.category === "common")!.id];
  state.skills.ninja = [skills.find(skill => skill.category === "ninja")!.id];
  const result = loadoutEffectSummary(state, "samurai");
  assert.equal(result.effects.length, 0); assert.equal(result.pending.length, 3);
  assert.ok(result.pending.every(row => !row.source.includes("技能")));
});

test("mechanisms are separated regardless of number formatting; ordinary values and grades remain ordinary", () => {
  for (const id of [1, 9192, 11014, 27311, 44634, 46611, 56002, 57328, 20781, 60020, 63934]) {
    const state = initialLoadoutSelection();
    state.equipment.samurai.head = piece(id, 0);
    const effect = loadoutEffectSummary(state, "samurai").effects[0];
    assert.ok(effect, String(id)); assert.ok(isSpecialEffect(effect), String(id));
  }
  assert.ok(isSpecialEffect({ groupId: 14522, kind: "value" }));
  assert.ok(isSpecialEffect({ groupId: 34056, kind: "value" }));
  assert.equal(isSpecialEffect({ groupId: 15118, kind: "value" }), false);
  const grade = Object.values(catalog.groups).find(group => group.grade)!;
  assert.equal(isSpecialEffect({ groupId: grade.id, kind: "value" }), false);
});

test("equipment details preserve native values, empty units, and unknown values", () => {
  assert.equal(equipmentEffectDetail(luckIds[0], 10).display, "+10");
  assert.equal(equipmentEffectDetail(1, 0).display, "");
  assert.equal(equipmentEffectDetail(0x12345678, 0).display, null);
  assert.equal(equipmentEffectDetail(21460, 0).display, "−1");
});

test("presence-based requirement reduction is visible and never sums duplicate copies", () => {
  const state = initialLoadoutSelection();
  state.equipment.samurai.head = piece(21460, 0);
  state.equipment.samurai.body = piece(39430, 10);
  const total = loadoutEffectSummary(state, "samurai").effects.find(effect => effect.groupId === 14522)!;
  assert.equal(total.display, "−1");
  assert.equal(total.sources.length, 2);
  assert.equal(isSpecialEffect(total), true);
  assert.equal(loadoutEffectSummary(state, "samurai").pending.length, 0);
});
test("essence presence is actual equipment data and deduplicates without invented strength", () => {
  const state = initialLoadoutSelection();
  state.equipment.samurai.head = piece(0xf9be, 0);
  state.equipment.shared.soul0 = piece(0xf9be, 0);
  const effect = loadoutEffectSummary(state, "samurai").effects.find(row => row.groupId === nativeGroupForEffect(0xf9be))!;
  assert.equal(effect.kind, "presence"); assert.equal(effect.sources.length, 2); assert.equal(effect.display, "");
});
test("matching labels do not merge different native groups and set markers grant no bonuses", () => {
  const state = initialLoadoutSelection();
  state.scrolls.samurai = { ...data.samples[0], effects: [
    { id: "1405", name: "相同显示名", raw: 10, roll: 0, role: "副词条" },
    { id: "475", name: "相同显示名", raw: 100, roll: 0, role: "副词条" },
  ] } as Sample;
  state.equipment.samurai.head = piece(47805, 100);
  const result = loadoutEffectSummary(state, "samurai");
  assert.equal(result.effects.length, 2);
  assert.notEqual(result.effects[0].groupId, result.effects[1].groupId);
  assert.ok(result.effects.every(row => row.sources.length === 1));
  assert.equal(result.pending.length, 0);
});
test("guardian and equipment values share the same group while selection remains immutable", () => {
  const state = setGuardian(initialLoadoutSelection(), "samurai", { guardianId: 292, level: 40 });
  const slot = guardians.find(row => row.guardian_id === 292)!.slots[2];
  const id = idsByGroup.get(slot.effect_group_id)![0];
  state.equipment.samurai.head = piece(id, 10);
  const snapshot = JSON.stringify(state);
  const total = loadoutEffectSummary(state, "samurai").effects.find(row => row.groupId === slot.effect_group_id)!;
  assert.equal(total.display, "+49");
  assert.equal(total.sources.length, 2);
  assert.equal(JSON.stringify(state), snapshot);
});
