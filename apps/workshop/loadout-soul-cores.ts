import catalog from "./loadout-soul-cores.json";
import { equipmentEffectDetail } from "./loadout-effect-summary";
import type { LoadoutPiece } from "./loadout-code";

const definitions: Record<string, (typeof catalog.cores)[keyof typeof catalog.cores]> = catalog.cores;
export function soulCoreDefinition(id: number) { return definitions[id]; }
export function soulCoreEffects(piece: LoadoutPiece) {
  const innate: readonly number[] = soulCoreDefinition(piece.item_id)?.innate_effect_ids ?? [];
  return piece.effects.flatMap((effect, index) => {
    if (effect.effect_id === 0xffffffff) return [];
    const { label, display: value } = equipmentEffectDetail(effect.effect_id, effect.value);
    return [{ index, id: effect.effect_id, label: label.replaceAll("{}", "（对象待确认）"),
      display: value ?? "数值待确认",
      innate: innate.includes(effect.effect_id), star: !!effect.star }];
  });
}
