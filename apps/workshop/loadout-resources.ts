import tables from "./loadout-resources.json";
import { loadoutEffectSummary, mergeNativeEffectValues, nativeEffectGroup } from "./loadout-effect-summary";
import type { LoadoutSelection, LoadoutStyle } from "./loadout-state";
import type { StatRange } from "./loadout-stats";
import { armorRequirements } from "./loadout-requirements";
import { plannedEquipmentPiece } from "./loadout-remodel";
import { fitsSlot } from "./loadout-equipment";

const f32 = Math.fround;
const bounds = (values: number[]): StatRange => ({ minimum: Math.min(...values), maximum: Math.max(...values) });
const armorSlots = ["head", "body", "arms", "legs", "feet"];
export const resourceGroups = { life: 0xB1B6, ki: 0x7F69, stamina: 0x5D3A, gauge: 0xB95E } as const;

/** The planner supplies ordinary allocated attributes with no level-sync/context correction. */
export function playerResourceBase(level: number, constitution: number, heart: number, stamina: number) {
  if (!Number.isInteger(level) || level < 1 || level > 450 ||
      [constitution, heart, stamina].some(value => !Number.isInteger(value) || value < 5 || value > 150)) return null;
  const staminaHP = tables.stamina_hp[stamina - 1];
  return { hp: tables.level_hp[level - 1] + tables.constitution_hp[constitution - 1] + staminaHP,
    ki: tables.heart_ki[heart - 1], staminaHP };
}

/** FA3E uses point 1 and the Stamina term, not total HP; every multiply is float32. */
export function lifeReflectionBonus(stamina: number, selector: number | null): number {
  if (selector === null) return 0;
  const percent = tables.life_reflection_percent[Math.min(selector >>> 0, 17)];
  return Math.trunc(f32(f32(percent * f32(0.01)) * f32(tables.stamina_hp[stamina - 1])));
}

/** CBE1 is added after Ki multipliers; negative modifier gates are inactive. */
export function amritaKiBonus(selector: number | null, percent: number): number {
  if (selector === null || (selector | 0) < 0) return 0;
  const grade = Math.min(selector >>> 0, 17);
  const point = Math.trunc(f32(f32(f32(percent) / f32(100)) * f32(1000)));
  const weight = f32(f32(point) / f32(1000));
  const lower = tables.amrita_ki_start[grade], upper = tables.amrita_ki_full[grade];
  return Math.trunc(f32(f32(weight * f32(upper - lower)) + lower));
}

/** Active, current-style sources only. Unknown gates produce bounds, never a silent activation. */
export function loadoutResources(selection: LoadoutSelection, style: LoadoutStyle, attributes: readonly { value: number }[],
  summary = loadoutEffectSummary(selection, style)) {
  const reasons: string[] = [], notes: string[] = [];
  const level = attributes.reduce((sum, row) => sum + row.value, 0) - 35;
  const base = attributes.length === 7 && attributes.every(row => Number.isInteger(row.value) && row.value >= 5 && row.value <= 150)
    ? playerResourceBase(level, attributes[0].value, attributes[1].value, attributes[2].value) : null;
  function armorGate(source: { slot?: string }): "active" | "inactive" | "unknown" {
    if (!source.slot || !armorSlots.includes(source.slot)) return "active";
    const piece = plannedEquipmentPiece(selection, style, source.slot);
    if (!piece || !fitsSlot(piece, style, source.slot)) return "unknown";
    const requirement = armorRequirements(piece, selection.remodelStage);
    if (!requirement?.known) return "unknown";
    if (requirement.values.some(value => (attributes[value.index]?.value ?? 0) < value.minimum)) {
      notes.push("防具属性需求未满足，相关体力或精力词条未计入");
      return "inactive";
    }
    return requirement.values.some(value => (attributes[value.index]?.value ?? 0) < value.maximum) ? "unknown" : "active";
  }
  function values(groupId: number): (number | null)[] | null {
    const group = nativeEffectGroup(groupId)!;
    const active: number[] = [], optional: number[] = [];
    for (const source of summary.effects.find(effect => effect.groupId === groupId)?.sources ?? []) {
      const gate = armorGate(source);
      if (gate !== "inactive") (gate === "active" ? active : optional).push(source.raw);
    }
    for (const source of summary.pending.filter(effect => effect.groupId === groupId)) {
      if (source.reason.startsWith("未满足地狱之楔条件")) continue;
      if (source.raw === undefined) {
        reasons.push("体力或精力词条数值待确认");
        return null;
      }
      const knownValue = source.reason === "数值或单位尚未解析" || source.reason === "合并后的数值尚无法安全显示";
      const gate = knownValue ? armorGate(source) : "unknown";
      if (gate !== "inactive") (gate === "active" ? active : optional).push(source.raw);
    }
    if ([...active, ...optional].some(value => !Number.isInteger(value) || value < -0x80000000 || value > 0xffffffff) || optional.length > 10) {
      reasons.push("体力或精力词条数值待确认");
      return null;
    }
    if (optional.length) reasons.push("体力或精力词条生效条件待确认");
    const variants = Array.from({ length: 2 ** optional.length }, (_, mask) => [...active, ...optional.filter((_, index) => mask & (1 << index))]);
    return [...new Set(variants.map(raw => raw.length ? mergeNativeEffectValues(group, raw).raw : null))];
  }
  const hpFlat = values(resourceGroups.life), kiFlat = values(resourceGroups.ki);
  const reflection = values(resourceGroups.stamina), gauge = values(resourceGroups.gauge);
  const usesAmritaGauge = gauge === null || gauge.some(raw => raw !== null && (raw | 0) >= 0);
  const percent = selection.amritaGaugePercent;
  const validGauge = percent === undefined || percent === null || Number.isFinite(percent) && percent >= 0 && percent <= 100;
  const gaugePoints = percent == null ? [0, 100] : [percent];
  if (usesAmritaGauge && percent == null) notes.push("精华槽未设置，精力显示空槽至满槽范围");
  let hp = base && hpFlat && reflection ? bounds(hpFlat.flatMap(raw => reflection.map(grade =>
    base.hp + (raw === null ? 0 : raw | 0) + lifeReflectionBonus(attributes[2].value, grade)))) : null;
  let ki = base && kiFlat && gauge && validGauge ? bounds(kiFlat.flatMap(raw => gauge.flatMap(grade => gaugePoints.map(point =>
    f32(f32(base.ki + (raw === null ? 0 : raw | 0)) + f32(amritaKiBonus(grade, point))))))) : null;
  if (hp && (hp.minimum < 0 || hp.maximum > 0xffffffff || !Number.isFinite(hp.maximum))) hp = null;
  if (ki && (ki.minimum < 0 || ki.maximum > 0xffffffff || !Number.isFinite(ki.maximum))) ki = null;
  return { hp, ki, base, usesAmritaGauge, reasons: [...new Set(reasons)], notes: [...new Set(notes)] };
}
