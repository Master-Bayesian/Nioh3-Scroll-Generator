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

test("nested business failures trigger capture even in a completed worker job", async () => {
  for (const result of [
    { commit_status: "unknown", operation_id: "save-1", warning: "SAVE_COMMIT_UNCERTAIN" },
    { live_add: { state: "rejected_before_dispatch", operation_id: "live-1" } },
    { live_batch: { state: "partial", batch_id: "batch-1" } },
  ]) {
    const captured: string[] = [];
    const job = { job_id: "j", state: "completed", result };
    const invoke = createDiagnosticInvoker(async channel => {
      captured.push(channel);
      return channel === "operations:snapshot" ? job : null;
    });
    assert.equal(await invoke("operations:snapshot"), job);
    assert.equal(captured.filter(c => c === "review:copy-log").length, 1);
  }
});

test("hung clipboard and log sinks cannot suppress the original error", async () => {
  const failure = new Error("ORIGINAL_WRITE_FAILURE");
  const invoke = createDiagnosticInvoker(async channel => {
    if (channel === "operations:execute") throw failure;
    return new Promise(() => {});
  }, 5);
  await assert.rejects(() => invoke("operations:execute"), error => error === failure);
});

test("diagnostic serialization cannot convert successful execution into failure", async () => {
  const result: any = { job_id: "j", state: "completed", result: { counter: 7n } };
  result.result.circular = result;
  const seen: string[] = [];
  const invoke = createDiagnosticInvoker(async channel => {
    seen.push(channel);
    return channel === "operations:execute" ? result : null;
  });
  assert.equal(await invoke("operations:execute", { counter: 8n }), result);
  assert.equal(seen.filter(c => c === "operations:execute").length, 1);
  assert.ok(!seen.includes("review:copy-log"));
});

test("multibyte diagnostic messages respect the broker UTF-8 byte bound", async () => {
  const logs: string[] = [];
  const invoke = createDiagnosticInvoker(async (channel, value) => {
    if (channel === "review:log") logs.push(String(value));
    return null;
  });
  await invoke("operations:execute", { text: "错".repeat(20_000) });
  assert.ok(logs.every(value => Buffer.byteLength(value) <= 16_384));
});
