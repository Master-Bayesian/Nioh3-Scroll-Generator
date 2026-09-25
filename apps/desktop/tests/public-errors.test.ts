import assert from "node:assert/strict";
import test from "node:test";
import { errorText } from "../../workshop/public-errors";

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
