import { EffectLabel, EffectValue, effectText } from "./effect-presentation";
import React from "react";
import { loadoutSetSummary } from "./loadout-set-summary";
import { formatNativeEffectValue, nativeEffectGroup } from "./loadout-effect-summary";
import type { LoadoutSelection, LoadoutStyle } from "./loadout-state";

export function LoadoutSetSummary({ selection, style }: { selection: LoadoutSelection; style: LoadoutStyle }) {
  const { sets, pending } = loadoutSetSummary(selection, style);
  return <section className="loadout-subsection" aria-label="套装 / 恩宠">
    <h3>套装 / 恩宠</h3>
    {!sets.length && <p className="loadout-effect-empty">尚未配置套装或恩宠。</p>}
    <ul className="loadout-set-list loadout-real-sets">
      {sets.map(set => <li key={`${set.style}-${set.key}`} data-set-key={set.key} data-set-style={set.style}>
        <details>
          <summary><span>{set.name}{set.style !== style && <small>另一风格</small>}</span><strong>{set.count} / {set.maxPieces}</strong><small>{set.tiers.filter(tier => tier.active).length}档生效</small></summary>
          {set.reduction > 0 && <p className="loadout-set-note">所需装备数 −1（不叠加；显示实际件数）</p>}
          <ul className="loadout-set-tiers">
            {set.tiers.map(tier => {
              const group = nativeEffectGroup(tier.group_id);
              const display = group && tier.raw !== null ? formatNativeEffectValue(group, tier.raw) : null;
              const status = !tier.active ? "未激活" : display === null ? "数值待确认" : "已激活";
              return <li key={tier.index} data-set-tier={tier.index} data-active={tier.active} className={tier.active ? "active" : ""}>
                <span className="loadout-set-threshold">{tier.required}件{set.reduction > 0 && <small>需{tier.needed}件</small>}</span>
                <span title={tier.reason ? effectText(tier.reason) : undefined}><EffectLabel text={tier.label} />{tier.cross_style && <small>两风格通用</small>}</span>
                <EffectValue display={display} raw={tier.raw ?? undefined} /><small>{status}</small>
              </li>;
            })}
          </ul>
          <p className="loadout-set-note">计件来源：{set.pieces.map(piece => piece.slot).join("、")}</p>
        </details>
      </li>)}
    </ul>
    {pending.length > 0 && <p className="loadout-set-note">有 {pending.length} 项组合归属待确认，详见效果汇总的“未计入合计”。</p>}
  </section>;
}
