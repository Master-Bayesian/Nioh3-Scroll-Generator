import { test } from 'node:test';
import assert from 'node:assert/strict';
import { initialQuery } from '../../workshop/model';
test('Fresh production searches do not inherit fixture conditions', () => {
  const query = initialQuery();
  for (const key of ['effects', 'graces', 'enemies', 'rules', 'terrains', 'capacities'] as const) assert.deepEqual(query[key], []);
  assert.equal(query.count, 25);
});
