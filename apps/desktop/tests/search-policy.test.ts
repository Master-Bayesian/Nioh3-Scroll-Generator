import { test } from "node:test";
import assert from "node:assert/strict";
import type { DesktopApi } from "../src/api";
import type { Handshake, JobSnapshot } from "../../../packages/contracts/responses";
import { SearchController } from "../src/search-controller";
import type { StartParams } from "../src/worker-client";
import { initialQuery } from "../../workshop/model";
import { workerQuery } from "../../workshop/desktop-bridge";
import locales from "../../workshop/ui-locales.json";
import {
  SEARCH_JOB_TRIALS,
  SEARCH_PAGE_TRIALS,
  searchStartParams,
  searchStatusText,
} from "../../workshop/search-policy";

const handshake: Handshake = {
  protocol: 1,
  role: "offline_search",
  contract_digest: "digest",
  context: {
    product_version: "test",
    game_profile: "test",
    resources_digest: "test",
    algorithm_version: "test",
    policy_version: "test",
    seed_accelerator_abi: 2,
    seed_accelerator_build_id: "test",
    context_digest: "a".repeat(64),
  },
  capabilities: {
    playthroughs: [3],
    rarities: [3, 4, 5],
    cuda_pivot_and_auxiliary: false,
    directcompute_effect_filter: false,
    cpu_exact_replay: true,
    bulk_cpu_requires_opt_in: true,
    save_write: false,
    runtime_calls: false,
  },
};

const snapshot = (values: Partial<JobSnapshot>): JobSnapshot =>
  ({
    job_id: "job",
    state: "completed",
    sequence: 1,
    query_digest: "query",
    context_digest: "context",
    start_cursor: 0,
    cursor: 0,
    candidates: [],
    progress: null,
    stop_reason: null,
    error: null,
    resume_token: null,
    elapsed_ms: 12300,
    ...values,
  }) as JobSnapshot;

// The reported regression search: three special-rule variants, all required.
const reportedRules = [
  { id: "1667", name: "一难横行（配件）", keys: [64956], variant: "64956" },
  { id: "113", name: "神器掉落率上升", keys: [113], variant: "113" },
  {
    id: "3272",
    name: "优先掉落率上升（素盏呜尊的恩宠）",
    keys: [20893],
    variant: "20893",
  },
];

test("normal search submits one continuing job with native-sized pages", () => {
  const query = { ...initialQuery(), rules: reportedRules };
  const params = searchStartParams({
    query: workerQuery(query),
    contextDigest: "a".repeat(64),
    resultCount: query.count,
    allowCpuFallback: false,
  });
  assert.equal(params.continue_until_complete, true);
  assert.equal(params.page_trials, SEARCH_PAGE_TRIALS);
  assert.equal(params.page_trials, 100_000_000);
  assert.equal(params.job_trials, SEARCH_JOB_TRIALS);
  assert.equal(params.job_trials, 10_000_000);
  assert.equal(params.resume_token, null);
  assert.equal(params.result_count, 25);
  // The three reported conditions stay exact: no widened family, and no grace,
  // ordinary effect, enemy, terrain or capacity filter is invented.
  assert.deepEqual(params.query.auxiliary.required_special_rule_key_groups, [
    [64956],
    [113],
    [20893],
  ]);
  assert.deepEqual(params.query.auxiliary.required_special_rule_keys, []);
  assert.deepEqual(params.query.primary_effect_ids, []);
  assert.deepEqual(params.query.required_secondary_ids, []);
  assert.deepEqual(params.query.required_secondary_id_groups, []);
  assert.deepEqual(params.query.effect_occurrences, []);
  assert.deepEqual(params.query.minimum_roll_percent_by_effect_id, []);
  assert.equal(params.query.grace_effect_id, null);
  assert.deepEqual(params.query.grace_effect_ids, []);
  assert.deepEqual(params.query.auxiliary.required_enemy_lookup_keys, []);
  assert.deepEqual(params.query.auxiliary.required_enemy_lookup_key_groups, []);
  assert.deepEqual(params.query.auxiliary.required_terrain_effect_keys, []);
  assert.deepEqual(params.query.auxiliary.required_terrain_effect_key_groups, []);
  assert.deepEqual(params.query.terrain_selection_ids, []);
  assert.deepEqual(params.query.initial_challenge_counts, []);
  assert.equal(params.query.enemy_variant, "solo");
});

test("next batch resumes the checkpoint with the same continuation policy", async () => {
  let started: StartParams | null = null;
  const api = {
    handshake: async () => handshake,
    currentSearch: async () => ({ job: null, submitted: null }),
    startSearch: async (params: StartParams) => {
      started = params;
      return snapshot({ candidates: [], resume_token: "token-1" });
    },
    snapshot: async () => snapshot({ candidates: [], resume_token: "token-1" }),
  } as unknown as DesktopApi;
  const controller = new SearchController(api);
  try {
    await controller.connect();
    const params = searchStartParams({
      query: workerQuery({ ...initialQuery(), rules: reportedRules }),
      contextDigest: handshake.context.context_digest,
      resultCount: 25,
      allowCpuFallback: false,
    });
    await controller.start(params);
    assert.equal(started!.continue_until_complete, true);
    await controller.resume();
    assert.equal(started!.resume_token, "token-1");
    assert.equal(started!.continue_until_complete, true);
    assert.equal(started!.page_trials, SEARCH_PAGE_TRIALS);
    assert.equal(started!.job_trials, SEARCH_JOB_TRIALS);
    assert.deepEqual(started!.query.auxiliary.required_special_rule_key_groups, [
      [64956],
      [113],
      [20893],
    ]);
  } finally {
    controller.dispose();
  }
});

test("status wording distinguishes stop reasons without claiming false exhaustion", () => {
  const none: Pick<JobSnapshot, "candidates"> = { candidates: [] };
  const met = searchStatusText(
    snapshot({ ...none, stop_reason: "result_limit" }),
  );
  const exhausted = searchStatusText(
    snapshot({ ...none, stop_reason: "family_exhausted" }),
  );
  const bounded = searchStatusText(
    snapshot({ ...none, stop_reason: "budget_reached", resume_token: "token" }),
  );
  const unknown = searchStatusText(snapshot({ ...none, stop_reason: null }));
  const cancelled = searchStatusText(
    snapshot({ ...none, state: "cancelled", stop_reason: "cancelled" }),
  );
  const running = searchStatusText(snapshot({ ...none, state: "running" }));
  const failed = searchStatusText(
    snapshot({
      ...none,
      state: "failed",
      stop_reason: "error",
      error: { code: "SEARCH_FAILED", message: "worker unavailable" },
    }),
  );
  const failedWithoutError = searchStatusText(
    snapshot({ ...none, state: "failed", stop_reason: "error" }),
  );

  assert.match(met, /本批找到 0 张绘卷，已达候选数量，耗时 12\.3 秒。/);
  assert.match(exhausted, /搜索范围已穷尽，本批找到 0 张绘卷，耗时 12\.3 秒。/);
  assert.match(
    bounded,
    /本批在试验预算内找到 0 张绘卷；条件尚未穷尽，可继续搜索下一批，耗时 12\.3 秒。/,
  );
  assert.match(running, /正在搜索，已找到 0 张绘卷…/);
  assert.match(cancelled, /已取消，保留已找到的绘卷。/);
  assert.equal(failed, "worker unavailable");
  assert.equal(failedWithoutError, "搜索失败，请重试或复制诊断信息。");
  // A bounded or unknown batch with no candidates must not read as exhausted.
  // The exhausted wording is scoped to the current batch, never a global total.
  for (const text of [bounded, unknown, running, failedWithoutError])
    assert.doesNotMatch(text, /已穷尽/);
  assert.doesNotMatch(exhausted, /共找到/);
  assert.equal(new Set([met, exhausted, bounded, unknown]).size, 4);
  assert.notEqual(cancelled, running);
});

test("every canonical status fragment has both shipped translations", () => {
  const fragments = [
    "本批找到",
    "张绘卷，耗时",
    "秒。",
    "正在搜索，已找到",
    "张绘卷…",
    "已取消，保留已找到的绘卷。",
    "本批在试验预算内找到",
    "张绘卷；条件尚未穷尽，可继续搜索下一批，耗时",
    "张绘卷，已达候选数量，耗时",
    "搜索范围已穷尽，本批找到",
    "搜索失败，请重试或复制诊断信息。",
  ];
  for (const fragment of fragments) {
    const entry = (locales.ui as Record<string, string[]>)[fragment];
    assert.ok(entry, `Missing UI translation row for ${fragment}`);
    assert.equal(entry.length, 2, `Incomplete translations for ${fragment}`);
    assert.ok(entry.every((value) => value.trim().length > 0));
  }
  // The failed fallback must be genuinely translated, not a Chinese fallback.
  const failed = (locales.ui as Record<string, string[]>)["搜索失败，请重试或复制诊断信息。"];
  assert.doesNotMatch(failed[0], /[\u3400-\u9fff]/);
  assert.notEqual(failed[1], "搜索失败，请重试或复制诊断信息。");
  assert.ok(failed[1].includes("検索"));
});
