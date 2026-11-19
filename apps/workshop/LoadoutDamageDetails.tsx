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
      return <details className="loadout-damage-contacts" data-testid="loadout-damage-contacts">
        <summary>逐击伤害</summary>
        <p className="character-hint">总伤害包含物理和属性伤害。</p>
        <table><thead><tr><th>命中</th><th>物理伤害</th><th>属性伤害</th><th>总伤害</th><th>{referenceOutfit?"较参考穿戴":"较当前穿戴"}</th></tr></thead>
          <tbody>{details.contacts.map((hit,index)=>{
            const before=previous[index];
            const delta=before?.template===hit.template?hit.total-before.total:undefined;
            return <tr key={hit.contact}><th>{hit.contact}</th><td>{hit.physical.toFixed(0)}</td>
              <td>{hit.elemental.toFixed(0)}</td><td>{hit.total.toFixed(0)}</td>
              <td>{delta===undefined?(baseline?.state==="available" && index>=previous.length?<span>新增命中</span>:"—"):`${delta>0?"+":""}${delta.toFixed(0)}`}</td></tr>;
          })}</tbody></table>
      </details>;
    }
  }
}
