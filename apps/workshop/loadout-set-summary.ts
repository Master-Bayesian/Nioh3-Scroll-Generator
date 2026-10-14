import catalog from "./loadout-sets.json";
import effectCatalog from "./loadout-effect-catalog.json";
import itemSets from "./item-sets.json";
import type { LoadoutSelection, LoadoutStyle } from "./loadout-state";
import { fillTemplateSlots, plainGameText } from "./game-text";

export interface SetTier { index: number; required: number; cross_style: boolean; group_id: number; label_zh: string; label_resolved: boolean; value_mode: number; base_raw: number; growth_raw: number }
export interface SetDefinition { key: number; name_zh: string | null; members: number[]; tiers: SetTier[] }
const sets: Record<string, SetDefinition> = catalog.sets;
const markers: Record<string, number> = catalog.effect_to_set;
const effectGroups: Record<string, number> = effectCatalog.effect_to_group;
const fixedMarkers: Record<string, number> = itemSets.items;
const clean = (name: string) => plainGameText(fillTemplateSlots(name, "正面效果", "异常状态"));
const slotNames: Record<string, string> = {
  melee0: "近战1", melee1: "近战2", ranged: "远程", head: "头", body: "身", arms: "手",
  legs: "腰", feet: "脚", accessory0: "饰品1", accessory1: "饰品2",
};
export const isSetMarker = (effectId: number) => markers[String(effectId)] !== undefined;
export const setForEffect = (effectId: number) => sets[String(markers[String(effectId)])];

export interface SetPiece { slot: string; itemId: number; level: number | null }
export interface ComputedSetTier extends SetTier {
  label: string;
  needed: number;
  active: boolean;
  raw: number | null;
  averageLevel: number | null;
  reason: string | null;
}
export interface SetTotal {
  key: number;
  style: LoadoutStyle;
  name: string;
  count: number;
  maxPieces: number;
  reduction: number;
  pieces: SetPiece[];
  tiers: ComputedSetTier[];
}
export interface SetPending { source: string; label: string; reason: string }

/** RVA 0x3FA150: integer native growth curve, scaled /1000, then base. */
export function nativeSetTierValue(tier: SetTier, level: number): number | null {
  if (!Number.isInteger(level) || level < 0 || level > 0xffff) return null;
  const curve = catalog.growth_by_level[Math.min(level, 500)]?.[tier.value_mode];
  if (curve === undefined) return null;
  const product = Math.imul(curve, tier.growth_raw);
  return (Math.trunc(product / 1000) + tier.base_raw) >>> 0;
}

/** Read selected marker records; an item name alone cannot grant a set bonus. */
export function loadoutSetSummary(selection: LoadoutSelection, currentStyle: LoadoutStyle) {
  const totals: SetTotal[] = [];
  const pending: SetPending[] = [];
  for (const style of [currentStyle, currentStyle === "samurai" ? "ninja" : "samurai"] as const) {
    const buckets = new Map<number, SetPiece[]>();
    let reduction = 0;
    const pieces: { slot: string; itemId: number; level: number | null; effects: { effect_id: number; value: number }[] }[] = Object.entries(slotNames).flatMap(([key, slot]) => {
      const piece = selection.equipment[style][key];
      return piece ? [{ slot, itemId: piece.item_id, level: piece.level, effects: piece.effects }] : [];
    });
    const scroll = selection.scrolls[style];
    if (scroll) pieces.push({ slot: "绘卷", itemId: 0, level: scroll.level ?? null,
      effects: scroll.effects.map(effect => ({ effect_id: Number(effect.id), value: effect.raw })) });
    for (const piece of pieces) {
      if (piece.effects.some(effect => effectGroups[String(effect.effect_id)] === catalog.native_contract.reduction_group)) reduction = 1;
      const keys = new Set(piece.effects.flatMap(effect => markers[String(effect.effect_id)] === undefined ? [] : [markers[String(effect.effect_id)]]));
      const source = `${style === "samurai" ? "武士" : "忍者"} · ${piece.slot}`;
      // Multiple different markers in one record have no reviewed legal composition.
      if (keys.size > 1) { pending.push({ source, label: "套装 / 恩宠", reason: "同一装备含有多个组合标记，未计件" }); continue; }
      const key = [...keys][0];
      if (key === undefined) {
        if (fixedMarkers[String(piece.itemId)]) pending.push({ source, label: "套装", reason: "装备缺少对应套装词条，未计件" });
        continue;
      }
      const definition = sets[String(key)];
      const fixed = fixedMarkers[String(piece.itemId)];
      if (fixed && markers[String(fixed)] !== key) {
        pending.push({ source, label: clean(definition.name_zh ?? "套装 / 恩宠"), reason: "装备自带套装与当前组合词条不符，未计件" }); continue;
      }
      if (definition.members.length && !definition.members.includes(piece.itemId)) {
        pending.push({ source, label: clean(definition.name_zh ?? "套装"), reason: "套装词条与装备归属不符，未计件" }); continue;
      }
      if (piece.level !== null && (!Number.isInteger(piece.level) || piece.level <= 0 || piece.level > 0xffff)) {
        pending.push({ source, label: clean(definition.name_zh ?? "套装 / 恩宠"), reason: "装备等级无效，未计件" }); continue;
      }
      const entry = { slot: piece.slot, itemId: piece.itemId, level: piece.level === null ? null : Math.min(piece.level, catalog.native_contract.equipment_level_cap) };
      const bucket = buckets.get(key) ?? [];
      if (definition.members.length) {
        const old = bucket.findIndex(row => row.itemId === entry.itemId);
        if (old < 0) bucket.push(entry);
        else if ((bucket[old].level ?? 0) < (entry.level ?? 0)) bucket[old] = entry;
      } else bucket.push(entry);
      buckets.set(key, bucket);
    }
    for (const [key, members] of buckets) {
      const definition = sets[String(key)];
      const levels = members.map(piece => piece.level).filter((level): level is number => level !== null).sort((a, b) => b - a);
      const missingLevel = levels.length !== members.length;
      const count = Math.min(members.length, 7);
      const tiers = definition.tiers.map(tier => {
        const needed = Math.max(1, tier.required - reduction);
        const active = count + reduction > 1 && count >= needed && (style === currentStyle || tier.cross_style);
        // Native prefix averages retain the final available average for locked tiers.
        const used = Math.min(needed, count);
        const averageLevel = used > 0 && !missingLevel ? Math.floor(levels.slice(0, used).reduce((total, level) => total + level, 0) / used) : null;
        // Native grace bucket keeps seven records; overfull mixed-level ordering needs a reviewed slot mapping.
        const ambiguous = !definition.members.length && members.length > 7 && new Set(levels).size > 1;
        const independent = tier.growth_raw === 0 || tier.value_mode === 0;
        const raw = !tier.label_resolved ? null : independent ? nativeSetTierValue(tier, 0) : averageLevel === null || ambiguous ? null : nativeSetTierValue(tier, averageLevel);
        return { ...tier, label: clean(tier.label_zh), needed, active, raw, averageLevel,
          reason: !tier.label_resolved ? "词条对象尚未解析，未计入合计" : !independent && ambiguous ? "超过七件且装备等级不同，档位数值待确认" : raw === null && missingLevel ? "绘卷等级未保存，档位数值待确认" : raw === null && active ? "档位数值尚未解析" : null };
      });
      if (style !== currentStyle && !tiers.some(tier => tier.cross_style)) continue;
      totals.push({ key, style, name: clean(definition.name_zh ?? `组合 ${key}`), count: members.length,
        maxPieces: Math.max(...definition.tiers.map(tier => tier.required), 1), reduction, pieces: members, tiers });
    }
  }
  return { sets: totals, pending: pending.filter(row => row.source.startsWith(currentStyle === "samurai" ? "武士" : "忍者")) };
}
