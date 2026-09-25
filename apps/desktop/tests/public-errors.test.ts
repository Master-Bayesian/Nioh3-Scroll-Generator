import assert from "node:assert/strict";
import test from "node:test";
import { errorText, isFailureText, publicError } from "../../workshop/public-errors";

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

test("an expired save snapshot asks for a refresh", () => {
  assert.match(
    publicError('Error: {"code":"OPERATION_FAILED","message":"Snapshot expired; refresh inventory"}'),
    /重新读取/,
  );
});

test("a thrown Chinese message loses the Error prefix and still reads as a failure", () => {
  assert.equal(publicError("Error: 没有待核对的实时添加。"), "没有待核对的实时添加。");
  assert.ok(isFailureText("Error: 请先选择并读取存档。"));
  assert.ok(!isFailureText("已验证添加 1 / 1 张。"));
  assert.ok(!isFailureText(""));
});

test("an unexplained backend failure names its error code and the way to report it", () => {
  const text = publicError("OPERATION_REJECTED: Unknown operation job");
  assert.match(text, /OPERATION_REJECTED/);
  assert.match(text, /导出反馈文件/);
  assert.ok(isFailureText("OPERATION_REJECTED: Unknown operation job"));
  assert.match(publicError("CATALOG_UNAVAILABLE"), /错误代码：CATALOG_UNAVAILABLE/);
  // Ordinary interface text without Chinese is never rewritten.
  for (const plain of ["R4", "GitHub", "Lv.180", "10030565"]) assert.equal(publicError(plain), plain);
});