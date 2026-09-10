import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { ProtectedClient } from '../src/protected-client';
import { WorkerClient, type StartParams } from '../src/worker-client';
import { OperationController } from '../src/operation-controller';
import { SaveSession, saveGateway, editFromEntry } from '../src/save-session';
import { publicCurrentJob, requirePublicJob } from '../src/public-operation-jobs';
import type { OperationsApi } from '../src/operations-api';
import type { SaveReference } from '../../../packages/contracts/protected-responses';

test('real encrypted synthetic save: review, edit, receipt recovery, delete, restore and search installation through IPC', { timeout: 60000 }, async () => {
  const directory = await mkdtemp(join(tmpdir(), 'nioh3-save-workflow-'));
  const previous = process.env.NIOH3_STATE_ROOT;
  process.env.NIOH3_STATE_ROOT = join(directory, 'state');
  let client: ProtectedClient | undefined;
  let observer: OperationController | undefined;
  let source: WorkerClient | undefined;
  try {
    const fixture = await promisify(execFile)(process.env.NIOH3_PYTHON || 'python', [
      resolve('apps/desktop/tests/fixtures/create-save.py'), join(directory, 'fixture')], { windowsHide: true });
    const { path } = JSON.parse(fixture.stdout);
    const original = await readFile(path);
    const executable = process.env.NIOH3_PROTECTED_WORKER_EXE;
    client = new ProtectedClient(resolve('.'), executable || process.env.NIOH3_PYTHON || 'python', 'save', !!executable);
    const reference = await client.run('save.register', { path }) as unknown as SaveReference;
    const api: Pick<OperationsApi, 'execute' | 'current' | 'snapshot' | 'cancel' | 'prepareInstall'> = {
      execute: command => client!.call(command.method, command.params),
      current: async () => publicCurrentJob((await client!.call('job.current', {})).job),
      snapshot: async (_role, jobId) => requirePublicJob(await client!.call('job.snapshot', { job_id: jobId })),
      cancel: async (_role, jobId) => requirePublicJob(await client!.call('job.cancel', { job_id: jobId })),
      prepareInstall: async params => {
        assert.equal(params.source, 'search');
        const candidate = await source!.exportCandidate(params.job_id, params.candidate_id);
        return client!.call('save.prepare_install', { save_id: params.save_id, snapshot_id: params.snapshot_id,
          candidate, recommended_level: params.recommended_level, transfer_count: params.transfer_count });
      },
    };
    observer = new OperationController('save', api, 5);
    const session = new SaveSession(saveGateway(api, observer));
    await session.select(reference);
    const entry = session.getSnapshot().inventory!.entries[0];
    assert.deepEqual(entry.derived, { initial_challenge_capacity: 4, remaining_challenge_attempts: 3,
      recommended_displayed_level: 160, recommended_raw_was_clamped: false });
    const edit = editFromEntry(entry); edit.header.recommended_level = 585;
    assert.equal('derived' in edit, false);
    const plan = await session.prepareEdit([edit]);
    assert.deepEqual(await readFile(path), original, 'Preview is read-only');
    assert.equal((await session.commit(plan.plan_id)).commit_status, 'committed');
    const edited = await readFile(path);
    assert.notDeepEqual(edited, original);
    await session.refresh();
    assert.equal(session.getSnapshot().inventory!.entries[0].derived.recommended_displayed_level, 350);
    const deleted = await session.prepareDelete([entry.slot_index]);
    assert.equal((await session.commit(deleted.plan_id)).commit_status, 'committed');
    await session.refresh(); assert.equal(session.getSnapshot().inventory!.entries.length, 0);
    const receipt = await client.run('save.operation', { plan_id: deleted.plan_id });
    assert.equal(receipt.commit_status, 'committed');
    const { backups } = await client.run('save.backups', { save_id: reference.save_id }) as { backups: { backup_id: string; action: string }[] };
    // The latest backup is the exact encrypted pre-delete snapshot.
    assert.ok(backups.length >= 2, JSON.stringify(backups));
    const restore = await session.prepareRestore(backups[0].backup_id);
    assert.equal((await session.commit(restore.plan_id)).commit_status, 'committed');
    assert.deepEqual(await readFile(path), edited);
    await session.refresh(); assert.equal(session.getSnapshot().inventory!.entries.length, 1);
    const searchExecutable = process.env.NIOH3_WORKER_EXE;
    source = new WorkerClient(resolve('.'), searchExecutable || process.env.NIOH3_PYTHON || 'python', !!searchExecutable);
    const hello = await source.handshake();
    const params: StartParams = { context_digest: hello.context.context_digest, result_count: 1,
      page_trials: 100000, job_trials: 1000000, allow_cpu_fallback: true, resume_token: null,
      query: { playthrough: 3, rarity: 4, level: 180, initial_challenge_counts: [7],
        primary_effect_ids: [44634], required_secondary_ids: [], required_secondary_id_groups: [],
        grace_effect_id: null, minimum_roll_percent_by_effect_id: [], auxiliary: {
          required_terrain_effect_keys: [], required_terrain_effect_key_groups: [], required_special_rule_keys: [],
          required_special_rule_key_groups: [], required_enemy_lookup_keys: [], required_enemy_lookup_key_groups: [] } } };
    let job = await source.start(params);
    const deadline = Date.now() + 15000;
    while (!['completed', 'cancelled', 'failed'].includes(job.state)) {
      if (Date.now() > deadline) throw new Error('Search exceeded the workflow deadline');
      await new Promise(resolve => setTimeout(resolve, 20)); job = await source.snapshot(job.job_id);
    }
    assert.equal(job.state, 'completed', JSON.stringify(job.error)); assert.equal(job.candidates.length, 1);
    const install = await session.prepareInstall({ source: 'search', job_id: job.job_id, candidate_id: job.candidates[0].candidate_id,
      recommended_level: 585, transfer_count: 0 });
    assert.equal((await session.commit(install.plan_id)).commit_status, 'committed');
    await session.refresh();
    const installed = session.getSnapshot().inventory!.entries;
    assert.equal(installed.length, 2);
    assert.equal(installed[1].header.seed, job.candidates[0].seed);
    assert.equal(installed[1].derived.initial_challenge_capacity, 7);
    assert.equal(installed[1].derived.remaining_challenge_attempts, 7);
    assert.equal(installed[1].derived.recommended_displayed_level, 350);
  } finally {
    observer?.dispose();
    await source?.close();
    if (client) assert.equal(await client.close(), true);
    if (previous === undefined) delete process.env.NIOH3_STATE_ROOT; else process.env.NIOH3_STATE_ROOT = previous;
    await rm(directory, { recursive: true, force: true });
  }
});
