import assert from "node:assert/strict";
import { test } from "node:test";
import { initialLoadoutSelection } from "../../workshop/loadout-state";
import { data } from "../../workshop/model";
import { LOADOUT_STORAGE_KEY, parseLoadoutWorkspace, readLoadoutWorkspace, saveLoadoutWorkspace, snapshotLoadoutWorkspace, type LoadoutDraft } from "../../workshop/loadout-storage";

function storage() {
  const values = new Map<string, string>();
  return { values, getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => { values.set(key, value); } };
}
function draft(id = 1): LoadoutDraft {
  return { id, name: `Build ${id}`, style: "samurai", attributes: [..."体心刚武技智咒"].map((name, index) => ({ name, value: index === 3 ? 6 : 5 })), selection: initialLoadoutSelection() };
}
test("all planner metadata round-trips and the latest active snapshot overrides its stale draft", () => {
  const saved = draft(), current = draft(), next = draft(2), store = storage();
  current.style = "ninja"; current.attributes[2].value = 150;
  current.selection.remodelStage = 3;
  current.selection.equipment.ninja.head = { item_id: 18054, level: 180, level_before_forge: 179, plus: 20, rarity: 4,
    effects: [{ effect_id: 0xEA53, value: 100, star: true }], familiarity: 99, armor_remodel: [2, 1], remodel_stage: 3, native_flags: 0x80002000, origin: "save",
    source: { saveId: "retained", digest: "sha", slotIndex: 6, inventoryKey: 42 }, audit: { natural: false, verdict: "unverified", findings: [], unverified: [{ code: "unknown_flag" }] } };
  current.selection.scrolls.samurai = { ...structuredClone(data.samples[0]), backend: { candidateId: "session-capability", installable: true } };
  current.selection.guardians.samurai = { guardianId: 292, level: 50 }; current.selection.guardians.ninja = { guardianId: 197, level: 1 };
  current.selection.clans.samurai = [{ clanId: 1, slotIndex: 0, rankIndex: null, savedRaw: 10 }, null];
  current.selection.skills.common = [42]; current.selection.skills.ninja = [123]; current.selection.capacities.ninja = 30;
  current.selection.savedExtras = { equipment: { samurai: [{ position: 2, piece: structuredClone(current.selection.equipment.ninja.head) }], ninja: [] }, scrolls: { samurai: [structuredClone(current.selection.scrolls.samurai)], ninja: [] }, soulCores: [], otherSkills: [{ id: 99, stored_rank: 4, selected_rank: 2 }] };
  const workspace = snapshotLoadoutWorkspace([saved, next], 1, current);
  assert.ok(current.selection.scrolls.samurai.backend); assert.equal(workspace.drafts[0].selection.scrolls.samurai?.backend, undefined);
  assert.equal(workspace.drafts[0].selection.savedExtras!.scrolls.samurai[0].backend, undefined);
  assert.ok(current.selection.savedExtras.scrolls.samurai[0].backend);
  saveLoadoutWorkspace(store, workspace);
  assert.deepEqual(readLoadoutWorkspace(store), workspace);
  assert.equal(workspace.drafts[0].style, "ninja"); assert.equal(workspace.drafts[0].attributes[2].value, 150);
  assert.equal(saved.attributes[2].value, 5); assert.deepEqual(workspace.drafts[1], next);
});
test("active identity and renamed drafts survive repeated save/load cycles", () => {
  const store = storage(), one = draft(), two = draft(2); two.name = "My ninja"; two.style = "ninja";
  saveLoadoutWorkspace(store, snapshotLoadoutWorkspace([one, two], 2, two));
  const restored = readLoadoutWorkspace(store)!;
  restored.drafts[1].name = "Renamed";
  saveLoadoutWorkspace(store, restored);
  assert.equal(readLoadoutWorkspace(store)?.activeId, 2); assert.equal(readLoadoutWorkspace(store)?.drafts[1].name, "Renamed");
});
test("corrupt and newer-version snapshots are rejected without overwriting their original bytes", () => {
  const store = storage();
  assert.equal(readLoadoutWorkspace(store), null);
  for (const raw of ["{broken", JSON.stringify({ version: 2, drafts: [] })]) {
    store.setItem(LOADOUT_STORAGE_KEY, raw); assert.throws(() => readLoadoutWorkspace(store));
    assert.equal(store.getItem(LOADOUT_STORAGE_KEY), raw);
  }
});
test("explicit recovery preserves byte-exact original data before replacing the active snapshot", () => {
  const store = storage(), original = "{retained malformed data"; store.setItem(LOADOUT_STORAGE_KEY, original);
  saveLoadoutWorkspace(store, snapshotLoadoutWorkspace([draft()], 1, draft()), true);
  assert.ok([...store.values].some(([key, value]) => key !== LOADOUT_STORAGE_KEY && value === original));
  assert.equal(readLoadoutWorkspace(store)?.activeId, 1);
});
test("quota or failed backup aborts recovery before replacing the original", () => {
  const store = storage(); store.setItem(LOADOUT_STORAGE_KEY, "original");
  const fail = { getItem: store.getItem, setItem: () => { throw new Error("Quota exceeded"); } };
  assert.throws(() => saveLoadoutWorkspace(fail, snapshotLoadoutWorkspace([draft()], 1, draft()), true), /Quota/);
  assert.equal(store.getItem(LOADOUT_STORAGE_KEY), "original");
  assert.throws(() => saveLoadoutWorkspace(fail, snapshotLoadoutWorkspace([draft()], 1, draft())), /Quota/);
});
test("invalid nested records, duplicate identities, invalid totals and unsupported guardian levels fail at the storage boundary", () => {
  const workspace = snapshotLoadoutWorkspace([draft()], 1, draft());
  for (const mutate of [
    (w: typeof workspace) => { w.activeId = 2; },
    (w: typeof workspace) => { w.drafts.push(structuredClone(w.drafts[0])); },
    (w: typeof workspace) => { w.drafts[0].attributes.forEach(row => { row.value = 150; }); },
    (w: typeof workspace) => { w.drafts[0].selection.guardians.samurai = { guardianId: 292, level: 51 }; },
    (w: typeof workspace) => { (w.drafts[0].selection as any).skills.common = null; },
    (w: typeof workspace) => { (w.drafts[0].selection as any).equipment.samurai.head = { item_id: 1 }; },
    (w: typeof workspace) => { (w.drafts[0].selection as any).equipment.samurai.head = { item_id: 1, level: 180, level_before_forge: 180, plus: 0, rarity: 4, effects: [], remodel_stage: 6 }; },
    (w: typeof workspace) => { (w.drafts[0].selection as any).scrolls.ninja = { seed: "1", effects: null }; },
  ]) { const invalid = structuredClone(workspace); mutate(invalid); assert.throws(() => parseLoadoutWorkspace(JSON.stringify(invalid))); }
});
