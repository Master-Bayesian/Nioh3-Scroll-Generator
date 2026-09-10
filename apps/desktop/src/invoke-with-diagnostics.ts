type Invoke = (channel: string, value?: unknown) => Promise<unknown>;

const REPORTING_CHANNELS = new Set([
  "review:copy-log",
  "review:log",
  "review:copy",
  "review:window",
]);

const TRACED_CHANNELS = new Set([
  "core:start",
  "core:cancel",
  "core:restart",
  "operations:select",
  "operations:prepare-count",
  "operations:install",
  "operations:live-add",
  "operations:generate",
  "operations:native-search",
  "operations:capture-grace",
  "operations:bind-cache",
  "operations:execute",
  "operations:cancel",
  "review:prepare-cart",
  "review:update",
]);

function diagnosticJson(value: unknown): string {
  const json = JSON.stringify(value ?? null);
  return json.length <= 14_000 ? json : `${json.slice(0, 14_000)}...[client trace truncated; worker trace retains full data]`;
}

function line(kind: string, channel: string, id: number, value?: unknown): string {
  const summary = value === undefined ? "" : ` ${diagnosticJson(value)}`;
  return `[operation] ${kind} request=${id} channel=${channel}${summary}`;
}

function jobFrom(result: unknown): Record<string, unknown> | null {
  if (!result || typeof result !== "object") return null;
  const value = result as Record<string, unknown> & { job?: unknown };
  if (typeof value.job_id === "string" && typeof value.state === "string") return value;
  if (value.job && typeof value.job === "object") {
    const job = value.job as Record<string, unknown>;
    if (typeof job.job_id === "string" && typeof job.state === "string") return job;
  }
  return null;
}

function operationFailure(result: unknown): string | null {
  if (!result || typeof result !== "object") return null;
  const value = result as {
    state?: unknown;
    error?: unknown;
    job?: { state?: unknown; error?: unknown } | null;
  };
  const failed = value.state === "failed" ? value : value.job?.state === "failed" ? value.job : null;
  if (!failed) return null;
  return typeof failed.error === "string"
    ? failed.error
    : JSON.stringify(failed.error ?? "Operation failed");
}

/** Add one bounded automatic support capture around a desktop transport. */
export function createDiagnosticInvoker(rawInvoke: Invoke): Invoke {
  let reporting: Promise<void> | null = null;
  let previousFailure = "";
  let previousFailureAt = 0;
  let requestSequence = 0;
  const jobStates = new Map<string, string>();

  const write = async (message: string) => {
    try {
      await rawInvoke("review:log", message);
    } catch {
      // Diagnostics are best effort and cannot alter the operation result.
    }
  };

  const recordJobTransition = async (channel: string, result: unknown) => {
    const job = jobFrom(result);
    if (!job) return;
    const jobId = String(job.job_id);
    const state = String(job.state);
    const signature = `${state}:${String(job.sequence ?? "")}`;
    if (jobStates.get(jobId) === signature) return;
    jobStates.set(jobId, signature);
    if (jobStates.size > 64) jobStates.delete(jobStates.keys().next().value!);
    await write(`[operation-job] channel=${channel} ${diagnosticJson(job)}`);
  };

  const report = async (channel: string, failure: string) => {
    if (REPORTING_CHANNELS.has(channel)) return;
    const signature = `${channel}:${failure}`;
    const now = Date.now();
    if (signature === previousFailure && now - previousFailureAt < 10_000) return;
    previousFailure = signature;
    previousFailureAt = now;
    if (reporting) return reporting;
    reporting = (async () => {
      try {
        await rawInvoke("review:log", `[automatic-failure] ${channel}: ${failure}`);
      } catch {
        // Keep the original operation error even when the log sink is unavailable.
      }
      try {
        await rawInvoke("review:copy-log", null);
      } catch {
        // Clipboard capture is best effort and must never replace the real error.
      }
    })().finally(() => {
      reporting = null;
    });
    return reporting;
  };

  return async (channel: string, value: unknown = null) => {
    const requestId = ++requestSequence;
    const traced = TRACED_CHANNELS.has(channel) && !(channel === "review:update" && (value as { action?: unknown } | null)?.action === "status");
    if (traced) await write(line("start", channel, requestId, value));
    try {
      const result = await rawInvoke(channel, value);
      await recordJobTransition(channel, result);
      const failure = operationFailure(result);
      if (failure) await report(channel, failure);
      else if (traced) await write(line("accepted", channel, requestId, result));
      return result;
    } catch (error) {
      if (traced) await write(line("error", channel, requestId, { error: String(error) }));
      await report(channel, String(error));
      throw error;
    }
  };
}
