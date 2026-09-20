/**
 * Compare source and standalone worker identities and exact candidate DTOs.
 *
 * Both launches carry the same explicit game file version, taken from
 * `NIOH3_PARITY_GAME_FILE_VERSION`: the packaged stage receives it as the Tauri
 * host injects its resolved version onto a packaged search launch, and the
 * source Python worker receives the identical value. The gate therefore
 * compares two launches of one identity, never falling back to the
 * non-production opt-in or an implicit default.
 */
import { WorkerClient, type StartParams } from '../src/worker-client';
import { mkdir, writeFile } from 'node:fs/promises';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import assert from 'node:assert/strict';

if (!process.env.NIOH3_WORKER_EXE) throw new Error('Set NIOH3_WORKER_EXE to the standalone worker path');
const root = resolve('.'); 

/**
 * The exact four-part game file version this packaged stage was built for.
 *
 * Required rather than defaulted: a staged worker launched without an identity
 * refuses to start, and a guessed version would compare two different
 * identities without saying so.
 */
function parityGameFileVersion(): string {
  const raw = process.env.NIOH3_PARITY_GAME_FILE_VERSION ?? '';
  if (!/^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$/.test(raw)) {
    throw new Error('Set NIOH3_PARITY_GAME_FILE_VERSION to the packaged stage\'s exact four-part game file version, for example 2.0.2.0');
  }
  return raw;
}

/**
 * The packaged worker's launch arguments.
 *
 * The Python backend contributes no staged base arguments (`[]`). The Rust
 * opt-in worker requires a launch-mode acknowledgement plus its resource roots,
 * so those base arguments come from `worker/worker-backend.json` - the same
 * declaration the packaged host reads - and only the roots are re-anchored onto
 * this checkout's staged runtime. The caller then appends the same explicit game
 * file version used by the source worker.
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

const gameFileVersion = parityGameFileVersion();
const packed = new WorkerClient(
  root,
  resolve(process.env.NIOH3_WORKER_EXE),
  true,
  undefined,
  // The packaged host injects this session's exact resolved version onto the
  // staged argv; the gate emulates that injection rather than reading a
  // handshake, so a stage whose resources do not match the pinned version fails
  // closed at startup.
  [...workerArgv(resolve(process.env.NIOH3_WORKER_EXE)), '--game-file-version', gameFileVersion],
);
const allowCpuFallback = process.env.NIOH3_PARITY_ALLOW_CPU === '1';
let source: WorkerClient | undefined;
try {
  const b = await packed.handshake();
  assert.equal(b.context.game_file_version, gameFileVersion, 'the packaged worker must publish the pinned version');
  assert.equal(b.context.production_authority, true, 'the packaged launch must resolve a production identity');
  source = new WorkerClient(root, process.env.NIOH3_PYTHON || 'python', false, undefined,
    ['--game-file-version', gameFileVersion]);
  const a = await source.handshake();
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
} finally { await source?.close(); await packed.close(); }
