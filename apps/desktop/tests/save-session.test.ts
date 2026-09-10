import { test } from 'node:test';
import assert from 'node:assert/strict';
import { SaveSession, type SaveGateway } from '../src/save-session';
import type { SaveReference, SaveInventory, SavePlan, OperationReceipt } from '../../../packages/contracts/protected-responses';

const reference: SaveReference = { save_id: 'a'.repeat(64), account_id: '123', path: 'test/SAVEDATA.BIN', save_slot: 0 };
const inventory: SaveInventory = { save_id: reference.save_id, account_id: '123', snapshot_id: 'b'.repeat(32),
  source_sha256: 'c'.repeat(64), empty_slots: 400, entries: [] };
const plan: SavePlan = { plan_id: 'd'.repeat(32), save_id: reference.save_id, kind: 'delete',
  source_sha256: inventory.source_sha256, expires_in_seconds: 600, preview: { slots: [0] } };
const receipt: OperationReceipt = { operation_id: plan.plan_id, save_id: reference.save_id, commit_status: 'committed', warning: null, details: {} };
function fixture() {
  const calls: string[] = [];
  let failCommit = false, returnedPlan = plan, history: OperationReceipt[] = [], returnedReceipt = receipt;
  const gateway: SaveGateway = {
    prepareInstall: async () => returnedPlan,
    execute: async (command, operationId) => {
      calls.push(command.method);
      if (command.method === 'save.inventory') return structuredClone(inventory);
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
    setPlan: (value: SavePlan) => { returnedPlan = value; },
    setHistory: (value: OperationReceipt[]) => { history = value; },
    setReceipt: (value: OperationReceipt) => { returnedReceipt = value; } };
}

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

test('unrelated receipts cannot resolve the submitted operation', async () => {
  const f = fixture(), session = new SaveSession(f.gateway);
  await session.select(reference); await session.prepareDelete([0]);
  f.setReceipt({ ...receipt, operation_id: 'e'.repeat(32) });
  await assert.rejects(session.commit(plan.plan_id), /OPERATION_RECEIPT_MISMATCH/);
  assert.equal(session.getSnapshot().uncertainOperationId, plan.plan_id);
});
