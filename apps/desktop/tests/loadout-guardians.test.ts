import assert from "node:assert/strict";
import { test } from "node:test";
import { guardians, guardiansForStyle, guardianEffects, essenceLimit, initialLoadoutSelection, setGuardian, setDestroyedSpikeCount } from "../../workshop/loadout-state";

test("reviewed guardian catalog preserves native style and same-name form identities", () => {
  assert.equal(guardians.length, 22);
  assert.equal(guardiansForStyle("samurai").length, 10);
  assert.equal(guardiansForStyle("ninja").length, 12);
  assert.equal(new Set(guardians.map(row => row.guardian_id)).size, 22);
  assert.equal(guardians.reduce((count, row) => count + row.slots.length, 0), 110);
  assert.equal(guardiansForStyle("samurai").filter(row => row.name_zh === "草薙").length, 2);
});
test("guardian configurations stay independent and reject incompatible or unknown IDs", () => {
  const empty = initialLoadoutSelection();
  let state = setGuardian(empty, "samurai", { guardianId: 292, level: 40 });
  state = setGuardian(state, "ninja", { guardianId: 197, level: 10 });
  assert.deepEqual(state.guardians.samurai, { guardianId: 292, level: 40 });
  assert.deepEqual(state.guardians.ninja, { guardianId: 197, level: 10 });
  assert.equal(setGuardian(state, "samurai", { guardianId: 197, level: 1 }), state);
  assert.equal(setGuardian(state, "ninja", { guardianId: -1, level: 1 }), state);
  assert.equal(setGuardian(state, "samurai", { guardianId: 292, level: 101 }), state);
  for (const level of [0, -1, 1.5, 51, 100, NaN, Infinity]) assert.equal(setGuardian(state, "samurai", { guardianId: 292, level }), state);
  assert.equal(setGuardian(state, "samurai", { guardianId: 292, level: 50 }).guardians.samurai?.level, 50);
  const cleared = setGuardian(state, "samurai", null);
  assert.equal(cleared.guardians.samurai, null);
  assert.deepEqual(cleared.guardians.ninja, state.guardians.ninja);
  assert.equal(empty.guardians.samurai, null);
});
test("native display arrays retain percentages, grades and hidden values across levels", () => {
  let state = setGuardian(initialLoadoutSelection(), "samurai", { guardianId: 292, level: 1 });
  assert.equal(guardianEffects(state, "samurai")[2].display, "+13");
  state = setGuardian(state, "samurai", { guardianId: 292, level: 40 });
  assert.equal(guardianEffects(state, "samurai")[2].display, "+39");
  assert.equal(guardianEffects(state, "samurai")[3].display, "4%");
  assert.equal(guardianEffects(state, "samurai")[0].display, "");
  state = setGuardian(state, "ninja", { guardianId: 115, level: 10 });
  assert.equal(guardianEffects(state, "ninja")[0].display, "A-");
  const malformed = { ...state, guardians: { ...state.guardians, ninja: { guardianId: 115, level: 101 } } };
  assert.deepEqual(guardianEffects(malformed, "ninja"), []);
});
test("destroyed spike thresholds are distinct from level and fail closed when unknown", () => {
  const state = setGuardian(initialLoadoutSelection(), "samurai", { guardianId: 292, level: 50 });
  assert.deepEqual(guardianEffects(state, "samurai").map(row => row.activation), ["active", "active", "active", "unknown", "unknown"]);
  const low = setDestroyedSpikeCount(state, 11);
  assert.equal(guardianEffects(low, "samurai")[3].activation, "locked");
  const exact = setDestroyedSpikeCount(low, 12);
  assert.equal(guardianEffects(exact, "samurai")[3].activation, "active");
  assert.equal(guardianEffects(exact, "samurai")[4].activation, "locked");
  const next = setDestroyedSpikeCount(exact, 13);
  assert.equal(guardianEffects(next, "samurai")[4].activation, "active");
  assert.equal(setDestroyedSpikeCount(next, null).destroyedSpikeCount, null);
  for (const count of [-1, 65536, 1.5, NaN, Infinity]) assert.equal(setDestroyedSpikeCount(next, count), next);
  assert.equal(setDestroyedSpikeCount(next, 0).destroyedSpikeCount, 0);
});
test("guardian effects only expose the active style and cannot increase essence limits", () => {
  let state = setGuardian(initialLoadoutSelection(), "samurai", { guardianId: 292, level: 40 });
  state = setGuardian(state, "ninja", { guardianId: 197, level: 40 });
  assert.ok(guardianEffects(state, "samurai").every(row => row.guardian_id === 292));
  assert.ok(guardianEffects(state, "ninja").every(row => row.guardian_id === 197));
  assert.deepEqual(state.equipment, initialLoadoutSelection().equipment);
  for (const category of ["common", "samurai", "ninja"] as const) assert.equal(essenceLimit(state, "samurai", category), 1);
});
