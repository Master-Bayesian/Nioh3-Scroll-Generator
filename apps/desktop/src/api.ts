import type { Handshake, JobSnapshot, SearchCatalog, RecommendedLevelResolution } from '../../../packages/contracts/responses';
import type { StartParams } from './worker-client';
export interface DesktopApi {
  handshake(): Promise<Handshake>;
  searchCatalog(rarity: 3 | 4 | 5, locale: 'en-US' | 'zh-CN' | 'ja-JP'): Promise<SearchCatalog>;
  resolveRecommendedLevel(displayedLevel: number): Promise<RecommendedLevelResolution>;
  /** The worker's structural preflight for one query; never starts a job. */
  checkFeasibility(query: StartParams['query']): Promise<{ checked: boolean; feasible: boolean; reason: string | null }>;
  startSearch(params: StartParams): Promise<JobSnapshot>;
  currentSearch(): Promise<{ job: JobSnapshot | null; submitted: StartParams | null }>;
  snapshot(jobId: string): Promise<JobSnapshot>;
  cancelSearch(jobId: string): Promise<JobSnapshot>;
  restartWorker(): Promise<Handshake>;
}
declare global { interface Window { nioh: DesktopApi } }
