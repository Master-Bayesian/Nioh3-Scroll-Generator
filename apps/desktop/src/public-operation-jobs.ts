import type { ProtectedJob } from '../../../packages/contracts/protected-responses';

/** Job recovery must not expose private templates, cache JSON, or raw records. */
const publicKinds = new Set<ProtectedJob['kind']>([
  'runtime.count_prepare','runtime.count_execute','runtime.count_status','runtime.count_recover',
  'save.recycle_backups', 'save.discover', 'save.register', 'save.inventory', 'save.prepare_edit', 'save.prepare_delete',
  'save.prepare_install_many','runtime.live_batch_prepare','runtime.live_batch_execute','runtime.live_batch_status','runtime.live_batch_cancel',
  'save.prepare_install', 'save.backups', 'save.prepare_restore', 'save.discard', 'save.commit',
  'save.operation', 'save.operations', 'runtime.generate', 'runtime.search', 'runtime.capture_grace',
  'runtime.start_override', 'runtime.stop_override',
  'runtime.live_add_prepare', 'runtime.live_add_execute', 'runtime.live_add_status',
  'runtime.live_add_recover', 'runtime.live_add_cancel',
]);
export const operationTerminal = (job: ProtectedJob) => job.state === 'completed' || job.state === 'failed';
export function requirePublicJob(job: ProtectedJob): ProtectedJob {
  if (!publicKinds.has(job.kind)) throw new Error('PRIVATE_OPERATION_JOB');
  return job;
}
export function publicCurrentJob(job: ProtectedJob | null) {
  return { job: job && publicKinds.has(job.kind) ? job : null, busy: !!job && !operationTerminal(job) };
}
