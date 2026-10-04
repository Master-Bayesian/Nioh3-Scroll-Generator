import { test } from 'node:test';
import assert from 'node:assert/strict';
import { decodeLoadout, encodeLoadout, LOADOUT_PREFIX, type LoadoutPiece } from '../../workshop/loadout-code';

const pieces: LoadoutPiece[] = [
  {
    item_id: 0x1a2b, level: 230, level_before_forge: 200, plus: 12, rarity: 5, hell: true, hell_skill: 0,
    effects: [
      { effect_id: 0xffffffff, value: 0 },
      { effect_id: 1234, value: 4_000_000_000, star: true },
      { effect_id: 7, value: 15 },
    ],
  },
  { item_id: 3, level: 1, level_before_forge: 1, plus: 0, rarity: 0, hell: false, hell_skill: 0, effects: [] },
];

test('a loadout code round-trips every piece and value', () => {
  const code = encodeLoadout(pieces);
  assert.ok(code.startsWith(LOADOUT_PREFIX));
  assert.match(code.slice(LOADOUT_PREFIX.length), /^[A-Za-z0-9_-]+$/);
  assert.deepEqual(decodeLoadout(code), pieces);
});

test('whitespace a chat app adds is ignored', () => {
  const code = encodeLoadout(pieces);
  assert.deepEqual(decodeLoadout('  ' + code.slice(0, 10) + '\n' + code.slice(10) + ' '), pieces);
});

test('a damaged or foreign code is rejected with a Chinese instruction', () => {
  const code = encodeLoadout(pieces);
  const flipped = code.slice(0, -3) + (code.at(-3) === 'A' ? 'B' : 'A') + code.slice(-2);
  assert.throws(() => decodeLoadout(flipped), /配装码无效/);
  assert.throws(() => decodeLoadout(code.slice(0, -4)), /配装码无效/);
  assert.throws(() => decodeLoadout('hello'), /不是配装码/);
  assert.throws(() => decodeLoadout('N3E9-AAAA'), /更新工具/);
  assert.throws(() => decodeLoadout(''), /粘贴/);
});

test('encoding refuses values the format cannot carry', () => {
  assert.throws(() => encodeLoadout([]), /没有/);
  assert.throws(() => encodeLoadout([{ ...pieces[1], level: -1 }]), /无法写入/);
});
