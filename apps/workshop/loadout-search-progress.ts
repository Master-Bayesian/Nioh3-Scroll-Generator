import type { ProtectedJob } from "../../packages/contracts/protected-responses";
import type { LoadoutWearBinding } from "./loadout-context-state";

export type SearchStage = "planning" | "verification" | "evaluation" | "retrying" | "polishing" | "finalizing";
export type SearchBackend = "unselected" | "cpu" | "card";
export interface LoadoutSearchOwner { binding: LoadoutWearBinding; jobId: string | null; clientRequestId: string }
export interface LoadoutSearchProgress {
  stage: SearchStage; backend: SearchBackend;
  completed: number; total: number | null;
  batchCompleted: number; batchTotal: number | null;
  batchIndex: number; attempt: number;
}
function counter(value: unknown): value is number { return typeof value === "number" && Number.isSafeInteger(value) && value >= 0; }
function nullableCounter(value: unknown): value is number | null { return value === null || counter(value); }
function stage(value: unknown): value is SearchStage {
  return value === "planning" || value === "verification" || value === "evaluation" || value === "retrying" || value === "polishing" || value === "finalizing";
}
function backend(value: unknown): value is SearchBackend { return value === "unselected" || value === "cpu" || value === "card"; }
/** A lost acknowledgement can be identified only by the unique echoed request and scope, not by "latest job". */
export function identifiedLoadoutSearchJob(job: Pick<ProtectedJob, "job_id" | "kind" | "progress"> | null, owner: LoadoutSearchOwner): string | null {
  if (!job || job.kind !== "save.loadout_search") return null;
  if (owner.jobId != null) return owner.jobId === job.job_id ? owner.jobId : null;
  const raw: unknown = job.progress;
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null;
  const p = raw as Record<string, unknown>, b = owner.binding;
  return p.schema === "nioh3-loadout-progress/v1" && p.client_request_id === owner.clientRequestId &&
    p.context_id === b.contextId && p.bundle === b.bundleId && p.save_id === b.saveId && p.source_sha256 === b.sourceSha256 ? job.job_id : null;
}
/** Unknown progress is not a zero count. The exact job and input owner must match. */
export function loadoutSearchProgress(job: Pick<ProtectedJob, "job_id" | "kind" | "progress"> | null, owner: LoadoutSearchOwner | null): LoadoutSearchProgress | null {
  if (!owner || !job || identifiedLoadoutSearchJob(job, owner) !== job.job_id) return null;
  const raw: unknown = job.progress;
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null;
  const p = raw as Record<string, unknown>, binding = owner.binding;
  if (p.schema !== "nioh3-loadout-progress/v1" || p.client_request_id !== owner.clientRequestId || p.context_id !== binding.contextId || p.bundle !== binding.bundleId ||
      p.save_id !== binding.saveId || p.source_sha256 !== binding.sourceSha256 || !stage(p.stage) || !backend(p.backend) ||
      !counter(p.completed) || !nullableCounter(p.total) || !counter(p.batch_completed) || !nullableCounter(p.batch_total) ||
      !counter(p.batch_index) || !counter(p.attempt) || p.total !== null && p.completed > p.total ||
      p.batch_total !== null && p.batch_completed > p.batch_total) return null;
  return { stage: p.stage, backend: p.backend, completed: p.completed, total: p.total,
    batchCompleted: p.batch_completed, batchTotal: p.batch_total, batchIndex: p.batch_index, attempt: p.attempt };
}
export function loadoutSearchStageLabel(value: SearchStage): string {
  switch (value) {
    case "planning": return "准备搜索";
    case "verification": return "核对计算结果";
    case "evaluation": return "计算搭配";
    case "retrying": return "重算当前批次";
    case "polishing": return "调整候选搭配";
    case "finalizing": return "整理结果";
  }
}
