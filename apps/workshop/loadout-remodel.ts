import type { LoadoutSelection, LoadoutStyle } from "./loadout-state";
import { fitsSlot } from "./loadout-equipment";

export const armorRemodelModes = [
  { key: "0/0", label: "未改造", bytes: [0, 0] },
  { key: "1/0", label: "强化", bytes: [1, 0] },
  { key: "2/0", label: "厚重", bytes: [2, 0] },
  { key: "1/1", label: "极限强化", bytes: [1, 1] },
  { key: "2/2", label: "极限厚重", bytes: [2, 2] },
  { key: "1/2", label: "强化＋厚重", bytes: [1, 2] },
] as const;
export type ArmorRemodelMode = typeof armorRemodelModes[number]["key"];
export const armorRemodelSlots = ["head", "body", "arms", "legs", "feet"] as const;

export function canRemodelArmor(selection: LoadoutSelection, style: LoadoutStyle, slot: string) {
  const piece = selection.equipment[style][slot];
  return armorRemodelSlots.some(key => key === slot) && !!piece && piece.plus > 0 && fitsSlot(piece, style, slot);
}

/** Keep imported records, audit evidence and inventory identity unchanged. */
export function plannedEquipmentPiece(selection: LoadoutSelection, style: LoadoutStyle, slot: string) {
  const piece = selection.equipment[style][slot];
  const mode = armorRemodelModes.find(mode => mode.key === selection.armorRemodels?.[style][slot]);
  return mode && canRemodelArmor(selection, style, slot) ? { ...piece, armor_remodel: [...mode.bytes] as [number, number] } : piece;
}

export function setArmorRemodel(selection: LoadoutSelection, style: LoadoutStyle, slot: string, mode: string | null): LoadoutSelection {
  if (!canRemodelArmor(selection, style, slot) || (mode !== null && !armorRemodelModes.some(row => row.key === mode))) return selection;
  const overrides = { ...selection.armorRemodels?.[style] };
  if (mode === null) delete overrides[slot];
  else overrides[slot] = mode as ArmorRemodelMode;
  return { ...selection, armorRemodels: { samurai: {}, ninja: {}, ...selection.armorRemodels, [style]: overrides } };
}
