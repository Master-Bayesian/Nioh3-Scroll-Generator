import type { OperationsApi, ProtectedResult } from './operations-api';
import type { ProtectedJob } from '../../../packages/contracts/protected-responses';
import { operationTerminal } from './public-operation-jobs';

export type OperationRole = 'save' | 'runtime';
const JOB_GONE = 'Unknown operation job';
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
  /** Every pending poll owns an epoch/revision lease; cancel refresh retires old reads. */
  private pollRevision = 0;
  private disposed = false;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private waiters = new Set<(state: OperationState) => void>();
  constructor(readonly role: OperationRole, private api: Pick<OperationsApi, 'snapshot' | 'cancel' | 'current' | 'execute'>,
    private pollMs = 150, private startWaitMs = 10_000) {}
  getSnapshot = () => this.state;
  subscribe = (listener: () => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener); }; };
  private update(values: Partial<OperationState>) {
    const state = { ...this.state, ...values };
    this.state = state;
    // A completed request owns this immutable outcome even when a subscriber
    // synchronously starts the next operation during notification.
    this.waiters.forEach(listener => listener(state));
    this.listeners.forEach(listener => listener());
  }
  connect() {
    if (this.disposed && (this.state.phase === 'running' || this.state.phase === 'submitting'))
      this.update({ phase: 'interrupted', error: 'OPERATION_OBSERVER_RECONNECTED' });
    this.disposed = false;
    return this.recover();
  }
  async run(operation: () => Promise<ProtectedResult | null>, operationId: string | null = null, options: { signal?: AbortSignal } = {}) {
    if (options.signal?.aborted) throw new Error('OPERATION_CANCELLED_BEFORE_SUBMISSION');
    // A refused or unanswered submission leaves the observer interrupted. That
    // must not lock every later operation: reading the worker's current state
    // (recovery never replays) settles it, and only real ownership still refuses.
    if (this.state.phase === 'interrupted') await this.recover();
    // A background read (an inventory refresh, a backup list) may still be
    // settling when the user acts. Queue behind it instead of refusing.
    if (!this.canStart()) await this.whenStartable(this.startWaitMs, options.signal);
    if (options.signal?.aborted) throw new Error('OPERATION_CANCELLED_BEFORE_SUBMISSION');
    if (!this.canStart()) throw new Error('BUSY: recover the protected operation before continuing');
    // Watch this operation's epoch from the moment it starts. An operation that answers without a job completes
    // inside start(), and an action queued behind it starts right then; watching only after start() returned would
    // see that newer epoch and reject a call that had succeeded.
    const started = this.start(operation, operationId);
    const ownerEpoch = this.epoch;
    const result = this.waitForResult(ownerEpoch);
    result.catch(() => {});
    let abortDispatch: "waiting_for_job" | "sent" = "waiting_for_job";
    const cancelOwned = async () => {
      if (ownerEpoch !== this.epoch || abortDispatch === "sent" || !this.state.job) return;
      abortDispatch = "sent";
      await this.cancel();
    };
    const onAbort = () => { void cancelOwned(); };
    options.signal?.addEventListener('abort', onAbort);
    try {
      await started;
      if (options.signal?.aborted) await cancelOwned();
      // An abort request does not prove cleanup. Keep the owner until a real
      // terminal snapshot (or an explicit interrupted/unknown outcome).
      return await result;
    } finally { options.signal?.removeEventListener('abort', onAbort); }
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
  /** Resolve once a new operation may start, or after `timeoutMs`. It never starts one. */
  private whenStartable(timeoutMs: number, signal?: AbortSignal) {
    return new Promise<void>(resolve => {
      const done = () => { clearTimeout(timer); signal?.removeEventListener('abort', done); this.waiters.delete(check); resolve(); };
      const check = () => { if (this.disposed || this.canStart() || signal?.aborted) done(); };
      const timer = setTimeout(done, timeoutMs);
      signal?.addEventListener('abort', done, { once: true });
      this.waiters.add(check); check();
    });
  }
  private interrupted(error: unknown) {
    // A missing response does not prove that a protected write failed or stopped.
    clearTimeout(this.timer); ++this.pollRevision;
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
    if (operationTerminal(job)) { clearTimeout(this.timer); ++this.pollRevision; }
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
    if (this.disposed || epoch !== this.epoch || this.state.phase !== 'running') return;
    const jobId = this.state.job!.job_id;
    clearTimeout(this.timer);
    const revision = ++this.pollRevision;
    this.timer = setTimeout(async () => {
      if (this.disposed || epoch !== this.epoch || revision !== this.pollRevision || this.state.phase !== 'running') return;
      try {
        const job = await this.api.snapshot(this.role, jobId);
        if (epoch !== this.epoch || revision !== this.pollRevision) return;
        if (job.job_id !== jobId) throw new Error('OPERATION_ID_MISMATCH');
        this.accept(job); this.schedule(epoch);
      } catch (error) { if (epoch === this.epoch && revision === this.pollRevision && this.state.phase === 'running') this.interrupted(error); }
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
        this.schedule(epoch);
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
    // A job this observer is already polling needs no recovery, and restarting
    // the epoch would reject the caller still waiting for it.
    if (this.disposed || this.state.phase === 'submitting' || this.state.phase === 'running') return;
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
      } else await this.settleWithoutJob(epoch);
    } catch (error) {
      if (epoch !== this.epoch) return;
      if (!this.state.job || !String(error).includes(JOB_GONE)) { this.interrupted(error); return; }
      // A replaced worker no longer knows the job. Its identity is dead; the receipt or the current state decides.
      this.update({ job: null });
      await this.settleWithoutJob(epoch).catch(next => { if (epoch === this.epoch) this.interrupted(next); });
    }
  }
  private async settleWithoutJob(epoch: number) {
    const current = await this.api.current(this.role);
    if (epoch !== this.epoch) return;
    if (this.state.operationId) {
      // A lost acknowledgement cannot be correlated to an arbitrary latest job.
      await this.inspectReceipt();
    } else if (current.job) { this.accept(current.job); this.schedule(epoch); }
    else if (current.busy) {
      this.update({ phase: 'busy_elsewhere' });
      this.timer = setTimeout(() => { if (epoch === this.epoch) void this.recover(); }, this.pollMs);
    } else this.update({ phase: 'idle' });
  }
  dispose() {
    this.disposed = true; ++this.epoch; clearTimeout(this.timer);
    this.waiters.forEach(listener => listener(this.state)); this.waiters.clear(); this.listeners.clear();
  }
}
