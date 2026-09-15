/**
 * The legacy WorkerClient call shape must survive the packaged-worker argv
 * parameter.
 *
 * `packaged-parity.ts` now passes an explicit argv for the Rust opt-in graph, so
 * the shipped shape has to be proven unchanged rather than assumed: with the
 * parameter omitted, the client must still launch the source Python worker with
 * `-u -m nioh3_scroll_editor.search_worker`, and an argv supplied alongside
 * `executable = false` must not leak into that command line.
 *
 * The assertion is a real launch and handshake, not a parser check: a worker
 * that started with the wrong arguments could not answer `handshake` with the
 * offline-search role and the contract digest recomputed from disk.
 */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { resolve } from 'node:path';
import { WorkerClient } from '../src/worker-client';

const root = resolve('.');
const python = process.env.NIOH3_PYTHON || 'python';

async function handshakeWith(client: WorkerClient) {
  try {
    return await client.handshake();
  } finally {
    await client.close();
  }
}

test('the default call shape still launches the source worker', async () => {
  const client = new WorkerClient(root, python);
  const handshake = await handshakeWith(client);
  assert.equal(handshake.role, 'offline_search');
  assert.equal(handshake.protocol, 1);
  assert.match(handshake.contract_digest, /^[0-9a-f]{64}$/);
});

test('an explicit argv does not leak into the source worker command line', async () => {
  // `executable = false` must ignore the argv entirely: these flags would make
  // the Python module invocation fail if they were ever appended.
  const client = new WorkerClient(root, python, false, undefined, [
    '--packaged-worker',
    '--data-root',
    'C:/definitely-not-a-staged-runtime',
  ]);
  const handshake = await handshakeWith(client);
  assert.equal(handshake.role, 'offline_search');
});

test('the packaged shape forwards the argv it is given', async (t) => {
  const executable = process.env.NIOH3_PACKAGED_WORKER_EXE;
  if (!executable) {
    // The real packaged pass runs through `npm run test:packaged`; this test
    // only pins the forwarding when a staged binary is offered.
    t.skip('set NIOH3_PACKAGED_WORKER_EXE to pin the packaged forwarding');
    return;
  }
  const stage = resolve(executable, '..', '..');
  const client = new WorkerClient(root, resolve(executable), true, undefined, [
    '--packaged-worker',
    '--data-root',
    resolve(stage, 'worker/runtime/nioh3_scroll_editor/data'),
    '--contract-dir',
    resolve(stage, 'packages/contracts'),
  ]);
  const handshake = await handshakeWith(client);
  assert.equal(handshake.role, 'offline_search');
});
