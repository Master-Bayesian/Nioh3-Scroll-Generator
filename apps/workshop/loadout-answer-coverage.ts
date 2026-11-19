/** Completeness of an already returned search result; not search lifecycle. */
export type LoadoutAnswerCoverage = "complete" | "partial" | "unanswered" | "infeasible";

/** Upstream source provenance is independent of search-domain coverage. */
export type LoadoutSourceModelScope = "legacy_unverified" | "conditional_rebuild" | "runtime_bound_rebuild";

export function loadoutSourceModelScope(scope: unknown): LoadoutSourceModelScope {
  if (scope === "conditional_rebuild" || scope === "runtime_bound_rebuild") return scope;
  return "legacy_unverified";
}

export function loadoutAnswerCoverage(bestCount: number, unansweredCount: number, completeSearch: boolean, sourceUncomputed: number | null = 0): LoadoutAnswerCoverage {
  if (unansweredCount > 0 || sourceUncomputed === null || sourceUncomputed > 0 || !completeSearch) return bestCount > 0 ? "partial" : "unanswered";
  return bestCount > 0 ? "complete" : "infeasible";
}

/** Explain execution coverage separately from the heuristic attribute search. */
export function loadoutIncompleteReasons(unansweredCount: number, completeSearch: boolean, movePoints: boolean, sourceUncomputed: number | null = 0): string[] {
  const reasons: string[] = [];
  if (unansweredCount > 0) reasons.push("仍有搭配未算出伤害。");
  if (sourceUncomputed === null) reasons.push("尚未报告武器来源的搜索覆盖范围。");
  else if (sourceUncomputed > 0) reasons.push("部分同类型武器的来源尚未完成重建。");
  if (!completeSearch && (sourceUncomputed === 0 || movePoints)) reasons.push(movePoints ?
    "属性点搜索采用候选筛选与局部调整，未穷尽全部搭配和属性分配。" : "搜索结果尚不完整。");
  return reasons;
}
