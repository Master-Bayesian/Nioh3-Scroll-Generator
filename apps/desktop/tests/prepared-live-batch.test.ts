import assert from "node:assert/strict";
import test from "node:test";
import { PreparedLiveBatchOwner } from "../../workshop/prepared-live-batch";
import type { LiveBatch } from "../../../packages/contracts/protected-responses";

const batch: LiveBatch = {
  batch_id: "batch-one", plan_digest: "digest-one", state: "prepared", count: 1,
  verified_count: 0, child_operation_ids: [],
};

test("closing an unconfirmed live plan cancels it and clears its recovery marker", async () => {
  const cancelled: string[] = [], cleared: string[] = [];
  const owner = new PreparedLiveBatchOwner(async value => {
    cancelled.push(value.batch_id);
    return { ...value, state: "cancelled" };
  }, value => cleared.push(value.batch_id));
  await owner.adopt(batch);
  await owner.close();
  assert.deepEqual(cancelled, [batch.batch_id]);
  assert.deepEqual(cleared, [batch.batch_id]);
});

test("a preparation response arriving after the dialog closes is also cancelled", async () => {
  const cancelled: string[] = [];
  const owner = new PreparedLiveBatchOwner(async value => {
    cancelled.push(value.batch_id);
    return { ...value, state: "cancelled" };
  }, () => {});
  await owner.close();
  assert.equal(await owner.adopt(batch), false);
  assert.deepEqual(cancelled, [batch.batch_id]);
});

test("closing after execution submission preserves ownership and recovery", async () => {
  const owner = new PreparedLiveBatchOwner(async () => {
    assert.fail("The view must never cancel an executing or uncertain batch");
  }, () => assert.fail("The execution marker must remain available for recovery"));
  await owner.adopt(batch);
  owner.beginExecution(batch.batch_id);
  await owner.close();
});

test("busy or interrupted cleanup retains the recovery marker", async () => {
  const owner = new PreparedLiveBatchOwner(async () => {
    throw Error("LIVE_BATCH_CLEANUP_DEFERRED_BUSY");
  }, () => assert.fail("A missing cancellation acknowledgement cannot clear recovery"));
  await owner.adopt(batch);
  await assert.rejects(owner.close(), /CLEANUP_DEFERRED_BUSY/);
});

test("unmatched and uncertain cancellation receipts cannot clear recovery", async () => {
  for (const receipt of [
    { ...batch, batch_id: "another-batch", state: "cancelled" as const },
    { ...batch, plan_digest: "another-plan", state: "cancelled" as const },
    { ...batch, state: "uncertain" as const },
    { ...batch, state: "cancelled" as const, verified_count: 1 },
  ]) {
    const owner = new PreparedLiveBatchOwner(async () => receipt,
      () => assert.fail("Only the verified cancellation of this unexecuted plan may clear recovery"));
    await owner.adopt(batch);
    await assert.rejects(owner.close(), /CANCEL_RECEIPT_MISMATCH/);
  }
});

test("a closed view cannot start execution", async () => {
  const owner = new PreparedLiveBatchOwner(async value => ({ ...value, state: "cancelled" }), () => {});
  await owner.adopt(batch);
  await owner.close();
  assert.throws(() => owner.beginExecution(batch.batch_id), /OWNED_PREPARED_BATCH_REQUIRED/);
});
