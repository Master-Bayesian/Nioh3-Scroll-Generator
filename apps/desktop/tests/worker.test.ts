import { test } from 'node:test';
import assert from 'node:assert/strict';
import { resolve } from 'node:path';
import { WorkerClient, type StartParams } from '../src/worker-client';

const python = process.env.NIOH3_PYTHON || 'python';
const pause = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
function params(digest: string): StartParams {
  return { context_digest: digest, result_count: 2, page_trials: 100000, job_trials: 1000000, allow_cpu_fallback: true, resume_token: null,
    query: { playthrough: 3, rarity: 4, level: 180, primary_effect_ids: [44634], required_secondary_ids: [], required_secondary_id_groups: [],
      grace_effect_id: null, minimum_roll_percent_by_effect_id: [], auxiliary: { required_terrain_effect_keys: [], required_terrain_effect_key_groups: [], required_special_rule_keys: [], required_special_rule_key_groups: [], required_enemy_lookup_keys: [], required_enemy_lookup_key_groups: [] } } };
}

test('real Python IPC: handshake, search, resume, cancellation, validation and exit', { timeout: 45000 }, async () => {
  const executable = process.env.NIOH3_WORKER_EXE;
  const worker = new WorkerClient(resolve('.'), executable || python, !!executable);
  try {
    const hello = await worker.handshake();
    assert.equal(hello.role, 'offline_search');
    assert.equal(hello.capabilities.save_write, false);
    const catalog = await worker.catalog(4, 'en-US');
    assert.ok(catalog.ordinary_effects.some(effect => effect.effect_id === 44634 && effect.name === 'Ultimate Skill'));
    assert.ok(catalog.grace_effects.length > 0);
    const p = params(hello.context.context_digest);
    await assert.rejects(worker.start({ ...p, result_count: true } as unknown as StartParams), /INVALID_REQUEST/);
    await assert.rejects(worker.start({ ...p, context_digest: '0'.repeat(64) }), /CONTEXT_MISMATCH/);
    const wait = async (jobId: string) => {
      for (let i = 0; i < 250; i++) {
        const snapshot = await worker.snapshot(jobId);
        if (['completed', 'failed', 'cancelled'].includes(snapshot.state)) return snapshot;
        await pause(50);
      }
      throw new Error('Job did not terminate');
    };
    const first = await wait((await worker.start(p)).job_id);
    assert.equal(first.state, 'completed', JSON.stringify(first.error));
    assert.equal(first.candidates.length, 2);
    assert.equal(first.candidates[0].effects[0].effect_id, 44634);
    assert.equal((await worker.current()).job!.job_id, first.job_id);
    const retained = (await worker.current()).submitted!;
    retained.query.level = 170;
    assert.equal((await worker.current()).submitted!.query.level, 180);
    const second = await wait((await worker.start({ ...p, resume_token: first.resume_token })).job_id);
    assert.equal(second.state, 'completed');
    assert.equal(new Set([...first.candidates, ...second.candidates].map(c => c.candidate_id)).size, 4);
    const filtered = await wait((await worker.start({ ...p, query: { ...p.query, initial_challenge_counts: [7] } })).job_id);
    assert.equal(filtered.state, 'completed', JSON.stringify(filtered.error));
    assert.equal(filtered.candidates.length, 2);
    assert.ok(filtered.candidates.every(candidate => candidate.initial_challenge_capacity === 7));
    const slow = await worker.start({ ...p, result_count: 100, job_trials: 4294967296 });
    const cancelled = await worker.cancel(slow.job_id);
    assert.ok(['cancel_requested', 'completed', 'cancelled'].includes(cancelled.state));
    const end = await wait(slow.job_id);
    assert.ok(['completed', 'cancelled'].includes(end.state));
    await assert.rejects(worker.snapshot(first.job_id), /JOB_NOT_FOUND/);
    process.kill(worker.pid!);
    // Process termination and Node's exit notification are asynchronous,
    // especially for a packaged worker under concurrent integration tests.
    const exitDeadline = Date.now() + 5000;
    while (worker.diagnostics().connection !== 'unavailable' && Date.now() < exitDeadline) await pause(20);
    assert.equal(worker.diagnostics().connection, 'unavailable', 'Killed worker did not report exit within five seconds');
    await assert.rejects(worker.snapshot(slow.job_id), /WORKER_EXITED|EPIPE|ECONNRESET/);
  } finally { await worker.close(); }
});
