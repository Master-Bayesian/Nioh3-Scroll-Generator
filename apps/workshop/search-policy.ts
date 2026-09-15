import type { JobSnapshot } from "../../packages/contracts/responses";
import type { StartParams } from "../desktop/src/worker-client";

/**
 * Native parity for the normal offline search: one backend job keeps consuming
 * bounded solver pages until the result target is met, the family is exhausted,
 * the user cancels, or the job fails. The UI must not build that continuation by
 * starting new jobs, which would drop candidate ownership and the checkpoint.
 */
export const SEARCH_PAGE_TRIALS = 100_000_000;
export const SEARCH_JOB_TRIALS = 10_000_000;

export function searchStartParams(input: {
  query: StartParams["query"];
  contextDigest: string;
  resultCount: number;
  allowCpuFallback: boolean;
  cacheId?: string | null;
  resumeToken?: string | null;
}): StartParams {
  return {
    query: input.query,
    context_digest: input.contextDigest,
    result_count: input.resultCount,
    allow_cpu_fallback: input.allowCpuFallback,
    resume_token: input.resumeToken ?? null,
    ...(input.cacheId ? { cache_id: input.cacheId } : {}),
    // `job_trials` only bounds the compat mode, which this flag turns off.
    continue_until_complete: true,
    page_trials: SEARCH_PAGE_TRIALS,
    job_trials: SEARCH_JOB_TRIALS,
  };
}

type SearchStatusJob = Pick<
  JobSnapshot,
  "state" | "stop_reason" | "candidates" | "elapsed_ms" | "error"
>;

/**
 * Canonical Chinese status text per stop reason; the JSX factory localizes it at
 * render time, so a locale switch retranslates an already visible status instead
 * of freezing the locale that was active when the job finished. Wording is per
 * batch: after a manual next batch, N is the count of the current batch only. A
 * bounded batch that ran out of budget, or an unknown/absent reason, must never
 * read as proof that nothing exists.
 */
export function searchStatusText(job: SearchStatusJob): string {
  const found = job.candidates.length;
  const seconds = (job.elapsed_ms / 1000).toFixed(1);
  if (job.error) return job.error.message;
  if (job.state === "failed") return "搜索失败，请重试或复制诊断信息。";
  if (job.state === "cancelled" || job.stop_reason === "cancelled")
    return "已取消，保留已找到的绘卷。";
  if (job.state !== "completed") return `正在搜索，已找到 ${found} 张绘卷…`;
  switch (job.stop_reason) {
    case "result_limit":
      return `本批找到 ${found} 张绘卷，已达候选数量，耗时 ${seconds} 秒。`;
    case "family_exhausted":
      return `搜索范围已穷尽，本批找到 ${found} 张绘卷，耗时 ${seconds} 秒。`;
    case "budget_reached":
      return `本批在试验预算内找到 ${found} 张绘卷；条件尚未穷尽，可继续搜索下一批，耗时 ${seconds} 秒。`;
    default:
      return `本批找到 ${found} 张绘卷，耗时 ${seconds} 秒。`;
  }
}
