import React from "react";
import type { LoadoutDamageDetails } from "../../packages/contracts/protected-responses";

export function DamageSummary({ details }: { details:LoadoutDamageDetails|undefined }) {
  if (details===undefined) return null;
  switch (details.state) {
    case "unavailable":
      return <small className="loadout-damage-unavailable">伤害明细暂不可用</small>;
    case "available":
      return <small className="loadout-damage-summary" data-testid="loadout-damage-summary">
        <span>物理伤害</span> {details.physical.toFixed(0)}{" · "}<span>属性伤害</span> {details.elemental.toFixed(0)}
      </small>;
  }
}

export function DamageContacts({ details,baseline,referenceOutfit=false }: {
  details:LoadoutDamageDetails|undefined;
  baseline?:LoadoutDamageDetails;
  referenceOutfit?:boolean;
}) {
  if (details===undefined) return null;
  switch (details.state) {
    case "unavailable":return null;
    case "available": {
      const previous=baseline?.state==="available"?baseline.contacts:[];
      const byTemplate=new Map<number,number[]>();
      for (const hit of previous) {
        const totals=byTemplate.get(hit.template);
        if (totals) totals.push(hit.total);
        else byTemplate.set(hit.template,[hit.total]);
      }
      const seen=new Map<number,number>();
      return <details className="loadout-damage-contacts" data-testid="loadout-damage-contacts">
        <summary>逐击伤害</summary>
        <p className="character-hint">总伤害包含物理和属性伤害。</p>
        <table><thead><tr><th>命中</th><th>物理伤害</th><th>属性伤害</th><th>总伤害</th><th>{referenceOutfit?"较参考穿戴":"较当前穿戴"}</th></tr></thead>
          <tbody>{details.contacts.map(hit=>{
            const occurrence=seen.get(hit.template)??0;
            seen.set(hit.template,occurrence+1);
            const before=byTemplate.get(hit.template)?.[occurrence];
            const delta=before===undefined?undefined:hit.total-before;
            return <tr key={hit.contact}><th>{hit.contact}</th><td>{hit.physical.toFixed(0)}</td>
              <td>{hit.elemental.toFixed(0)}</td><td>{hit.total.toFixed(0)}</td>
              <td>{delta===undefined?(baseline?.state==="available"?<span>新增命中</span>:"—"):`${delta>0?"+":""}${delta.toFixed(0)}`}</td></tr>;
          })}</tbody></table>
      </details>;
    }
  }
}
