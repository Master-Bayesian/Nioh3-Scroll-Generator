import { test } from 'node:test';
import assert from 'node:assert/strict';
import { resolve } from 'node:path';
import { ProtectedClient } from '../src/protected-client';
import { once } from 'node:events';
import {mkdtemp} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';

test('count receipt endpoint is bundled and rejects an unknown operation without a game', {timeout:15000}, async()=>{
  const directory=await mkdtemp(join(tmpdir(),'nioh3-count-endpoint-'));
  const previous=process.env.NIOH3_STATE_ROOT;
  process.env.NIOH3_STATE_ROOT=directory;
  const executable=process.env.NIOH3_PROTECTED_WORKER_EXE;
  const client=new ProtectedClient(resolve('.'),executable||process.env.NIOH3_PYTHON||'python','runtime',!!executable);
  if(previous===undefined)delete process.env.NIOH3_STATE_ROOT;else process.env.NIOH3_STATE_ROOT=previous;
  try{
    await assert.rejects(client.run('runtime.count_status',{operation_id:'00000000-0000-4000-8000-000000000001'}),/No such file.*plan\.json/);
    assert.equal((await client.call('runtime.status',{})).safe_to_shutdown,true);
  }finally{assert.equal(await client.close(),true)}
});

for (const role of ['save', 'runtime'] as const) test(`protected ${role} real IPC role and shutdown`, { timeout: 15000 }, async () => {
  const executable = process.env.NIOH3_PROTECTED_WORKER_EXE;
  const client = new ProtectedClient(resolve('.'), executable || process.env.NIOH3_PYTHON || 'python', role, !!executable);
  try {
    const hello = await client.handshake() as { role: string; kill_safe: boolean };
    assert.equal(hello.role, role);
    assert.equal(hello.kill_safe, false);
    assert.deepEqual(await client.call('job.current', {}), { job: null });
    if (role === 'runtime') {
      const status = await client.call('runtime.status', {});
      assert.equal(status.safe_to_shutdown, true);
      await assert.rejects(client.call('save.inventory', { save_id: '0'.repeat(64) }), /ROLE_MISMATCH/);
    } else {
      await assert.rejects(client.run('save.inventory', { save_id: '0'.repeat(64) }), /Unknown save ID/);
    }
    await assert.rejects(client.call('job.snapshot', { job_id: true } as any), /INVALID_REQUEST/);
  } finally { assert.equal(await client.close(), true); }
});

test('protected timeout accepts late frames without replaying or killing the owner', { timeout: 15000 }, async () => {
  const executable = process.env.NIOH3_PROTECTED_WORKER_EXE;
  const client = new ProtectedClient(resolve('.'), executable || process.env.NIOH3_PYTHON || 'python', 'runtime', !!executable);
  const internals = client as any;
  const child = internals.child;
  child.kill = () => { throw new Error('Protected owner must never be killed'); };
  try {
    await client.handshake();
    const listeners = child.stdout.listeners('data');
    child.stdout.removeAllListeners('data');
    let writes = 0;
    const write = child.stdin.write.bind(child.stdin);
    child.stdin.write = (...args: any[]) => { writes++; return write(...args); };
    const held: Buffer[] = [];
    const hold = (bytes: Buffer) => held.push(bytes);
    child.stdout.on('data', hold);
    await assert.rejects(internals.request('runtime.status', {}, 20), /outcome unknown; never replay automatically/);
    // Wait for the real host response, then deliver its framed bytes after expiry.
    for (let i = 0; i < 100 && held.length === 0; i++) await new Promise(resolve => setTimeout(resolve, 10));
    assert.ok(held.length > 0);
    child.stdout.removeListener('data', hold);
    for (const listener of listeners) child.stdout.on('data', listener);
    for (const bytes of held) child.stdout.emit('data', bytes);
    assert.equal(writes, 1, 'A timed-out operation is never automatically replayed');
    assert.equal((await client.call('runtime.status', {})).safe_to_shutdown, true);
  } finally { assert.equal(await client.close(), true); }
});

test('protected malformed transport uses EOF cleanup and cannot claim safe shutdown', { timeout: 15000 }, async () => {
  const executable = process.env.NIOH3_PROTECTED_WORKER_EXE;
  const client = new ProtectedClient(resolve('.'), executable || process.env.NIOH3_PYTHON || 'python', 'runtime', !!executable);
  const child = (client as any).child;
  child.kill = () => { throw new Error('Protected owner must never be killed'); };
  await client.handshake();
  const exited = once(child, 'exit');
  const invalid = Buffer.alloc(4); invalid.writeUInt32LE(4194305);
  child.stdout.emit('data', invalid);
  await assert.rejects(client.call('runtime.status', {}), /INVALID_FRAME_SIZE/);
  assert.equal(await client.close(), false);
  const [code, signal] = await exited;
  assert.equal(code, 0); assert.equal(signal, null);
});
