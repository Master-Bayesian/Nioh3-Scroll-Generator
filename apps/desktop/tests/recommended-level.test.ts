import { test } from 'node:test';
import assert from 'node:assert/strict';
import { resolve } from 'node:path';
import { WorkerClient } from '../src/worker-client';

test('displayed level selector uses the exact worker curve without clamping', { timeout: 30000 }, async () => {
  const executable = process.env.NIOH3_WORKER_EXE;
  const worker = new WorkerClient(resolve('.'), executable || process.env.NIOH3_PYTHON || 'python', !!executable);
  try {
    const catalog = await worker.catalog(4, 'en-US');
    assert.equal(catalog.recommended_level.minimum_displayed_level, 142);
    assert.equal(catalog.recommended_level.maximum_displayed_level, 700);
    const selected = await worker.resolveRecommendedLevel(350);
    assert.equal(selected.status, 'exact');
    assert.deepEqual(selected.canonical_internal_levels, [585, 586]);
    assert.equal(selected.selected_internal_level, 585);
    assert.equal(selected.metadata.evidence, 'captured_native_curve_prediction');
    for (const requested of [141, 701, 0]) {
      const unavailable = await worker.resolveRecommendedLevel(requested);
      assert.equal(unavailable.status, 'out_of_range');
      assert.deepEqual(unavailable.canonical_internal_levels, []);
      assert.equal(unavailable.selected_internal_level, null);
    }
    for (const requested of [true, '350', 350.5, NaN, Infinity]) {
      await assert.rejects(worker.resolveRecommendedLevel(requested as number), /INVALID_REQUEST/);
    }
  } finally { await worker.close(); }
});
