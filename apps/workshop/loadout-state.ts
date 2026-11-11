import skillCatalog from "./loadout-skills.json";
import clanCatalog from "./loadout-clans.json";
import guardianCatalog from "./loadout-guardians.json";
import type { LoadoutPiece } from "./loadout-code";
import type { Sample } from "./model";
import type { ArmorRemodelMode } from "./loadout-remodel";

export type LoadoutStyle = "samurai" | "ninja";
export type SkillCategory = "common" | LoadoutStyle;
export const skills = skillCatalog.skills;
export const clans = clanCatalog.clans;
export const clanRanks = clanCatalog.ranks;
export const guardians = guardianCatalog.guardians;
export interface GuardianChoice { guardianId: number; level: number }
export const GUARDIAN_LEVEL_MAX = 50;
export const ESSENCE_EFFECTS: Record<SkillCategory, number> = {
  common: 0xf9be, samurai: 0x512d, ninja: 0xea74,
};
export interface ClanProtectionChoice {
  clanId: number;
  slotIndex: number | null;
  rankIndex: number | null;
  savedRaw?: number;
}
export interface LoadoutSelection {
  /** One character difficulty snapshot governs armor in both styles. */
  remodelStage?: number;
  /** Optional ordinary-planning gauge; missing values show the complete Ki interval. */
  amritaGaugePercent?: number | null;
  /** Per-slot planner simulations; source records are retained unchanged. */
  armorRemodels?: Record<LoadoutStyle, Record<string, ArmorRemodelMode>>;
  equipment: Record<LoadoutStyle | "shared", Record<string, LoadoutPiece>>;
  scrolls: Record<LoadoutStyle, Sample | null>;
  clans: Record<LoadoutStyle, (ClanProtectionChoice | null)[]>;
  joinedClan: number | null;
  guardians: Record<LoadoutStyle, GuardianChoice | null>;
  destroyedSpikeCount: number | null;
  skills: Record<SkillCategory, number[]>;
  capacities: Record<SkillCategory, number | null>;
  savedExtras?: {
    equipment: Record<LoadoutStyle, { position: number; piece: LoadoutPiece }[]>;
    scrolls: Record<LoadoutStyle, Sample[]>;
    soulCores: { position: number; piece: LoadoutPiece }[];
    otherSkills?: { id: number; stored_rank: number; selected_rank: number }[];
  };
}
export function initialLoadoutSelection(): LoadoutSelection {
  return {
    equipment: { samurai: {}, ninja: {}, shared: {} },
    scrolls: { samurai: null, ninja: null },
    clans: { samurai: [null, null], ninja: [null, null] },
    joinedClan: null,
    guardians: { samurai: null, ninja: null },
    destroyedSpikeCount: null,
    skills: { common: [], samurai: [], ninja: [] },
    capacities: { common: null, samurai: null, ninja: null },
  };
}
export function guardiansForStyle(style: LoadoutStyle) {
  return guardians.filter(guardian => guardian.style === style);
}
export function setGuardian(selection: LoadoutSelection, style: LoadoutStyle, choice: GuardianChoice | null): LoadoutSelection {
  const guardian = choice && guardiansForStyle(style).find(row => row.guardian_id === choice.guardianId);
  if (choice && (!guardian || !Number.isInteger(choice.level) || choice.level < 1 ||
      choice.level > GUARDIAN_LEVEL_MAX ||
      guardian.slots.some(slot => choice.level >= slot.display_values_by_level_index.length))) return selection;
  return { ...selection, guardians: { ...selection.guardians, [style]: choice } };
}
export function setDestroyedSpikeCount(selection: LoadoutSelection, count: number | null): LoadoutSelection {
  if (count !== null && (!Number.isInteger(count) || count < 0 || count > 65535)) return selection;
  return { ...selection, destroyedSpikeCount: count };
}
export function guardianEffects(selection: LoadoutSelection, style: LoadoutStyle) {
  const choice = selection.guardians[style];
  const guardian = choice && guardiansForStyle(style).find(row => row.guardian_id === choice.guardianId);
  if (!guardian || !choice || !Number.isInteger(choice.level) || choice.level < 1 ||
      choice.level > GUARDIAN_LEVEL_MAX ||
      guardian.slots.some(slot => choice.level >= slot.display_values_by_level_index.length)) return [];
  return guardian.slots.map(slot => ({ ...slot, guardian_id: guardian.guardian_id, guardian_name: guardian.name_zh,
    display: slot.display_values_by_level_index[choice.level],
    activation: slot.activation_threshold === 0 ? "active" : selection.destroyedSpikeCount === null
      ? "unknown" : selection.destroyedSpikeCount >= slot.activation_threshold ? "active" : "locked",
  }));
}
export function equippedPieces(selection: LoadoutSelection, style: LoadoutStyle) {
  const scroll = selection.scrolls[style];
  return [
    ...Object.values(selection.equipment[style]), ...Object.values(selection.equipment.shared),
    ...(scroll ? [{ effects: scroll.effects.map(effect => ({ effect_id: Number(effect.id), value: effect.raw })) }] : []),
  ];
}
export function essenceLimit(selection: LoadoutSelection, style: LoadoutStyle, category: SkillCategory): number {
  return equippedPieces(selection, style).some(piece =>
    piece.effects.some(effect => effect.effect_id === ESSENCE_EFFECTS[category])) ? 2 : 1;
}
export function skillUsage(selection: LoadoutSelection, category: SkillCategory) {
  const chosen = skills.filter(skill => selection.skills[category].includes(skill.id));
  return {
    cost: chosen.reduce((total, skill) => total + skill.cost, 0),
    essences: chosen.filter(skill => skill.type === "essence").length,
    chosen,
  };
}
export function canAddSkill(selection: LoadoutSelection, style: LoadoutStyle, id: number): boolean {
  const skill = skills.find(skill => skill.id === id);
  if (!skill || !skill.active_styles.includes(style)) return false;
  const category = skill.category as SkillCategory;
  if (selection.skills[category].includes(id)) return false;
  const used = skillUsage(selection, category);
  const capacity = selection.capacities[category];
  return (capacity === null || used.cost + skill.cost <= capacity) &&
    (skill.type !== "essence" || used.essences < essenceLimit(selection, style, category));
}
function isValidRankIndex(rankIndex: number): boolean {
  return Number.isInteger(rankIndex) && rankIndex >= 0 && rankIndex < clanRanks.length;
}

function nativeClan(id: number) {
  return clans.find(clan => clan.clan_id === id);
}

export function clanValues(selection: LoadoutSelection, id: number, rankIndex = 0) {
  const clan = nativeClan(id);
  if (!clan || !isValidRankIndex(rankIndex)) return [];
  const relation = selection.joinedClan === id ? "current_clan" : "other_clan";
  return clan.slots.map(slot => ({
    ...slot, display: slot.display_values_by_rank_index[relation][rankIndex],
  }));
}
/** Saved effect values stand on their own; neither clan membership nor rank is inferred. */
export function savedClanProtectionDisplay(choice: ClanProtectionChoice): string | null {
  if (!Number.isInteger(choice.savedRaw)) return null;
  const effect = nativeClan(choice.clanId)?.slots.find(slot => slot.slot_index === choice.slotIndex);
  if (!effect) return null;
  for (const relation of ["current_clan", "other_clan"] as const) {
    const index = effect.raw_values_by_rank_index[relation].indexOf(choice.savedRaw!);
    if (index >= 0) return effect.display_values_by_rank_index[relation][index];
  }
  return null;
}
export function clanProtectionValue(selection: LoadoutSelection, choice: ClanProtectionChoice) {
  if (choice.slotIndex === null || (choice.rankIndex === null ? !Number.isInteger(choice.savedRaw) : !isValidRankIndex(choice.rankIndex))) return null;
  const clan = nativeClan(choice.clanId);
  const effect = clan && clan.slots.find(slot => slot.slot_index === choice.slotIndex);
  if (!clan || !effect) return null;
  const relation = selection.joinedClan === choice.clanId ? "current_clan" : "other_clan";
  const savedDisplay = choice.rankIndex === null ? savedClanProtectionDisplay(choice) : null;
  if (choice.rankIndex === null && savedDisplay === null) return null;
  return {
    ...effect,
    display: choice.rankIndex === null ? savedDisplay! : effect.display_values_by_rank_index[relation][choice.rankIndex],
    clan_id: clan.clan_id,
    clan_name: clan.name_zh,
  };
}
export function canSelectClanProtection(selection: LoadoutSelection, style: LoadoutStyle, slot: number, choice: ClanProtectionChoice) {
  const clan = nativeClan(choice.clanId);
  const validSlot = choice.slotIndex === null ||
    (Number.isInteger(choice.slotIndex) && !!clan?.slots.some(effect => effect.slot_index === choice.slotIndex));
  if (!Number.isInteger(slot) || slot < 0 || slot >= selection.clans[style].length ||
      !clan || (choice.rankIndex === null ? savedClanProtectionDisplay(choice) === null :
        !isValidRankIndex(choice.rankIndex)) || !validSlot) return false;
  return choice.slotIndex === null ||
    !selection.clans[style].some((other, index) => index !== slot && other?.clanId === choice.clanId && other.slotIndex === choice.slotIndex);
}
export function setClanProtection(selection: LoadoutSelection, style: LoadoutStyle, slot: number, choice: ClanProtectionChoice | null): LoadoutSelection {
  if (!Number.isInteger(slot) || slot < 0 || slot >= selection.clans[style].length ||
      (choice !== null && !canSelectClanProtection(selection, style, slot, choice))) return selection;
  return { ...selection, clans: { ...selection.clans, [style]: selection.clans[style].map((other, index) => index === slot ? choice : other) } };
}
