import armor from "./loadout-armor-requirements.json";
import type { LoadoutPiece } from "./loadout-code";
import { nativeGroupForEffect } from "./loadout-effect-summary";

export const attributeNames = ["体", "心", "刚", "武", "技", "智", "咒"] as const;
const profoundIds = [0xDAC2, 0xB613, 0x23E8, 0xDFF0, 0xAE5A, 0x6AAF, 0x2B06];
const words: Record<string, number[]> = armor.items;
const modes = ["0/0", "1/0", "2/0", "1/1", "2/2", "1/2"];
export function profoundRequirement(groupId: number, attributes: readonly { value: number }[]) {
  const index = profoundIds.findIndex(id => nativeGroupForEffect(id) === groupId);
  if (index < 0) return null;
  const current = attributes[index]?.value;
  return { attribute: attributeNames[index], minimum: 150, current,
    active: Number.isInteger(current) && current >= 150 };
}

/** Stage is the serialized current-difficulty byte, not item level or character level. */
export function armorStageCaps(stage: number | undefined): number[] | null {
  if (stage === undefined) return armor.stage_caps;
  return Number.isInteger(stage) && stage >= 1 && stage <= 5 ? [armor.stage_caps[stage - 1]] : null;
}

/** Original-row requirements and strengthened nonzero delta; missing snapshot data is bounded. */
export function armorRequirements(piece: LoadoutPiece, stage = piece.remodel_stage) {
  const row = words[String(piece.item_id)];
  if (!row) return null;
  if (!Number.isInteger(piece.rarity) || piece.rarity < 0 || piece.rarity > 5 ||
      !Number.isInteger(piece.plus) || piece.plus < 0 || piece.plus > 65535)
    return { known: false as const, reason: "装备稀有度或强化值超出计算范围", values: [] };
  const supplied = piece.armor_remodel?.join("/");
  if (supplied && !modes.includes(supplied))
    return { known: false as const, reason: "防具改造状态未识别", values: [] };
  const caps = armorStageCaps(stage);
  if (!caps) return { known: false as const, reason: "防具改造阶段未识别", values: [] };
  const counts = piece.rarity < 3 ? [0] : supplied
    ? [piece.armor_remodel!.filter(value => value === 1).length] : [0, 1, 2];
  const deltas = counts.flatMap(count => caps.map(cap =>
    Math.floor(armor.requirement_coefficients[Math.min(piece.plus, cap)] * count / 2)));
  const minimum = Math.min(...deltas), maximum = Math.max(...deltas);
  const values = row.map((word, index) => {
    const base = piece.rarity >= 5 ? word >>> 8 : word & 255;
    return { attribute: attributeNames[index], index, minimum: base ? base + minimum : 0, maximum: base ? base + maximum : 0 };
  }).filter(value => value.minimum > 0);
  return { known: true as const, values, reason: minimum === maximum ? "" :
    supplied ? "强化改造的阶段上限待确认" : "未记录防具改造状态" };
}

export function armorRequirementLabel(value: { attribute: string; minimum: number; maximum: number }) {
  return `${value.attribute} ≥ ${value.minimum}${value.maximum === value.minimum ? "" : `–${value.maximum}`}`;
}
