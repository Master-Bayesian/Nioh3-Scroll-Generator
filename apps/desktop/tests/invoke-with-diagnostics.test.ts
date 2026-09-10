import assert from "node:assert/strict";
import test from "node:test";
import { createDiagnosticInvoker } from "../src/invoke-with-diagnostics";

test("request failures are logged and copied without replacing the original error", async () => {
  const calls: Array<{ channel: string; value: unknown }> = [];
  const failure = new Error("BROKER_FAILED");
  const invoke = createDiagnosticInvoker(async (channel, value) => {
    calls.push({ channel, value });
    if (channel === "core:start") throw failure;
    return null;
  });
  await assert.rejects(() => invoke("core:start"), (error) => error === failure);
  assert.deepEqual(calls.map(({ channel }) => channel), [
    "review:log",
    "core:start",
    "review:log",
    "review:log",
    "review:copy-log",
  ]);
  assert.match(String(calls[0].value), /\[operation\] start/);
  assert.match(String(calls[2].value), /\[operation\] error/);
  assert.match(String(calls[3].value), /\[automatic-failure\]/);
});

test("a failed job payload triggers one automatic capture", async () => {
  const calls: string[] = [];
  const invoke = createDiagnosticInvoker(async (channel) => {
    calls.push(channel);
    return channel === "core:snapshot"
      ? { state: "failed", error: { code: "SEARCH_FAILED", message: "bad" } }
      : null;
  });
  await invoke("core:snapshot");
  await invoke("core:snapshot");
  assert.deepEqual(calls, [
    "core:snapshot",
    "review:log",
    "review:copy-log",
    "core:snapshot",
  ]);
});

test("operation traces retain diagnostic fields including raw candidate data", async () => {
  const calls: Array<{ channel: string; value: unknown }> = [];
  const invoke = createDiagnosticInvoker(async (channel, value) => {
    calls.push({ channel, value });
    if (channel === "operations:execute") {
      return { job_id: "job-1", kind: "runtime.live_batch_execute", state: "running", sequence: 2 };
    }
    return null;
  });
  await invoke("operations:execute", {
    method: "runtime.live_batch_execute",
    params: { batch_id: "batch-1", plan_digest: "secret", record_hex: "private" },
  });
  const messages = calls.filter(({ channel }) => channel === "review:log").map(({ value }) => String(value));
  assert.equal(messages.length, 3);
  assert.match(messages[0], /runtime\.live_batch_execute/);
  assert.match(messages[0], /batch-1/);
  assert.match(messages.join("\n"), /secret/);
  assert.match(messages.join("\n"), /private/);
  assert.match(messages[1], /\[operation-job\]/);
  assert.match(messages[2], /accepted/);
});
