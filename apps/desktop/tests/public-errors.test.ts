import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import {
  errorText,
  isFailureText,
  isLiveAddLockError,
  isStaleSnapshotError,
  isUserCorrectable,
  publicError,
} from "../../workshop/public-errors";

test("compatibility refusals describe the recovery action for actual host codes", () => {
  for (const code of ["COMPATIBILITY_PLAN_MISMATCH", "COMPATIBILITY_PLAN_EXPIRED"])
    assert.match(publicError(code + ": stale fixture plan"), /重新准备.*新计划/);
  for (const code of ["COMPATIBILITY_BACKUP_REQUIRED", "COMPATIBILITY_BACKUP_UNVERIFIED"])
    assert.match(publicError(code + ": changed fixture source"), /备份.*重试/);
  assert.match(publicError("COMPATIBILITY_AUDIT_FAILED: fixture directory"), /未能安全保存.*重新准备/);
  assert.match(publicError("COUNT_SOURCE_CHANGED: COMPATIBILITY_IDENTITY_CHANGED: expected PID 10, actual PID 20"), /重新连接.*不能直接重试/);
  assert.match(publicError("COMPATIBILITY_RESOURCE_CONTEXT_CHANGED: expected data 2.01, actual game 2.02"), /选择当前运行.*重新打开工作室/);
  assert.match(publicError("COMPATIBILITY_FEATURE_UNSUPPORTED: live_equipment_add"), /不支持这项操作.*功能范围/);
});

test("equipment backup refusals identify the source, copy, and safe recovery", () => {
  const required = publicError("EQUIPMENT_BACKUP_SOURCE_REQUIRED: More than one save was found. Select the current character's SAVEDATA.BIN backup path, then prepare again; no native preview has run.");
  assert.match(required, /发现多个存档.*本次未执行.*准备另一件装备.*目标存档/);
  assert.match(required, /选中当前角色的存档/);
  assert.doesNotMatch(required, /发给开发者|导出反馈/);
  assert.match(publicError("No save was found under D:/fixture; expected <account>/SAVEDATAxx/SAVEDATA.BIN. Create a save in game or restore access to this folder, then prepare again"), /搜索目录.*创建存档.*手动定位/);
  const resume = "Cancel this equipment plan, select the current character save, and prepare again; do not replay an uncertain operation.";
  for (const raw of ["Source save changed during backup", "Source save changed after equipment preparation at D:/source. " + resume, "Source save cannot be read at D:/source: access denied. " + resume]) {
    assert.match(publicError(raw), /源存档.*源文件路径.*结果不明.*不要重复添加/);
  }
  for (const raw of ["Automatic save backup verification failed", "Verified backup changed after equipment preparation at D:/copy. " + resume, "Verified backup cannot be read at D:/copy: access denied. " + resume]) {
    assert.match(publicError(raw), /备份副本.*备份路径.*新的已验证备份.*不要重复添加/);
  }
  for (const raw of ["Equipment save checkpoint is missing backup_path", "Equipment save checkpoint belongs to another operation", "Equipment save checkpoint has an invalid source hash", "Equipment save checkpoint cannot read D:/copy: access denied", "Equipment save checkpoint must be a separate backup file", "A verified save checkpoint is required before equipment preview"]) {
    assert.match(publicError(raw), /缺少有效的备份记录.*具体原因.*当前角色的存档.*不要重复添加/);
  }
  // Keep unrelated file access failures outside the equipment-specific advice.
  assert.doesNotMatch(publicError("IO_ERROR: D:/unrelated: access denied"), /要备份的存档路径/);
});

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

test("module-snapshot access denial gives the same recovery as process-open denial", () => {
  const denied = "OPERATION_REJECTED: CreateToolhelp32Snapshot(modules, 107352) failed with error 5";
  const recovery = publicError("OPERATION_REJECTED: OpenProcess(107352) failed with error 5");
  for (const raw of [denied, `Error: ${denied}`, JSON.stringify({ code: "OPERATION_REJECTED", message: denied })]) {
    assert.equal(publicError(raw), recovery);
    assert.match(publicError(raw), /系统拒绝访问游戏进程.*可能.*权限级别不同/);
    assert.match(publicError(raw), /如果游戏以管理员身份运行.*以管理员身份重新启动本工具/);
    assert.match(publicError(raw), /也可以关闭游戏的.*设置.*重新启动游戏和本工具.*重试读取/);
    assert.ok(isFailureText(raw));
    assert.doesNotMatch(publicError(raw), /没有.*文件.*权限|游戏已退出|已经修复|已检测到.*权限|必须.*管理员/);
  }
  for (const code of [24, 50, 299]) {
    const raw = `OPERATION_REJECTED: CreateToolhelp32Snapshot(modules, 107352) failed with error ${code}`;
    assert.doesNotMatch(publicError(raw), /管理员/);
    assert.match(publicError(raw), /OPERATION_REJECTED/);
  }
});

test("access-denied translations offer both conditional restart paths", () => {
  const recovery = publicError("OPERATION_REJECTED: OpenProcess(107352) failed with error 5");
  const locales = JSON.parse(readFileSync(new URL("../../workshop/ui-locales.json", import.meta.url), "utf8"));
  const [english, japanese] = locales.ui[recovery];
  assert.match(english, /may have different privilege levels/);
  assert.match(english, /If the game runs as administrator, restart this tool as administrator/);
  assert.match(english, /alternatively, disable the game's.*setting, restart both the game and tool, then retry reading/);
  assert.match(japanese, /権限レベルが異なる可能性/);
  assert.match(japanese, /管理者として実行している場合.*ツールも管理者として再起動/);
  assert.match(japanese, /または.*設定を解除.*ゲームとツールの両方を再起動.*読み取りを再試行/);
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

test("an impossible condition set names the clashing effects and what to change", () => {
  const text = publicError(
    "INVALID_REQUEST: the selected effect combination has no solution in the native generation structure: " +
      "the selected effects 0x2614、0x6CE3 share native category 0x0C, which holds at most 1, but 2 were selected",
  );
  assert.match(text, /“一难的解除时间延长”、“不消耗使役符”/);
  assert.match(text, /最多只有 1 个/);
  assert.match(text, /去掉其中 1 个/);
  assert.doesNotMatch(text, /错误代码/);
  // The shipped worker text without member names still explains itself.
  assert.match(
    publicError(
      "INVALID_REQUEST: the selected effect combination has no solution in the native generation structure: " +
        "the selected effects share native category 0x0C, which holds at most 1, but 2 were selected",
    ),
    /所选词条中有 2 个/,
  );
  assert.match(
    publicError(
      "INVALID_REQUEST: the selected effect combination has no solution in the native generation structure: " +
        "0x2614 and 0x6CE3 belong to native conflict groups and cannot appear together",
    ),
    /互相冲突/,
  );
});

test("save and runtime refusals from the Rust layers read like the Python backend did", () => {
  assert.match(publicError('OPERATION_FAILED: there is no contiguous run of 2 free scroll slots starting at 399'), /400 张/);
  assert.match(publicError('OPERATION_FAILED: C:\\x\\SAVEDATA.BIN changed after the operation was prepared; refresh before committing'), /重新读取/);
  assert.match(publicError('OPERATION_FAILED: exactly one Nioh3.exe must be running, but 2 were found'), /多个《仁王3》/);
  assert.match(publicError('OPERATION_FAILED: OpenProcess(1234) failed with error 5'), /管理员/);
  assert.match(publicError('OPERATION_FAILED: no authentic scroll template is available for playthrough 3'), /模板/);
});
test("rarer Rust refusals keep the explanations the Python backend gave", () => {
  const cases: [string, RegExp][] = [
    ["INVALID_REQUEST: the one-wildcard rarity-5 composition has no legal native path: x", /无法组成一张合法的绘卷/],
    ["INVALID_REQUEST: rarity-5 Grace 0x6553 has no draw-1 preimage: x", /恩宠不会出现/],
    ["SEARCH_BACKEND_UNAVAILABLE: natural: the native seed accelerator refused to run (CudaUnavailable); CPU fallback is disabled", /允许使用 CPU 搜索/],
    ["HOOK_MODIFIED: the hook at 0x1234 was changed by another program; it will not be overwritten", /Cheat Engine/],
    ["HOOK_RESTORE_UNVERIFIED: the temporary override could not be read or restored and the process has not exited (x)", /退出游戏/],
    ["LIVE_ADD_REJECTED: The game has not initialized its player identity yet; try again shortly", /还没有加载好角色/],
    ["COUNT_INSTANCE_UNAVAILABLE: Scroll instance 12 is no longer in the current inventory", /不在当前背包/],
    ["PROCESS_GONE: process 1234 is no longer running", /游戏已退出/],
    ["PROFILE_NOT_APPROVED: PC v2.02 runtime profile is not approved for product use", /尚未支持/],
    ["OPERATION_REJECTED: INVALID_SCROLL_ID: this scroll ID cannot occur in the game", /不是游戏能生成的/],
    // How a failed native search job reached a user (feedback 2026-09-27).
    ["OPERATION_FAILED: PC v2.02 runtime profile is not approved for product use", /尚未支持/],
    ["OPERATION_FAILED: the stored backup manifest failed validation: the manifest does not declare the v2 schema", /旧版本工具创建的备份/],
    ["OPERATION_FAILED: the restore bundle is missing role main", /备份文件不完整/],
    ["OPERATION_FAILED: the recycle-bin move was cancelled", /已取消删除备份/],
    ["WORKER_TIMEOUT: outcome unknown; do not replay writes", /先核对结果/],
    ["UPDATE_HASH_MISMATCH: x", /没有通过校验/],
    ["GAME_EXECUTABLE_NOT_FOUND: no candidate", /没有找到《仁王3》的游戏程序/],
  ];
  for (const [raw, expected] of cases) {
    const text = publicError(raw);
    assert.match(text, expected, raw);
    assert.doesNotMatch(text, /错误代码/, raw);
  }
  // Refusals the player fixes by changing the selection are hints, not failures.
  assert.ok(isUserCorrectable("INVALID_REQUEST: the complete rarity-5 composition has no legal native path: x"));
  assert.ok(isUserCorrectable("INVALID_REQUEST: rarity-5 Grace 0x6553 has no draw-1 preimage: x"));
});

test("character editing refusals are explained in the player's terms", () => {
  const cases: [string, RegExp][] = [
    ["OPERATION_REJECTED: no running process matches Nioh3.exe", /没有检测到正在运行的仁王3/],
    ["OPERATION_REJECTED: inventory chain: character layout: no character is loaded", /还没有读档进入角色/],
    ["OPERATION_REJECTED: character layout: the player object's vtable does not match this build", /游戏没有被改动/],
    ["the game changed a value since it was read; reload and try again", /重新读取/],
    ["The equipment changed in game since it was read; reload and try again", /重新读取/],
    ["the game process changed since the character was read", /游戏已经重启过/],
    ["a written value did not read back as written", /结果不确定/],
    ["OPERATION_REJECTED: Save changed since it was read; reload the character", /存档文件刚刚发生了变化/],
    ["OPERATION_REJECTED: Only occupied equipment slots may be edited", /装备格是空的/],
    ["OPERATION_REJECTED: Nothing to change", /没有需要修改的内容/],
  ];
  for (const [raw, expected] of cases) assert.match(publicError(raw), expected, raw);
});

test("a stale live-add lock names the reset the player can press", () => {
  const raw =
    'OPERATION_FAILED: Another native executor is admitting an operation';
  assert.ok(isLiveAddLockError(raw));
  assert.ok(isFailureText(raw));
  const text = publicError(raw);
  assert.match(text, /重置实时添加状态/);
  assert.doesNotMatch(text, /Another native executor/);
  assert.ok(!isLiveAddLockError("Previous native operation x is unresolved; recover it, never replay"));
});

test("a save the game wrote after preview is a stale snapshot, not a failure to report", () => {
  for (const raw of [
    '{"code":"OPERATION_FAILED","message":"Save changed after preview; refresh inventory"}',
    "OPERATION_FAILED: Snapshot expired; refresh inventory",
  ]) {
    assert.ok(isStaleSnapshotError(raw));
    assert.match(publicError(raw), /重新读取/);
  }
  assert.ok(!isStaleSnapshotError("Save changed since it was read"));
});

test("refusals the player can clear say which step clears them", () => {
  const cases: [string, RegExp][] = [
    ["Save changed after preparation; no write attempted", /重新读取/],
    ["Inventory changed between batch items", /停下来/],
    ["Insufficient scroll capacity", /空位/],
    ["Inventory data is not loaded", /进入角色/],
    ["Live addition is not accepted for this game version", /检查更新/],
    ["Live addition requires an idle runtime host", /还没有结束/],
    ["Native candidate expired; generate again", /重新搜索/],
    ["Access is denied. (os error 5)", /权限/],
    ["另一个程序正在使用此文件，进程无法访问。 (os error 32)", /占用/],
  ];
  for (const [raw, expected] of cases)
    for (const form of [
      `Error: ${raw}`,
      JSON.stringify({ code: "OPERATION_FAILED", message: raw }),
    ])
      assert.match(publicError(form), expected, form);
});

test("an unexplained structured failure still reads as a Chinese next step", () => {
  const raw = '{"code":"OPERATION_FAILED","message":"Duplicate full serial index key"}';
  const text = publicError(raw);
  assert.match(text, /^操作没有完成/);
  assert.match(text, /导出反馈文件/);
  assert.ok(isFailureText(raw));
});
