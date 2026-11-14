import assert from "node:assert/strict";
import test from "node:test";
import {excludes, matchesWanted, seedOptions} from "../../workshop/equipment-seed-options";
import type {EquipmentRules} from "../../../packages/contracts/protected-responses";

const rules: EquipmentRules = {
  item_id: 1, known: true, graces: [90],
  random_pool: [
    {effect_id: 1, star: false, min: 10, max: 20, group: 1, masks: [1, 0]},
    {effect_id: 2, star: false, min: 10, max: 20, group: 1, masks: [1, 0]},
    {effect_id: 3, star: true, min: 10, max: 20, group: 1, masks: [1, 0]},
    {effect_id: 4, star: false, min: 20, max: 30, group: 1, masks: [1, 0]},
  ],
};
test("same-name variants are alternatives while stars and value ranges stay distinct", () => {
  const options = [...seedOptions(rules, 4, () => "effect").values()];
  assert.deepEqual(options.map(option => option.ids), [[1, 2], [3], [4], [90]]);
  assert.equal(seedOptions({...rules, set_effect: 99}, 4, String).size, 4);
  assert.equal(seedOptions({...rules, soul_core: true}, 4, String).size, 4);
  assert.equal(seedOptions(rules, 3, String).size, 4);
  assert.equal(seedOptions({...rules, known: false}, 4, String).size, 0);
});
test("draw conflicts include groups and both mask words, but fixed effects have no drawn group", () => {
  const candidate = {id: 1, group: 1, masks: [1, 4]};
  assert(excludes(candidate, {id: 2, group: 1, masks: [0, 0]}));
  assert(excludes(candidate, {id: 2, group: 2, masks: [1, 0]}));
  assert(excludes(candidate, {id: 2, group: 2, masks: [0, 4]}));
  assert(!excludes(candidate, {id: 2, group: 2, masks: [2, 8]}));
  assert(!excludes(candidate, {id: 90}));
});
test("native preview must satisfy every wanted group using exact effect IDs", () => {
  assert(matchesWanted([{effect_id: 2}, {effect_id: 90}], [[1, 2], [90]]));
  assert(!matchesWanted([{effect_id: 2}], [[1, 2], [90]]));
  assert(!matchesWanted([{effect_id: 3}], [[1, 2]]));
  assert(matchesWanted([], []));
});
