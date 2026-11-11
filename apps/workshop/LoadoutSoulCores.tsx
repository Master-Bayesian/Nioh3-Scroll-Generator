import { EffectLabel, EffectValue } from "./effect-presentation";
import React from "react";
import { PlannerEquipmentRows, type SelectionSetter } from "./LoadoutPickers";
import { soulCoreDefinition, soulCoreEffects } from "./loadout-soul-cores";
import type { LoadoutSelection, LoadoutStyle } from "./loadout-state";
import { equipmentSourceLabel } from "./loadout-equipment";

export function SoulCorePanel({ style, selection, setSelection }: {
  style: LoadoutStyle; selection: LoadoutSelection; setSelection: SelectionSetter;
}) {
  return <section className="loadout-subsection loadout-soul-section" aria-labelledby="loadout-soul-heading">
    <h3 id="loadout-soul-heading">魂核</h3>
    <div className="loadout-soul-grid">{[0, 1].map(index => {
      const slot = `soul${index}`;
      const piece = selection.equipment.shared[slot];
      const definition = piece && soulCoreDefinition(piece.item_id);
      return <div className="loadout-soul-card" key={slot} data-soul-slot={slot}>
        <PlannerEquipmentRows rows={[{ slot: `魂核${index + 1}`, key: slot }]} style={style} shared selection={selection} setSelection={setSelection} />
        {piece && <>
          <div className="loadout-soul-meta"><span>{`Lv.${piece.level} · +${piece.plus} · 稀有度 ${piece.rarity}`}</span><span>来源：{equipmentSourceLabel(piece)}</span></div>
          <details className="loadout-soul-details"><summary>词条详情 <small>{soulCoreEffects(piece).length}</small></summary>
            <ul className="loadout-effect-list">{soulCoreEffects(piece).map(effect => <li key={effect.index} data-soul-effect={effect.id}>
              <div className="loadout-soul-effect-name"><span>{effect.star && "✦ "}<EffectLabel text={effect.label} /></span><small>{effect.innate ? "固有" : "附加"}</small></div>
              <EffectValue display={effect.display} />
            </li>)}</ul>
            {!definition && <p className="loadout-invalid">魂核身份尚未确认，请更换为当前版本的魂核。</p>}
            {!soulCoreEffects(piece).length && <p>没有词条记录。</p>}
          </details>
        </>}
      </div>;
    })}</div>
  </section>;
}
