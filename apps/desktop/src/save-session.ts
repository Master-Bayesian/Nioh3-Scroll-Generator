import type { SaveInventory, SavePlan, SaveReference, OperationReceipt, ProtectedJob } from '../../../packages/contracts/protected-responses';
import type { ProtectedParams } from './protected-client';
import type { OperationsApi, PublicOperation } from './operations-api';
import { OperationController } from './operation-controller';

type Result = NonNullable<ProtectedJob['result']>;
export interface SaveGateway {
  execute(command: PublicOperation, operationId?: string): Promise<Result>;
  prepareInstall(params: Parameters<OperationsApi['prepareInstall']>[0]): Promise<Result>;
}
export function saveGateway(api: Pick<OperationsApi, 'execute' | 'prepareInstall'>, observer: OperationController): SaveGateway {
  return {
    execute: async (command, operationId) => {
      if (command.method === 'save.operation' && !observer.canStart() &&
        observer.getSnapshot().operationId === command.params.plan_id) {
        await observer.inspectReceipt();
        return await observer.waitForResult() as Result;
      }
      return await observer.run(() => api.execute(command), operationId) as Result;
    },
    prepareInstall: async params => await observer.run(() => api.prepareInstall(params)) as Result,
  };
}
export interface SaveSessionState {
  selected: SaveReference | null;
  inventory: SaveInventory | null;
  plan: SavePlan | null;
  planExpiresAt: number | null;
  receipt: OperationReceipt | null;
  uncertainOperationId: string | null;
  refreshedAfterUncertainty: boolean;
  busy: boolean;
  error: string | null;
}
type Edit = ProtectedParams<'save.prepare_edit'>['edits'][number];
export function editFromEntry(entry: SaveInventory['entries'][number]): Edit {
  return structuredClone({ slot_index: entry.slot_index, header: entry.header, effects: entry.effects });
}

/** Save identity, snapshot and reviewed plan are independent of a view's draft. */
export class SaveSession {
  private state: SaveSessionState = { selected: null, inventory: null, plan: null, planExpiresAt: null,
    receipt: null, uncertainOperationId: null, refreshedAfterUncertainty: false, busy: false, error: null };
  private listeners = new Set<() => void>();
  private acknowledgedOutcomes = new Set<string>();
  constructor(private gateway: SaveGateway, private clock = Date.now) {}
  getSnapshot = () => this.state;
  subscribe = (listener: () => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener); }; };
  private update(value: Partial<SaveSessionState>) {
    this.state = { ...this.state, ...value }; this.listeners.forEach(listener => listener());
  }
  private async perform<T>(action: () => Promise<T>) {
    if (this.state.busy) throw new Error('BUSY: save workflow is already running');
    this.update({ busy: true, error: null });
    try { return await action(); }
    catch (error) { this.update({ error: String(error) }); throw error; }
    finally { this.update({ busy: false }); }
  }
  private snapshot() {
    if (!this.state.selected || !this.state.inventory || this.state.selected.save_id !== this.state.inventory.save_id) {
      throw new Error('SAVE_SNAPSHOT_REQUIRED');
    }
    return this.state.inventory;
  }
  private requireReviewedOutcome() {
    if (this.state.uncertainOperationId) throw new Error('UNCERTAIN_OPERATION: inspect receipt and refresh inventory before further changes');
  }
  private async discardPlan() {
    if (this.state.plan) await this.gateway.execute({ method: 'save.discard', params: { plan_id: this.state.plan.plan_id } });
    this.update({ plan: null, planExpiresAt: null });
  }
  private async loadInventory() {
    if (!this.state.selected) throw new Error('SAVE_SELECTION_REQUIRED');
    const result = await this.gateway.execute({ method: 'save.inventory', params: { save_id: this.state.selected.save_id } });
    if (!('snapshot_id' in result) || result.save_id !== this.state.selected.save_id) throw new Error('SAVE_IDENTITY_MISMATCH');
    const history = await this.gateway.execute({ method: 'save.operations', params: { save_id: result.save_id } });
    if (!('operations' in history)) throw new Error('OPERATION_HISTORY_EXPECTED');
    const unresolved = history.operations.find(receipt => receipt.save_id === result.save_id &&
      !this.acknowledgedOutcomes.has(receipt.operation_id) &&
      (receipt.commit_status === 'unknown' || receipt.commit_status === 'executing'));
    const previousUncertainty = this.state.uncertainOperationId;
    this.update({ inventory: structuredClone(result),
      uncertainOperationId: previousUncertainty || unresolved?.operation_id || null,
      receipt: unresolved || this.state.receipt,
      refreshedAfterUncertainty: !!previousUncertainty });
    return result;
  }
  select(reference: SaveReference) {
    return this.perform(async () => {
      this.requireReviewedOutcome(); await this.discardPlan();
      this.update({ selected: structuredClone(reference), inventory: null, receipt: null });
      return this.loadInventory();
    });
  }
  refresh() {
    return this.perform(async () => { await this.discardPlan(); this.update({ inventory: null }); return this.loadInventory(); });
  }
  private async prepare(action: (snapshot: SaveInventory) => Promise<Result>) {
    return this.perform(async () => {
      this.requireReviewedOutcome(); const snapshot = this.snapshot(); await this.discardPlan();
      const result = await action(snapshot);
      if (!('plan_id' in result) || result.save_id !== snapshot.save_id || result.source_sha256 !== snapshot.source_sha256) {
        throw new Error('SAVE_PLAN_IDENTITY_MISMATCH');
      }
      this.update({ plan: structuredClone(result), planExpiresAt: this.clock() + result.expires_in_seconds * 1000, receipt: null });
      return result;
    });
  }
  prepareEdit(edits: ProtectedParams<'save.prepare_edit'>['edits']) {
    const draft = structuredClone(edits);
    return this.prepare(snapshot => this.gateway.execute({ method: 'save.prepare_edit', params: {
      save_id: snapshot.save_id, snapshot_id: snapshot.snapshot_id, edits: draft } }));
  }
  prepareDelete(slots: ProtectedParams<'save.prepare_delete'>['slots']) {
    const selected = structuredClone(slots);
    return this.prepare(snapshot => this.gateway.execute({ method: 'save.prepare_delete', params: {
      save_id: snapshot.save_id, snapshot_id: snapshot.snapshot_id, slots: selected } }));
  }
  prepareRestore(backupId: string) {
    return this.prepare(snapshot => this.gateway.execute({ method: 'save.prepare_restore', params: {
      save_id: snapshot.save_id, snapshot_id: snapshot.snapshot_id, backup_id: backupId } }));
  }
  prepareInstall(candidate: Omit<Parameters<OperationsApi['prepareInstall']>[0], 'save_id' | 'snapshot_id'>) {
    const selected = structuredClone(candidate);
    return this.prepare(snapshot => this.gateway.prepareInstall({ ...selected, save_id: snapshot.save_id, snapshot_id: snapshot.snapshot_id }));
  }
  prepareCart(action:(snapshot:SaveInventory)=>Promise<Result>){return this.prepare(action)}
  commit(reviewedPlanId: string) {
    return this.perform(async () => {
      this.requireReviewedOutcome(); const snapshot = this.snapshot(), plan = this.state.plan;
      if (!plan || plan.plan_id !== reviewedPlanId || plan.save_id !== snapshot.save_id || plan.source_sha256 !== snapshot.source_sha256) {
        throw new Error('REVIEWED_PLAN_REQUIRED');
      }
      if (!this.state.planExpiresAt || this.clock() >= this.state.planExpiresAt) throw new Error('SAVE_PLAN_EXPIRED');
      // Once submitted, an interrupted transport must not make this plan reusable.
      this.update({ plan: null, planExpiresAt: null, inventory: null, uncertainOperationId: plan.plan_id, refreshedAfterUncertainty: false });
      const result = await this.gateway.execute({ method: 'save.commit', params: { plan_id: plan.plan_id } }, plan.plan_id);
      return this.acceptReceipt(result, plan.plan_id);
    });
  }
  private acceptReceipt(result: Result, operationId: string) {
    if (!('operation_id' in result) || result.operation_id !== operationId || result.save_id !== this.state.selected?.save_id) {
      throw new Error('OPERATION_RECEIPT_MISMATCH');
    }
    const uncertain = result.commit_status === 'unknown' || result.commit_status === 'executing';
    this.update({ receipt: structuredClone(result), uncertainOperationId: uncertain ? operationId : null });
    return result;
  }
  recoverReceipt() {
    return this.perform(async () => {
      const id = this.state.uncertainOperationId;
      if (!id) throw new Error('NO_UNCERTAIN_OPERATION');
      const result = await this.gateway.execute({ method: 'save.operation', params: { plan_id: id } }, id);
      return this.acceptReceipt(result, id);
    });
  }
  acknowledgeReviewedUncertainty(operationId: string) {
    if (this.state.busy || this.state.uncertainOperationId !== operationId || !this.state.refreshedAfterUncertainty) {
      throw new Error('REFRESH_AND_REVIEW_REQUIRED');
    }
    // User acknowledgement only. It does not rewrite the durable unknown receipt.
    this.acknowledgedOutcomes.add(operationId);
    this.update({ uncertainOperationId: null, refreshedAfterUncertainty: false });
  }
  discard() { return this.perform(() => this.discardPlan()); }
}
