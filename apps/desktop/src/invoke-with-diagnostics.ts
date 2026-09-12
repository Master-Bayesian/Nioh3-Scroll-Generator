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
  // Formatting must never prevent a submission or turn success into failure.
  try {
    const json = JSON.stringify(value ?? null);
    const bytes = new TextEncoder().encode(json);
    if (bytes.length <= 14_000) return json;
    let end = 14_000;
    while (end > 0 && (bytes[end] & 0xc0) === 0x80) end--;
    return new TextDecoder().decode(bytes.slice(0, end)) + "...[trace truncated]";
  } catch {
    return "[diagnostic value could not be serialized]";
  }
}

async function bestEffort(action: () => Promise<unknown>, timeoutMs: number): Promise<void> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    await Promise.race([
      Promise.resolve().then(action).then(() => undefined, () => undefined),
      new Promise<void>(resolve => { timer = setTimeout(resolve, timeoutMs); }),
    ]);
  } finally { clearTimeout(timer); }
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
  const seen = new Set<unknown>();
  const inspect = (input: unknown, depth: number): string | null => {
    if (!input || typeof input !== "object" || seen.has(input) || depth > 4) return null;
    seen.add(input);
    const value = input as Record<string, unknown>;
    if (value.state === "failed") return diagnosticJson(value.error ?? "Operation failed");
    if (["not_committed", "unknown", "committed_with_warning"].includes(String(value.commit_status)))
      return diagnosticJson({ operation_id: value.operation_id, commit_status: value.commit_status, warning: value.warning, details: value.details });
    if (["uncertain", "rejected_before_dispatch", "partial"].includes(String(value.state)))
      return diagnosticJson({ operation_id: value.operation_id, batch_id: value.batch_id, state: value.state, error: value.error, receipt: value.receipt });
    for (const key of ["job", "result", "live_add", "live_batch"] as const) {
      const failure = inspect(value[key], depth + 1);
      if (failure) return failure;
    }
    return null;
  };
  return inspect(result, 0);
}

/** Add one bounded automatic support capture around a desktop transport. */
export function createDiagnosticInvoker(rawInvoke: Invoke, diagnosticTimeoutMs = 750): Invoke {
  let reporting: Promise<void> | null = null;
  let previousFailure = "";
  let previousFailureAt = 0;
  let requestSequence = 0;
  const jobStates = new Map<string, string>();

  const write = (message: string) => bestEffort(
    () => rawInvoke("review:log", message), diagnosticTimeoutMs,
  );

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
      await write(`[automatic-failure] ${channel}: ${failure}`);
      await bestEffort(() => rawInvoke("review:copy-log", null), diagnosticTimeoutMs);
    })().finally(() => { reporting = null; });
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
