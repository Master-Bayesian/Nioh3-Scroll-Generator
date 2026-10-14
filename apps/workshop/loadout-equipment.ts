import { ADD_CATALOG } from "./equipment-facets";
import type { CharacterEquipment, SaveCharacter } from "../../packages/contracts/protected-responses";
import { encodeLoadout, type LoadoutPiece } from "./loadout-code";
import type { LoadoutSelection, LoadoutStyle } from "./loadout-state";
const items = new Map(ADD_CATALOG.map(item => [item.id, item]));

export function fitsSlot(piece: LoadoutPiece, style: LoadoutStyle, slot: string): boolean {
  const item = items.get(piece.item_id);
  if (!item) return false;
  const armorSlots: Record<string, string> = { head: "头部", body: "身体", arms: "手臂", legs: "腿部", feet: "足部" };
  if (slot === "soul0" || slot === "soul1") return item.major === "魂核";
  if (slot.startsWith("accessory")) return item.major === "饰品" && !item.type.includes(style === "samurai" ? "忍者" : "武士");
  if (armorSlots[slot]) return item.major === "防具" && item.type === armorSlots[slot] && (!item.school || item.school === (style === "samurai" ? "武士" : "忍者"));
  if (slot === "ranged") return item.major === "武器" && ["弓", "火枪", "大炮"].includes(item.type);
  if (slot.startsWith("melee")) return item.major === "武器" && (style === "samurai" ? ["刀", "双刀", "枪", "大太刀", "斧", "薙刀镰", "手甲", "盾矛"] : ["忍刀", "忍双刀", "锁镰", "手甲钩", "手斧", "旋棍", "机关棍", "忍盾矛"]).includes(item.type);
  return false;
}

export function savedRemodelStage(character: Pick<SaveCharacter, "generation">): number | undefined {
  const stage = character.generation?.difficulty;
  return typeof stage === "number" && Number.isInteger(stage) && stage >= 1 && stage <= 5 ? stage : undefined;
}

/** Preserve read-only audit evidence and inventory identity without exporting it in N3E1. */
export function savedLoadoutPiece(piece: CharacterEquipment, character: Pick<SaveCharacter, "save_id" | "source_sha256" | "generation">): LoadoutPiece {
  return { item_id: piece.item_id, level: piece.level, level_before_forge: piece.level_before_forge,
    plus: piece.plus, rarity: piece.rarity, familiarity: piece.familiarity, armor_remodel: piece.armor_remodel ? [...piece.armor_remodel] : undefined, native_flags: piece.native_flags, origin: "save", hell: piece.hell, hell_skill: piece.hell_skill,
    remodel_stage: savedRemodelStage(character),
    effects: piece.effects.map(effect => ({ effect_id: effect.effect_id, value: effect.value, star: effect.star })),
    audit: piece.audit ? structuredClone(piece.audit) : piece.audit,
    source: { saveId: character.save_id, digest: character.source_sha256, slotIndex: piece.slot_index, inventoryKey: piece.inventory_key } };
}

/** Distinct save records may have identical rolls; codes have no inventory identity. */
export function sameLoadoutPiece(a: LoadoutPiece, b: LoadoutPiece): boolean {
  if (a.source && b.source) {
    if (a.source.saveId !== b.source.saveId) return false;
    if (a.source.inventoryKey > 0 && b.source.inventoryKey > 0) return a.source.inventoryKey === b.source.inventoryKey;
    return a.source.slotIndex === b.source.slotIndex;
  }
  return encodeLoadout([a]) === encodeLoadout([b]);
}

export function equipmentOccupiedSlot(selection: LoadoutSelection, style: LoadoutStyle, owner: LoadoutStyle | "shared", slot: string, piece: LoadoutPiece): string | null {
  for (const bucket of [style, "shared"] as const) for (const [key, selected] of Object.entries(selection.equipment[bucket])) {
    if (bucket === owner && key === slot) continue;
    if (sameLoadoutPiece(selected, piece)) return key;
  }
  return null;
}

export function setLoadoutEquipment(selection: LoadoutSelection, style: LoadoutStyle, owner: LoadoutStyle | "shared", slot: string, piece: LoadoutPiece | null): LoadoutSelection {
  if (owner !== style && owner !== "shared") return selection;
  if ((owner === "shared") !== (slot === "soul0" || slot === "soul1")) return selection;
  if (piece && (!fitsSlot(piece, style, slot) || equipmentOccupiedSlot(selection, style, owner, slot, piece))) return selection;
  const equipment = { ...selection.equipment[owner] };
  if (piece) equipment[slot] = { ...piece, effects: piece.effects.map(effect => ({ ...effect })) };
  else delete equipment[slot];
  const stage = piece?.remodel_stage;
  const armorRemodels = selection.armorRemodels && structuredClone(selection.armorRemodels);
  if (armorRemodels && owner !== "shared") delete armorRemodels[owner][slot];
  return { ...selection, ...(stage !== undefined && Number.isInteger(stage) && stage >= 1 && stage <= 5 ? { remodelStage: stage } : {}),
    ...(armorRemodels ? { armorRemodels } : {}),
    equipment: { ...selection.equipment, [owner]: equipment } };
}

export function equipmentSourceLabel(piece: LoadoutPiece): string {
  return piece.source || piece.origin === "save" ? "存档" : piece.origin === "favorites" ? "装备收藏" : piece.origin === "code" ? "装备码" : "未记录";
}
