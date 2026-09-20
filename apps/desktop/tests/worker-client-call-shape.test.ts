/**
 * The WorkerClient call shape for both launch modes.
 *
 * The source worker refuses to start unless exactly one explicit identity is
 * named (`--game-file-version <A.B.C.D>` for production, `--legacy-test-context`
 * for tests), and the client forwards its `argv` to that source launch
 * unchanged. Three shapes are worth pinning with real launches rather than
 * parser checks:
 *
 * - an omitted source argv must fail closed on the worker's exit-2 refusal and
 *   publish the identity guidance instead of a job surface;
 * - an explicit `--legacy-test-context` argv must reach the source worker and
 *   answer `handshake` with the offline-search role and the contract digest
 *   recomputed from disk;
 * - a packaged executable's argv is its whole command line, so the forwarding
 *   the packaged parity gate depends on stays unchanged.
 *
 * A worker that started with the wrong arguments could not answer `handshake`,
 * so every assertion below is a real launch and handshake.
 */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { resolve } from 'node:path';
import { WorkerClient } from '../src/worker-client';

const root = resolve('.');
const python = process.env.NIOH3_PYTHON || 'python';
const pause = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));

/**
 * The exact game file version a packaged worker must be launched with.
 *
 * Required rather than defaulted: a staged worker launched without an identity
 * refuses to start, so the forwarding this test pins would never be exercised.
 */
function packagedGameFileVersion(): string {
  const raw = process.env.NIOH3_PARITY_GAME_FILE_VERSION ?? '';
  if (!/^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$/.test(raw)) {
    throw new Error('Set NIOH3_PARITY_GAME_FILE_VERSION to the packaged worker\'s exact four-part game file version');
  }
  return raw;
}

async function handshakeWith(client: WorkerClient) {
  try {
    return await client.handshake();
  } finally {
    await client.close();
  }
}

test('an omitted source argv fails closed with the identity refusal', async () => {
  const client = new WorkerClient(root, python);
  try {
    // Let the refusal land before asking, so the reported failure is the
    // worker's own exit code rather than a write racing a dead child.
    const deadline = Date.now() + 10_000;
    while (client.diagnostics().connection !== 'unavailable' && Date.now() < deadline) await pause(20);
    assert.equal(client.diagnostics().connection, 'unavailable', 'the identity-free source launch kept running');
    await assert.rejects(client.handshake(), /WORKER_EXITED: 2/);
    while (!/refusing to start/.test(client.stderr.join('')) && Date.now() < deadline) await pause(20);
    const guidance = client.stderr.join('');
    assert.match(guidance, /refusing to start/);
    assert.match(guidance, /--game-file-version/);
    assert.match(guidance, /--legacy-test-context/);
  } finally { await client.close(); }
});

test('the legacy test identity is forwarded to the source worker', async () => {
  const client = new WorkerClient(root, python, false, undefined, ['--legacy-test-context']);
  const handshake = await handshakeWith(client);
  assert.equal(handshake.role, 'offline_search');
  assert.equal(handshake.protocol, 1);
  assert.match(handshake.contract_digest, /^[0-9a-f]{64}$/);
  // Only a forwarded `--legacy-test-context` publishes the pre-version opt-in.
  assert.equal(handshake.context.production_authority, false);
  assert.match(handshake.context.context_digest, /^[0-9a-f]{64}$/);
});

test('the packaged shape forwards the argv it is given', async (t) => {
  const executable = process.env.NIOH3_PACKAGED_WORKER_EXE;
  if (!executable) {
    // The real packaged pass runs `apps/desktop/tests/packaged-parity.ts`; this
    // test only pins the forwarding when a staged binary is offered.
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
    '--game-file-version',
    packagedGameFileVersion(),
  ]);
  const handshake = await handshakeWith(client);
  assert.equal(handshake.role, 'offline_search');
});
