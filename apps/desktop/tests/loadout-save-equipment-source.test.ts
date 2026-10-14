import assert from "node:assert/strict";
import { test } from "node:test";
import type { CharacterEquipment, SaveCharacter, SaveReference } from "../../../packages/contracts/protected-responses";
import {
  LoadoutSaveEquipmentSource,
  type LoadoutSaveEquipmentGateway,
} from "../../workshop/loadout-save-equipment-source";

function reference(saveId: string, saveSlot = 0): SaveReference {
  return { save_id: saveId, path: `saves/${saveId}.bin`, account_id: `account-${saveId}`, save_slot: saveSlot };
}

function equipment(slotIndex: number, itemId = 100): CharacterEquipment {
  return {
    slot_index: slotIndex,
    item_id: itemId,
    appearance_id: itemId,
    quantity: 1,
    level: 180,
    level_before_forge: 180,
    plus: 0,
    familiarity: 0,
    inventory_key: slotIndex,
    seed: slotIndex,
    rarity: 5,
    effects: [],
  };
}

function character(saveId: string, rows: CharacterEquipment[] = [equipment(0)]): SaveCharacter {
  return {
    save_id: saveId,
    source_sha256: `${saveId}-source`,
    currencies: { amrita: null, gold: null },
    equipment_slots: 32,
    equipment: rows,
  };
}

function gateway(overrides: Partial<LoadoutSaveEquipmentGateway> = {}): LoadoutSaveEquipmentGateway {
  return {
    discover: async () => [],
    pick: async () => null,
    read: async ref => character(ref.save_id),
    ...overrides,
  };
}

interface Deferred<T> {
  promise: Promise<T>;
  resolve(value: T): void;
  reject(error: unknown): void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

test("discover reads the only save and keeps the snapshot accessor stable", async () => {
  const saved = reference("one");
  let reads = 0;
  const source = new LoadoutSaveEquipmentSource(gateway({
    discover: async () => [saved],
    read: async ref => { reads++; return character(ref.save_id); },
  }));
  const initial = source.getSnapshot();
  const getSnapshot = source.getSnapshot;
  const unsubscribe = source.subscribe(() => undefined);

  await source.discover();

  assert.strictEqual(source.getSnapshot(), source.getSnapshot());
  assert.notStrictEqual(source.getSnapshot(), initial);
  assert.deepEqual(source.getSnapshot().saves, [saved]);
  assert.deepEqual(source.getSnapshot().selected, saved);
  assert.equal(source.getSnapshot().character?.save_id, saved.save_id);
  assert.equal(source.getSnapshot().busy, false);
  assert.equal(reads, 1);
  assert.strictEqual(getSnapshot(), source.getSnapshot());
  unsubscribe();
  source.dispose();
});

test("multiple saves stay unselected, while an explicit preferred save may be outside discovery", async () => {
  const first = reference("first");
  const second = reference("second", 1);
  const preferred = reference("manual", 2);
  const readIds: string[] = [];
  const source = new LoadoutSaveEquipmentSource(gateway({
    discover: async () => [first, second],
    read: async ref => { readIds.push(ref.save_id); return character(ref.save_id); },
  }));

  await source.discover();
  assert.equal(source.getSnapshot().selected, null);
  assert.equal(source.getSnapshot().character, null);
  assert.deepEqual(readIds, []);

  await source.discover(preferred);
  assert.deepEqual(source.getSnapshot().saves, [first, second]);
  assert.deepEqual(source.getSnapshot().selected, preferred);
  assert.equal(source.getSnapshot().character?.save_id, preferred.save_id);
  assert.deepEqual(readIds, [preferred.save_id]);
  source.dispose();
});

test("character identity and top-level equipment fields are validated", async () => {
  const saved = reference("expected");
  const responses: unknown[] = [
    character("different"),
    { ...character(saved.save_id), source_sha256: 7 },
    { ...character(saved.save_id), equipment_slots: -1 },
    { ...character(saved.save_id), equipment: {} },
    character(saved.save_id),
  ];
  const source = new LoadoutSaveEquipmentSource(gateway({
    read: async () => responses.shift() as SaveCharacter,
  }));

  await source.select(saved);
  assert.equal(source.getSnapshot().error, "SAVE_IDENTITY_MISMATCH");
  await source.refresh();
  assert.equal(source.getSnapshot().error, "SAVE_CHARACTER_SOURCE_SHA256_EXPECTED");
  await source.refresh();
  assert.equal(source.getSnapshot().error, "SAVE_CHARACTER_EQUIPMENT_SLOTS_INVALID");
  await source.refresh();
  assert.equal(source.getSnapshot().error, "SAVE_CHARACTER_EQUIPMENT_EXPECTED");
  await source.refresh();
  assert.equal(source.getSnapshot().character?.save_id, saved.save_id);
  assert.equal(source.getSnapshot().error, null);
  source.dispose();
});

test("backend failures remain visible and a retry can recover", async () => {
  const saved = reference("retry");
  let attempts = 0;
  const source = new LoadoutSaveEquipmentSource(gateway({
    discover: async () => {
      attempts++;
      if (attempts === 1) throw new Error("backend unavailable");
      return [saved];
    },
  }));

  await source.discover();
  assert.equal(source.getSnapshot().error, "backend unavailable");
  assert.equal(source.getSnapshot().busy, false);
  await source.discover();
  assert.equal(source.getSnapshot().error, null);
  assert.equal(source.getSnapshot().character?.save_id, saved.save_id);
  source.dispose();
});

test("refresh clears the old character before a failed read and permits retry", async () => {
  const saved = reference("refresh");
  let attempts = 0;
  const source = new LoadoutSaveEquipmentSource(gateway({
    read: async ref => {
      attempts++;
      if (attempts === 2) throw new Error("read failed");
      return character(ref.save_id);
    },
  }));

  await source.select(saved);
  const loaded = source.getSnapshot().character;
  assert.ok(loaded);
  await source.refresh();
  assert.equal(source.getSnapshot().character, null);
  assert.equal(source.getSnapshot().error, "read failed");
  await source.refresh();
  assert.equal(source.getSnapshot().character?.save_id, saved.save_id);
  assert.equal(source.getSnapshot().error, null);
  source.dispose();
});

test("equipment rows with the same item identity retain distinct slot indexes", async () => {
  const saved = reference("rows");
  const rows = [equipment(7, 404), equipment(12, 404)];
  const source = new LoadoutSaveEquipmentSource(gateway({ read: async () => character(saved.save_id, rows) }));

  await source.select(saved);

  assert.deepEqual(source.getSnapshot().character?.equipment.map(row => row.slot_index), [7, 12]);
  assert.equal(source.getSnapshot().character?.equipment.length, 2);
  source.dispose();
});

test("out-of-order discovery and refresh responses cannot replace newer state", async () => {
  const first = reference("first");
  const second = reference("second");
  const discoveries = [deferred<SaveReference[]>(), deferred<SaveReference[]>()];
  let discoveryCall = 0;
  const source = new LoadoutSaveEquipmentSource(gateway({
    discover: () => discoveries[discoveryCall++].promise,
  }));

  const oldDiscovery = source.discover();
  const newDiscovery = source.discover();
  discoveries[1].resolve([second]);
  await newDiscovery;
  discoveries[0].resolve([first]);
  await oldDiscovery;
  assert.deepEqual(source.getSnapshot().saves, [second]);
  assert.deepEqual(source.getSnapshot().selected, second);

  const refreshResult = deferred<SaveCharacter>();
  const selectResult = deferred<SaveCharacter>();
  let readCall = 0;
  const reading = new LoadoutSaveEquipmentSource(gateway({
    read: () => (readCall++ === 0 ? Promise.resolve(character(second.save_id)) : readCall === 2 ? refreshResult.promise : selectResult.promise),
  }));
  await reading.select(second);
  const staleRefresh = reading.refresh();
  const newerSelection = reading.select(first);
  selectResult.resolve(character(first.save_id));
  await newerSelection;
  refreshResult.resolve(character(second.save_id));
  await staleRefresh;
  assert.deepEqual(reading.getSnapshot().selected, first);
  assert.equal(reading.getSnapshot().character?.save_id, first.save_id);
  assert.equal(reading.getSnapshot().busy, false);
  source.dispose();
  reading.dispose();
});

test("picker cancellation preserves selection and character, while a picked save is added", async () => {
  const selected = reference("selected");
  const picked = reference("picked");
  let pickResult: SaveReference | null = null;
  const source = new LoadoutSaveEquipmentSource(gateway({
    pick: async () => pickResult,
  }));
  await source.select(selected);
  const before = source.getSnapshot();

  await source.pick();
  const cancelled = source.getSnapshot();
  assert.strictEqual(cancelled.selected, before.selected);
  assert.strictEqual(cancelled.character, before.character);
  assert.equal(cancelled.busy, false);

  pickResult = picked;
  await source.pick();
  assert.deepEqual(source.getSnapshot().saves, [picked]);
  assert.deepEqual(source.getSnapshot().selected, picked);
  assert.equal(source.getSnapshot().character?.save_id, picked.save_id);
  source.dispose();
});

test("dispose invalidates an outstanding read", async () => {
  const saved = reference("dispose");
  const result = deferred<SaveCharacter>();
  const source = new LoadoutSaveEquipmentSource(gateway({ read: async () => result.promise }));
  const pending = source.select(saved);
  assert.equal(source.getSnapshot().busy, true);
  source.dispose();
  result.resolve(character(saved.save_id));
  await pending;

  assert.deepEqual(source.getSnapshot().selected, saved);
  assert.equal(source.getSnapshot().character, null);
  assert.equal(source.getSnapshot().error, null);
  assert.equal(source.getSnapshot().busy, false);
});
