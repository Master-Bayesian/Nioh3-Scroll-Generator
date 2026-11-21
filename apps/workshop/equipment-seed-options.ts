import type {EquipmentRules} from "../../packages/contracts/protected-responses";

export interface SeedCandidate {
  id: number;
  star?: boolean;
  min?: number;
  max?: number;
  group?: number;
  masks?: number[];
}
export interface WantedEffect { key: string; ids: number[] }

/** The same group or either conflict-mask bit excludes a simultaneous draw. */
export function excludes(a: SeedCandidate, b: SeedCandidate) {
  if (a.group == null || b.group == null || !a.masks || !b.masks) return false;
  return a.group === b.group || (a.masks[0] & b.masks[0]) !== 0 || (a.masks[1] & b.masks[1]) !== 0;
}

/** Keep same-name table variants as alternatives, retaining star and value distinctions. */
export function seedOptions(rules: EquipmentRules | null, rarity: number | null, name: (id: number) => string | undefined) {
  const options = new Map<string, {candidate: SeedCandidate; ids: number[]}>();
  if (!rules?.known) return options;
  const pool: SeedCandidate[] = (rules.random_pool ?? []).map(entry => ({
    id: entry.effect_id, star: entry.star, min: entry.min, max: entry.max, group: entry.group, masks: entry.masks,
  }));
  const graces: SeedCandidate[] = rarity !== null && rarity >= 4 && rules.set_effect == null && !rules.soul_core
    ? (rules.graces ?? []).map(id => ({id})) : [];
  for (const candidate of [...pool, ...graces]) {
    const key = name(candidate.id) + "|" + !!candidate.star + "|" + candidate.min + "|" + candidate.max;
    const option = options.get(key);
    if (option) option.ids.push(candidate.id);
    else options.set(key, {candidate, ids: [candidate.id]});
  }
  return options;
}

/** Every requested group must be represented by at least one actual effect. */
export function matchesWanted(effects: readonly {effect_id: number}[], wanted: readonly (readonly number[])[]) {
  return wanted.every(ids => effects.some(effect => ids.includes(effect.effect_id)));
}
