import type { SaveCharacter } from "../../packages/contracts/protected-responses";
import { skills, clans, savedClanProtectionDisplay, initialLoadoutSelection, type LoadoutSelection, type LoadoutStyle } from "./loadout-state";
import { equipmentOccupiedSlot, fitsSlot, savedLoadoutPiece, savedRemodelStage } from "./loadout-equipment";
import { entrySample } from "./save-workspace";
import { publicError, stripErrorPrefix } from "./public-errors";
import nativeSkillScope from "./loadout-native-skill-scope.json";
const outsidePlannerSkills = new Set(nativeSkillScope.outside_planner_ids);
const array = (values: unknown, length: number): values is number[] => Array.isArray(values) && values.length === length &&
  values.every(value => Number.isInteger(value) && value >= 0 && value <= 0xffffffff);
const empty = 0xffffffff;
const gearSlots: Record<number, string> = { 0: "melee0", 1: "melee1", 4: "head", 5: "body", 6: "arms", 7: "legs", 8: "feet", 9: "accessory0", 10: "accessory1", 13: "ranged" };
export interface SavedLoadoutImport { attributes: number[]; selection: LoadoutSelection }
const skippedFields = new Set(["guardian_selection", "guardian_ownership", "clan_membership"]);

/** Import represented fields without replacing manual guardian or matching-clan ranks. */
export function applySavedLoadoutImport(current: LoadoutSelection, value: SavedLoadoutImport): LoadoutSelection {
  const next = structuredClone(value.selection);
  next.guardians = structuredClone(current.guardians);
  next.joinedClan = current.joinedClan;
  if (current.amritaGaugePercent !== undefined) next.amritaGaugePercent = current.amritaGaugePercent;
  for (const style of ["samurai", "ninja"] as const) next.clans[style] = next.clans[style].map((choice, index) => {
    const manual = current.clans[style][index];
    if (!choice || !manual || choice.clanId !== manual.clanId || manual.rankIndex === null) return choice;
    return { clanId: choice.clanId, slotIndex: choice.slotIndex, rankIndex: manual.rankIndex };
  });
  return next;
}

/** Build the full replacement first; incomplete reads never partially mutate the planner. */
export function savedConfigReview(character: SaveCharacter) {
  const config = character.loadout;
  if (!config || config.profile !== "pc_v2_02" || !Array.isArray(config.unavailable) || config.unavailable.some(key => typeof key !== "string") ||
      (config.skills !== null && !Array.isArray(config.skills)) || (config.clan_protections !== null && !Array.isArray(config.clan_protections)))
    return { config: null, selectedSkills: [], issues: ["此存档接口尚未提供已验证的角色配置字段。"], notes: [], canRestore: false, restoration: null };
  const unavailable = config.unavailable.filter(key => !skippedFields.has(key));
  const issues = unavailable.map(() => "角色配置字段不可读取，请重新读取存档。");
  if (character.loadout_read_error) issues.push(`已装备绘卷无法读取：${publicError(stripErrorPrefix(character.loadout_read_error))}`);
  const notes: string[] = [], selection = initialLoadoutSelection();
  const stage = savedRemodelStage(character);
  if (stage !== undefined) selection.remodelStage = stage;
  selection.savedExtras = { equipment: { samurai: [], ninja: [] }, scrolls: { samurai: [], ninja: [] }, soulCores: [] };
  if (!config.complete && (unavailable.length > 0 || config.unavailable.length === 0)) issues.push("角色配置字段未完整读取。");
  if (!array(config.attributes, 7) || config.attributes.some(value => value < 5 || value > 150)) issues.push("属性记录缺失或超出当前配装器范围。");
  if (!array(config.initial_attributes, 7) || config.initial_attributes.reduce((sum, value) => sum + value, 0) !== 36) issues.push("初始属性不属于当前已验证的 36 点基准。");
  if (!Number.isInteger(config.level) || config.level === null || config.level < 1 || config.level > 450) issues.push("等级记录缺失或超出当前范围。");
  else if (array(config.attributes, 7) && config.level !== config.attributes.reduce((sum, value) => sum + value, 0) - 35) issues.push("存档等级与属性合计不一致。");
  if (!array(config.capacities, 3)) issues.push("技能容量记录缺失或无效。");
  else selection.capacities = { common: config.capacities[0]!, samurai: config.capacities[1]!, ninja: config.capacities[2]! };
  if (!Number.isInteger(config.destroyed_spike_count) || config.destroyed_spike_count === null || config.destroyed_spike_count < 0 || config.destroyed_spike_count > 65535) issues.push("地狱之楔破坏数缺失或无效。");
  else selection.destroyedSpikeCount = config.destroyed_spike_count;
  const selectedSkills = skills.flatMap(skill => {
    const records = config.skills?.filter(row => row.id === skill.id) ?? [];
    if (records.length > 1) issues.push("已选技能记录重复。");
    const record = records[0];
    if (!record || record.selected_rank === 0) return [];
    if (record.selected_rank !== 1) issues.push("已选技能等级无法由当前配装器表示。");
    selection.skills[skill.category as keyof typeof selection.skills].push(skill.id);
    return [{ ...skill, rank: record.selected_rank }];
  });
  if (!config.skills) issues.push("已选技能记录不可读取。");
  else {
    const otherSkills = config.skills.filter(record => record.selected_rank !== 0 && !skills.some(skill => skill.id === record.id));
    if (otherSkills.some(record => !outsidePlannerSkills.has(record.id)))
      issues.push("存档含目录未识别的已选技能，无法完整恢复。");
    else {
      selection.savedExtras!.otherSkills = structuredClone(otherSkills);
    }
  }
  if (!config.clan_protections || config.clan_protections.length !== 4) issues.push("四条武家庇佑记录不完整。");
  else config.clan_protections.forEach((record, index) => {
    if (record.clan_id === 0 && record.group_id === 0) return;
    const style = index < 2 ? "samurai" : "ninja";
    const clan = clans.find(row => row.clan_id === record.clan_id);
    const effect = clan?.slots.find(row => row.slot_index === record.effect_index && row.effect_group_id === record.group_id);
    const choice = { clanId: record.clan_id, slotIndex: record.effect_index, rankIndex: null, savedRaw: record.raw };
    if (!effect || savedClanProtectionDisplay(choice) === null) { issues.push("武家庇佑词条或实际数值无法核对。"); return; }
    selection.clans[style][index % 2] = choice;
  });
  for (const choices of Object.values(selection.clans)) if (choices[0] && choices[1] && choices[0].clanId === choices[1].clanId && choices[0].slotIndex === choices[1].slotIndex)
    issues.push("同一风格的两条武家庇佑重复选择了同一武家的同一词条。");
  if (!Array.isArray(config.equipped_indices) || config.equipped_indices.length !== 2 || !config.equipped_indices.every(row => array(row, 17))) issues.push("两套装备索引不完整。");
  else config.equipped_indices.forEach((indices, index) => {
    const style: LoadoutStyle = index === 0 ? "samurai" : "ninja";
    indices.forEach((inventoryIndex, position) => {
      if (inventoryIndex === empty || position >= 15) return;
      const slot = gearSlots[position];
      const extraSlot = [2,3].includes(position) ? "melee0" : [11,12].includes(position) ? "accessory0" : null;
      if (!slot && !extraSlot) { issues.push("存档含未识别的原生装备栏位，无法完整恢复。"); return; }
      const records = character.equipment.filter(row => row.slot_index === inventoryIndex);
      const record = records[0];
      if (records.length !== 1 || !record || !fitsSlot(record, style, slot ?? extraSlot!) || (record.set_positions && record.set_positions[index] !== position)) issues.push("装备库存记录与风格或栏位不匹配。");
      else if (slot) selection.equipment[style][slot] = savedLoadoutPiece(record, character);
      else selection.savedExtras!.equipment[style].push({ position, piece: savedLoadoutPiece(record, character) });
    });
    const scrolls = indices.slice(15, 17).filter(value => value !== empty);
    scrolls.forEach((slot, scrollIndex) => {
      const rows = character.loadout_scrolls?.filter(row => row.slot_index === slot) ?? [];
      if (rows.length !== 1) issues.push("已装备绘卷记录缺失或重复。");
      else if (scrollIndex === 0) selection.scrolls[style] = entrySample(rows[0]);
      else selection.savedExtras!.scrolls[style].push(entrySample(rows[0]));
    });
  });
  if (!array(config.soul_core_indices, 10)) issues.push("魂核栏位索引不可读取。");
  else config.soul_core_indices.forEach((inventoryIndex, position) => {
    if (inventoryIndex === empty) return;
    if (position >= 8) { issues.push("魂核索引包含当前未验证的原生保留栏位。"); return; }
    const records = character.equipment.filter(row => row.slot_index === inventoryIndex);
    if (records.length !== 1 || !fitsSlot(records[0], "samurai", "soul0")) issues.push("魂核索引与库存记录不匹配。");
    else if (position < 2) selection.equipment.shared[`soul${position}`] = savedLoadoutPiece(records[0], character);
    else selection.savedExtras!.soulCores.push({ position, piece: savedLoadoutPiece(records[0], character) });
  });
  for (const style of ["samurai", "ninja"] as const) for (const owner of [style, "shared"] as const)
    for (const [slot, piece] of Object.entries(selection.equipment[owner]))
      if (equipmentOccupiedSlot(selection, style, owner, slot, piece)) issues.push("同一件装备不能重复占用配装栏位。");
  const errors = [...new Set(issues)];
  const restoration: SavedLoadoutImport | null = errors.length ? null : { attributes: [...config.attributes!], selection };
  return { config, selectedSkills, issues: errors, notes: [...new Set(notes)], canRestore: restoration !== null, restoration };
}
