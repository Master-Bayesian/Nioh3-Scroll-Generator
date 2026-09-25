import assert from "node:assert/strict";
import test from "node:test";
import { errorText, publicError } from "../../workshop/public-errors";

test("errorText never returns an empty status for a rejection", () => {
  assert.equal(errorText(new Error("推荐等级无法转换。")), "推荐等级无法转换。");
  // Tauri command failures reject with a plain string.
  assert.equal(
    errorText("INVALID_REQUEST: displayed level out of range"),
    "INVALID_REQUEST: displayed level out of range",
  );
  assert.equal(
    errorText({ code: "OPERATION_FAILED", message: "profile missing" }),
    "OPERATION_FAILED: profile missing",
  );
  assert.equal(errorText({ message: "no code" }), "no code");
  assert.equal(errorText({ detail: 1 }), '{"detail":1}');
  for (const empty of [undefined, null, "", "  ", new Error(""), {}])
    assert.ok(errorText(empty).length > 0, `fallback for ${String(empty)}`);
  assert.equal(errorText(undefined, "核对失败"), "核对失败");
});

test("duplicate-serial refusals explain themselves instead of asking for the log", () => {
  for (const raw of [
    "Error: OPERATION_FAILED: APPEND_ONLY_REPAIR_REQUIRED: existing scroll generation serials collide",
    "Error: OPERATION_FAILED: Invalid inventory capacity or duplicate serials",
  ]) {
    const text = publicError(raw);
    assert.match(text, /序列号重复/);
    assert.doesNotMatch(text, /复制日志/);
  }
});

test("a rejected native preview says nothing was added", () => {
  const text = publicError(
    "Error: CANDIDATE_REJECTED: Native preview for 15998f73 was rejected after dispatch (child 8ec444b1); " +
      "its container and serial index are proven unchanged and its cleanup is terminal, so recover 8ec444b1 " +
      "(read-only) instead of replaying it: Native builder output differs from reviewed record",
  );
  assert.match(text, /没有添加/);
  assert.doesNotMatch(text, /不要重复添加/);
});

test("a vanished game process asks for the game again", () => {
  assert.match(
    publicError("Error: OPERATION_FAILED: QueryFullProcessImageNameW(28160) failed with error 31"),
    /重新启动/,
  );
});
