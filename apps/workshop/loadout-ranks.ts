/** PC v2.02 native rank predicates, recovered offline. The planner's displayed
 * sample weight/capacity/toughness are not inputs to these production helpers.
 * Evidence: local rank-parameters log and bounded native bodies in the October
 * 12 loadout-names-ranks delivery. Attributes/conditional effects must be
 * resolved before supplying ultimateStamina or redArmorPromotion.
 */
export type AgilityRank = "AA" | "A" | "B" | "C" | "D";
export type ToughnessRank = "AA" | "A" | "B" | "C";

export function agilityRank(weightRaw: number, capacityRaw: number, ultimateStamina = false, redArmorPromotion = false): AgilityRank | null {
  if (!Number.isInteger(weightRaw) || weightRaw < 0 || weightRaw * 1000 > 0xffffffff ||
      !Number.isInteger(capacityRaw) || capacityRaw <= 0 || capacityRaw > 0xffffffff) return null;
  if (weightRaw > capacityRaw) return "D";
  // Native getter truncates to tenths of a percent before comparing thresholds.
  const rate = Math.floor(weightRaw * 1000 / capacityRaw);
  let tier = ultimateStamina && rate <= 200 ? 0 : rate <= 300 ? 1 : rate <= 700 ? 2 : 3;
  if (redArmorPromotion) tier = Math.max(ultimateStamina ? 0 : 1, tier - 1);
  return (["AA", "A", "B", "C", "D"] as const)[tier];
}

export function toughnessRank(toughness: number, ultimateStamina = false): ToughnessRank | null {
  if (!Number.isInteger(toughness) || toughness < 0 || toughness > 0xffffffff) return null;
  return ultimateStamina && toughness >= 320 ? "AA" : toughness >= 260 ? "A" : toughness >= 130 ? "B" : "C";
}
