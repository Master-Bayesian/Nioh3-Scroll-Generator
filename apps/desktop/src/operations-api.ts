import type { ProtectedParams } from './protected-client';
import type { ProtectedResponse, ProtectedJob } from '../../../packages/contracts/protected-responses';
export type ProtectedResult = Extract<ProtectedResponse, { ok: true }>['result'];

// The renderer cannot register arbitrary paths, obtain templates, or submit raw records.
export type PublicOperation =
  | {method:'runtime.count_recover';params:ProtectedParams<'runtime.count_recover'>}
  | {method:'runtime.count_execute';params:ProtectedParams<'runtime.count_execute'>}
  | {method:'runtime.count_status';params:ProtectedParams<'runtime.count_status'>}
  | { method: 'save.discover'; params: ProtectedParams<'save.discover'> }
  | { method: 'save.inventory'; params: ProtectedParams<'save.inventory'> }
  | { method: 'save.prepare_edit'; params: ProtectedParams<'save.prepare_edit'> }
  | { method: 'save.prepare_delete'; params: ProtectedParams<'save.prepare_delete'> }
  | { method: 'save.recycle_backups'; params: ProtectedParams<'save.recycle_backups'> }
  | { method: 'save.backups'; params: ProtectedParams<'save.backups'> }
  | { method: 'save.prepare_restore'; params: ProtectedParams<'save.prepare_restore'> }
  | { method: 'save.discard'; params: ProtectedParams<'save.discard'> }
  | { method: 'save.commit'; params: ProtectedParams<'save.commit'> }
  | { method: 'save.operation'; params: ProtectedParams<'save.operation'> }
  | { method: 'save.operations'; params: ProtectedParams<'save.operations'> }
  | { method: 'runtime.live_batch_execute'; params: ProtectedParams<'runtime.live_batch_execute'> }
  | { method: 'runtime.live_batch_status'; params: ProtectedParams<'runtime.live_batch_status'> }
  | { method: 'runtime.live_batch_cancel'; params: ProtectedParams<'runtime.live_batch_cancel'> }
  | { method: 'runtime.status'; params: ProtectedParams<'runtime.status'> }
  | { method: 'runtime.live_add_execute'; params: ProtectedParams<'runtime.live_add_execute'> }
  | { method: 'runtime.live_add_status'; params: ProtectedParams<'runtime.live_add_status'> }
  | { method: 'runtime.live_add_recover'; params: ProtectedParams<'runtime.live_add_recover'> }
  | { method: 'runtime.live_add_cancel'; params: ProtectedParams<'runtime.live_add_cancel'> }
  | { method: 'runtime.start_override'; params: ProtectedParams<'runtime.start_override'> }
  | { method: 'runtime.stop_override'; params: ProtectedParams<'runtime.stop_override'> };
export interface OperationsApi {
  prepareCount(params: ProtectedParams<'save.count_edit_source'> & {new_count:number}):Promise<ProtectedResult>;
  prepareLiveAdd(params: { save_id: string; snapshot_id: string; source: 'search' | 'runtime'; job_id: string; candidate_id: string }): Promise<ProtectedResult>;
  selectSave(): Promise<ProtectedResult | null>;
  execute(command: PublicOperation): Promise<ProtectedResult>;
  snapshot(role: 'save' | 'runtime', jobId: string): Promise<ProtectedJob>;
  cancel(role: 'save' | 'runtime', jobId: string): Promise<ProtectedJob>;
  current(role: 'save' | 'runtime'): Promise<{ job: ProtectedJob | null; busy: boolean }>;
  prepareInstall(params: { save_id: string; snapshot_id: string; source: 'search' | 'runtime'; job_id: string;
    candidate_id: string; recommended_level: number; transfer_count: number }): Promise<ProtectedResult>;
  generate(params: Omit<ProtectedParams<'runtime.generate'>, 'template'> & { save_id: string; snapshot_id: string }): Promise<ProtectedResult>;
  searchNative(params: Omit<ProtectedParams<'runtime.search'>, 'template'> & { save_id: string; snapshot_id: string }): Promise<ProtectedResult>;
  captureGrace(params: Omit<ProtectedParams<'runtime.capture_grace'>, 'template'> & { save_id: string; snapshot_id: string }): Promise<ProtectedResult>;
  bindCachedSearch(params: ProtectedParams<'save.cached_grace'>): Promise<{ cache_id: string }>;
}
declare global { interface Window { operations: OperationsApi } }
