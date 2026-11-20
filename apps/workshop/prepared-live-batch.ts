import type { LiveBatch } from "../../packages/contracts/protected-responses";

type Ownership =
  | { kind: "empty" }
  | { kind: "prepared" | "retry"; batch: LiveBatch }
  | { kind: "cancelling"; batch: LiveBatch; completion: Promise<void> };

export function verifyCancellation(batch: LiveBatch, receipt: LiveBatch) {
  if (receipt.batch_id !== batch.batch_id || receipt.plan_digest !== batch.plan_digest ||
      receipt.state !== "cancelled" || receipt.verified_count !== 0)
    throw new Error("LIVE_BATCH_CANCEL_RECEIPT_MISMATCH");
}

/** The view owns preparation and cancellation until a verified receipt or execution handoff. */
export class PreparedLiveBatchOwner {
  private owner: Ownership = { kind: "empty" };
  closed = false;

  constructor(
    private cancel: (batch: LiveBatch) => Promise<LiveBatch>,
    private clearMarker: (batch: LiveBatch) => void,
  ) {}

  async adopt(batch: LiveBatch) {
    if (batch.state !== "prepared" || this.owner.kind !== "empty") throw new Error("LIVE_BATCH_ADOPTION_CONFLICT");
    this.owner = { kind: "prepared", batch };
    if (this.closed) await this.discard();
    return !this.closed;
  }

  beginExecution(batchId: string, persist?: () => void) {
    if (this.closed || this.owner.kind !== "prepared" || this.owner.batch.batch_id !== batchId)
      throw new Error("OWNED_PREPARED_BATCH_REQUIRED");
    persist?.();
    this.owner = { kind: "empty" };
  }

  async discard() {
    if (this.owner.kind === "empty") return;
    if (this.owner.kind === "cancelling") return this.owner.completion;
    const batch = this.owner.batch;
    const completion = Promise.resolve().then(async () => {
      try {
        verifyCancellation(batch, await this.cancel(batch));
        this.clearMarker(batch);
        this.owner = { kind: "empty" };
      } catch (error) {
        this.owner = { kind: "retry", batch };
        throw error;
      }
    });
    this.owner = { kind: "cancelling", batch, completion };
    return completion;
  }

  async close() {
    this.closed = true;
    await this.discard();
  }
}
