import { test } from 'node:test';
import assert from 'node:assert/strict';
import { resolve } from 'node:path';
import { ProtectedClient } from '../src/protected-client';
import { requirePublicJob } from '../src/public-operation-jobs';

test('live-add contract rejects raw renderer-style fields and wrong roles with game closed', { timeout: 15000 }, async () => {
  const executable = process.env.NIOH3_PROTECTED_WORKER_EXE;
  const client = new ProtectedClient(resolve('.'), executable || process.env.NIOH3_PYTHON || 'python', 'runtime', !!executable);
  try {
    await client.handshake();
    await assert.rejects(client.call('runtime.live_add_execute', {
      operation_id: '00000000-0000-0000-0000-000000000000', plan_digest: '0'.repeat(64),
      address: 123, record_hex: '00',
    } as any), /INVALID_REQUEST/);
    await assert.rejects(client.call('save.live_add_source', { save_id: '0'.repeat(64), snapshot_id: '0'.repeat(32) }), /ROLE_MISMATCH/);
    await assert.rejects(client.run('runtime.live_add_status', { operation_id: '00000000-0000-0000-0000-000000000000' }), /No such file|cannot find|not find/);
    assert.equal((await client.call('runtime.status', {})).safe_to_shutdown, true);
  } finally { assert.equal(await client.close(), true); }
});

test('live-add operation results are public but save-source handoff stays private', () => {
  const base = { job_id: '0'.repeat(32), state: 'completed', sequence: 1, cancellable: false,
    progress: null, result: null, error: null } as const;
  assert.equal(requirePublicJob({ ...base, kind: 'runtime.live_add_status' }).kind, 'runtime.live_add_status');
  assert.throws(() => requirePublicJob({ ...base, kind: 'save.live_add_source' }), /PRIVATE_OPERATION_JOB/);
});
