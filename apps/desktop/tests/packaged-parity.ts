/** Compare source and standalone worker identities and exact candidate DTOs. */
import { WorkerClient, type StartParams } from '../src/worker-client';
import { mkdir, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import assert from 'node:assert/strict';

if (!process.env.NIOH3_WORKER_EXE) throw new Error('Set NIOH3_WORKER_EXE to the standalone worker path');
const root = resolve('.');
const source = new WorkerClient(root, process.env.NIOH3_PYTHON || 'python');
const packed = new WorkerClient(root, resolve(process.env.NIOH3_WORKER_EXE), true);
const allowCpuFallback = process.env.NIOH3_PARITY_ALLOW_CPU === '1';
try {
  const [a, b] = await Promise.all([source.handshake(), packed.handshake()]);
  assert.deepEqual(a, b);
  const results = [];
  for (const rarity of [3, 4, 5] as const) {
    const params: StartParams = { context_digest: a.context.context_digest, result_count: 1, page_trials: 100000, job_trials: 1000000,
      allow_cpu_fallback: allowCpuFallback, resume_token: null,
      query: { playthrough: 3, rarity, level: 180, primary_effect_ids: [44634], required_secondary_ids: [], required_secondary_id_groups: [],
        grace_effect_id: null, minimum_roll_percent_by_effect_id: [], auxiliary: { required_terrain_effect_keys: [], required_terrain_effect_key_groups: [], required_special_rule_keys: [], required_special_rule_key_groups: [], required_enemy_lookup_keys: [], required_enemy_lookup_key_groups: [] } } };
    async function run(worker: WorkerClient) {
      let job = await worker.start(params);
      const deadline = Date.now() + 30_000;
      while (!['completed', 'failed', 'cancelled'].includes(job.state)) {
        if (Date.now() > deadline) throw new Error('Search exceeded the smoke deadline');
        await new Promise(resolve => setTimeout(resolve, 30)); job = await worker.snapshot(job.job_id);
      }
      assert.equal(job.state, 'completed', JSON.stringify(job.error)); assert.equal(job.candidates.length, 1);
      return job;
    }
    const x = await run(source), y = await run(packed);
    assert.deepEqual(x.candidates, y.candidates); assert.equal(x.cursor, y.cursor);
    results.push({ rarity, cursor: x.cursor, seed: x.candidates[0].seed, candidate_id: x.candidates[0].candidate_id });
  }
  await mkdir('deliverables/frontend-v2', { recursive: true });
  await writeFile('deliverables/frontend-v2/packaged-parity.json', JSON.stringify({ passed: true, allowCpuFallback, handshake: a, results }, null, 2));
  console.log('PACKAGED_R3_R4_R5_PARITY_OK');
} finally { await source.close(); await packed.close(); }
