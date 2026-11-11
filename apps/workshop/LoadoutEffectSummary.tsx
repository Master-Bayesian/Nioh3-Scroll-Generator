import { EffectLabel } from "./effect-presentation";
import React from "react";
import { isSpecialEffect, loadoutEffectSummary, nativeEffectGroup, type EffectTotal, type PendingEffect } from "./loadout-effect-summary";
import type { LoadoutSelection, LoadoutStyle } from "./loadout-state";
import { profoundRequirement } from "./loadout-requirements";

function EffectList({ effects, special = false, attributes = [] }: { effects: EffectTotal[]; special?: boolean; attributes?: readonly { value: number }[] }) {
  return <ul className="loadout-effect-list loadout-real-effects">
      {effects.map(effect => {
        const requirement = profoundRequirement(effect.groupId, attributes);
        return <li key={effect.groupId} data-effect-group={effect.groupId} data-profound-active={requirement?.active} className={special ? "special-effect" : ""}>
        <details>
          <summary><EffectLabel text={effect.label} />{(requirement || effect.display) && <strong>{requirement ? requirement.active ? "已启用" : "未启用" : effect.display}</strong>}<small>{effect.sources.length}处</small></summary>
          <ul className="loadout-effect-sources">
            {effect.sources.map((source, index) => <li key={index}><span>{source.source}</span>{source.display && <strong>{source.display}</strong>}</li>)}
          </ul>
          {effect.capped && <p>已按原生词条上限截取。</p>}
          {special && nativeEffectGroup(effect.groupId)?.description_zh && <p><EffectLabel text={nativeEffectGroup(effect.groupId)!.description_zh!} /></p>}
          {requirement && <p>启用条件：{requirement.attribute} ≥ 150；当前 {requirement.current ?? "—"}。</p>}
          {effect.sources.length > 1 && effect.kind === "value" && !special && <p>按同一原生词条组归并，合计不代表最终战斗伤害。</p>}
        </details>
      </li>; })}
    </ul>;
}

function PendingList({ entries }: { entries: PendingEffect[] }) {
  return entries.length > 0 ? <details className="loadout-effect-pending">
    <summary>未计入合计 · {entries.length}项</summary>
    <ul>{entries.map((entry, index) => <li key={index}><strong><EffectLabel text={entry.label} /></strong><span>{entry.source} · {entry.reason}</span></li>)}</ul>
  </details> : null;
}

export function LoadoutEffectSummary({ selection, style, attributes = [] }: { selection: LoadoutSelection; style: LoadoutStyle; attributes?: readonly { value: number }[] }) {
  const { effects, pending } = loadoutEffectSummary(selection, style);
  const specialPending = (entry: PendingEffect) => {
    const group = nativeEffectGroup(entry.groupId ?? -1);
    return isSpecialEffect({ groupId: entry.groupId ?? -1, kind: group && !group.grade && !group.unit_template ? "presence" : "value" });
  };
  const ordinary = effects.filter(effect => !isSpecialEffect(effect));
  const special = effects.filter(isSpecialEffect);
  return <>
    <section className="loadout-subsection" aria-label="效果汇总">
      <details className="loadout-summary-disclosure" open>
      <summary><h3>效果汇总</h3><small>{ordinary.length}项</small></summary>
      <p className="loadout-effect-note">普通词条按当前配置汇总；点击可查看来源。</p>
      {!ordinary.length && <p className="loadout-effect-empty">暂无普通词条。</p>}
      <EffectList effects={ordinary} />
      <PendingList entries={pending.filter(entry => !specialPending(entry))} />
      </details>
    </section>
    <section className="loadout-subsection loadout-special-effects" aria-label="特殊效果">
      <details className="loadout-summary-disclosure" open>
      <summary><h3>特殊效果</h3><small>{special.length}项</small></summary>
      {!special.length && <p className="loadout-effect-empty">暂无特殊效果。</p>}
      <EffectList effects={special} special attributes={attributes} />
      <PendingList entries={pending.filter(specialPending)} />
      </details>
    </section>
  </>;
}
