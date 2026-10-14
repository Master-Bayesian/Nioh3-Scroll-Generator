import assert from "node:assert/strict";
import { test } from "node:test";
import { initialLoadoutSelection } from "../../workshop/loadout-state";
import { armorRemodelModes, canRemodelArmor, plannedEquipmentPiece, setArmorRemodel } from "../../workshop/loadout-remodel";
import { setLoadoutEquipment, sameLoadoutPiece } from "../../workshop/loadout-equipment";
import { armorRequirements } from "../../workshop/loadout-requirements";
import { loadoutStats } from "../../workshop/loadout-stats";
import { loadoutWarnings } from "../../workshop/loadout-warnings";
import { parseLoadoutWorkspace, snapshotLoadoutWorkspace } from "../../workshop/loadout-storage";
import type { LoadoutPiece } from "../../workshop/loadout-code";

const source: LoadoutPiece = { item_id: 49550, level: 180, level_before_forge: 180, plus: 20, rarity: 4,
  effects: [], armor_remodel: [0, 0], native_flags: 0, remodel_stage: 3, origin: "save",
  source: { saveId: "fixture", digest: "retained", slotIndex: 11, inventoryKey: 42 },
  audit: { natural: true, findings: [] } };
const attrs = [..."体心刚武技智咒"].map(name => ({ name, value: 10 }));
function configured() {
  return setLoadoutEquipment(initialLoadoutSelection(), "ninja", "ninja", "body", source);
}

test("all six planner modes propagate captured requirements, weight and warnings without mutating source evidence", () => {
  const original = configured(), retained = structuredClone(original);
  const expected = [[12, 6, 5], [12, 13, 12], [16, 6, 5], [12, 21, 20], [21, 6, 5], [16, 13, 12]];
  armorRemodelModes.forEach((mode, index) => {
    const selection = setArmorRemodel(original, "ninja", "body", mode.key);
    const effective = plannedEquipmentPiece(selection, "ninja", "body");
    assert.deepEqual(effective.armor_remodel, mode.bytes);
    assert.equal(loadoutStats(selection, "ninja", attrs).weight?.minimum, expected[index][0]);
    assert.deepEqual(armorRequirements(effective, selection.remodelStage)!.values.map(row => row.minimum), expected[index].slice(1));
    assert.equal(loadoutWarnings(selection, "ninja", attrs).some(row => row.code === "armor_requirements"), [1, 3, 5].includes(index));
    assert.deepEqual(selection.equipment.ninja.body, retained.equipment.ninja.body);
    assert(sameLoadoutPiece(effective, source));
  });
  assert.deepEqual(original, retained);
});

test("reset restores the source mode, including missing or unrecognized source metadata", () => {
  for (const mode of [undefined, [3, 0], [1, 2]] as (LoadoutPiece["armor_remodel"])[]) {
    const selection = configured(); selection.equipment.ninja.body.armor_remodel = mode;
    const planned = setArmorRemodel(selection, "ninja", "body", "0/0");
    assert.deepEqual(plannedEquipmentPiece(planned, "ninja", "body").armor_remodel, [0, 0]);
    const reset = setArmorRemodel(planned, "ninja", "body", null);
    assert.strictEqual(plannedEquipmentPiece(reset, "ninja", "body"), selection.equipment.ninja.body);
  }
});

test("replacement and clearing reset only the affected slot; invalid choices and non-armor slots cannot simulate", () => {
  let selection = setArmorRemodel(configured(), "ninja", "body", "2/2");
  selection.armorRemodels!.samurai.head = "1/0";
  assert.strictEqual(setArmorRemodel(selection, "ninja", "body", "3/0"), selection);
  assert.strictEqual(setArmorRemodel(selection, "ninja", "melee0", "1/0"), selection);
  assert.strictEqual(setArmorRemodel(selection, "samurai", "body", "1/0"), selection);
  for (const piece of [source, null]) {
    const replaced = setLoadoutEquipment(selection, "ninja", "ninja", "body", piece);
    assert.equal(replaced.armorRemodels!.ninja.body, undefined);
    assert.equal(replaced.armorRemodels!.samurai.head, "1/0");
  }
});

test("style and named-draft simulations persist independently and remain backward compatible", () => {
  const selection = setArmorRemodel(configured(), "ninja", "body", "1/2");
  selection.equipment.samurai.head = { ...source, item_id: 18054 };
  const planned = setArmorRemodel(selection, "samurai", "head", "1/1");
  const draft = { id: 1, name: "Planned", style: "ninja" as const, attributes: attrs, selection: planned };
  const other = { ...draft, id: 2, name: "Original", selection: configured() };
  const workspace = parseLoadoutWorkspace(JSON.stringify(snapshotLoadoutWorkspace([draft, other], 1, draft)));
  assert.equal(workspace.drafts[0].selection.armorRemodels!.ninja.body, "1/2");
  assert.equal(workspace.drafts[0].selection.armorRemodels!.samurai.head, "1/1");
  assert.equal(workspace.drafts[1].selection.armorRemodels, undefined);
  assert.deepEqual(workspace.drafts[0].selection.equipment.ninja.body, source);
  for (const overrides of [{ samurai: {}, ninja: { body: "9/9" } }, { samurai: {}, ninja: { melee0: "1/0" } }]) {
    const invalid = structuredClone(workspace); (invalid.drafts[0].selection as any).armorRemodels = overrides;
    assert.throws(() => parseLoadoutWorkspace(JSON.stringify(invalid)));
  }
});

test("manual mode resolves only remodel metadata; absent stage still yields bounds", () => {
  const selection = configured(); delete selection.remodelStage; delete selection.equipment.ninja.body.remodel_stage;
  const planned = setArmorRemodel(selection, "ninja", "body", "2/2");
  const weight = loadoutStats(planned, "ninja", attrs).weight!;
  assert(weight.maximum > weight.minimum);
  assert.equal(planned.remodelStage, undefined);
});

test("zero-plus armor cannot simulate any remodel, including stale saved overrides", () => {
  const original = configured();
  original.equipment.ninja.body.plus = 0;
  assert.equal(canRemodelArmor(original, "ninja", "body"), false);
  for (const mode of armorRemodelModes) {
    assert.strictEqual(setArmorRemodel(original, "ninja", "body", mode.key), original);
    const stale = { ...original, armorRemodels: { samurai: {}, ninja: { body: mode.key } } };
    assert.strictEqual(plannedEquipmentPiece(stale, "ninja", "body"), original.equipment.ninja.body);
  }
});

test("toughness remains independent of every selectable remodel mode", () => {
  for (const plus of [1, 20]) {
    const original = configured();
    original.equipment.ninja.body.plus = plus;
    const baseline = loadoutStats(original, "ninja", attrs);
    for (const mode of armorRemodelModes) {
      const selection = setArmorRemodel(original, "ninja", "body", mode.key);
      assert.deepEqual(loadoutStats(selection, "ninja", attrs).toughness, baseline.toughness);
      assert.equal(selection.armorRemodels!.ninja.body, mode.key);
    }
  }
});
