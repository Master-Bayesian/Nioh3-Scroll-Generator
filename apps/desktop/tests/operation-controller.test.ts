import { test } from 'node:test';
import assert from 'node:assert/strict';
import { OperationController } from '../src/operation-controller';
import { publicCurrentJob, requirePublicJob } from '../src/public-operation-jobs';
import type { ProtectedJob } from '../../../packages/contracts/protected-responses';
import type { OperationsApi } from '../src/operations-api';

const running: ProtectedJob = { job_id: 'a'.repeat(32), kind: 'runtime.search', state: 'running', sequence: 5,
  cancellable: true, progress: null, result: null, error: null };
const pause = () => new Promise(resolve => setTimeout(resolve, 20));
function api(overrides: Partial<OperationsApi> = {}): OperationsApi {
  return { current: async () => ({ job: null, busy: false }), snapshot: async () => running,
    cancel: async () => ({ ...running, sequence: 6, state: 'cancel_requested' }),
    execute: async () => { throw new Error('Unexpected submission'); }, ...overrides } as OperationsApi;
}

test('protected interruption retains job identity and never replays an operation', async () => {
  let submits = 0, available = false;
  const controller = new OperationController('runtime', api({ snapshot: async (_role, id) => {
    assert.equal(id, running.job_id);
    if (!available) throw new Error('Transport unavailable');
    return { ...running, state: 'completed', sequence: 7, result: { candidate: null } };
  } }), 1);
  try {
    await controller.start(async () => { submits++; return running; }); await pause();
    assert.equal(controller.getSnapshot().phase, 'interrupted');
    assert.equal(controller.getSnapshot().job!.state, 'running'); assert.equal(controller.canStart(), false);
    assert.equal(await controller.start(async () => { submits++; return running; }), false);
    available = true; await controller.recover(); assert.equal(controller.getSnapshot().phase, 'completed');
    assert.equal(submits, 1);
  } finally { controller.dispose(); }
});

test('stale polling cannot undo cancellation, and save writes cannot be cancelled', async () => {
  let cancels = 0;
  const controller = new OperationController('runtime', api({ cancel: async () => {
    cancels++; return { ...running, state: 'cancel_requested', sequence: 6 };
  } }), 1);
  try {
    await controller.start(async () => running); await controller.cancel(); await pause();
    assert.equal(controller.getSnapshot().job!.state, 'cancel_requested'); assert.equal(cancels, 1);
  } finally { controller.dispose(); }
  const save = new OperationController('save', api({ cancel: async () => { throw new Error('Must not cancel writes'); } }), 10000);
  try { await save.start(async () => ({ ...running, kind: 'save.commit', cancellable: false })); await save.cancel(); }
  finally { save.dispose(); }
});

test('lost commit acknowledgement recovers only the exact durable receipt', async () => {
  const operationId = 'b'.repeat(32), commands: string[] = [];
  const controller = new OperationController('save', api({ execute: async command => {
    commands.push(command.method); assert.equal(command.method, 'save.operation');
    assert.deepEqual(command.params, { plan_id: operationId });
    return { ...running, kind: 'save.operation', state: 'completed', cancellable: false,
      result: { operation_id: operationId, save_id: 'c'.repeat(64), commit_status: 'unknown', warning: null, details: {} } };
  } }), 1);
  try {
    await controller.start(async () => { throw new Error('Acknowledgement lost'); }, operationId); await controller.recover();
    assert.equal(controller.getSnapshot().phase, 'completed');
    assert.equal((controller.getSnapshot().result as { commit_status: string }).commit_status, 'unknown');
    assert.deepEqual(commands, ['save.operation']);
  } finally { controller.dispose(); }
});

test('reload observes the current public job and does not resurrect a disposed observer', async () => {
  let resolveCurrent!: (value: { job: ProtectedJob; busy: boolean }) => void;
  const controller = new OperationController('runtime', api({ current: () => new Promise(resolve => { resolveCurrent = resolve; }) }), 1);
  const recovery = controller.recover(); controller.dispose(); resolveCurrent({ job: running, busy: true });
  await recovery; assert.equal(controller.getSnapshot().job, null);
  const reloaded = new OperationController('runtime', api({ current: async () => ({ job: { ...running, state: 'completed' }, busy: false }) }), 1);
  try { await reloaded.recover(); assert.equal(reloaded.getSnapshot().phase, 'completed'); assert.equal(reloaded.getSnapshot().recovered, true); }
  finally { reloaded.dispose(); }
});

test('private job recovery exposes neither template data nor raw candidate/cache payloads', () => {
  for (const kind of ['save.template', 'save.cached_grace', 'runtime.export'] as const) {
    const privateJob = { ...running, kind, result: { template_hex: 'private bytes' } } as unknown as ProtectedJob;
    assert.throws(() => requirePublicJob(privateJob), /PRIVATE_OPERATION_JOB/);
    assert.deepEqual(publicCurrentJob(privateJob), { job: null, busy: true });
    assert.deepEqual(publicCurrentJob({ ...privateJob, state: 'completed' }), { job: null, busy: false });
  }
  assert.deepEqual(publicCurrentJob(running), { job: running, busy: true });
});
