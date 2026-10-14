import { fitsSlot, sameLoadoutPiece } from "./loadout-equipment";
import type { LoadoutPiece } from "./loadout-code";
import { soulCoreDefinition } from "./loadout-soul-cores";
import { GUARDIAN_LEVEL_MAX, guardianEffects, guardiansForStyle, clans, clanRanks, savedClanProtectionDisplay, essenceLimit, skills, skillUsage, type LoadoutSelection, type LoadoutStyle, type SkillCategory } from "./loadout-state";
import { loadoutSetSummary } from "./loadout-set-summary";
import { loadoutEffectSummary } from "./loadout-effect-summary";
import { data } from "./model";
import { armorRequirements, armorRequirementLabel, profoundRequirement } from "./loadout-requirements";
import { loadoutStats } from "./loadout-stats";
import { plannedEquipmentPiece } from "./loadout-remodel";

export interface LoadoutWarning { code: string; severity: "conflict" | "pending"; source: string; message: string }
const categories: Record<SkillCategory, string> = { common: "共通技能", samurai: "武士技能", ninja: "忍者技能" };
const slots: Record<string, string> = { melee0: "近战1", melee1: "近战2", ranged: "远程", head: "头", body: "身", arms: "手", legs: "腰", feet: "脚", accessory0: "饰品1", accessory1: "饰品2", soul0: "魂核1", soul1: "魂核2" };
const knownEffects = new Set(data.editorEffects.map(row => Number(row.id)));
const auditLabels: Record<string, string> = {
  unknown_item: "物品未识别", unsupported_rarity: "稀有度超出自然范围", effect_count: "词条数量与稀有度不符",
  unknown_effect: "词条未识别", missing_innate: "缺少固有词条", missing_set: "缺少固有套装词条", missing_grace: "缺少恩宠词条",
  unexpected_fixed: "多出套装或恩宠词条", not_in_pool: "词条不属于该装备词条池", group_conflict: "同一装备词条互斥",
  value_not_natural: "词条数值不是自然生成值", value_above_formula: "词条数值高于公式，额外加成待确认",
  star_below_rarity: "星号词条稀有度不足", multiple_stars: "星号词条数量超出自然范围", star_flag_mismatch: "星号标记不符",
  hell_effect_on_normal: "普通装备含地狱词条", missing_hell_effect: "缺少地狱词条", hell_on_ineligible_item: "该物品不能成为地狱武器",
  roll_out_of_range: "词条分位超出自然范围", replaced_effect: "词条存在替换痕迹",
};

/** Bounded checks: an empty conflict list is never proof of in-game legality. */
export function loadoutWarnings(selection: LoadoutSelection, style: LoadoutStyle, attributes: readonly { value: number }[]): LoadoutWarning[] {
  const rows: LoadoutWarning[] = [];
  const add = (code: string, severity: LoadoutWarning["severity"], source: string, message: string) => rows.push({ code, severity, source, message });
  const stats = loadoutStats(selection, style, attributes);
  if (stats.reasons.length) add("status_pending", "pending", "当前状态", `${stats.reasons.join("；")}。显示范围中的数值尚不能确定为单一结果。`);
  if (stats.weight && stats.capacity && stats.weight.minimum > stats.capacity.maximum) add("overload", "conflict", "负重", "当前防具重量超过负重上限，敏捷为 D。");
  const validAttributes = attributes.length === 7 && attributes.every(row => Number.isInteger(row.value) && row.value >= 5 && row.value <= 150);
  const level = attributes.reduce((sum, row) => sum + row.value, 0) - 35;
  if (!validAttributes || level < 1 || level > 450) add("attributes", "conflict", "角色属性", "七项属性应为 5–150 的整数，属性合计对应等级应为 1–450。");
  for (const category of ["common", style] as const) {
    const used = skillUsage(selection, category), capacity = selection.capacities[category];
    if (capacity !== null && (!Number.isInteger(capacity) || capacity < 0 || used.cost > capacity)) add("capacity", "conflict", categories[category], `已用容量 ${used.cost}，当前上限 ${capacity}。请移除技能或核对容量。`);
    const limit = essenceLimit(selection, style, category);
    if (used.essences > limit) add("essence", "conflict", categories[category], `已选精髓 ${used.essences} / ${limit}。请移除精髓，或配置对应类型的精髓并存词条。`);
    if (new Set(selection.skills[category]).size !== selection.skills[category].length) add("skill_duplicate", "conflict", categories[category], "同一技能被重复选择，请移除重复项。");
    if (selection.skills[category].some(id => !skills.some(skill => skill.id === id && skill.category === category && skill.active_styles.includes(style)))) add("skill_identity", "conflict", categories[category], "存在未识别或不属于当前技能类型的技能。");
  }
  const seen: LoadoutPiece[] = [];
  for (const [bucket, equipment] of [[style, selection.equipment[style]], ["shared", selection.equipment.shared]] as const) {
    for (const [slot, piece] of Object.entries(equipment)) {
      const source = slots[slot] ?? slot;
      if (!fitsSlot(piece, style, slot)) add("equipment_slot", "conflict", source, "装备种类或风格与当前栏位不符，请更换装备。");
      if (seen.some(other => sameLoadoutPiece(other, piece))) add("equipment_duplicate", "conflict", source, "同一件装备已在当前配置中使用，不能重复占用栏位。");
      seen.push(piece);
      const requirements = armorRequirements(bucket === style ? plannedEquipmentPiece(selection, style, slot) : piece, selection.remodelStage);
      if (requirements) {
        if (!requirements.known) add("armor_requirements_pending", "pending", source, `${requirements.reason}。`);
        else {
          const unmet = requirements.values.filter(value => (attributes[value.index]?.value ?? 0) < value.minimum);
          if (unmet.length) add("armor_requirements", "conflict", source,
            `防具属性需求未满足：${unmet.map(value => `${armorRequirementLabel(value)}（当前 ${attributes[value.index]?.value ?? "—"}）`).join("、")}。`);
          const uncertain = requirements.values.filter(value => {
            const current = attributes[value.index]?.value;
            return value.minimum !== value.maximum && current >= value.minimum && current < value.maximum;
          });
          if (uncertain.length) add("armor_requirements_pending", "pending", source,
            `${requirements.reason}：${uncertain.map(armorRequirementLabel).join("、")}；尚不能确认属性条件已满足。`);
        }
      }
      if (piece.effects.some(effect => effect.effect_id !== 0xffffffff && !knownEffects.has(effect.effect_id))) add("unknown_effect", "pending", source, "含有未识别词条，相关效果不能完整计算。");
      const audit = piece.audit;
      if (audit) {
        const verdict = audit.verdict ?? (audit.natural ? "natural" : "unnatural");
        for (const finding of audit.findings) add(`audit_${finding.code}`, verdict === "unnatural" ? "conflict" : "pending", source,
          `${auditLabels[finding.code] ?? `装备检查：${finding.code}`}${finding.slot === undefined ? "" : `（词条 ${finding.slot + 1}）`}。`);
        for (const finding of audit.unverified ?? []) add(`audit_${finding.code}`, "pending", source, `${auditLabels[finding.code] ?? `装备检查：${finding.code}`}。`);
        if (verdict !== "natural" && !audit.findings.length && !audit.unverified?.length) add("audit_verdict", verdict === "unnatural" ? "conflict" : "pending", source, verdict === "unnatural" ? "存档装备检查判定为非自然装备。" : "该装备的自然生成检查尚未确认。");
      } else add("audit_missing", "pending", source, "缺少完整记录的原生装备检查；装备码与收藏只提供配装数据，不能据此确认自然生成。");
      if (bucket === "shared") {
        const definition = soulCoreDefinition(piece.item_id);
        if (!definition) add("soul_identity", "pending", source, "魂核身份未识别。");
        else if (!audit && definition.innate_effect_ids.some(id => !piece.effects.some(effect => effect.effect_id === id))) add("soul_innate", "pending", source, "未包含物品表中全部固有词条；当前稀有度所需档位应以原生装备检查为准。");
      }
    }
  }
  const chosenClans = selection.clans[style];
  chosenClans.forEach((choice, index) => {
    if (!choice) return;
    const source = `武家栏位 ${index + 1}`, clan = clans.find(row => row.clan_id === choice.clanId);
    if (!clan || (choice.rankIndex === null ? savedClanProtectionDisplay(choice) === null : !Number.isInteger(choice.rankIndex) || choice.rankIndex < 0 || choice.rankIndex >= clanRanks.length) ||
        (choice.slotIndex !== null && !clan.slots.some(row => row.slot_index === choice.slotIndex))) add("clan_identity", "conflict", source, "武家、阶级或词条不匹配，请重新选择。");
    else if (choice.slotIndex === null) add("clan_unselected", "pending", source, "请从该武家的两条词条中选择一条。");
    if (choice.slotIndex !== null && chosenClans.some((other, otherIndex) => otherIndex < index && other?.clanId === choice.clanId && other.slotIndex === choice.slotIndex)) add("clan_duplicate", "conflict", source, "不能重复选择同一武家的同一词条。");
  });
  const guardian = selection.guardians[style];
  if (guardian && !guardiansForStyle(style).some(row => row.guardian_id === guardian.guardianId && Number.isInteger(guardian.level) && guardian.level >= 1 && guardian.level <= GUARDIAN_LEVEL_MAX && row.slots.every(slot => guardian.level < slot.raw_values_by_level_index.length))) add("guardian_identity", "conflict", "守护灵", "守护灵风格不符或等级超出 1–50 范围。");
  if (guardianEffects(selection, style).some(row => row.activation === "unknown")) add("spikes_unknown", "pending", "守护灵", "地狱之楔破坏数未设置，条件效果未计入汇总。");
  for (const row of loadoutSetSummary(selection, style).pending) add("set_pending", "conflict", row.source, `${row.label}：${row.reason}。`);
  const summary = loadoutEffectSummary(selection, style);
  const pending = summary.pending;
  if (pending.length) add("effect_pending", "pending", "效果汇总", `${pending.length} 项效果尚不能完整解析，详见效果汇总中的待确认列表。`);
  for (const effect of summary.effects) {
    const requirement = profoundRequirement(effect.groupId, attributes);
    if (requirement && !requirement.active) add("profound_requirement", "conflict", effect.label,
      `未启用：需要${requirement.attribute} ${requirement.minimum}，当前 ${requirement.current ?? "—"}。`);
  }
  const auditMissing = rows.filter(row => row.code === "audit_missing");
  const compact = rows.filter(row => row.code !== "audit_missing");
  if (auditMissing.length) compact.push({ ...auditMissing[0], source: auditMissing.map(row => row.source).join("、") });
  return compact.filter((row, index) => compact.findIndex(other => other.code === row.code && other.source === row.source && other.message === row.message) === index);
}
