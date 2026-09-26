import { data } from "./model";

const hasCjk =(text: string) => /[㐀-鿿]/.test(text);

/** The message without the `Error:` prefix `String(error)` adds. */
export function stripErrorPrefix(message: string): string {
  return message.replace(/^\s*(?:Uncaught\s+)?Error:\s*/, "");
}

/**
 * Whether a status value reports a failure. Interface texts are authored in
 * Chinese, so a value with no Chinese at all came from a backend failure.
 */
export function isFailureText(message: string): boolean {
  const text = message.trim();
  if (!text) return false;
  const stripped = stripErrorPrefix(text);
  return stripped !== text || !hasCjk(stripped) || publicError(stripped) !== stripped;
}

/** The display name of an effect the worker names as `0xNNNN`. */
function effectName(hex: string): string {
  const id = String(parseInt(hex, 16));
  const found =
    data.editorEffects.find((effect) => effect.id === id) ||
    Object.values(data.contexts)
      .flatMap((context) => [...context.effects, ...context.graces])
      .find((effect) => effect.id === id);
  return found ? `“${found.name}”` : hex;
}

const effectNames = (text: string) =>
  (text.match(/0x[0-9A-F]{4}\b/gi) || []).map(effectName).join("、");

/** Why the search worker refused a condition set, in the player's terms. */
function infeasibleConditions(detail: string): string {
  const category = detail.match(/holds at most (\d+), but (\d+) were selected/);
  if (category) {
    const [capacity, count] = [Number(category[1]), Number(category[2])];
    const names = effectNames(detail.split("share native category")[0]);
    return `${names ? `所选的 ${names} ` : `所选词条中有 ${count} 个词条`}在游戏里属于同一类词条，一张绘卷上这一类最多只有 ${capacity} 个，所以它们不可能同时出现。请去掉其中 ${count - capacity} 个再搜索。`;
  }
  if (/belong to native conflict groups/.test(detail))
    return `${effectNames(detail)} 在游戏里互相冲突，不会出现在同一张绘卷上。请去掉其中一个再搜索。`;
  const slots = detail.match(/ordinary secondary slots but this structure has only (\d+)/);
  if (slots)
    return `选的副词条太多了：这种绘卷最多只有 ${slots[1]} 个普通副词条位置。请减少副词条再搜索。`;
  if (/cannot be generated for this scroll type|weight 0 for this playthrough and rarity|not in the native parameter table/.test(detail))
    return `${effectNames(detail)} 不会出现在当前周目和稀有度的绘卷上。请换一个周目或稀有度，或去掉这个词条。`;
  if (/rarity 5 has a single deep slot/.test(detail))
    return `R5 绘卷只有一个深层词条位置，而且它会成为主词条，所以 ${effectNames(detail)} 只能作为主词条出现。请把它设为主词条，或去掉它再搜索。`;
  return "这个词条组合在游戏里不可能出现，请调整筛选条件后再搜索。";
}

/**
 * A refusal the player resolves by changing their own choices. It is shown
 * as a hint, never as a failure to report.
 */
export function isUserCorrectable(message: string): boolean {
  return /no solution in the native generation structure|原生生成结构中无解|no legal native path|compiled no plan family|has no draw-1 preimage|at most 32 scratch keys|FAVORITES_CAPACITY_REACHED|CART_CAPACITY_REACHED|Possessed is available only/i.test(
    message,
  );
}

/** User-facing next steps; the broker keeps full technical errors in rolling logs. */
export function publicError(message: string): string {
  if (hasCjk(message)) message = stripErrorPrefix(message);
  const infeasible = message.match(/no solution in the native generation structure: ([\s\S]*)$/);
  if (infeasible) return infeasibleConditions(infeasible[1]);
  const cases: [RegExp, string][] = [
    // The Rust save and runtime layers report in English; these restore the
    // explanations the Python backend gave players for the same refusals.
    [
      /no legal native path|compiled no plan family/i,
      "所选词条在游戏里无法组成一张合法的绘卷（无论哪个作为主词条，生成到一半都会冲突或无词条可抽）。请调整词条后再搜索。",
    ],
    [
      /has no draw-1 preimage/i,
      "所选恩宠不会出现在这种绘卷上。请换一个恩宠或稀有度后再搜索。",
    ],
    [
      /at most 32 scratch keys/i,
      "选择的特殊规则太多了（最多 32 条）。请减少特殊规则后再搜索。",
    ],
    [
      /SEARCH_BACKEND_UNAVAILABLE|accelerator refused to run|accelerator is (?:unavailable|missing)|CPU fallback is disabled/i,
      "没有可用的显卡搜索加速，已停止搜索。请更新显卡驱动后重试；也可以在“设置”里打开“允许使用 CPU 搜索”（会慢很多）。",
    ],
    [
      /HOOK_MODIFIED|was changed by another program/i,
      "游戏里的同一处代码已被其他程序（例如 Cheat Engine 脚本）修改，本工具不会覆盖它。请先关闭相关脚本或重启游戏再试。",
    ],
    [
      /HOOK_RESTORE_UNVERIFIED|HOOK_NOT_RESTORED|could not be read or restored|temporary override at 0x[0-9a-f]+ was not removed/i,
      "无法确认临时修改已经撤销。为安全起见，请直接退出游戏；重启游戏后临时修改会自然消失。",
    ],
    [/SESSION_NOT_OPEN|override session is not open/i, "临时修改已经停止。"],
    [
      /has not initialized its player identity yet/i,
      "游戏还没有加载好角色。请进入角色存档、能自由行动后再试。",
    ],
    [
      /COUNT_INSTANCE_UNAVAILABLE|is no longer in the current inventory/i,
      "这张绘卷已不在当前背包里（可能已被使用或丢弃）。请重新读取后再试。",
    ],
    [
      /COUNT_SOURCE_CHANGED|COUNT_INSTANCE_CHANGED/,
      "游戏里的绘卷状态已经变化。请重新读取后再试。",
    ],
    [
      /manifest does not declare the v2 schema/i,
      "这是旧版本工具创建的备份，缺少身份清单，不能自动恢复。当前存档没有被修改。",
    ],
    [
      /manifest save-layout profile differs from this build/i,
      "这个备份的存档结构版本与当前程序不一致，不能恢复。当前存档没有被修改。",
    ],
    [
      /restore bundle is missing role|restore bundle must declare exactly three roles|is missing or is not a regular file/i,
      "备份文件不完整（缺少文件），不能恢复。当前存档没有被修改。",
    ],
    [
      /has an invalid SHA-256|manifest operation id is not 32 lowercase|is declared more than once|unknown restore role/i,
      "备份清单格式无效，不能恢复。当前存档没有被修改。",
    ],
    [
      /selected restore bundle changed after preparation/i,
      "所选备份在准备恢复后又发生了变化，已停止恢复。请重新选择备份。",
    ],
    [
      /escapes the managed backups root|is a link to another location|backup path .* is unsafe or aliased/i,
      "拒绝操作不在本工具备份目录里的备份。",
    ],
    [/recycle-bin move was cancelled/i, "已取消删除备份。"],
    [
      /Windows refused the recycle-bin move/i,
      "Windows 拒绝把备份移入回收站，备份没有被删除。",
    ],
    [
      /WORKER_TIMEOUT|WORKER_PIPE_CLOSED|OFFLINE_WORKER_CLOSED|RUST_(?:PROTECTED_)?WORKER_MISSING/,
      "后台组件没有响应或意外退出。如果刚才在写入，请先核对结果再重试；请重启本工具，反复出现请导出反馈文件发给开发者。",
    ],
    [
      /UPDATE_(?:HASH_MISMATCH|SIGNATURE_INVALID|ARCHIVE_INVALID|MANIFEST_INVALID|ASSET_INVALID|ASSET_ORIGIN_INVALID|KEY_INVALID|TOO_LARGE|RESPONSE_TOO_LARGE|TOO_MANY_FILES)/,
      "更新文件没有通过校验，已停止更新，当前版本不受影响。请稍后重试，或到 GitHub 手动下载新版本。",
    ],
    [
      /GAME_EXECUTABLE_NOT_FOUND/,
      "没有找到《仁王3》的游戏程序，无法确认游戏版本。请确认游戏已安装，然后重启本工具。",
    ],
    [
      /GAME_EXECUTABLE_AMBIGUOUS/,
      "找到了多个《仁王3》游戏程序，无法确定使用哪一个。请只保留一个安装后重启本工具。",
    ],
    [
      /GAME_EXECUTABLE_UNREADABLE|GAME_VERSION_(?:UNREADABLE|MALFORMED)|FILE_VERSION_UNREADABLE/,
      "无法读取《仁王3》游戏程序的版本信息。请确认游戏文件完整（可在 Steam 里验证游戏文件），然后重启本工具。",
    ],
    [
      /no contiguous run of \d+ free scroll slots|All 400 scroll slots/i,
      "存档里没有足够的空绘卷栏位（最多 400 张），本次没有写入。请先在游戏里处理掉一些绘卷再添加。",
    ],
    [
      /slot \d+ is occupied|is not fully zeroed, so it cannot receive/i,
      "目标绘卷栏位已被占用（存档可能刚被游戏修改），已拒绝写入。请点“重新读取”后再试。",
    ],
    [
      /no unused .+ remains in this save/i,
      "这个存档里可分配给新绘卷的编号已经用尽，无法再添加，存档没有被修改。",
    ],
    [
      /changed after the operation was prepared/i,
      "准备期间游戏存档发生了变化（游戏可能刚保存过），本次没有写入。请点“重新读取”后再试。",
    ],
    [
      /no authentic scroll template is available/i,
      "存档里没有可作为模板的同周目绘卷（至少需要一张现有绘卷），本次没有写入。",
    ],
    [
      /has no numeric Steam account directory|is not a SAVEDATA\?\? directory entry|is not a readable save root directory/i,
      "无法从这个路径识别存档。请用“手动定位”选择 Savedata\\<数字账号>\\SAVEDATAxx\\SAVEDATA.BIN。",
    ],
    [
      /not a shipped container size|must (?:start with|decrypt to a buffer starting with) RNNUSR|save is too small for the fixed inventory region/i,
      "这个文件不是《仁王3》的角色存档，或者存档版本不受支持。存档没有被修改。",
    ],
    [
      /backup account \d+ slot \d+|does not match the save path account/i,
      "这个备份属于其他账号或其他存档栏位，不能恢复到当前存档。",
    ],
    [
      /digest mismatch: expected/i,
      "备份文件校验失败（文件可能被改动或损坏），已停止恢复，当前存档没有被修改。",
    ],
    [
      /commit finished in an unprovable state/i,
      "写入结果无法确认。请先核对存档和操作记录，不要重复添加。",
    ],
    [
      /exactly one .+ must be running, but \d+ were found|AMBIGUOUS_PROCESS/,
      "检测到同时运行了多个《仁王3》，请只保留一个再试。",
    ],
    [
      /OpenProcess\(\d+\) failed with error 5\b/,
      "系统拒绝访问游戏进程。如果游戏或 Steam 是以管理员身份运行的，请也以管理员身份运行本工具。",
    ],
    [
      /module .+ was not found in process/i,
      "游戏还没有完全启动，请进入角色存档后再试。",
    ],
    [
      /GAME_RUNNING|GAME_STATE_UNKNOWN/,
      "请完全关闭《仁王3》后再写入存档。本次没有修改存档。",
    ],
    [/Stop temporary overrides before editing remaining count/, "请先停止临时修改，再核对当前次数。"],
    [/Current scroll state is not supported for count editing/, "这张绘卷的当前状态暂不支持修改次数。"],
    [/FAVORITES_CAPACITY_REACHED/, "收藏夹最多保存 50 张绘卷。"],
    [/CART_CAPACITY_REACHED/, "购物车最多保存 50 张绘卷。"],
    [
      /Possessed is available only for eligible low-pool enemy variants/i,
      "这个敌人没有地狱附身变体，请关闭开关或选择标有“可附身”的低手敌人。",
    ],
    [
      /INVALID_REQUEST:\s*search\.start/i,
      "筛选条件无法提交。请更新应用；若仍出现，请导出反馈文件发给开发者。",
    ],
    [
      /Saved defined record fields differ|Saved inventory serial set differs/i,
      "存档与当前绘卷状态不一致。请先在神社保存，关闭菜单后重新核对添加。",
    ],
    [
      /context has changed|context.*differs|identity does not match|CART_REFERENCE_EXPIRED/i,
      "候选已失效，请按原 ID 重新搜索后添加。",
    ],
    [
      /No.*Nioh3|not.*running|process.*not found/i,
      "请先启动游戏并进入角色存档。",
    ],
    [
      /Pickup queue|not idle|scheduler|menu/i,
      "请关闭游戏菜单，原地等待片刻后重新核对。",
    ],
    [
      /backup.*(missing|changed|invalid|corrupt)|Backup.*(differ|match)/i,
      "自动备份无法通过校验，本次没有继续添加。请重新核对添加。",
    ],
    [
      /Snapshot expired/,
      "存档在游戏里保存后已经变化，请点“重新读取”后再添加。",
    ],
    [
      /Foreign native receipt/,
      "实时添加的状态目录里有不属于本工具操作的回执文件，添加已停止，游戏没有被改动。请导出反馈文件发给开发者。",
    ],
    [
      /was rejected after dispatch|Native builder output differs|Native assembly differs/i,
      "游戏生成的绘卷与预期不一致，这次没有添加，背包和存档都没有改动，可以直接重试。若反复出现，请导出反馈文件发给开发者。",
    ],
    [
      /QueryFullProcessImageNameW|PROCESS_INSTANCE_CHANGED|PROCESS_GONE|replaced by a different process instance|is no longer running/i,
      "游戏已退出或重新启动。请进入角色存档后重新核对添加。",
    ],
    [
      /uncertain|unresolved|already submitted/i,
      "上次操作尚未确认，请先核对结果，不要重复添加。",
    ],
    [
      /APPEND_ONLY_REPAIR_REQUIRED|duplicate serials/i,
      "这个存档里已有绘卷的序列号重复，为避免改动已有绘卷，“添加到存档”已停止，存档没有被修改。请改用“游戏内实时添加”。",
    ],
    [/BUSY|occupied/i, "另一项操作仍在进行，请等待完成。"],
    [
      /not a verified supported version|requires verified|requires accepted|profile changed|UNSUPPORTED_GAME_VERSION|GAME_EXECUTABLE_UNSUPPORTED|PROFILE_NOT_APPROVED|PROFILE_INTEGRITY|PROFILE_UNRESOLVED|SIGNATURE_MISMATCH|unsupported Nioh 3 executable version/i,
      "当前游戏版本尚未支持，请检查更新。",
    ],
    [
      /FAVORITE.*INVALID|Unexpected token.*JSON/,
      "收藏夹文件无法读取，原文件已保留。请导出反馈文件发给开发者。",
    ],
  ];
  for (const [pattern, text] of cases) if (pattern.test(message)) return text;
  if (
    (/^Error(?: invoking remote method|:)/.test(message) ||
      /^[A-Z][A-Z0-9]*(?:_[A-Z0-9]+)+(?::|$)/.test(message)) &&
    !/[\u3400-\u9fff]/.test(message)
  ) {
    const code = message.match(/\b[A-Z][A-Z0-9]*(?:_[A-Z0-9]+)+\b/)?.[0];
    return `操作没有完成。${code ? `错误代码：${code}。` : ""}${FAILURE_ADVICE}`;
  }
  return message;
}

/** The next step every unexplained failure ends with. */
export const FAILURE_ADVICE =
  "涉及写入时，请先核对结果再重试；反复出现请导出反馈文件发给开发者。";

/**
 * A non-empty display text for any rejection value.
 *
 * Tauri commands reject with plain strings and the diagnostic invoker rethrows
 * them unchanged, so `(error as Error).message` is `undefined` for most
 * backend failures and a status line would silently go blank. Structured
 * `{ code, message }` payloads keep their code.
 */
export function errorText(
  error: unknown,
  fallback = "操作没有完成，也没有返回原因；请导出反馈文件发给开发者。",
): string {
  if (error instanceof Error && error.message.trim()) return error.message;
  if (typeof error === "string" && error.trim()) return error;
  if (error !== null && typeof error === "object") {
    const value = error as Record<string, unknown>;
    if (typeof value.message === "string" && value.message.trim())
      return typeof value.code === "string" && value.code.trim()
        ? `${value.code}: ${value.message}`
        : value.message;
    try {
      const text = JSON.stringify(error);
      if (text && text !== "{}") return text;
    } catch {
      // Fall through to the fallback below.
    }
  }
  return fallback;
}
