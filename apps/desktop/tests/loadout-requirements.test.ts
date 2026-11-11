import assert from "node:assert/strict";
import { test } from "node:test";
import { armorRequirements, attributeNames, profoundRequirement } from "../../workshop/loadout-requirements";
import { nativeGroupForEffect } from "../../workshop/loadout-effect-summary";
import { loadoutWarnings } from "../../workshop/loadout-warnings";
import { initialLoadoutSelection } from "../../workshop/loadout-state";
import { savedLoadoutPiece } from "../../workshop/loadout-equipment";
import { decodeLoadout, encodeLoadout, type LoadoutPiece } from "../../workshop/loadout-code";
import type { CharacterEquipment } from "../../../packages/contracts/protected-responses";

const piece = (override: Partial<LoadoutPiece> = {}): LoadoutPiece => ({ item_id: 0xC18E,
  level: 180, level_before_forge: 180, plus: 20, rarity: 4, effects: [], armor_remodel: [0, 0], ...override });
const attrs = (value = 50) => attributeNames.map(() => ({ value }));

test("reviewed original armor row supplies requirements; thickening never increases them", () => {
  for (const mode of [[0, 0], [2, 0], [2, 2]] as [number, number][]) {
    const result = armorRequirements(piece({ armor_remodel: mode }))!;
    assert.equal(result.known, true);
    assert.deepEqual(result.values.map(row => [row.attribute, row.minimum, row.maximum]), [["体", 6, 6], ["技", 5, 5]]);
    assert.equal(result.reason, "");
  }
  assert.equal(armorRequirements(piece({ item_id: 424 })), null);
});

test("strengthening adds the captured coefficient only to nonzero requirements", () => {
  const single = armorRequirements(piece({ plus: 5, armor_remodel: [1, 0] }))!;
  const double = armorRequirements(piece({ plus: 5, armor_remodel: [1, 1] }))!;
  assert.deepEqual(single.values.map(row => row.minimum), [8, 7]);
  assert.deepEqual(double.values.map(row => row.minimum), [11, 10]);
  assert.equal(double.values.length, 2);
  assert.equal(double.reason, "");
  assert.deepEqual(armorRequirements(piece({ rarity: 2, armor_remodel: [1, 1] }))!.values.map(row => row.minimum), [6, 5]);
  assert.deepEqual(armorRequirements(piece({ rarity: 5 }))!.values.map(row => row.minimum), [3, 3]);
});

test("unknown getter stage or code remodel produces bounds rather than guessed requirements", () => {
  const staged = armorRequirements(piece({ armor_remodel: [1, 1] }))!;
  assert.deepEqual(staged.values.map(row => [row.minimum, row.maximum]), [[11, 21], [10, 20]]);
  const unknown = armorRequirements(piece({ armor_remodel: undefined }))!;
  assert.deepEqual(unknown.values.map(row => [row.minimum, row.maximum]), [[6, 21], [5, 20]]);
  assert.notEqual(unknown.reason, "");
  assert.equal(armorRequirements(piece({ armor_remodel: [3, 0] }))!.known, false);
  assert.equal(armorRequirements(piece({ rarity: 6 }))!.known, false);
});

test("the serialized stage snapshot makes captured stage-3 strengthening exact", () => {
  const single = armorRequirements(piece({ remodel_stage: 3, armor_remodel: [1, 0] }))!;
  assert.deepEqual(single.values.map(row => [row.minimum, row.maximum]), [[13, 13], [12, 12]]);
  assert.equal(single.reason, "");
  const double = armorRequirements(piece({ remodel_stage: 3, armor_remodel: [1, 1] }))!;
  assert.deepEqual(double.values.map(row => [row.minimum, row.maximum]), [[21, 21], [20, 20]]);
  const early = armorRequirements(piece({ remodel_stage: 1, armor_remodel: [1, 1] }))!;
  assert.deepEqual(early.values.map(row => [row.minimum, row.maximum]), [[11, 11], [10, 10]]);
  assert.equal(armorRequirements(piece({ remodel_stage: 6 }))!.known, false);
});

test("save selection uses its paired current-difficulty field and never persists it in legacy codes", () => {
  const record: CharacterEquipment = { ...piece({ armor_remodel: [1, 2] }), slot_index: 0,
    appearance_id: 0, quantity: 1, familiarity: 0, inventory_key: 4, seed: 1, effects: [] };
  const character = { save_id: "fixture", source_sha256: "a".repeat(64), generation: { difficulty: 3, difficulties: [] as [] } };
  const selected = savedLoadoutPiece(record, character);
  character.generation.difficulty = 5;
  assert.equal(selected.remodel_stage, 3);
  assert.deepEqual(armorRequirements(selected)!.values.map(row => [row.minimum, row.maximum]), [[13, 13], [12, 12]]);
  assert.equal(decodeLoadout(encodeLoadout([selected]))[0].remodel_stage, undefined);
  for (const stage of [0, 6, 1.5, NaN]) assert.equal(savedLoadoutPiece(record, { ...character, generation: { difficulty: stage, difficulties: [] } }).remodel_stage, undefined);
  assert.equal(savedLoadoutPiece(record, { save_id: "fixture", source_sha256: "a".repeat(64) }).remodel_stage, undefined);
});

test("armor warnings distinguish unmet, uncertain and certainly satisfied requirements", () => {
  const selection = initialLoadoutSelection();
  selection.equipment.samurai.body = piece();
  assert.ok(loadoutWarnings(selection, "samurai", attrs(5)).some(row => row.code === "armor_requirements" && row.message.includes("体 ≥ 6")));
  assert.ok(!loadoutWarnings(selection, "samurai", attrs(6)).some(row => row.code.startsWith("armor_requirements")));
  selection.equipment.samurai.body = piece({ armor_remodel: undefined });
  assert.ok(loadoutWarnings(selection, "samurai", attrs(10)).some(row => row.code === "armor_requirements_pending"));
  assert.ok(!loadoutWarnings(selection, "samurai", attrs(21)).some(row => row.code.startsWith("armor_requirements")));
  assert.ok(!loadoutWarnings(selection, "ninja", attrs(5)).some(row => row.code.startsWith("armor_requirements")));
});

test("all seven profound effects require exactly their corresponding attribute at 150", () => {
  const ids = [0xDAC2, 0xB613, 0x23E8, 0xDFF0, 0xAE5A, 0x6AAF, 0x2B06];
  ids.forEach((id, index) => {
    const values = attrs(150); values[index].value = 149;
    const group = nativeGroupForEffect(id)!;
    assert.equal(profoundRequirement(group, values)?.attribute, attributeNames[index]);
    assert.equal(profoundRequirement(group, values)?.active, false);
    const selection = initialLoadoutSelection();
    selection.equipment.samurai.melee0 = piece({ item_id: 424, effects: [{ effect_id: id, value: 0 }] });
    assert.ok(loadoutWarnings(selection, "samurai", values).some(row => row.code === "profound_requirement" && row.message.includes("149")));
    values[index].value = 150;
    assert.equal(profoundRequirement(group, values)?.active, true);
    assert.ok(!loadoutWarnings(selection, "samurai", values).some(row => row.code === "profound_requirement"));
    assert.ok(!loadoutWarnings(selection, "ninja", attrs(5)).some(row => row.code === "profound_requirement"));
  });
  assert.equal(profoundRequirement(-1, attrs()), null);
});

test("save remodel bytes survive selection independently and remain absent from legacy codes", () => {
  const record: CharacterEquipment = { ...piece(), armor_remodel: [1, 2], native_flags: 0x40000, slot_index: 0,
    appearance_id: 0, quantity: 1, familiarity: 0, inventory_key: 4, seed: 1, effects: [] };
  const selected = savedLoadoutPiece(record, { save_id: "fixture", source_sha256: "a".repeat(64) });
  record.armor_remodel![0] = 0;
  assert.deepEqual(selected.armor_remodel, [1, 2]);
  assert.equal(selected.native_flags, 0x40000);
  assert.equal(decodeLoadout(encodeLoadout([selected]))[0].native_flags, undefined);
  assert.equal(decodeLoadout(encodeLoadout([selected]))[0].armor_remodel, undefined);
});
