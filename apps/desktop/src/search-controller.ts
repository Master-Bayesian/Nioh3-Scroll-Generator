import type { DesktopApi } from './api';
import type { Handshake, JobSnapshot } from '../../../packages/contracts/responses';
import type { StartParams } from './worker-client';

export const terminal = (job: JobSnapshot) => ['completed', 'cancelled', 'failed'].includes(job.state);
export interface SearchState {
  handshake: Handshake | null;
  job: JobSnapshot | null;
  submitted: StartParams | null;
  error: string | null;
  busy: boolean;
}

/** Framework-independent session state. The form owns its separate draft. */
export class SearchController {
  private state: SearchState = { handshake: null, job: null, submitted: null, error: null, busy: false };
  private listeners = new Set<() => void>();
  private epoch = 0;
  private timer: ReturnType<typeof setTimeout> | undefined;
  constructor(private api: DesktopApi) {}
  getSnapshot = () => this.state;
  subscribe = (listener: () => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener); }; };
  private update(values: Partial<SearchState>) {
    this.state = { ...this.state, ...values }; this.listeners.forEach(listener => listener());
  }
  async connect(restart = false) {
    const epoch = ++this.epoch;
    clearTimeout(this.timer);
    this.update({ busy: true, error: null, job: null, submitted: null, handshake: null });
    try {
      const handshake = await (restart ? this.api.restartWorker() : this.api.handshake());
      if (epoch !== this.epoch) return;
      const current = await this.api.currentSearch();
      if (epoch !== this.epoch) return;
      if (current.job && current.job.context_digest !== handshake.context.context_digest) throw new Error('CONTEXT_MISMATCH');
      this.update({ handshake, busy: false, job: current.job, submitted: current.submitted });
      this.schedule(epoch);
    } catch (error) { if (epoch === this.epoch) this.update({ error: String(error), busy: false }); }
  }
  async start(params: StartParams) {
    if (this.state.busy || !this.state.handshake || (this.state.job && !terminal(this.state.job))) return;
    const submitted = structuredClone(params);
    const epoch = ++this.epoch;
    this.update({ busy: true, error: null, submitted, job: null });
    try {
      const job = await this.api.startSearch(submitted);
      if (epoch !== this.epoch) return;
      this.update({ job, busy: false });
      this.schedule(epoch);
    } catch (error) { if (epoch === this.epoch) this.update({ error: String(error), busy: false }); }
  }
  private accept(job: JobSnapshot) {
    if (this.state.job?.job_id !== job.job_id || job.sequence < this.state.job.sequence) return;
    this.update({ job });
  }
  private unavailable(error: unknown) {
    const message = String(error);
    this.update({ error: message, busy: false, handshake: null,
      job: this.state.job ? { ...this.state.job, state: 'failed', stop_reason: 'error', resume_token: null,
        error: { code: 'WORKER_UNAVAILABLE', message } } : null });
  }
  private schedule(epoch: number) {
    if (!this.state.job || terminal(this.state.job)) return;
    this.timer = setTimeout(async () => {
      try {
        const job = await this.api.snapshot(this.state.job!.job_id);
        if (epoch !== this.epoch) return;
        this.accept(job); this.schedule(epoch);
      } catch (error) { if (epoch === this.epoch) this.unavailable(error); }
    }, 150);
  }
  async cancel() {
    if (!this.state.job || terminal(this.state.job)) return;
    const epoch = this.epoch;
    try { const job = await this.api.cancelSearch(this.state.job.job_id); if (epoch === this.epoch) this.accept(job); }
    catch (error) { if (epoch === this.epoch) this.unavailable(error); }
  }
  resume() {
    if (!this.state.submitted || !this.state.job?.resume_token) return;
    return this.start({ ...this.state.submitted, resume_token: this.state.job.resume_token });
  }
  dispose() { ++this.epoch; clearTimeout(this.timer); this.listeners.clear(); }
}
