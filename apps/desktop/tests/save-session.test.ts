import { test } from 'node:test';
import assert from 'node:assert/strict';
import { SaveSession, saveGateway, type SaveGateway } from '../src/save-session';
import { OperationController } from '../src/operation-controller';
import type { OperationsApi } from '../src/operations-api';
import type { SaveReference, SaveInventory, SavePlan, ScrollAudit, OperationReceipt, ProtectedJob } from '../../../packages/contracts/protected-responses';

const reference: SaveReference = { save_id: 'a'.repeat(64), account_id: '123', path: 'test/SAVEDATA.BIN', save_slot: 0 };
const inventory: SaveInventory = { save_id: reference.save_id, account_id: '123', snapshot_id: 'b'.repeat(32),
  source_sha256: 'c'.repeat(64), empty_slots: 400, entries: [] };
const plan: SavePlan = { plan_id: 'd'.repeat(32), save_id: reference.save_id, kind: 'delete',
  source_sha256: inventory.source_sha256, expires_in_seconds: 600, preview: { slots: [0] } };
const receipt: OperationReceipt = { operation_id: plan.plan_id, save_id: reference.save_id, commit_status: 'committed', warning: null, details: {} };
function fixture() {
  const calls: string[] = [];
  let failCommit = false, returnedPlan = plan, history: OperationReceipt[] = [], returnedReceipt = receipt, returnAuditAsInventory = false;
  const audit: ScrollAudit = {
    save_id: reference.save_id,
    snapshot_id: inventory.snapshot_id,
    source_sha256: inventory.source_sha256,
    status: 'insufficient_data',
    coverage_scope: 'generated_effect_projection',
    context: {
      product_version: 'test', game_profile: 'pc', game_file_version: '2.0.2.0',
      versioned_resource_dir: 'resources/2.0.2.0', bundle_digest: null, versioned_digest: null,
      resources_digest: 'e'.repeat(64), algorithm_version: 'test', policy_version: 'test',
      context_digest: 'f'.repeat(64), legacy_context_digest: null, production_authority: false,
      seed_accelerator_abi: null, seed_accelerator_build_id: null,
    },
    rows: [],
  };
  const gateway: SaveGateway = {
    prepareInstall: async () => returnedPlan,
    execute: async (command, operationId) => {
      calls.push(command.method);
      if (command.method === 'save.inventory') return returnAuditAsInventory ? structuredClone(audit) : structuredClone(inventory);
      if (command.method === 'save.operations') return { operations: history };
      if (command.method === 'save.discard') return { discarded: true };
      if (command.method.startsWith('save.prepare_')) return structuredClone(returnedPlan);
      if (command.method === 'save.commit') {
        assert.equal(operationId, plan.plan_id);
        if (failCommit) throw new Error('Lost commit acknowledgement');
        return returnedReceipt;
      }
      if (command.method === 'save.operation') return returnedReceipt;
      throw new Error('Unexpected command');
    },
  };
  return { gateway, calls, fail: () => { failCommit = true; },
    returnAudit: () => { returnAuditAsInventory = true; },
    setPlan: (value: SavePlan) => { returnedPlan = value; },
    setHistory: (value: OperationReceipt[]) => { history = value; },
    setReceipt: (value: OperationReceipt) => { returnedReceipt = value; } };
}

test('a scroll audit payload cannot replace the inventory snapshot', async () => {
  const f = fixture(), session = new SaveSession(f.gateway);
  await session.select(reference);
  f.returnAudit();
  await assert.rejects(session.refresh(), /SAVE_INVENTORY_EXPECTED/);
  assert.equal(session.getSnapshot().inventory, null);
  assert.equal(session.getSnapshot().selected?.save_id, reference.save_id);
});

test('save changes require the exact reviewed plan, and commit invalidates its snapshot', async () => {
  const f = fixture(), session = new SaveSession(f.gateway);
  await session.select(reference); await session.prepareDelete([0]);
  assert.equal(f.calls.includes('save.commit'), false);
  await assert.rejects(session.commit('different plan'), /REVIEWED_PLAN_REQUIRED/);
  const result = await session.commit(plan.plan_id);
  assert.equal(result.commit_status, 'committed');
  assert.equal(session.getSnapshot().inventory, null); assert.equal(session.getSnapshot().plan, null);
  await assert.rejects(session.commit(plan.plan_id), /SAVE_SNAPSHOT_REQUIRED/);
  assert.equal(f.calls.filter(method => method === 'save.commit').length, 1);
});

test('refresh discards a previous review and mismatched or expired plans cannot be committed', async () => {
  const f = fixture(); let now = 100;
  const session = new SaveSession(f.gateway, () => now);
  await session.select(reference); await session.prepareDelete([0]); await session.refresh();
  assert.equal(session.getSnapshot().plan, null); assert.ok(f.calls.includes('save.discard'));
  f.setPlan({ ...plan, source_sha256: 'f'.repeat(64) });
  await assert.rejects(session.prepareDelete([0]), /SAVE_PLAN_IDENTITY_MISMATCH/);
  f.setPlan(plan); await session.prepareDelete([0]); now += 600000;
  await assert.rejects(session.commit(plan.plan_id), /SAVE_PLAN_EXPIRED/);
  assert.equal(f.calls.includes('save.commit'), false);
});

test('lost commit acknowledgement blocks new writes and recovers without replay', async () => {
  const f = fixture(), session = new SaveSession(f.gateway);
  await session.select(reference); await session.prepareDelete([0]); f.fail();
  await assert.rejects(session.commit(plan.plan_id), /Lost commit/);
  assert.equal(session.getSnapshot().uncertainOperationId, plan.plan_id);
  await assert.rejects(session.prepareDelete([0]), /UNCERTAIN_OPERATION/);
  await session.recoverReceipt();
  assert.equal(session.getSnapshot().uncertainOperationId, null);
  assert.equal(f.calls.filter(method => method === 'save.commit').length, 1);
  assert.equal(f.calls.at(-1), 'save.operation');
});

test('a restarted view surfaces unknown history and requires refresh before explicit acknowledgement', async () => {
  const f = fixture(), session = new SaveSession(f.gateway);
  f.setHistory([{ ...receipt, commit_status: 'unknown' }]);
  await session.select(reference);
  assert.equal(session.getSnapshot().uncertainOperationId, plan.plan_id);
  assert.throws(() => session.acknowledgeReviewedUncertainty(plan.plan_id), /REFRESH_AND_REVIEW_REQUIRED/);
  await session.refresh(); session.acknowledgeReviewedUncertainty(plan.plan_id);
  assert.equal(session.getSnapshot().receipt!.commit_status, 'unknown', 'Acknowledgement does not falsify the durable receipt');
  await session.refresh();
  assert.equal(session.getSnapshot().uncertainOperationId, null, 'An acknowledged outcome stays reviewed within this session');
  await session.prepareDelete([0]); assert.equal(f.calls.includes('save.commit'), false);
});

test('an acknowledged unprovable operation is durable and unlocks the save', async () => {
  const f = fixture(), session = new SaveSession(f.gateway);
  const unknown: OperationReceipt = { ...receipt, commit_status: 'unknown' };
  f.setHistory([unknown]); f.setReceipt(unknown);
  await session.select(reference);
  // Checking again cannot prove it, so the save stays locked.
  await session.recoverReceipt();
  assert.equal(session.getSnapshot().uncertainOperationId, plan.plan_id);
  await assert.rejects(session.prepareDelete([0]), /UNCERTAIN_OPERATION/);
  // The host records the user's acknowledgement; the history now reads it back.
  const acknowledged: OperationReceipt = { ...receipt, commit_status: 'acknowledged' };
  f.setReceipt(acknowledged); f.setHistory([acknowledged]);
  const settled = await session.acknowledgeOperation();
  assert.equal(settled.commit_status, 'acknowledged');
  assert.equal(session.getSnapshot().uncertainOperationId, null);
  assert.ok(session.getSnapshot().inventory, 'the save is re-read after acknowledgement');
  // A new session (a restart) is not locked again by the acknowledged record.
  const restarted = new SaveSession(f.gateway);
  await restarted.select(reference);
  assert.equal(restarted.getSnapshot().uncertainOperationId, null);
  await restarted.prepareDelete([0]);
  assert.equal(f.calls.includes('save.commit'), false);
});

test('unrelated receipts cannot resolve the submitted operation', async () => {
  const f = fixture(), session = new SaveSession(f.gateway);
  await session.select(reference); await session.prepareDelete([0]);
  f.setReceipt({ ...receipt, operation_id: 'e'.repeat(32) });
  await assert.rejects(session.commit(plan.plan_id), /OPERATION_RECEIPT_MISMATCH/);
  assert.equal(session.getSnapshot().uncertainOperationId, plan.plan_id);
});

function observedCommitFailure(error: NonNullable<ProtectedJob['error']>) {
  const calls: string[] = [];
  let returnedPlan = plan, serial = 0;
  const failed: ProtectedJob = { job_id: 'e'.repeat(32), kind: 'save.commit', state: 'failed',
    sequence: 2, cancellable: false, progress: null, result: null, error };
  const completed = (kind: ProtectedJob['kind'], result: ProtectedJob['result']): ProtectedJob => ({
    job_id: (++serial).toString(16).padStart(32, '0'), kind, state: 'completed',
    sequence: 2, cancellable: false, progress: null, result, error: null,
  });
  const api: Pick<OperationsApi, 'execute' | 'prepareInstall' | 'current' | 'snapshot' | 'cancel'> = {
    execute: async command => {
      calls.push(command.method);
      switch (command.method) {
        case 'save.inventory': return completed(command.method, structuredClone(inventory));
        case 'save.operations': return completed(command.method, { operations: [] });
        case 'save.prepare_delete': return completed(command.method, structuredClone(returnedPlan));
        case 'save.discard': return completed(command.method, { discarded: true });
        case 'save.commit': return failed;
        case 'save.operation': return completed(command.method, structuredClone(receipt));
        default: throw new Error('Unexpected command: ' + command.method);
      }
    },
    prepareInstall: async () => completed('save.prepare_install', structuredClone(returnedPlan)),
    current: async () => ({ job: null, busy: false }),
    snapshot: async () => failed,
    cancel: async () => failed,
  };
  const observer = new OperationController('save', api);
  // The renderer's TTL has not expired when the host rejects the plan.
  const session = new SaveSession(saveGateway(api, observer), () => 100);
  return { calls, session, observer, setPlan: (value: SavePlan) => { returnedPlan = value; } };
}

for (const code of ['OPERATION_REJECTED', 'OPERATION_FAILED']) {
  test('host-expired plan releases the provisional fence (' + code + ')', async () => {
    const f = observedCommitFailure({ code, message: 'Plan expired; prepare a new plan' });
    try {
      await f.session.select(reference); await f.session.prepareDelete([0]);
      await assert.rejects(f.session.commit(plan.plan_id), /Plan expired; prepare a new plan/);
      assert.equal(f.session.getSnapshot().uncertainOperationId, null);
      assert.equal(f.session.getSnapshot().plan, null, 'the rejected plan cannot be reused');
      assert.equal(f.session.getSnapshot().inventory, null, 'a fresh snapshot is required');
      assert.equal(f.session.getSnapshot().receipt, null, 'a refusal does not invent a receipt');
      await f.session.refresh();
      f.setPlan({ ...plan, plan_id: 'f'.repeat(32) });
      await f.session.prepareDelete([0]);
      assert.equal(f.session.getSnapshot().plan?.plan_id, 'f'.repeat(32));
      assert.equal(f.calls.filter(method => method === 'save.commit').length, 1);
      assert.equal(f.calls.includes('save.operation'), false, 'no nonexistent ledger is queried');
    } finally { f.observer.dispose(); }
  });
}

for (const message of [
  'Connection closed; diagnostic path contains Plan expired; prepare a new plan',
  'Plan expired; prepare a new plan; commit outcome unknown',
]) {
  test('an expiry lookalike retains receipt recovery: ' + message, async () => {
    const f = observedCommitFailure({ code: 'OPERATION_FAILED', message });
    try {
      await f.session.select(reference); await f.session.prepareDelete([0]);
      await assert.rejects(f.session.commit(plan.plan_id));
      assert.equal(f.session.getSnapshot().uncertainOperationId, plan.plan_id);
      await assert.rejects(f.session.prepareDelete([0]), /UNCERTAIN_OPERATION/);
      await f.session.recoverReceipt();
      assert.equal(f.session.getSnapshot().uncertainOperationId, null);
      assert.equal(f.calls.at(-1), 'save.operation');
      assert.equal(f.calls.filter(method => method === 'save.commit').length, 1);
    } finally { f.observer.dispose(); }
  });
}
