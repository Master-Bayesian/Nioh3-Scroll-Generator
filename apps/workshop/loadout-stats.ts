import stats from "./loadout-stats.json";
import type { LoadoutPiece } from "./loadout-code";
import type { LoadoutSelection, LoadoutStyle } from "./loadout-state";
import { loadoutEffectSummary, mergeNativeEffectValues, nativeEffectGroup, nativeGroupForEffect } from "./loadout-effect-summary";
import { armorStageCaps, profoundRequirement } from "./loadout-requirements";
import { agilityRank, toughnessRank } from "./loadout-ranks";
import { fitsSlot } from "./loadout-equipment";
import { plannedEquipmentPiece } from "./loadout-remodel";
import { loadoutResources } from "./loadout-resources";

const armorSlots = ["head", "body", "arms", "legs", "feet"] as const;
const statSlots = [...armorSlots, "accessory0", "accessory1"] as const;
const modes = [[0, 0], [1, 0], [2, 0], [1, 1], [2, 2], [1, 2]];
const items: Record<string, (number | boolean)[]> = stats.items;
export interface StatRange { minimum: number; maximum: number }
const range = (values: number[]): StatRange => ({ minimum: Math.min(...values), maximum: Math.max(...values) });
const add = (a: StatRange, b: StatRange): StatRange => ({ minimum: a.minimum + b.minimum, maximum: a.maximum + b.maximum });
const zero = (): StatRange => ({ minimum: 0, maximum: 0 });
const uint = (value: number, max: number) => Number.isInteger(value) && value >= 0 && value <= max;

/** Replay the reviewed getter. Missing save-only metadata is bounded, never defaulted. */
export function armorStats(piece: LoadoutPiece, stage = piece.remodel_stage) {
  const row = items[String(piece.item_id)];
  if (!row || !uint(piece.rarity, 5) || !uint(piece.plus, 65535) || !uint(piece.level, 65535) ||
      (piece.native_flags !== undefined && !uint(piece.native_flags, 0xffffffff))) return null;
  const supplied = piece.armor_remodel;
  if (supplied && !modes.some(mode => mode[0] === supplied[0] && mode[1] === supplied[1])) return null;
  const caps = armorStageCaps(stage);
  if (!caps) return null;
  const counts = piece.rarity < 3 ? [0] : (supplied ? [supplied.filter(value => value === 2).length] : [0, 1, 2]);
  const weight = range(counts.flatMap(count => caps.map(cap =>
    Number(row[piece.rarity >= 4 ? 1 : 0]) + Math.floor(stats.weight_coefficients[Math.min(piece.plus, cap)] * count / 2))));
  const base = Number(row[piece.rarity >= 4 ? 3 : 2]);
  const flags = piece.native_flags === undefined ? [0, 0x40000, 0x240000] : [piece.native_flags];
  const toughness = range(flags.map(flag => {
    const cold = stats.cold_rows[Number(row[4])];
    if (!(flag & 0x40000) || !cold) return base;
    const factor = flag & 0x200000 ? 0 : Math.min(piece.level, 180);
    const candidate = Math.trunc(Math.fround(Math.fround(factor * Math.fround(cold[0] * Math.fround(0.01))) + cold[1]));
    return Math.min(base, candidate);
  }));
  return { weight, toughness, redDemon: row[5] === true };
}

export function statRangeLabel(value: StatRange | null, divisor = 1) {
  if (!value) return "待确认";
  const format = (number: number) => divisor === 10 ? (number / 10).toFixed(1) : String(number);
  return `${format(value.minimum)}${value.maximum === value.minimum ? "" : `–${format(value.maximum)}`}`;
}

/** Native armor/accessory totals plus active effect contributions, independently per style. */
export function loadoutStats(selection: LoadoutSelection, style: LoadoutStyle, attributes: readonly { value: number }[]) {
  const summary = loadoutEffectSummary(selection, style);
  const resources = loadoutResources(selection, style, attributes, summary);
  const reasons: string[] = [];
  function contribution(groupId: number): StatRange {
    const total = summary.effects.find(effect => effect.groupId === groupId);
    const pending = summary.pending.filter(effect => effect.groupId === groupId);
    const unresolved = pending.filter(effect => !effect.reason.includes("未满足") &&
      effect.reason !== "数值或单位尚未解析" && effect.reason !== "合并后的数值尚无法安全显示");
    const known = pending.filter(effect => effect.reason === "数值或单位尚未解析" || effect.reason === "合并后的数值尚无法安全显示");
    const group = nativeEffectGroup(groupId);
    const values = [...(total ? [total.raw] : []), ...known.flatMap(effect => effect.raw === undefined ? [] : [effect.raw])];
    const raw = values.length && group ? mergeNativeEffectValues(group, values).raw | 0 : 0;
    if (unresolved.length) {
      reasons.push("状态相关条件词条尚未确认");
      if (unresolved.some(effect => effect.raw === undefined)) return { minimum: NaN, maximum: NaN };
      const possible = unresolved.reduce((sum, effect) => sum + (effect.raw! | 0), 0);
      return range([raw, raw + possible]);
    }
    return { minimum: raw, maximum: raw };
  }
  let weight: StatRange | null = zero(), toughness: StatRange | null = zero();
  let redCount = 0;
  for (const slot of statSlots) {
    const piece = plannedEquipmentPiece(selection, style, slot);
    if (!piece) continue;
    const result = fitsSlot(piece, style, slot) ? armorStats(piece, selection.remodelStage) : null;
    if (!result) {
      reasons.push("装备数据超出已验证计算范围"); weight = null; toughness = null;
      continue;
    }
    if (weight) weight = add(weight, result.weight);
    if (toughness) toughness = add(toughness, result.toughness);
    if (armorSlots.some(key => key === slot) && result.redDemon) redCount++;
    if (result.weight.minimum !== result.weight.maximum) reasons.push(piece.armor_remodel ? "厚重改造的阶段上限待确认" : "装备码或收藏未记录装备改造状态");
    if (result.toughness.minimum !== result.toughness.maximum) reasons.push("防具特殊状态标记未记录");
  }
  const bonus = contribution(0xECC1);
  toughness = toughness && add(toughness, bonus);
  if (toughness && (!Number.isFinite(toughness.minimum) || toughness.minimum < 0)) toughness = null;
  const stamina = attributes[2]?.value;
  const baseCapacity = uint(stamina, 150) && stamina >= 5 ? stats.capacity_raw[style][stamina - 1] : undefined;
  const capacityBonus = contribution(0xB3FE);
  const capacity = baseCapacity === undefined || !Number.isFinite(capacityBonus.minimum) ? null : range(
    [capacityBonus.minimum, capacityBonus.maximum].map(raw => baseCapacity + Math.trunc(
      Math.fround(Math.fround(Math.fround(baseCapacity) * Math.fround(raw)) * Math.fround(0.001)))));
  const ultimateGroup = nativeGroupForEffect(0x23E8)!;
  const ultimate = summary.effects.some(effect => effect.groupId === ultimateGroup) && !!profoundRequirement(ultimateGroup, attributes)?.active;
  const redEffect = summary.effects.some(effect => effect.groupId === 0xE63F);
  const redPromotion = redEffect && redCount === 5;
  const agilityValues = weight && capacity && capacity.minimum > 0 ? [
    agilityRank(weight.minimum, capacity.maximum, ultimate, redPromotion),
    agilityRank(weight.maximum, capacity.minimum, ultimate, redPromotion),
  ] : [];
  const toughnessValues = toughness ? [toughnessRank(toughness.minimum, ultimate), toughnessRank(toughness.maximum, ultimate)] : [];
  const singleRank = (values: (string | null)[]) => values.length && values[0] && values.every(value => value === values[0]) ? values[0] : null;
  const ratio = weight && capacity && capacity.minimum > 0 ? {
    minimum: weight.minimum / capacity.maximum * 100, maximum: weight.maximum / capacity.minimum * 100,
  } : null;
  return { weight, capacity, ratio, toughness, agility: singleRank(agilityValues), toughnessRank: singleRank(toughnessValues),
    ultimate, redPromotion, resources, reasons: [...new Set([...reasons, ...resources.reasons])] };
}
