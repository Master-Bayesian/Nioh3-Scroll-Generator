import catalog from "./loadout-effect-catalog.json";
import setCatalog from "./loadout-sets.json";
import itemNames from "./item-names.json";
import { isSetMarker, loadoutSetSummary } from "./loadout-set-summary";
import { data } from "./model";
import { plainGameText } from "./game-text";
import { clanProtectionValue, clanRanks, guardianEffects, type LoadoutSelection, type LoadoutStyle } from "./loadout-state";

export type EffectGroup = (typeof catalog.groups)[keyof typeof catalog.groups];
const groups: Record<string, EffectGroup> = { ...catalog.groups };
for (const [key, template] of Object.entries(setCatalog.unit_templates)) {
  const group = groups[key];
  if (group) groups[key] = { ...group, unit_template: template,
    format_supported: [0, 1, 2].includes(group.decimal_mode) };
}
const identities: Record<string, number> = catalog.effect_to_group;
const names = new Map(data.editorEffects.map(effect => [Number(effect.id), effect.name]));
const items: Record<string, string[]> = itemNames.items;
const slotNames: Record<string, string> = {
  melee0: "近战1", melee1: "近战2", ranged: "远程", head: "头", body: "身", arms: "手",
  legs: "腰", feet: "脚", accessory0: "饰品1", accessory1: "饰品2", soul0: "魂核1", soul1: "魂核2",
};
const cleanName = (name: string) => plainGameText(name);
function resolvedLabel(name: string | undefined, group: EffectGroup | undefined, fallback: string) {
  const native = group?.label_zh;
  return cleanName(name && !name.includes("{}") ? name : native ?? name ?? fallback);
}
const hex = (id: number) => `0x${id.toString(16).toUpperCase().padStart(4, "0")}`;
export interface EffectSource {
  source: string;
  label: string;
  raw: number;
  effectId?: number;
  groupId: number;
  display: string | null;
  slot?: string;
}
export interface EffectTotal {
  groupId: number;
  label: string;
  display: string;
  kind: "value" | "presence";
  raw: number;
  capped: boolean;
  sources: EffectSource[];
}
export interface PendingEffect { source: string; label: string; reason: string; raw?: number; groupId?: number; effectId?: number; slot?: string }

export function nativeEffectGroup(id: number): EffectGroup | undefined { return groups[String(id)]; }
export function nativeGroupForEffect(id: number): number | undefined { return identities[String(id)]; }

/** Mechanisms remain separate even when their native representation has a number. */
export function isSpecialEffect(effect: Pick<EffectTotal, "groupId" | "kind">): boolean {
  return effect.kind === "presence" || effect.groupId === setCatalog.native_contract.reduction_group
    || effect.groupId === setCatalog.native_contract.secondary_guardian_group;
}

/** Format individual equipment slots without merging away their identity or stars. */
export function equipmentEffectDetail(effectId: number, raw: number) {
  const group = nativeEffectGroup(nativeGroupForEffect(effectId) ?? -1);
  const label = resolvedLabel(names.get(effectId), group, `词条 ${hex(effectId)}`);
  return { label, display: isSetMarker(effectId) ? "" : group ? formatNativeEffectValue(group, raw) : null,
    description: group?.description_zh ? cleanName(group.description_zh) : null };
}

/** Native PC v2.02 units plus the verified presence-based set-requirement mechanism. */
export function formatNativeEffectValue(group: EffectGroup, raw: number): string | null {
  if (!Number.isInteger(raw) || raw < -0x80000000 || raw > 0xffffffff) return null;
  // The set consumer tests group presence, not raw strength; duplicates never stack.
  if (group.id === setCatalog.native_contract.reduction_group) return "−1";
  if (!group.format_supported) return null;
  const unsigned = raw >>> 0;
  if (group.grade) return catalog.grade_labels[Math.max(0, 17 - unsigned)];
  if (!group.unit_template) return "";
  const signed = unsigned | 0;
  if (group.format_kind === "spirit_integer_decimal") return group.unit_template.replace("{}", `${signed}.0`).replace(/^\+-(?=\d)/, "-");
  if (group.decimal_mode !== 0 && signed < 0) return null;
  const divisor = 10 ** group.decimal_mode;
  const parts = group.decimal_mode === 0 ? [String(signed), "0"]
    : [String(Math.floor(signed / divisor)), String(signed % divisor).padStart(group.decimal_mode, "0")];
  let result = group.unit_template.replaceAll("<dot>", ".");
  for (const part of parts) result = result.replace("{}", part);
  return result.includes("{}") ? null : result.replace(/^\+-(?=\d)/, "-");
}

/** Apply the captured native group accumulator in deterministic slot order. */
export function mergeNativeEffectValues(group: EffectGroup, values: number[]) {
  let raw = values[0] >>> 0;
  let capped = false;
  if (group.merge_kind === "grade") {
    raw = (Math.max(...values.map(value => value >>> 0)) + ((values.length - 1) & 0xffff)) >>> 0;
  } else {
    for (const value of values.slice(1)) {
      raw = group.merge_kind === "max" ? Math.max(raw, value >>> 0) : (raw + (value >>> 0)) >>> 0;
      if (group.merge_kind === "sum" && group.raw_cap_i32 >= 0 && raw > group.raw_cap_i32) {
        raw = group.raw_cap_i32; capped = true;
      }
    }
  }
  return { raw, capped };
}

export function loadoutEffectSummary(selection: LoadoutSelection, style: LoadoutStyle) {
  const sources: EffectSource[] = [];
  const pending: PendingEffect[] = [];
  function add(groupId: number | undefined, raw: number, source: string, label: string, effectId?: number, slot?: string) {
    label = cleanName(label);
    const group = groupId === undefined ? undefined : groups[String(groupId)];
    const display = group && formatNativeEffectValue(group, raw);
    if (!group || display === null || display === undefined || label.includes("{}")) {
      pending.push({ source, label, raw, groupId, effectId, ...(slot ? { slot } : {}),
        reason: !group ? "词条组未识别" : label.includes("{}") ? "词条对象尚未解析" : "数值或单位尚未解析" });
      return;
    }
    sources.push({ source, label, raw: raw >>> 0, groupId: group.id, effectId, display, ...(slot ? { slot } : {}) });
  }
  for (const [bucket, slots] of [[style, Object.keys(slotNames).filter(key => !key.startsWith("soul"))], ["shared", ["soul0", "soul1"]]] as const) {
    for (const slot of slots) {
      const piece = selection.equipment[bucket][slot];
      if (!piece) continue;
      const source = `${slotNames[slot]} · ${cleanName(items[String(piece.item_id)]?.[0] ?? `装备 ${hex(piece.item_id)}`)}`;
      for (const effect of piece.effects) {
        if (effect.effect_id === 0xffffffff || isSetMarker(effect.effect_id)) continue;
        const groupId = identities[String(effect.effect_id)];
        add(groupId, effect.value, source, resolvedLabel(names.get(effect.effect_id), groups[String(groupId)], `词条 ${hex(effect.effect_id)}`), effect.effect_id, slot);
      }
    }
  }
  const scroll = selection.scrolls[style];
  if (scroll) for (const effect of scroll.effects) {
    const id = Number(effect.id);
    if (effect.role === "恩宠" || id === 0xffffffff || isSetMarker(id)) continue;
    add(identities[String(id)], effect.raw, "绘卷", resolvedLabel(effect.name, groups[String(identities[String(id)])], `词条 ${hex(id)}`), id);
  }
  selection.clans[style].forEach(choice => {
    const effect = choice && clanProtectionValue(selection, choice);
    if (!choice || !effect) return;
    const relation = selection.joinedClan === choice.clanId ? "current_clan" : "other_clan";
    add(effect.effect_group_id, choice.rankIndex === null ? choice.savedRaw! : effect.raw_values_by_rank_index[relation][choice.rankIndex],
      `武家 · ${effect.clan_name} · ${choice.rankIndex === null ? "存档数值" : clanRanks[choice.rankIndex].name_zh}`, effect.label_zh);
  });
  const setSummary = loadoutSetSummary(selection, style);
  pending.push(...setSummary.pending);
  for (const set of setSummary.sets) for (const tier of set.tiers) {
    if (!tier.active) continue;
    const source = `${set.style === "samurai" ? "武士" : "忍者"} · ${set.name} · ${tier.required}件效果`;
    if (tier.raw === null) pending.push({ source, label: tier.label, groupId: tier.group_id, reason: tier.reason ?? "档位数值尚未解析" });
    else add(tier.group_id, tier.raw, source, tier.label);
  }
  const doubleGuardian = setSummary.sets.some(set => set.tiers.some(tier => tier.active && tier.group_id === 0x8508 && tier.raw !== null && (tier.raw | 0) >= 0));
  const guardianStyles: LoadoutStyle[] = doubleGuardian ? [style, style === "samurai" ? "ninja" : "samurai"] : [style];
  for (const guardianStyle of guardianStyles) for (const effect of guardianEffects(selection, guardianStyle)) {
    const source = `${guardianStyle !== style ? "双灵庇佑 · " : ""}守护灵 · ${effect.guardian_name} · 等级 ${selection.guardians[guardianStyle]!.level}`;
    const raw = effect.raw_values_by_level_index[selection.guardians[guardianStyle]!.level];
    if (effect.activation === "active") add(effect.effect_group_id, raw, source, effect.label_zh);
    else pending.push({ source, label: effect.label_zh, raw: raw >>> 0, groupId: effect.effect_group_id,
      reason: effect.activation === "unknown" ? `地狱之楔破坏数待确认（需要 ${effect.activation_threshold}）` : `未满足地狱之楔条件（需要 ${effect.activation_threshold}）` });
  }
  const buckets = new Map<number, EffectSource[]>();
  for (const source of sources) buckets.set(source.groupId, [...(buckets.get(source.groupId) ?? []), source]);
  const effects: EffectTotal[] = [];
  for (const [groupId, entries] of buckets) {
    const group = groups[String(groupId)];
    const merged = mergeNativeEffectValues(group, entries.map(entry => entry.raw));
    const display = formatNativeEffectValue(group, merged.raw);
    if (display === null) {
      for (const entry of entries) pending.push({ ...entry, reason: "合并后的数值尚无法安全显示" });
      continue;
    }
    // Prefer resolved effect names; a generic group label can lose buff arguments.
    const label = entries[0].label;
    effects.push({ groupId, label, display, kind: display === "" ? "presence" : "value", ...merged, sources: entries });
  }
  return { effects, pending };
}
