import { EffectLabel, EffectValue, effectText } from "./effect-presentation";
import React from "react";
import { ADD_CATALOG, nameLabel } from "./equipment-facets";
import { equipmentEffectDetail } from "./loadout-effect-summary";
import type { LoadoutSelection, LoadoutStyle } from "./loadout-state";
import hellSkillNames from "./hell-skill-names.json";
import { equipmentSourceLabel } from "./loadout-equipment";
import { armorRequirements, armorRequirementLabel } from "./loadout-requirements";
import { armorStats, statRangeLabel } from "./loadout-stats";
import { armorRemodelModes, armorRemodelSlots, canRemodelArmor, plannedEquipmentPiece } from "./loadout-remodel";
import { fitsSlot } from "./loadout-equipment";

export const detailSlots = [
  ["melee0", "近战1"], ["melee1", "近战2"], ["ranged", "远程"],
  ["head", "头"], ["body", "身"], ["arms", "手"], ["legs", "腰"], ["feet", "脚"],
  ["accessory0", "饰品1"], ["accessory1", "饰品2"], ["scroll", "绘卷"],
] as const;

export function LoadoutEquipmentDetail({ selection, style, slot, onRemodelChange }: {
  selection: LoadoutSelection; style: LoadoutStyle; slot: string;
  onRemodelChange: (mode: string | null) => void;
}) {
  const original = selection.equipment[style][slot];
  const piece = plannedEquipmentPiece(selection, style, slot);
  const originalMode = armorRemodelModes.find(mode => mode.key === original?.armor_remodel?.join("/"));
  const scroll = slot === "scroll" ? selection.scrolls[style] : null;
  const item = piece && ADD_CATALOG.find(item => item.id === piece.item_id);
  const requirements = piece && armorRequirements(piece, selection.remodelStage);
  const stats = piece && armorStats(piece, selection.remodelStage);
  const effects = scroll ? scroll.effects.map(effect => ({ effect_id: Number(effect.id), value: effect.raw, star: false }))
    : piece?.effects ?? [];
  const zeroPlusArmor = !!piece && piece.plus === 0 && armorRemodelSlots.some(key => key === slot) && fitsSlot(piece, style, slot);
  return <section className="loadout-equipment-detail" aria-label="装备详情">
    <header><h3>装备详情</h3></header>
    {!piece && !scroll ? <p className="loadout-detail-empty">此栏位尚未选择装备。</p> : <>
      <div className="loadout-detail-name">{scroll ? `绘卷 · Seed ${scroll.seed}` : item ? nameLabel(item.name) : `装备 ${piece!.item_id}`}
        {piece?.hell && <span className="loadout-detail-badge">地狱</span>}
      </div>
      <dl className="loadout-detail-fields">
        <div><dt>等级</dt><dd>{piece?.level ?? scroll?.level ?? "—"}</dd></div>
        {piece && <>
          <div><dt>强化值</dt><dd>+{piece.plus}</dd></div>
          <div><dt>稀有度</dt><dd>{piece.rarity}</dd></div>
          <div><dt>爱用度</dt><dd>{piece.familiarity === undefined ? "—" : (piece.familiarity / 100).toLocaleString("zh-CN", { maximumFractionDigits: 2 })}</dd></div>
          <div><dt>来源</dt><dd>{equipmentSourceLabel(piece)}</dd></div>
          {stats && <><div><dt>重量</dt><dd data-detail-weight>{statRangeLabel(stats.weight, 10)}</dd></div><div><dt>坚忍度</dt><dd data-detail-toughness>{statRangeLabel(stats.toughness)}</dd></div></>}
        </>}
      </dl>
      {(canRemodelArmor(selection, style, slot) || zeroPlusArmor) && <div className="loadout-detail-remodel">
        <label>改造方式<select aria-label="选择防具改造" disabled={zeroPlusArmor} value={zeroPlusArmor ? "" : selection.armorRemodels?.[style][slot] ?? ""}
          onChange={event => onRemodelChange(event.target.value || null)}>
          <option value="">原改造：{originalMode?.label ?? (original?.armor_remodel ? "未识别" : "未记录")}</option>
          {armorRemodelModes.map(mode => <option key={mode.key} value={mode.key}>{mode.label}</option>)}
        </select></label>
        {!zeroPlusArmor && selection.armorRemodels?.[style][slot] && <small>配装模拟</small>}
      </div>}
      {(canRemodelArmor(selection, style, slot) || zeroPlusArmor) && <p className="loadout-detail-note" data-remodel-explanation>
        {zeroPlusArmor ? "防具强化值为 +0，无法改造。"
          : piece!.rarity < 3 ? "当前稀有度不产生重量和属性需求的改造增量；坚忍度不随改造变化。"
          : "强化提高属性需求，厚重增加重量；坚忍度不随改造变化。"}
      </p>}
      {requirements && <p className="loadout-detail-note" data-armor-requirements>
        属性需求：{requirements.known ? requirements.values.map(armorRequirementLabel).join(" / ") || "无" : requirements.reason}
        {requirements.known && requirements.reason && <small>（{requirements.reason}）</small>}
      </p>}
      <ul className="loadout-detail-effects">
        {effects.map((effect, index) => {
          const empty = effect.effect_id === 0xffffffff;
          if (empty) return null;
          const detail = equipmentEffectDetail(effect.effect_id, effect.value);
          return <li key={index}><span title={detail.description ? effectText(detail.description) : undefined}>
            {effect.star && !empty && <span className="loadout-detail-star" aria-label="星号词条">✦ </span>}<EffectLabel text={detail.label} />
          </span><EffectValue display={detail.display} raw={effect.value} /></li>;
        })}
      </ul>
      {!effects.some(effect => effect.effect_id !== 0xffffffff) && <p className="loadout-detail-empty">暂无词条。</p>}
      {piece?.hell && <p className="loadout-detail-note">地狱武技：{piece.hell_skill ? (hellSkillNames.skills as Record<string, string>)[String(piece.hell_skill)] ?? `0x${piece.hell_skill.toString(16).toUpperCase()}` : "—"}</p>}
    </>}
  </section>;
}
