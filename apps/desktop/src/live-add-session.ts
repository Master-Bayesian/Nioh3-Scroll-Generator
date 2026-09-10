import type { LiveAddOperation, ProtectedJob } from '../../../packages/contracts/protected-responses';
import type { OperationsApi, PublicOperation } from './operations-api';
import { OperationController } from './operation-controller';

type Result = NonNullable<ProtectedJob['result']>;
export interface LiveAddGateway {
  prepare(params: Parameters<OperationsApi['prepareLiveAdd']>[0]): Promise<Result>;
  execute(command: PublicOperation, operationId: string): Promise<Result>;
}
export function liveAddGateway(api: Pick<OperationsApi, 'prepareLiveAdd' | 'execute'>, observer: OperationController): LiveAddGateway {
  return {
    prepare: async params => await observer.run(() => api.prepareLiveAdd(params)) as Result,
    execute: async (command, id) => {
      if (!observer.canStart() && observer.getSnapshot().operationId === id) {
        await observer.inspectReceipt(); await observer.waitForResult();
      }
      return await observer.run(() => api.execute(command), id) as Result;
    },
  };
}
type Reference = { operation_id: string; plan_digest: string };
type Storage = Pick<globalThis.Storage, 'getItem' | 'setItem'>;

/** Final UI supplies controls; this session never replays an interrupted write. */
export class LiveAddSession {
  private state: { operation: LiveAddOperation | null; busy: boolean; uncertain: boolean; error: string | null } = {
    operation: null, busy: false, uncertain: false, error: null,
  };
  private reference: Reference | null = null;
  private listeners = new Set<() => void>();
  constructor(private gateway: LiveAddGateway, private storage?: Storage, private key = 'nioh3-live-add-operation') {}
  getSnapshot = () => this.state;
  subscribe = (listener: () => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener); }; };
  private update(value: Partial<typeof this.state>) {
    this.state = { ...this.state, ...value }; this.listeners.forEach(listener => listener());
  }
  private accept(result: Result, expected: Reference | null) {
    if (!('live_add' in result)) throw new Error('LIVE_ADD_RESULT_EXPECTED');
    const operation = result.live_add;
    if (expected && (operation.operation_id !== expected.operation_id || operation.plan_digest !== expected.plan_digest)) {
      throw new Error('LIVE_ADD_RECEIPT_MISMATCH');
    }
    this.reference = { operation_id: operation.operation_id, plan_digest: operation.plan_digest };
    this.storage?.setItem(this.key, JSON.stringify(this.reference));
    this.update({ operation, uncertain: operation.state === 'uncertain' });
    return operation;
  }
  private async perform(action: () => Promise<LiveAddOperation>) {
    if (this.state.busy) throw new Error('LIVE_ADD_BUSY');
    this.update({ busy: true, error: null });
    try { return await action(); }
    catch (error) { this.update({ error: String(error) }); throw error; }
    finally { this.update({ busy: false }); }
  }
  async connect() {
    const raw = this.storage?.getItem(this.key);
    if (!raw) return null;
    const ref = JSON.parse(raw) as Reference;
    if (!/^[0-9a-f-]{36}$/.test(ref.operation_id) || !/^[0-9a-f]{64}$/.test(ref.plan_digest)) throw new Error('INVALID_LIVE_ADD_REFERENCE');
    this.reference = ref; this.update({ uncertain: true });
    return this.refresh();
  }
  prepare(params: Parameters<OperationsApi['prepareLiveAdd']>[0]) {
    if (this.state.uncertain) throw new Error('LIVE_ADD_OUTCOME_UNKNOWN');
    return this.perform(async () => this.accept(await this.gateway.prepare(params), null));
  }
  execute() {
    const op = this.state.operation;
    if (!op || this.state.uncertain || op.state !== 'prepared' || !op.can_dispatch) throw new Error('LIVE_ADD_REVIEW_REQUIRED');
    const ref = { operation_id: op.operation_id, plan_digest: op.plan_digest };
    return this.perform(async () => {
      this.storage?.setItem(this.key, JSON.stringify(ref));
      this.update({ uncertain: true });
      return this.accept(await this.gateway.execute({ method: 'runtime.live_add_execute', params: ref }, ref.operation_id), ref);
    });
  }
  refresh() { return this.query('runtime.live_add_status'); }
  recover() { return this.query('runtime.live_add_recover'); }
  cancel() {
    if (this.state.uncertain || !this.state.operation?.can_cancel) throw new Error('LIVE_ADD_CANNOT_CANCEL');
    return this.query('runtime.live_add_cancel');
  }
  private query(method: 'runtime.live_add_status' | 'runtime.live_add_recover' | 'runtime.live_add_cancel') {
    const ref = this.reference;
    if (!ref) throw new Error('LIVE_ADD_REFERENCE_REQUIRED');
    return this.perform(async () => this.accept(await this.gateway.execute({ method, params: { operation_id: ref.operation_id } }, ref.operation_id), ref));
  }
}
