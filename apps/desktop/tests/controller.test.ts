import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { DesktopApi } from '../src/api';
import type { Handshake, JobSnapshot } from '../../../packages/contracts/responses';
import { SearchController } from '../src/search-controller';
import type { StartParams } from '../src/worker-client';

const hello: Handshake = { protocol: 1, role: 'offline_search', contract_digest: 'digest',
  context: { product_version: 'test', game_profile: 'test', resources_digest: 'test', algorithm_version: 'test', policy_version: 'test', seed_accelerator_abi: 2, seed_accelerator_build_id: 'test', context_digest: 'a'.repeat(64) },
  capabilities: { playthroughs: [3], rarities: [3, 4, 5], cuda_pivot_and_auxiliary: false, directcompute_effect_filter: false, cpu_exact_replay: true, bulk_cpu_requires_opt_in: true, save_write: false, runtime_calls: false } };
const job: JobSnapshot = { job_id: 'job', state: 'running', sequence: 5, query_digest: 'query', context_digest: 'context', start_cursor: 0, cursor: 0, candidates: [], progress: null, stop_reason: null, error: null, resume_token: null, elapsed_ms: 0 };
const params = { query: { level: 180 }, resume_token: null } as unknown as StartParams;
const pause = () => new Promise(resolve => setTimeout(resolve, 200));

test('submitted query is isolated, stale progress cannot undo cancel, restart clears session', async () => {
  const api: DesktopApi = { handshake: async () => hello, restartWorker: async () => hello,
    currentSearch: async () => ({ job: null, submitted: null }),
    searchCatalog: async () => ({ context_digest: 'context', ordinary_effects: [], grace_effects: [], recommended_level: {
      minimum_internal_level: 156, maximum_internal_level: 1400, minimum_displayed_level: 142, maximum_displayed_level: 700,
      selection_policy: 'lowest_canonical_internal_level', evidence: 'captured_native_curve_prediction' } }),
    resolveRecommendedLevel: async () => { throw new Error('Not used by the search controller'); },
    startSearch: async () => ({ ...job }), snapshot: async () => ({ ...job, sequence: 4 }),
    cancelSearch: async () => ({ ...job, state: 'cancel_requested', sequence: 6 }) };
  const controller = new SearchController(api);
  try {
    await controller.connect();
    const draft = structuredClone(params);
    await controller.start(draft); draft.query.level = 1;
    assert.equal(controller.getSnapshot().submitted!.query.level, 180);
    await controller.cancel(); await pause();
    assert.equal(controller.getSnapshot().job!.state, 'cancel_requested');
    await controller.connect(true);
    assert.equal(controller.getSnapshot().job, null);
  } finally { controller.dispose(); }
});

test('transport failure terminates the displayed job and removes resume eligibility', async () => {
  const api = { handshake: async () => hello, startSearch: async () => ({ ...job }),
    currentSearch: async () => ({ job: null, submitted: null }),
    snapshot: async () => { throw new Error('WORKER_EXITED'); } } as unknown as DesktopApi;
  const controller = new SearchController(api);
  try {
    await controller.connect(); await controller.start(params); await pause();
    assert.equal(controller.getSnapshot().job!.state, 'failed');
    assert.equal(controller.getSnapshot().job!.resume_token, null);
    assert.equal(controller.getSnapshot().handshake, null);
  } finally { controller.dispose(); }
});
