/** User-facing next steps; the broker keeps full technical errors in rolling logs. */
export function publicError(message: string): string {
  const cases: [RegExp, string][] = [
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
      "筛选条件无法提交。请更新应用；若仍出现，请复制日志反馈。",
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
      /was rejected after dispatch|Native builder output differs|Native assembly differs/i,
      "游戏生成的绘卷与预期不一致，这次没有添加，背包和存档都没有改动，可以直接重试。若反复出现，请复制日志反馈。",
    ],
    [
      /QueryFullProcessImageNameW|PROCESS_INSTANCE_CHANGED|replaced by a different process instance/i,
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
      /not a verified supported version|requires verified|requires accepted|profile changed/i,
      "当前游戏版本尚未支持，请检查更新。",
    ],
    [
      /FAVORITE.*INVALID|Unexpected token.*JSON/,
      "收藏夹文件无法读取，原文件已保留。请复制日志排查。",
    ],
  ];
  for (const [pattern, text] of cases) if (pattern.test(message)) return text;
  if (
    /^Error(?: invoking remote method|:)/.test(message) &&
    !/[\u3400-\u9fff]/.test(message)
  )
    return "操作未完成，请复制日志排查；涉及写入时，请先核对操作结果。";
  return message;
}

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
  fallback = "操作未完成，且没有返回错误说明；请复制日志排查。",
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
