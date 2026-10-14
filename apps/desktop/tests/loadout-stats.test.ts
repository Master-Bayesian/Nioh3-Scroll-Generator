import assert from "node:assert/strict";
import { test } from "node:test";
import { armorStats, loadoutStats } from "../../workshop/loadout-stats";
import { clans, initialLoadoutSelection } from "../../workshop/loadout-state";
import itemNames from "../../workshop/item-names.json";
import { fitsSlot, setLoadoutEquipment } from "../../workshop/loadout-equipment";
import type { LoadoutPiece } from "../../workshop/loadout-code";

const piece = (override: Partial<LoadoutPiece> = {}): LoadoutPiece => ({ item_id: 49550,
  level: 180, level_before_forge: 180, plus: 20, rarity: 4, effects: [], armor_remodel: [0, 0], native_flags: 0, ...override });
const attributes = (stamina = 40) => [40, 40, stamina, 40, 40, 40, 40].map(value => ({ value }));

test("capacity lookup reproduces the eight observed UI vectors and reacts only to stamina/style", () => {
  const selection = initialLoadoutSelection();
  for (const [stamina, samurai, ninja] of [[5, 295, 141], [40, 484, 228], [80, 537, 247], [99, 558, 252]]) {
    assert.equal(loadoutStats(selection, "samurai", attributes(stamina)).capacity?.minimum, samurai);
    assert.equal(loadoutStats(selection, "ninja", attributes(stamina)).capacity?.minimum, ninja);
  }
  assert.equal(loadoutStats(selection, "samurai", attributes()).agility, "A");
  assert.equal(loadoutStats(selection, "samurai", attributes()).toughnessRank, "C");
  assert.deepEqual(loadoutStats(selection, "samurai", attributes()).weight, { minimum: 0, maximum: 0 });
});

test("native alternate row and saved flags select exact ordinary armor returns", () => {
  assert.deepEqual(armorStats(piece())!.weight, { minimum: 12, maximum: 12 });
  assert.deepEqual(armorStats(piece())!.toughness, { minimum: 36, maximum: 36 });
  assert.deepEqual(armorStats(piece({ rarity: 2 }))!.toughness, { minimum: 18, maximum: 18 });
  const thick = armorStats(piece({ item_id: 24732, armor_remodel: [2, 0] }))!;
  assert(thick.weight.minimum <= 31 && thick.weight.maximum >= 31); // Captured stage-3 native return.
  assert.equal(armorStats(piece({ armor_remodel: [2, 2], plus: 0 }))!.weight.minimum, 12);
  assert.equal(armorStats(piece({ armor_remodel: [3, 0] })), null);
  assert.equal(armorStats(piece({ native_flags: -1 })), null);
});

test("missing metadata is bounded; no ordinary flags or remodel default is invented", () => {
  const unknown = armorStats(piece({ native_flags: undefined, armor_remodel: undefined }))!;
  assert(unknown.weight.maximum > unknown.weight.minimum);
  assert.equal(unknown.toughness.maximum, 36);
  assert.equal(unknown.toughness.minimum, 8);
  assert.equal(armorStats(piece({ native_flags: 0x240000 }))!.toughness.minimum, 8);
});

test("confirmed stage removes thickening bounds and preserves the captured weights", () => {
  assert.deepEqual(armorStats(piece({ remodel_stage: 3, armor_remodel: [2, 0] }))!.weight, { minimum: 16, maximum: 16 });
  assert.deepEqual(armorStats(piece({ remodel_stage: 3, armor_remodel: [2, 2] }))!.weight, { minimum: 21, maximum: 21 });
  assert.deepEqual(armorStats(piece({ item_id: 24732, remodel_stage: 3, armor_remodel: [2, 0] }))!.weight, { minimum: 31, maximum: 31 });
  const selection = initialLoadoutSelection();
  selection.equipment.ninja.body = piece({ remodel_stage: 3, armor_remodel: [2, 2] });
  assert.ok(!loadoutStats(selection, "ninja", attributes()).reasons.some(reason => reason.includes("阶段")));
  assert.equal(armorStats(piece({ remodel_stage: 0 })), null);
});

test("one character stage governs both styles and takes precedence over older equipment snapshots", () => {
  let selection = initialLoadoutSelection();
  selection.equipment.ninja.body = piece({ armor_remodel: [2, 2], remodel_stage: 3 });
  selection = setLoadoutEquipment(selection, "samurai", "samurai", "head", piece({ item_id: 18054, remodel_stage: 1 }));
  assert.equal(selection.remodelStage, 1);
  assert.deepEqual(loadoutStats(selection, "ninja", attributes()).weight, armorStats(selection.equipment.ninja.body, 1)!.weight);
  assert.notDeepEqual(loadoutStats(selection, "ninja", attributes()).weight, armorStats(selection.equipment.ninja.body, 3)!.weight);
  selection = setLoadoutEquipment(selection, "samurai", "samurai", "melee0", piece({ item_id: 424, remodel_stage: undefined }));
  assert.equal(selection.remodelStage, 1);
});

test("armor, flat toughness effects, style, clearing and profound gates feed live ranks", () => {
  const selection = initialLoadoutSelection();
  selection.equipment.ninja.body = piece();
  selection.equipment.ninja.melee0 = piece({ item_id: 457, effects: [{ effect_id: 0xEA53, value: 230 }] });
  const value = loadoutStats(selection, "ninja", attributes());
  assert.deepEqual(value.weight, { minimum: 12, maximum: 12 });
  assert.deepEqual(value.toughness, { minimum: 266, maximum: 266 });
  assert.equal(value.toughnessRank, "A");
  assert.equal(loadoutStats(selection, "samurai", attributes()).toughness?.minimum, 0);
  selection.equipment.ninja.melee0.effects = [{ effect_id: 0xEA53, value: 330 }, { effect_id: 0x23E8, value: 1 }];
  assert.equal(loadoutStats(selection, "ninja", attributes(149)).toughnessRank, "A");
  assert.equal(loadoutStats(selection, "ninja", attributes(150)).toughnessRank, "AA");
  assert.equal(loadoutStats(selection, "ninja", attributes(150)).agility, "AA");
  delete selection.equipment.ninja.body;
  assert.equal(loadoutStats(selection, "ninja", attributes()).weight?.minimum, 0);
});

test("accessory native weight is counted and weapon weight is not invented", () => {
  const selection = initialLoadoutSelection();
  const accessory = Number(Object.entries(itemNames.items).find(([id, row]) => row[1] === "饰品" &&
    fitsSlot(piece({ item_id: Number(id) }), "samurai", "accessory0"))![0]);
  selection.equipment.samurai.accessory0 = piece({ item_id: accessory });
  selection.equipment.samurai.melee0 = piece({ item_id: 424 });
  assert.deepEqual(loadoutStats(selection, "samurai", attributes()).weight, { minimum: 1, maximum: 1 });
  assert.deepEqual(loadoutStats(selection, "samurai", attributes()).toughness, { minimum: 0, maximum: 0 });
});

test("Red Demon agility promotion requires both its clan effect and five matching pieces", () => {
  const selection = initialLoadoutSelection();
  const armor = { head: 18054, body: 48648, arms: 14835, legs: 9847, feet: 63580 };
  for (const [slot, item_id] of Object.entries(armor)) selection.equipment.samurai[slot] = piece({ item_id });
  assert.equal(loadoutStats(selection, "samurai", attributes()).redPromotion, false);
  const clan = clans.find(row => row.slots.some(slot => slot.effect_group_id === 0xE63F))!;
  selection.clans.samurai[0] = { clanId: clan.clan_id, rankIndex: 0, slotIndex: 1 };
  assert.equal(loadoutStats(selection, "samurai", attributes()).redPromotion, true);
  delete selection.equipment.samurai.feet;
  assert.equal(loadoutStats(selection, "samurai", attributes()).redPromotion, false);
});
