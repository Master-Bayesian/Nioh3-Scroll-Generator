import type { OperationsApi, ProtectedResult } from './operations-api';
import type { ProtectedJob } from '../../../packages/contracts/protected-responses';
import { operationTerminal } from './public-operation-jobs';

export type OperationRole = 'save' | 'runtime';
export interface OperationState {
  phase: 'idle' | 'submitting' | 'running' | 'completed' | 'failed' | 'interrupted' | 'busy_elsewhere';
  job: ProtectedJob | null;
  result: ProtectedResult | ProtectedJob['result'];
  error: string | null;
  operationId: string | null;
  recovered: boolean;
}

/** UI-independent protected operation observer. Recovery reads; it never replays. */
export class OperationController {
  private state: OperationState = { phase: 'idle', job: null, result: null, error: null, operationId: null, recovered: false };
  private listeners = new Set<() => void>();
  private epoch = 0;
  private disposed = false;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private waiters = new Set<(state: OperationState) => void>();
  constructor(readonly role: OperationRole, private api: Pick<OperationsApi, 'snapshot' | 'cancel' | 'current' | 'execute'>,
    private pollMs = 150) {}
  getSnapshot = () => this.state;
  subscribe = (listener: () => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener); }; };
  private update(values: Partial<OperationState>) {
    this.state = { ...this.state, ...values }; this.listeners.forEach(listener => listener());
    this.waiters.forEach(listener => listener(this.state));
  }
  connect() { this.disposed = false; return this.recover(); }
  async run(operation: () => Promise<ProtectedResult | null>, operationId: string | null = null) {
    if (!this.canStart()) throw new Error('BUSY: recover the protected operation before continuing');
    const epoch = this.epoch + 1;
    await this.start(operation, operationId);
    return this.waitForResult(epoch);
  }
  waitForResult(epoch: number | null = null) {
    return new Promise<OperationState['result']>((resolve, reject) => {
      const check = (state: OperationState) => {
        if (this.disposed || (epoch !== null && epoch !== this.epoch) || ['failed', 'interrupted'].includes(state.phase)) {
          this.waiters.delete(check); reject(new Error(state.error || 'OPERATION_OBSERVER_DISPOSED'));
        } else if (['completed', 'idle'].includes(state.phase)) {
          this.waiters.delete(check); resolve(state.result);
        }
      };
      this.waiters.add(check); check(this.state);
    });
  }
  canStart() { return !this.disposed && ['idle', 'completed', 'failed'].includes(this.state.phase); }
  private interrupted(error: unknown) {
    // A missing response does not prove that a protected write failed or stopped.
    this.update({ phase: 'interrupted', error: String(error) });
  }
  private accept(job: ProtectedJob) {
    if (!job.kind.startsWith(this.role + '.')) throw new Error('OPERATION_ROLE_MISMATCH');
    if (this.state.job && (this.state.job.job_id !== job.job_id || job.sequence < this.state.job.sequence)) return;
    if (this.state.operationId && job.state === 'completed' &&
      (job.kind === 'save.commit' || job.kind === 'save.operation') &&
      (!job.result || !('operation_id' in job.result) || job.result.operation_id !== this.state.operationId)) {
      throw new Error('OPERATION_RECEIPT_MISMATCH');
    }
    if (this.state.operationId && job.state === 'completed' && job.kind.startsWith('runtime.live_add_') &&
      (!job.result || !('live_add' in job.result) || job.result.live_add.operation_id !== this.state.operationId)) {
      throw new Error('OPERATION_RECEIPT_MISMATCH');
    }
    this.update({ job, phase: operationTerminal(job) ? job.state as 'completed' | 'failed' : 'running',
      result: job.result, error: job.error ? `${job.error.code}: ${job.error.message}` : null });
  }
  async start(operation: () => Promise<ProtectedResult | null>, operationId: string | null = null) {
    if (!this.canStart()) return false;
    const epoch = ++this.epoch;
    clearTimeout(this.timer);
    this.update({ phase: 'submitting', job: null, result: null, error: null, operationId, recovered: false });
    try {
      const result = await operation();
      if (epoch !== this.epoch) return false;
      if (result && 'job_id' in result) { this.accept(result); this.schedule(epoch); }
      else this.update({ phase: result === null ? 'idle' : 'completed', result });
      return true;
    } catch (error) { if (epoch === this.epoch) this.interrupted(error); return false; }
  }
  private schedule(epoch: number) {
    if (this.disposed || this.state.phase !== 'running') return;
    const jobId = this.state.job!.job_id;
    this.timer = setTimeout(async () => {
      try {
        const job = await this.api.snapshot(this.role, jobId);
        if (epoch !== this.epoch) return;
        if (job.job_id !== jobId) throw new Error('OPERATION_ID_MISMATCH');
        this.accept(job); this.schedule(epoch);
      } catch (error) { if (epoch === this.epoch && this.state.phase === 'running') this.interrupted(error); }
    }, this.pollMs);
  }
  async cancel() {
    const current = this.state.job;
    if (this.disposed || !current?.cancellable || operationTerminal(current) || this.state.phase !== 'running') return;
    const epoch = this.epoch;
    try {
      const job = await this.api.cancel(this.role, current.job_id);
      if (epoch === this.epoch) {
        if (job.job_id !== current.job_id) throw new Error('OPERATION_ID_MISMATCH');
        this.accept(job);
      }
    } catch (error) { if (epoch === this.epoch && this.state.phase === 'running') this.interrupted(error); }
  }
  async inspectReceipt() {
    const operationId = this.state.operationId;
    if (this.disposed || !operationId || this.state.phase === 'submitting') return;
    const epoch = ++this.epoch;
    clearTimeout(this.timer);
    try {
      const current = await this.api.current(this.role);
      if (epoch !== this.epoch) return;
      if (current.busy) {
        this.update({ phase: 'busy_elsewhere', recovered: true });
        this.timer = setTimeout(() => { if (epoch === this.epoch) void this.inspectReceipt(); }, this.pollMs);
        return;
      }
      this.update({ phase: 'submitting', job: null, error: null, recovered: true });
      const method = this.role === 'save' ? 'save.operation' : 'runtime.live_add_status';
      const result = await this.api.execute(this.role === 'save'
        ? { method: 'save.operation', params: { plan_id: operationId } }
        : { method: 'runtime.live_add_status', params: { operation_id: operationId } });
      if (epoch !== this.epoch) return;
      if (!result || !('job_id' in result) || result.kind !== method) throw new Error('RECEIPT_JOB_EXPECTED');
      this.accept(result); this.schedule(epoch);
    } catch (error) { if (epoch === this.epoch) this.interrupted(error); }
  }
  async recover() {
    if (this.disposed || this.state.phase === 'submitting') return;
    const epoch = ++this.epoch;
    clearTimeout(this.timer);
    this.update({ error: null, recovered: true });
    try {
      // Preserve an existing job identity. An initial/reloaded view can adopt the
      // latest public job, but must not infer it is a particular lost submission.
      if (this.state.job) {
        const id = this.state.job.job_id;
        const job = await this.api.snapshot(this.role, id);
        if (epoch !== this.epoch) return;
        if (job.job_id !== id) throw new Error('OPERATION_ID_MISMATCH');
        this.accept(job); this.schedule(epoch);
      } else {
        const current = await this.api.current(this.role);
        if (epoch !== this.epoch) return;
        if (this.state.operationId) {
          // A lost acknowledgement cannot be correlated to an arbitrary latest job.
          await this.inspectReceipt();
        } else if (current.job) { this.accept(current.job); this.schedule(epoch); }
        else if (current.busy) {
          this.update({ phase: 'busy_elsewhere' });
          this.timer = setTimeout(() => { if (epoch === this.epoch) void this.recover(); }, this.pollMs);
        } else this.update({ phase: this.state.operationId ? 'interrupted' : 'idle' });
      }
    } catch (error) { if (epoch === this.epoch) this.interrupted(error); }
  }
  dispose() {
    this.disposed = true; ++this.epoch; clearTimeout(this.timer);
    this.waiters.forEach(listener => listener(this.state)); this.waiters.clear(); this.listeners.clear();
  }
}
