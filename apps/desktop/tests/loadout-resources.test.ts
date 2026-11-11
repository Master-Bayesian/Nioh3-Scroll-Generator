import assert from "node:assert/strict";
import { test } from "node:test";
import { loadoutResources, playerResourceBase, lifeReflectionBonus, amritaKiBonus, resourceGroups } from "../../workshop/loadout-resources";
import { initialLoadoutSelection, clans, guardians, setGuardian } from "../../workshop/loadout-state";
import type { LoadoutPiece } from "../../workshop/loadout-code";
import { data } from "../../workshop/model";
import { nativeGroupForEffect } from "../../workshop/loadout-effect-summary";
import { armorRequirements } from "../../workshop/loadout-requirements";
import { parseLoadoutWorkspace, snapshotLoadoutWorkspace, type LoadoutDraft } from "../../workshop/loadout-storage";
import { applySavedLoadoutImport } from "../../workshop/loadout-save-config";

const attributes = (values = [5, 5, 5, 6, 5, 5, 5]) => values.map(value => ({ value }));
const piece = (effects: [number, number][], item_id = 424): LoadoutPiece => ({ item_id, level: 180,
  level_before_forge: 180, plus: 0, rarity: 4, armor_remodel: [0, 0], native_flags: 0,
  effects: effects.map(([effect_id, value]) => ({ effect_id, value })) });
const hpId = 18845, kiId = 2610, gaugeId = 5593;

test("base resources reproduce the corrected native column anchors and input bounds", () => {
  assert.deepEqual(playerResourceBase(1, 5, 5, 5), { hp: 880, ki: 960, staminaHP: 30 });
  assert.deepEqual(playerResourceBase(364, 22, 150, 9), { hp: 2944, ki: 1370, staminaHP: 80 });
  for (const [heart, ki] of [[40, 1205], [80, 1306], [99, 1350], [150, 1370]])
    assert.equal(playerResourceBase(245, 40, heart, 40)!.ki, ki);
  for (const input of [[0, 5, 5, 5], [451, 5, 5, 5], [1, 4, 5, 5], [1, 5, 151, 5], [1, 5, 5, 5.1]])
    assert.equal(playerResourceBase(...input as [number, number, number, number]), null);
  const state = initialLoadoutSelection();
  assert.deepEqual(loadoutResources(state, "samurai", attributes()).hp, { minimum: 880, maximum: 880 });
  assert.equal(loadoutResources(state, "samurai", attributes()).ki?.minimum, 960);
  assert.equal(loadoutResources(state, "samurai", attributes([5, 5, 5, 5, 5, 5, 5])).hp, null);
});

test("Life Reflection uses only Stamina and native float32 truncation; the Todo B+ control is 1015", () => {
  assert.equal(lifeReflectionBonus(5, 8), 135);
  assert.equal(lifeReflectionBonus(5, 7), 128);
  assert.equal(lifeReflectionBonus(5, 5), 110);
  assert.equal(lifeReflectionBonus(40, 8), 823);
  assert.equal(lifeReflectionBonus(150, 8), 1813);
  const state = initialLoadoutSelection();
  const clan = clans.find(row => row.clan_id === 26)!;
  const effect = clan.slots.find(row => row.effect_group_id === resourceGroups.stamina)!;
  state.joinedClan = clan.clan_id;
  state.clans.samurai[0] = { clanId: clan.clan_id, slotIndex: effect.slot_index,
    rankIndex: effect.raw_values_by_rank_index.current_clan.indexOf(8) };
  assert.ok(state.clans.samurai[0].rankIndex! >= 0);
  assert.equal(loadoutResources(state, "samurai", attributes()).hp?.minimum, 1015);
  assert.equal(loadoutResources(state, "samurai", attributes()).ki?.minimum, 960);
  assert.equal(loadoutResources(state, "ninja", attributes()).hp?.minimum, 880);
});

test("active equipment, scrolls and the two soul slots contribute once; other styles/extras do not", () => {
  const state = initialLoadoutSelection();
  state.equipment.samurai.melee0 = piece([[hpId, 80], [kiId, 10]]);
  state.equipment.ninja.melee0 = piece([[hpId, 1000]]);
  state.equipment.shared.soul0 = piece([[hpId, 15]], 2669);
  state.scrolls.samurai = { ...data.samples[0], effects: [{ id: String(hpId), name: "体力", raw: 20, roll: 0, role: "副词条" }] };
  state.savedExtras = { equipment: { samurai: [{ position: 2, piece: piece([[hpId, 10000]]) }], ninja: [] },
    scrolls: { samurai: [], ninja: [] }, soulCores: [{ position: 2, piece: piece([[kiId, 500]]) }] };
  assert.equal(loadoutResources(state, "samurai", attributes()).hp?.minimum, 995);
  assert.equal(loadoutResources(state, "samurai", attributes()).ki?.minimum, 970);
  assert.equal(loadoutResources(state, "ninja", attributes()).hp?.minimum, 1895);
  delete state.equipment.samurai.melee0;
  assert.equal(loadoutResources(state, "samurai", attributes()).hp?.minimum, 915);
});

test("selected guardian style, level and active set tiers feed the same resource inputs", () => {
  let state = initialLoadoutSelection();
  for (const [style, id, level] of [["samurai", 3, 40], ["ninja", 240, 15]] as const) {
    state = setGuardian(state, style, { guardianId: id, level });
    const slot = guardians.find(row => row.guardian_id === id)!.slots.find(row => row.effect_group_id === resourceGroups.life)!;
    assert.equal(loadoutResources(state, style, attributes()).hp?.minimum, 880 + slot.raw_values_by_level_index[level]);
  }
  state = initialLoadoutSelection();
  state.equipment.samurai.melee0 = piece([[47805, 0]]);
  state.equipment.samurai.accessory0 = piece([[47805, 0]], 18823);
  assert.equal(loadoutResources(state, "samurai", attributes()).hp?.minimum, 1308);
  delete state.equipment.samurai.accessory0;
  assert.equal(loadoutResources(state, "samurai", attributes()).hp?.minimum, 880);
});

test("Amrita-gauge reflection is additive, conditional, grade-sensitive and bounded when unspecified", () => {
  assert.equal(nativeGroupForEffect(gaugeId), resourceGroups.gauge);
  for (const [percent, expected] of [[0, 0], [25, 12], [50, 24], [75, 36], [100, 48]]) assert.equal(amritaKiBonus(8, percent), expected);
  assert.equal(amritaKiBonus(10, 50), 25);
  assert.equal(amritaKiBonus(17, 100), 60);
  const state = initialLoadoutSelection();
  state.equipment.samurai.melee0 = piece([[gaugeId, 8], [kiId, 20]]);
  assert.deepEqual(loadoutResources(state, "samurai", attributes()).ki, { minimum: 980, maximum: 1028 });
  state.amritaGaugePercent = 50;
  assert.deepEqual(loadoutResources(state, "samurai", attributes()).ki, { minimum: 1004, maximum: 1004 });
  state.equipment.shared.soul0 = piece([[gaugeId, 8]], 2669);
  state.amritaGaugePercent = 100;
  assert.equal(loadoutResources(state, "samurai", attributes()).ki?.minimum, 1030); // Duplicate B+ -> A-.
  state.equipment.samurai.melee0 = piece([[gaugeId, 0xffffffff]]);
  delete state.equipment.shared.soul0;
  assert.equal(loadoutResources(state, "samurai", attributes()).ki?.minimum, 960);
  assert.equal(loadoutResources(state, "samurai", attributes()).usesAmritaGauge, false);
});

test("unmet armor requirements exclude its HP/Ki effects without changing equipment or other sources", () => {
  const state = initialLoadoutSelection();
  state.remodelStage = 3;
  state.equipment.samurai.head = piece([[hpId, 100], [kiId, 10]], 18054);
  const req = armorRequirements(state.equipment.samurai.head, 3)!;
  assert.ok(req.known && req.values.some(row => row.minimum > attributes()[row.index].value));
  const before = structuredClone(state);
  assert.equal(loadoutResources(state, "samurai", attributes()).hp?.minimum, 880);
  assert.equal(loadoutResources(state, "samurai", attributes()).ki?.minimum, 960);
  const high = attributes([40, 40, 40, 40, 40, 40, 40]);
  assert.equal(loadoutResources(state, "samurai", high).hp?.minimum, playerResourceBase(245, 40, 40, 40)!.hp + 100);
  assert.deepEqual(state, before);
});

test("unknown resource conditions remain bounded, and known inactive gates add nothing", () => {
  const state = initialLoadoutSelection();
  const pending = { source: "守护灵", label: "体力", raw: 100, groupId: resourceGroups.life, reason: "地狱之楔破坏数待确认（需要 1）" };
  assert.deepEqual(loadoutResources(state, "samurai", attributes(), { effects: [], pending: [pending] }).hp, { minimum: 880, maximum: 980 });
  assert.equal(loadoutResources(state, "samurai", attributes(), { effects: [], pending: [{ ...pending, reason: "未满足地狱之楔条件（需要 1）" }] }).hp?.maximum, 880);
  const unresolved = loadoutResources(state, "samurai", attributes(), { effects: [], pending: [{ ...pending, raw: undefined, groupId: resourceGroups.ki }] });
  assert.equal(unresolved.ki, null);
  assert.equal(unresolved.hp?.minimum, 880);
});

test("manual gauge simulation survives persistence and import without becoming a save field", () => {
  const state = initialLoadoutSelection(); state.amritaGaugePercent = 33.3;
  const draft: LoadoutDraft = { id: 1, name: "Resource", style: "samurai", attributes: [..."体心刚武技智咒"].map((name, i) =>
    ({ name, value: i === 3 ? 6 : 5 })), selection: state };
  const snapshot = snapshotLoadoutWorkspace([draft], 1, draft);
  assert.equal(parseLoadoutWorkspace(JSON.stringify(snapshot)).drafts[0].selection.amritaGaugePercent, 33.3);
  assert.equal(applySavedLoadoutImport(state, { attributes: [5, 5, 5, 6, 5, 5, 5], selection: initialLoadoutSelection() }).amritaGaugePercent, 33.3);
  for (const percent of [-1, 101]) {
    snapshot.drafts[0].selection.amritaGaugePercent = percent;
    assert.throws(() => parseLoadoutWorkspace(JSON.stringify(snapshot)));
  }
});
