import { test } from 'node:test';
import assert from 'node:assert/strict';
import { LiveAddSession } from '../src/live-add-session';
import type { LiveAddOperation } from '../../../packages/contracts/protected-responses';

test('live-add disconnect blocks replay; reload queries the exact operation', async () => {
  const op: LiveAddOperation = { operation_id: '00000000-0000-0000-0000-000000000001', plan_digest: 'a'.repeat(64),
    state: 'prepared', can_dispatch: true, can_cancel: true, receipt: null };
  const methods: string[] = []; let raw: string | null = null;
  const storage = { getItem: () => raw, setItem: (_key: string, value: string) => { raw = value; } };
  const gateway = { prepare: async () => ({ live_add: op }), execute: async (command: any) => {
    methods.push(command.method);
    if (command.method === 'runtime.live_add_execute') throw new Error('Lost acknowledgement');
    return { live_add: { ...op, state: 'verified' as const, can_dispatch: false, can_cancel: false } };
  } };
  const session = new LiveAddSession(gateway, storage);
  await session.prepare({ save_id: 'save', snapshot_id: 'snapshot', source: 'search', job_id: 'job', candidate_id: 'candidate' });
  await assert.rejects(session.execute(), /Lost acknowledgement/);
  assert.throws(() => session.execute(), /REVIEW_REQUIRED/);
  assert.throws(() => session.prepare({} as any), /OUTCOME_UNKNOWN/);
  const reopened = new LiveAddSession(gateway, storage);
  await reopened.connect();
  assert.deepEqual(methods, ['runtime.live_add_execute', 'runtime.live_add_status']);
  assert.equal(reopened.getSnapshot().operation?.state, 'verified');
});

test('unrelated live-add receipt cannot clear uncertainty', async () => {
  const op: LiveAddOperation = { operation_id: '00000000-0000-0000-0000-000000000001', plan_digest: 'a'.repeat(64),
    state: 'prepared', can_dispatch: true, can_cancel: true, receipt: null };
  const session = new LiveAddSession({ prepare: async () => ({ live_add: op }), execute: async () => ({ live_add: {
    ...op, operation_id: '00000000-0000-0000-0000-000000000002', state: 'verified', can_dispatch: false,
  } }) });
  await session.prepare({} as any);
  await assert.rejects(session.execute(), /RECEIPT_MISMATCH/);
  assert.equal(session.getSnapshot().uncertain, true);
});
