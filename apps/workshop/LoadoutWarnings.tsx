import { EffectLabel } from "./effect-presentation";
import React from "react";
import { loadoutWarnings } from "./loadout-warnings";
import type { LoadoutSelection, LoadoutStyle } from "./loadout-state";
export function LoadoutWarnings({ selection, style, attributes }: {
  selection: LoadoutSelection; style: LoadoutStyle; attributes: { value: number }[];
}) {
  const rows = loadoutWarnings(selection, style, attributes);
  const conflicts = rows.filter(row => row.severity === "conflict"), pending = rows.filter(row => row.severity === "pending");
  return <details className="loadout-warning-disclosure" open={conflicts.length > 0} aria-label="配装检查">
    <summary>警告 Tips <small>{`冲突 ${conflicts.length} · 待确认 ${pending.length}`}</small></summary>
    {!conflicts.length && <p>当前已接入检查未发现冲突。</p>}
    {[conflicts, pending].map((group, index) => group.length > 0 && <section key={index}>
      <h4>{index === 0 ? "需要调整" : "待确认"}</h4>
      <ul>{group.map((row, index) => <li key={index} data-warning-code={row.code} data-severity={row.severity}>
        <strong><EffectLabel text={row.source} /></strong><span><EffectLabel text={row.message} /></span>
      </li>)}</ul>
    </section>)}
  </details>;
}
