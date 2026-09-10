import catalog from "./catalog.json";
export const data = catalog;
export function toRecordTransferCount(value: number): number {
  if (!Number.isInteger(value) || value < -1 || value > 0xffffffff)
    throw new RangeError("Invalid transfer count");
  return value === -1 ? 0xffffffff : value;
}
export type Sample = (typeof catalog.samples)[number] & {
  level?: number;
  playthrough?: number;
  backend?: {
    jobId?: string;
    candidateId: string;
    referenceId?: string;
    installable: boolean;
  };
  saveEntry?: import("../../packages/contracts/protected-responses").SaveInventory["entries"][number];
};
export type SelectedEffect = {
  id: string;
  choiceId?: string;
  name: string;
  mode: number;
  roll: number;
  cross: boolean;
};
export const conditionKey = (item: { id: string; choiceId?: string }) =>
  item.choiceId || item.id;
export function effectRequirements(q: Query) {
  const primary = q.unrestricted ? [] : q.effects.slice(0, q.primaryCount),
    rest = q.unrestricted ? q.effects : q.effects.slice(q.primaryCount);
  const choice = (e: SelectedEffect) => ({
    effect_id: Number(e.id),
    minimum_roll_percent: e.roll,
  });
  const result: {
    scope: "primary" | "secondary" | "any";
    alternatives: ReturnType<typeof choice>[];
  }[] = [];
  if (primary.length)
    result.push({ scope: "primary", alternatives: primary.map(choice) });
  const seen = new Set<number>();
  for (const e of rest) {
    if (e.mode && seen.has(e.mode)) continue;
    if (e.mode) seen.add(e.mode);
    result.push({
      scope: q.unrestricted ? "any" : "secondary",
      alternatives: (e.mode ? rest.filter((v) => v.mode === e.mode) : [e]).map(
        choice,
      ),
    });
  }
  return result;
}
export function matchesEffectOccurrences(sample: Sample, q: Query) {
  const effects = sample.effects.filter(
    (e) => e.role === "主词条" || e.role === "副词条",
  );
  const choices = effectRequirements(q)
    .map((r) =>
      effects.flatMap((e, i) =>
        (r.scope === "primary" && i !== 0) ||
        (r.scope === "secondary" && i === 0)
          ? []
          : r.alternatives.some(
                (c) =>
                  c.effect_id === Number(e.id) &&
                  e.roll >= c.minimum_roll_percent,
              )
            ? [i]
            : [],
      ),
    )
    .sort((a, b) => a.length - b.length);
  const assign = (i: number, used: number): boolean =>
    i === choices.length ||
    choices[i].some(
      (slot) => !(used & (1 << slot)) && assign(i + 1, used | (1 << slot)),
    );
  return assign(0, 0);
}
export type EnemyCondition = {
  id: string;
  name: string;
  keys: number[];
  mode: number;
};
export type RuleCondition = {
  id: string;
  name: string;
  keys: number[];
  variant: string;
  mode?: number;
};
export const ANY_RULE_VALUE = "any-rule-value";
export function ruleFamilyValues(category: string): string[] {
  const members = data.rules.filter((rule) => rule.category === category);
  if (!members.length) return [];
  const common = new Set(members[0].variants.map((variant) => variant.label));
  for (const member of members.slice(1)) {
    const labels = new Set(member.variants.map((variant) => variant.label));
    for (const value of common) if (!labels.has(value)) common.delete(value);
  }
  return [...common].sort(
    (left, right) =>
      (Number.parseFloat(left) || 0) - (Number.parseFloat(right) || 0) ||
      left.localeCompare(right),
  );
}
export function ruleFamilyKeys(category: string, value = ANY_RULE_VALUE): number[] {
  const members = data.rules.filter((rule) => rule.category === category);
  return [
    ...new Set(
      members.flatMap((member) =>
        value === ANY_RULE_VALUE
          ? member.keys
          : member.variants
              .filter((variant) => variant.label === value)
              .map((variant) => variant.key),
      ),
    ),
  ];
}
export type Query = {
  effects: SelectedEffect[];
  enemies: EnemyCondition[];
  rules: RuleCondition[];
  terrains: string[];
  capacities: number[];
  graces: string[];
  unrestricted: boolean;
  primaryCount: number;
  ng: number;
  rarity: number;
  level: number;
  recommended: number;
  transfers: number;
  count: number;
};
export const initialSample = data.samples.find((s) => s.rarity === 4)!;
export function initialQuery(): Query {
  return {
    effects: [],
    enemies: [],
    rules: [],
    terrains: [],
    capacities: [],
    graces: [],
    unrestricted: false,
    primaryCount: 1,
    ng: 3,
    rarity: 4,
    level: 180,
    recommended: 350,
    transfers: -1,
    count: 25,
  };
}
export function matches(sample: Sample, q: Query): boolean {
  if (
    (sample.playthrough || 3) !== q.ng ||
    (sample.level ?? 180) !== q.level ||
    sample.rarity !== q.rarity
  )
    return false;
  if (!matchesEffectOccurrences(sample, q)) return false;
  const primary = q.unrestricted ? [] : q.effects.slice(0, q.primaryCount);
  const ordinary = sample.effects.filter(
    (e) => e.role === "主词条" || e.role === "副词条",
  );
  const actualPrimary = ordinary[0];
  if (
    primary.length &&
    !primary.some(
      (e) => e.id === actualPrimary?.id && actualPrimary.roll >= e.roll,
    )
  )
    return false;
  if (
    primary.some(
      (e) =>
        e.cross &&
        e.id !== actualPrimary?.id &&
        !ordinary.slice(1).some((v) => v.id === e.id && v.roll >= e.roll),
    )
  )
    return false;
  const secondary = q.unrestricted
    ? q.effects
    : q.effects.slice(q.primaryCount);
  const available = q.unrestricted ? ordinary : ordinary.slice(1);
  if (
    secondary
      .filter((e) => !e.mode)
      .some((e) => !available.some((v) => v.id === e.id && v.roll >= e.roll))
  )
    return false;
  for (const group of new Set(
    secondary.filter((e) => e.mode).map((e) => e.mode),
  ))
    if (
      !secondary
        .filter((e) => e.mode === group)
        .some((e) => available.some((v) => v.id === e.id && v.roll >= e.roll))
    )
      return false;
  if (
    q.graces.length &&
    !sample.effects.some((e) => e.role === "恩宠" && q.graces.includes(e.id))
  )
    return false;
  if (q.capacities.length && !q.capacities.includes(sample.capacity))
    return false;
  if (
    q.terrains.length &&
    !q.terrains.some((id) => {
      const t = data.terrains.find((t) => t.option_id === id)!;
      return t.aggregate
        ? t.effect_keys.every((k) => sample.terrainKeys.includes(k))
        : t.effect_keys.length === sample.terrainKeys.length &&
            t.effect_keys.every((k) => sample.terrainKeys.includes(k));
    })
  )
    return false;
  if (
    q.rules
      .filter((r) => !r.mode)
      .some((r) => !sample.rules.some((v) => r.keys.includes(v.key)))
  )
    return false;
  for (const group of new Set(q.rules.filter((r) => r.mode).map((r) => r.mode)))
    if (
      !q.rules
        .filter((r) => r.mode === group)
        .some((r) => sample.rules.some((v) => r.keys.includes(v.key)))
    )
      return false;
  if (
    q.enemies
      .filter((e) => !e.mode)
      .some((e) => !e.keys.some((k) => sample.enemyKeys.includes(k)))
  )
    return false;
  for (const group of new Set(
    q.enemies.filter((e) => e.mode).map((e) => e.mode),
  ))
    if (
      !q.enemies
        .filter((e) => e.mode === group)
        .some((e) => e.keys.some((k) => sample.enemyKeys.includes(k)))
    )
      return false;
  return true;
}
export function queryProblem(q: Query, realBackend = false): string {
  if (!Number.isInteger(q.count) || q.count < 1 || q.count > 25)
    return "候选数量应为 1–25。";
  if (
    !Number.isInteger(q.recommended) ||
    !(String(q.recommended) in data.levels)
  )
    return "推荐等级应为 142–700 内可转换的整数。";
  if (!Number.isInteger(q.level) || q.level < 0 || q.level > 180)
    return "绘卷等级应为 0–180。";
  if (
    !Number.isInteger(q.transfers) ||
    q.transfers < -1 ||
    q.transfers > 4294967295
  )
    return "转手次数应为 −1 或有效的无符号整数。";
  if (q.effects.length > 24)
    return "最多保留 24 个词条选项，任一组按一项逻辑要求计算。";
  if (!realBackend && (q.ng !== 3 || q.level !== 180))
    return "此预览暂仅支持搜索三周目、180 级绘卷。";
  return "";
}
