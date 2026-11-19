import React from "react";
import type { LoadoutContextReady } from "../../packages/contracts/protected-responses";

/** A comparison with the immutable captured baseline,not a causal non-equipment verdict. */
export function LoadoutSourceCheck({ context }: { context: LoadoutContextReady }) {
  if (context.source !== "live") return null;
  const check = context.source_check;
  let comparison: React.ReactNode = null;
  switch (check?.state) {
    case undefined: break;
    case "consistent":
      comparison = <p data-testid="loadout-source-check" data-state="consistent">按当前装备重算的加成与游戏读取一致。</p>;
      break;
    case "reobserved":
      comparison = <div data-testid="loadout-source-check" data-state="reobserved">
        <p>按当前装备重算的加成与游戏读取存在差异，已使用本次读取的状态。换装后的推荐结果需要核对。</p>
        <ul>
          {check.groups.length > 0 && <li>效果变化：{check.groups.length} 项</li>}
          {check.attack_gain && <li>攻击加成：{check.attack_gain.stored} → {check.attack_gain.now}</li>}
          {check.defense && <li>防御加成：{check.defense.stored} → {check.defense.now}</li>}
        </ul>
      </div>;
      break;
  }
  let recording: React.ReactNode = null;
  switch (context.source_recording?.state) {
    case undefined:
    case "recorded": break;
    case "snapshot_only":
      recording = <p data-testid="loadout-source-recording" data-state="snapshot_only">原始诊断记录已保留，记录索引未保存；本次角色读取仍然有效。</p>;
      break;
    case "unavailable":
      recording = <p data-testid="loadout-source-recording" data-state="unavailable">诊断记录未能保存；本次角色读取仍然有效。请保留反馈文件。</p>;
      break;
  }
  return <>
    {comparison}
    {check && context.source_reference?.captured_at && <p data-testid="loadout-source-reference" data-reference-id={context.source_reference.id}>
      参考快照时间：<time dateTime={context.source_reference.captured_at}>{context.source_reference.captured_at}</time>
    </p>}
    {recording}
  </>;
}
