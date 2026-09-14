import test from "node:test";
import assert from "node:assert/strict";
import {
  StartupUpdateCheck,
  type UpdateRequest,
  type UpdateResult,
} from "../../workshop/startup-update-check";

test("startup waits for readiness, checks exactly once, and prompts once", async () => {
  const calls: string[] = [];
  let ready = false;
  let checked = false;
  let prompts = 0;
  const request: UpdateRequest = async ({ action }) => {
    calls.push(action);
    if (action === "check") checked = true;
    const result: UpdateResult = checked
      ? {
          phase: "available",
          version: "0.7.4",
          notes: "Fixture release",
          canApply: true,
        }
      : { phase: "idle", canApply: ready };
    return result;
  };
  const startup = new StartupUpdateCheck(request, () => prompts++);

  await startup.poll("stable");
  assert.deepEqual(calls, ["status"]);
  ready = true;
  await Promise.all([startup.poll("stable"), startup.poll("stable")]);
  await startup.poll("stable");

  assert.equal(calls.filter((action) => action === "check").length, 1);
  assert.equal(prompts, 1);
});

test("a failed startup request remains retryable", async () => {
  let checks = 0;
  const request: UpdateRequest = async ({ action }) => {
    if (action === "status") return { phase: "idle", canApply: true };
    checks++;
    if (checks === 1) throw new Error("UPDATE_STARTUP_PENDING");
    return { phase: "checking", canApply: true };
  };
  const startup = new StartupUpdateCheck(request, () => {
    throw new Error("No prompt expected while checking");
  });

  await assert.rejects(startup.poll("stable"), /UPDATE_STARTUP_PENDING/);
  await startup.poll("stable");
  assert.equal(checks, 2);
});
