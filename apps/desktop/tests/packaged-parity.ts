/** Compare source and standalone worker identities and exact candidate DTOs. */
import { WorkerClient, type StartParams } from '../src/worker-client';
import { mkdir, writeFile } from 'node:fs/promises';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import assert from 'node:assert/strict';

if (!process.env.NIOH3_WORKER_EXE) throw new Error('Set NIOH3_WORKER_EXE to the standalone worker path');
const root = resolve('.'); 
const source = new WorkerClient(root, process.env.NIOH3_PYTHON || 'python');

/**
 * The packaged worker's launch arguments.
 *
 * The shipped Python worker EXE starts with no arguments (`[]`), and the Python
 * branch keeps that byte-for-byte. The Rust opt-in worker requires exactly one
 * launch-mode acknowledgement plus its resource roots, so the argv is taken from
 * the staged `worker/worker-backend.json` - the same declaration the packaged
 * host reads - and only the roots are re-anchored onto this checkout's staged
 * runtime. Deriving from the manifest rather than from a literal keeps this gate
 * comparing the source worker against the argv the package actually declares.
 */
function workerArgv(executable: string): string[] {
  if (process.env.NIOH3_WORKER_BACKEND !== 'rust') return [];
  // `<portable>/worker/nioh3-search-worker.exe` -> `<portable>`.
  const runtime = dirname(dirname(executable));
  const manifestPath = resolve(runtime, 'worker/worker-backend.json');
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8')) as {
    schema?: string;
    backend?: string;
    invocation?: { offline_search?: { mode?: string; binary?: string; argv?: string[] } };
  };
  assert.equal(manifest.schema, 'nioh3-worker-backend/v1', manifestPath);
  assert.equal(manifest.backend, 'rust', manifestPath);
  const role = manifest.invocation?.offline_search;
  assert.ok(role?.argv?.length, `the staged manifest declares no offline_search argv: ${manifestPath}`);
  assert.equal(role.mode, 'packaged', 'the packaged parity gate needs a packaged launch');
  assert.equal(
    resolve(executable),
    resolve(runtime, 'worker', role.binary ?? ''),
    'the staged manifest must name the executable under test',
  );
  const argv = role.argv.map((token) => token.replaceAll('<runtime>', runtime));
  assert.ok(argv.includes('--packaged-worker'), `packaged mode is required: ${argv.join(' ')}`);
  for (const flag of ['--data-root', '--contract-dir']) {
    const index = argv.indexOf(flag);
    assert.ok(index >= 0 && argv[index + 1], `the staged argv lacks ${flag}`);
    assert.ok(
      resolve(argv[index + 1]).startsWith(resolve(runtime)),
      `${flag} must stay inside the staged runtime: ${argv[index + 1]}`,
    );
  }
  return argv;
}

const packed = new WorkerClient(
  root,
  resolve(process.env.NIOH3_WORKER_EXE),
  true,
  undefined,
  workerArgv(resolve(process.env.NIOH3_WORKER_EXE)),
);
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
