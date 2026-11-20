import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { CharacterEquipment } from '../../../packages/contracts/protected-responses';
import { matchesLoadoutUndoItem, parseLoadoutUndo } from '../../workshop/loadout-undo';

const previous = {slot_index: 4, slot: 4, item_id: 141, inventory_key: 101, seed: 7};
const current: CharacterEquipment = { ...previous, appearance_id: 141, quantity: 1,
  level: 180, level_before_forge: 180, plus: 0, familiarity: 0, rarity: 5, effects: [], worn: false };

test('restore retains the physical item across changed wear and familiarity', () => {
  assert(matchesLoadoutUndoItem(previous, {...current, worn: true, familiarity: 100}));
});
test('same-id replacement and regenerated record cannot impersonate the original item', () => {
  assert(!matchesLoadoutUndoItem(previous, {...current, inventory_key: 102}));
  assert(!matchesLoadoutUndoItem(previous, {...current, seed: 8}));
  assert(!matchesLoadoutUndoItem(previous, {...current, slot_index: 5}));
  assert(!matchesLoadoutUndoItem(previous, undefined));
});
test('unbound historical undo and corrupt duplicate identities are not restored', () => {
  assert.equal(parseLoadoutUndo({school:'samurai',restore:[{slot_index:4,slot:4,item_id:141}]}), null);
  assert.equal(parseLoadoutUndo({school:'samurai',restore:[previous,previous]}), null);
  assert.equal(parseLoadoutUndo({school:'samurai',restore:[{...previous,inventory_key:-1}]}), null);
  assert.deepEqual(parseLoadoutUndo({school:'samurai',restore:[previous]}), {school:'samurai',restore:[previous]});
});
