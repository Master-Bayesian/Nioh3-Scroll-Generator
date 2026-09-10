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
      /uncertain|unresolved|already submitted/i,
      "上次操作尚未确认，请先核对结果，不要重复添加。",
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
