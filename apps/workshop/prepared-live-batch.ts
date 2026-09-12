import type { LiveBatch } from "../../packages/contracts/protected-responses";

/** A view owns only an unsubmitted plan; execution transfers ownership to recovery. */
export class PreparedLiveBatchOwner {
  private prepared: LiveBatch | null = null;
  closed = false;

  constructor(
    private cancel: (batch: LiveBatch) => Promise<LiveBatch>,
    private clearMarker: (batch: LiveBatch) => void,
  ) {}

  async adopt(batch: LiveBatch) {
    if (batch.state !== "prepared") throw new Error("PREPARED_LIVE_BATCH_EXPECTED");
    this.prepared = batch;
    if (this.closed) await this.discard();
    return !this.closed;
  }

  beginExecution(batchId: string) {
    if (this.closed || this.prepared?.batch_id !== batchId)
      throw new Error("OWNED_PREPARED_BATCH_REQUIRED");
    this.prepared = null;
  }

  async discard() {
    const batch = this.prepared;
    if (!batch) return;
    this.prepared = null;
    const receipt = await this.cancel(batch);
    if (receipt.batch_id !== batch.batch_id || receipt.plan_digest !== batch.plan_digest ||
        receipt.state !== "cancelled" || receipt.verified_count !== 0)
      throw new Error("LIVE_BATCH_CANCEL_RECEIPT_MISMATCH");
    this.clearMarker(batch);
  }

  async close() {
    this.closed = true;
    await this.discard();
  }
}
