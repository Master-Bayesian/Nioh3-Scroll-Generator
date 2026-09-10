import type { StartParams } from "../desktop/src/worker-client";
import type {
  JobSnapshot,
  SearchCatalog,
} from "../../packages/contracts/responses";
import "../desktop/src/api";
import "../desktop/src/review-api";
import "../desktop/src/operations-api";
import { SearchController } from "../desktop/src/search-controller";
import { data, effectRequirements, type Query, type Sample } from "./model";
export const desktop =
  typeof window !== "undefined" && !!window.nioh && !!window.review;
export const searchController = desktop
  ? new SearchController(window.nioh)
  : null;
const catalogs = new Map<number, SearchCatalog>();
export async function loadDesktopCatalog(rarity: number) {
  const catalog = await window.nioh.searchCatalog(rarity as 3 | 4 | 5, "zh-CN");
  catalogs.set(rarity, catalog);
  return catalog;
}
function keyGroups(items: { keys: number[]; mode?: number }[]) {
  const groups: number[][] = [];
  const seen = new Set<number>();
  for (const item of items) {
    if (!item.mode) groups.push(item.keys);
    else if (!seen.has(item.mode)) {
      seen.add(item.mode);
      groups.push([
        ...new Set(
          items.filter((v) => v.mode === item.mode).flatMap((v) => v.keys),
        ),
      ]);
    }
  }
  return groups;
}
export function workerQuery(q: Query): StartParams["query"] {
  const primary = q.unrestricted ? [] : q.effects.slice(0, q.primaryCount),
    secondary = q.unrestricted ? q.effects : q.effects.slice(q.primaryCount);
  const query = {
    playthrough: q.ng,
    rarity: q.rarity,
    level: q.level,
    primary_effect_ids: primary.map((e) => Number(e.id)),
    required_secondary_ids: [
      ...secondary.filter((e) => !e.mode).map((e) => Number(e.id)),
      ...primary
        .filter((e) => e.cross && primary.length > 1)
        .map((e) => Number(e.id)),
    ],
    required_secondary_id_groups: [
      ...new Set(secondary.filter((e) => e.mode).map((e) => e.mode)),
    ].map((mode) =>
      secondary.filter((e) => e.mode === mode).map((e) => Number(e.id)),
    ),
    grace_effect_id: q.graces.length === 1 ? Number(q.graces[0]) : null,
    grace_effect_ids: q.graces.map(Number),
    minimum_roll_percent_by_effect_id: [...primary, ...secondary]
      .filter((e) => e.roll)
      .map((e) => [Number(e.id), e.roll]),
    auxiliary: {
      required_terrain_effect_keys: [],
      required_terrain_effect_key_groups: [],
      required_special_rule_keys: [],
      required_special_rule_key_groups: keyGroups(q.rules),
      required_enemy_lookup_keys: [],
      required_enemy_lookup_key_groups: keyGroups(q.enemies),
    },
    terrain_selection_ids: q.terrains,
    initial_challenge_counts: q.capacities,
  };
  if (
    [
      query.required_secondary_id_groups,
      query.auxiliary.required_special_rule_key_groups,
      query.auxiliary.required_enemy_lookup_key_groups,
    ].some((groups) => groups.length > 8)
  )
    throw new Error("每类最多支持 8 组筛选条件，请合并或减少条件。");
  // Coarse acceleration uses unique IDs with the lowest requested threshold;
  // final display-slot matching enforces every occurrence independently.
  query.primary_effect_ids = [...new Set(query.primary_effect_ids)];
  query.required_secondary_ids = [...new Set(query.required_secondary_ids)];
  query.required_secondary_id_groups = query.required_secondary_id_groups.map(
    (g) => [...new Set(g)],
  );
  const coarseIds = new Set(query.required_secondary_ids);
  query.required_secondary_id_groups =
    query.required_secondary_id_groups.filter((group) => {
      if (group.some((id) => coarseIds.has(id))) return false;
      group.forEach((id) => coarseIds.add(id));
      return true;
    });
  const thresholds = new Map<number, number>();
  for (const e of [...primary, ...secondary])
    thresholds.set(
      Number(e.id),
      Math.min(thresholds.get(Number(e.id)) ?? 100, e.roll),
    );
  query.minimum_roll_percent_by_effect_id = [...thresholds].filter(
    ([, roll]) => roll > 0,
  );
  return {
    ...query,
    effect_occurrences: effectRequirements(q),
  } as unknown as StartParams["query"];
}
export function candidateSample(
  candidate: JobSnapshot["candidates"][number],
  level: number,
  jobId?: string,
  referenceId?: string,
): Sample {
  const catalog = catalogs.get(candidate.rarity);
  const names = new Map(
    [
      ...(catalog?.ordinary_effects || []),
      ...(catalog?.grace_effects || []),
    ].map((e) => [e.effect_id, e.name]),
  );
  const context =
    data.contexts[
      `${candidate.playthrough || 3}-${candidate.rarity}` as keyof typeof data.contexts
    ];
  const graces = new Set([
    ...(catalog?.grace_effects.map((e) => e.effect_id) || []),
    ...(context?.graces.map((e) => Number(e.id)) || []),
  ]);
  const enemyNames = new Map(
    catalog?.enemy_options?.map((e) => [e.lookup_key, e.name]),
  );
  const ruleNames = new Map(
    catalog?.special_rule_options?.map((e) => [e.key, e.name]),
  );
  const auxiliary = candidate.auxiliary;
  return {
    seed: String(candidate.seed),
    rarity: candidate.rarity,
    level,
    playthrough: candidate.playthrough || 3,
    backend: {
      jobId,
      candidateId: candidate.candidate_id,
      referenceId,
      installable: candidate.installation_available,
    },
    effects: candidate.effects
      .filter((e) => e.effect_id !== 0xffffffff)
      .map((e) => ({
        id: String(e.effect_id),
        name:
          names.get(e.effect_id) ||
          data.editorEffects.find((v) => v.id === String(e.effect_id))?.name ||
          "未知词条",
        roll: e.roll_percent ?? 0,
        raw: e.value,
        role:
          e.slot === 1
            ? "主词条"
            : graces.has(e.effect_id) &&
                ((candidate.rarity === 4 && e.slot === 5) ||
                  (candidate.rarity === 5 && e.slot === 6))
              ? "恩宠"
              : candidate.rarity === 3 && e.slot === 5 && e.effect_id === 1
                ? "成长词条"
                : "副词条",
      })),
    capacity: candidate.initial_challenge_capacity,
    enemyKeys:
      auxiliary?.enemy_groups.flatMap((g) => g.map((e) => e.lookup_key)) || [],
    enemySlotKeys:
      auxiliary?.enemy_groups
        .map((g) => g[0]?.lookup_key)
        .filter((k) => k !== undefined) || [],
    enemies: [
      ...new Set(
        auxiliary?.enemy_groups.flatMap((g) =>
          g.map((e) => enemyNames.get(e.lookup_key) || "未知敌人"),
        ) || [],
      ),
    ],
    terrainKeys: auxiliary?.terrain.display_effect_keys || [],
    rules:
      auxiliary?.special_rules.map((r) => ({
        key: r.key,
        name: ruleNames.get(r.key) || "未知规则",
        value:
          r.display_value === null
            ? r.display_grade || ""
            : String(r.display_value) +
              (r.display_unit === "percent"
                ? "%"
                : r.display_unit === "seconds"
                  ? " 秒"
                  : ""),
      })) || [],
  };
}
export async function retainSample(sample: Sample) {
  if (!sample.backend) throw new Error("请先搜索或查看真实绘卷。");
  if (sample.backend.referenceId) return sample;
  if (!sample.backend.jobId)
    return previewSeed(
      Number(sample.seed),
      sample.rarity,
      sample.level || 180,
      true,
    );
  const ref = await window.review.retain({
    job_id: sample.backend.jobId!,
    candidate_id: sample.backend.candidateId,
  });
  return {
    ...sample,
    backend: { ...sample.backend, referenceId: ref.reference_id },
  };
}
export async function previewSeed(
  seed: number,
  rarity: number,
  level: number,
  retain = false,
) {
  await loadDesktopCatalog(rarity);
  const result = await window.review.preview({
    seed,
    rarity: rarity as 3 | 4 | 5,
    level,
    retain,
  });
  return candidateSample(
    result.candidate,
    level,
    undefined,
    result.reference_id || undefined,
  );
}
export async function copyText(text: string) {
  if (desktop) return window.review.copyText(text);
  return navigator.clipboard.writeText(text);
}
export function formQuery(params: StartParams): Query {
  const value = params.query,
    context =
      data.contexts[
        `${value.playthrough}-${value.rarity}` as keyof typeof data.contexts
      ];
  const effects: Query["effects"] = [];
  const add = (id: number, mode = 0, cross = false) => {
    if (effects.some((e) => Number(e.id) === id)) return;
    effects.push({
      id: String(id),
      name:
        context?.effects.find((e) => e.id === String(id))?.name ||
        data.editorEffects.find((e) => e.id === String(id))?.name ||
        "未知词条",
      mode,
      cross,
      roll:
        value.minimum_roll_percent_by_effect_id.find(
          (pair) => pair[0] === id,
        )?.[1] || 0,
    });
  };
  value.primary_effect_ids.forEach((id) =>
    add(id, 0, value.required_secondary_ids.includes(id)),
  );
  value.required_secondary_ids.forEach((id) => add(id));
  value.required_secondary_id_groups.forEach((group, i) =>
    group.forEach((id) => add(id, i + 1)),
  );
  if (value.effect_occurrences?.length) {
    effects.length = 0;
    let group = 0;
    for (const requirement of value.effect_occurrences) {
      const mode =
        requirement.scope === "primary" || requirement.alternatives.length === 1
          ? 0
          : ++group;
      for (const item of requirement.alternatives)
        effects.push({
          id: String(item.effect_id),
          choiceId: crypto.randomUUID(),
          name:
            context?.effects.find((e) => e.id === String(item.effect_id))
              ?.name || String(item.effect_id),
          mode,
          cross:
            requirement.scope === "primary" &&
            value.required_secondary_ids.includes(item.effect_id),
          roll: item.minimum_roll_percent,
        });
    }
  }
  return {
    effects,
    enemies: value.auxiliary.required_enemy_lookup_key_groups.map(
      (keys, i) => ({
        id: "restored:" + i,
        name: keys
          .map(
            (k) =>
              data.enemies.find((e) => e.keys.includes(k))?.name || String(k),
          )
          .join("／"),
        keys,
        mode: 0,
      }),
    ),
    rules: value.auxiliary.required_special_rule_key_groups.map((keys, i) => ({
      id: "restored:" + i,
      name: keys
        .map(
          (k) => data.rules.find((r) => r.keys.includes(k))?.name || String(k),
        )
        .join("／"),
      keys,
      variant: "任意变体",
      mode: 0,
    })),
    terrains: value.terrain_selection_ids || [],
    capacities: value.initial_challenge_counts || [],
    graces: (
      value.grace_effect_ids ||
      (value.grace_effect_id === null ? [] : [value.grace_effect_id])
    ).map(String),
    unrestricted: !value.primary_effect_ids.length,
    primaryCount: Math.max(1, value.effect_occurrences?.find(r=>r.scope==='primary')?.alternatives.length??value.primary_effect_ids.length),
    ng: value.playthrough,
    rarity: value.rarity,
    level: value.level,
    recommended: 350,
    transfers: -1,
    count: params.result_count,
  } as Query;
}
