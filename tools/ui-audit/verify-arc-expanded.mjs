/**
 * Expanded Arc preview browser E2E acceptance.
 *
 * This is a browser-only fixture run.  It deliberately does not open a game,
 * read a user save, or commit a save write.  Protected review screens use the
 * synthetic bridge below so cancellation and ownership boundaries can still
 * be exercised.  The existing text-fit detector remains authoritative; the
 * explicit geometry checks below cover controls and long-list reachability it
 * does not inspect.
 *
 * Failure cases declared before the fixture/runner implementation:
 *   - supported navigation route is absent or cannot mount in a locale;
 *   - save picker loses the selected save after navigation;
 *   - restore/resign review cannot be opened, or cancel dispatches a commit;
 *   - equipment add review cannot be opened, or abandoning it dispatches a commit;
 *   - 1,500-row equipment list clips its header, first row, last row, or filters;
 *   - cart/history cards lose their footer/actions or cannot scroll to the last card;
 *   - settings popup/theme controls are missing or outside the viewport;
 *   - the existing detector reports clip, escape, overlap, outside-card, select,
 *     or pageX findings at any checked locale/theme/viewport.
 */
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import { execFileSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { parseArgs } from "node:util";
import { chromium } from "playwright";
import { detectTextFit } from "../../apps/workshop/text-fit-detect.mjs";
import { resolve } from "node:path";

const { values: options } = parseArgs({ options: { out: { type: "string" } } });
const repoRoot = resolve(import.meta.dirname, "../..");
const defaultOutput = resolve(repoRoot, "deliverables/arc-ui-debug-20261012/web/expanded");
const output = resolve(options.out || process.env.NIOH3_UI_OUTPUT || defaultOutput);
await mkdir(output, { recursive: true });

const FAILURE_CASES = [
  "supported navigation route is absent or cannot mount in a locale",
  "save picker loses the selected save after navigation",
  "restore/resign review cannot be opened, or cancel dispatches a commit",
  "equipment add review cannot be opened, or abandoning it dispatches a commit",
  "1,500-row equipment list clips its header, first row, last row, or filters",
  "cart/history cards lose their footer/actions or cannot scroll to the last card",
  "settings popup/theme controls are missing or outside the viewport",
  "existing text-fit detector reports a finding",
];

const MATRIX = [
  { label: "minimum", width: 1100, height: 740, dpr: 1 },
  { label: "wide", width: 1600, height: 1000, dpr: 1 },
  // A short CSS height approximates a high-DPI tester window while keeping
  // the product's documented minimum width.  This is browser evidence only.
  { label: "short-dpi", width: 1170, height: 620, dpr: 1.25 },
];
const LOCALES = ["zh-CN", "en-US", "ja-JP"];
const THEMES = ["light", "dark"];
const RUN_LOCALES = (process.env.NIOH3_UI_LOCALES || LOCALES.join(",")).split(",").filter(Boolean);
const RUN_THEMES = (process.env.NIOH3_UI_THEMES || THEMES.join(",")).split(",").filter(Boolean);
const RUN_MATRIX = (process.env.NIOH3_UI_MATRIX || MATRIX.map(item => item.label).join(","));

const evidence = {
  schema: "nioh3-arc-ui-expanded-e2e/v1",
  scenario: "arc-ui-debug-20261012-expanded-web",
  status: "running",
  pass: false,
  failureCases: FAILURE_CASES,
  boundary: {
    browser: "headless Chromium against the production workshop bundle",
    bridge: "synthetic runtime/save/review bridge; protected plans are cancelled before commit",
    gameProcess: false,
    userSave: false,
    commitCalled: false,
    nativeWindow: false,
    packageAcceptance: false,
    dpr: "recorded per matrix entry; browser emulation is not native DPI acceptance",
    bridgeRuns: [],
    forbiddenWriteMethods: [],
  },
  source: {},
  matrix: MATRIX,
  locales: LOCALES,
  themes: THEMES,
  runSelection: { locales: RUN_LOCALES, themes: RUN_THEMES, matrix: RUN_MATRIX.split(",").filter(Boolean) },
  routes: {},
  states: [],
  checks: [],
  failures: [],
  screenshots: [],
  detectorFindings: [],
  selectors: {
    routeDiscovery: "nav.nav > button -> expected page root selector",
    saveManagement: ".backup-page, .backup-list, .backup-confirm, .resign-panel",
    equipment: ".equipment-page.character-page, .character-table-wrap, .character-sections",
    cartHistory: "dialog[open] .cart-review, dialog[open] .history-pages",
    settings: ".side-popup.settings-menu, [data-theme-choice]",
    protectedCancel: ".backup-confirm button, [data-action=discard-character-plan]",
  },
};

function check(name, passed, details = null, category = "product") {
  const item = { name, category, passed: Boolean(passed), details };
  evidence.checks.push(item);
  if (!item.passed) evidence.failures.push(item);
  return item.passed;
}

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function safeName(value) {
  return String(value).replace(/[^a-z0-9_-]+/gi, "_").replace(/^_+|_+$/g, "");
}

function makeSaveReference() {
  return {
    save_id: "arc-synthetic-save",
    path: "D:/synthetic/SAVEDATA00.BIN",
    account_id: "10001",
    save_slot: 0,
    summary: {
      character_level: 750,
      play_time_seconds: 163 * 3600 + 43 * 60,
      saved_at_unix: 1791859200,
      playthrough: 3,
    },
  };
}

const bridge = String.raw`
(() => {
  const digest = 'c'.repeat(64);
  const saveReference = ${JSON.stringify(makeSaveReference())};
  const state = {
    calls: [],
    favourites: [],
    snapshot: 1,
    job: null,
    nextJob: 1,
    nextPlan: 1,
    compactUncertain: localStorage.getItem('arc-compact-uncertain') === '1',
  };
  window.__arc = state;
  const clone = (value) => structuredClone(value);
  const record = (method, params = {}) => {
    state.calls.push({ method, params: clone(params) });
    if (method === 'save.commit') state.commitCalled = true;
  };
  const equipment = (slot) => ({
    slot_index: slot, item_id: 171, appearance_id: 171, quantity: 1,
    level: 180, level_before_forge: 180, plus: slot % 10, rarity: 4,
    familiarity: 0, inventory_key: slot, seed: 1000 + slot,
    record_sha256: String(slot).padStart(64, 'e'), locked: slot % 7 === 0,
    favorite: slot === 0, worn: slot < 4, type_class: 1,
    effects: [15, 540, 0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff]
      .map((effect_id, index) => ({ index, effect_id, value: effect_id === 0xffffffff ? 0 : 50 + (slot % 10), star: false })),
  });
  const character = {
    save_id: saveReference.save_id,
    source_sha256: 'a'.repeat(64),
    currencies: { amrita: 123456, gold: 987654 },
    equipment_slots: 2000,
    equipment: Array.from({ length: 1500 }, (_, index) => equipment(index)),
    items: Array.from({ length: 24 }, (_, index) => ({
      container: index % 2 ? 'storage' : 'held', slot_index: index,
      item_id: 8001 + index, quantity: 5 + index, limit: 99,
      record_sha256: String(index).padStart(64, 'd'),
    })),
    generation: { difficulty: 3, difficulties: [
      { difficulty: 2, progress: [2, 1, 1, 0] },
      { difficulty: 3, progress: [3, 2, 1, 0] },
    ] },
  };
  const inventoryEntry = (slot) => ({
    slot_index: slot,
    header: { playthrough: 3, level: 180, recommended_level: 350, seed: 12000 + slot, rarity: 4, transfer_count: 0 },
    effects: Array.from({ length: 7 }, (_, index) => ({ slot_index: index, effect_id: index ? 0xffffffff : 15, value: index ? 0 : 50, prefix: 0, metadata: 0, tail_0: 0, tail_1: 0 })),
    derived: { initial_challenge_capacity: 3 },
  });
  const inventory = () => ({
    save_id: saveReference.save_id, snapshot_id: 'snapshot-' + state.snapshot,
    source_sha256: character.source_sha256, account_id: saveReference.account_id,
    empty_slots: 500, entries: [inventoryEntry(0), inventoryEntry(1)],
  });
  const backups = [
    { backup_id: 'arc-backup-newest', timestamp: '1791859200000', action: 'edit', file_count: 3 },
    { backup_id: 'arc-backup-restore', timestamp: '1791859100000', action: 'restore-backup', file_count: 3 },
    { backup_id: 'arc-backup-old', timestamp: '1791859000000', action: 'install', file_count: 2 },
  ];
  const effects = (seed) => [
    { slot: 1, effect_id: 15, value: 50, metadata: 0, prefix: 0, tail_0: 0, tail_1: 0, roll_percent: 90 },
    { slot: 2, effect_id: seed % 2 ? 540 : 1, value: 30, metadata: 0, prefix: 0, tail_0: 0, tail_1: 0, roll_percent: 80 },
    { slot: 3, effect_id: 0xffffffff, value: 0, metadata: 0, prefix: 0, tail_0: 0, tail_1: 0, roll_percent: null },
  ];
  const candidate = (seed, candidateId) => ({
    candidate_id: candidateId || 'arc-candidate-' + seed,
    context_digest: digest, seed, playthrough: 3, rarity: 4,
    record_stage: 'final_record', installable: true, install_blocker: null,
    effects: effects(seed), auxiliary: {
      terrain: { value: 0, display_effect_keys: [] }, enemy_groups: [], special_rules: [],
    }, enemy_states: null, cursor: null, evidence: 'certified_offline_replay',
    installation_available: true, initial_challenge_capacity: 3,
  });
  const candidateView = (seed) => ({
    candidate: candidate(seed), reference_id: null,
  });
  const handshake = {
    protocol: 1, contract_digest: digest, role: 'offline_search',
    context: {
      product_version: '0.8.8', game_profile: 'pc-2.02', game_file_version: '2.0.2.0',
      versioned_resource_dir: 'synthetic', bundle_digest: digest, versioned_digest: digest,
      resources_digest: digest, algorithm_version: 'synthetic', policy_version: 'synthetic',
      context_digest: digest, legacy_context_digest: digest, production_authority: true,
      seed_accelerator_abi: null, seed_accelerator_build_id: null,
    },
    capabilities: { playthroughs: [1, 2, 3], rarities: [3, 4, 5], cuda_pivot_and_auxiliary: false,
      directcompute_effect_filter: false, cpu_exact_replay: true, bulk_cpu_requires_opt_in: false },
  };
  const catalog = (rarity) => ({
    context_digest: digest,
    ordinary_effects: [{ effect_id: 15, name: '攻击力' }, { effect_id: 540, name: '近战伤害' }, { effect_id: 1, name: '体力' }],
    grace_effects: [], terrain_options: [], enemy_options: [], special_rule_options: [], special_rule_families: [],
    recommended_level: { minimum_internal_level: 1, maximum_internal_level: 999, minimum_displayed_level: 142, maximum_displayed_level: 356, selection_policy: 'lowest_canonical_internal_level' },
  });
  const savePlan = (kind, preview = {}) => ({
    plan_id: 'arc-plan-' + state.nextPlan++, save_id: saveReference.save_id,
    snapshot_id: 'snapshot-' + state.snapshot, source_sha256: character.source_sha256,
    expires_in_seconds: 300, kind, preview,
  });
  const receipt = (operation_id) => ({ operation_id, save_id: saveReference.save_id,
    commit_status: 'committed', restore_status: 'not_needed', backup_id: 'arc-commit-backup', warning: null });
  const rules = (item_id) => ({ item_id, known: true, hell_capable: false, soul_core: false,
    roles: ['random', 'random', 'random', 'random', 'random', 'random', 'random'],
    random_pool: [{ effect_id: 15, star: false, min: 1, max: 100, group: 1, masks: [0, 0] },
      { effect_id: 540, star: false, min: 1, max: 100, group: 2, masks: [0, 0] }],
    graces: [], hell_skills: [], innate: [], set_effect: null,
  });
  const seedResponse = (params = {}) => ({ item_id: params.item_id || 171, rarity: params.rarity || 4,
    level: params.level || 180, difficulty: params.difficulty || 3, seeds: 65536, empty: 0, matches: 2,
    outcomes: [{ seed: 1111, effects: [{ effect_id: 15, value: 50, roll: 1, star: false, role: 'random' }] },
      { seed: 2222, effects: [{ effect_id: 540, value: 34, roll: 2, star: true, role: 'random' }] }] });

  window.__arc.calls = [];
  window.__arc.commitCalled = false;
  window.preferences = { getLocale: () => Promise.resolve(localStorage.getItem('nioh3-ui-locale') || 'zh-CN'), setLocale: () => Promise.resolve() };
  window.support = { diagnostics: () => Promise.resolve({ version: '0.8.8-arc-preview', packageVerification: { sourceCommit: '2bde1e3c37815afe3978e226d995b70e6b014f96', ok: true } }), exportFeedback: () => Promise.resolve({ path: 'D:/synthetic/feedback.txt' }) };
  window.nioh = {
    handshake: () => Promise.resolve(clone(handshake)), restartWorker: () => Promise.resolve(clone(handshake)),
    searchCatalog: (rarity) => Promise.resolve(catalog(rarity)), resolveRecommendedLevel: (level) => Promise.resolve({ requested_displayed_level: level, status: 'exact', canonical_internal_levels: [level], selected_internal_level: level, metadata: catalog(4).recommended_level }),
    checkFeasibility: () => Promise.resolve({ checked: true, feasible: true, reason: null }),
    currentSearch: () => Promise.resolve({ job: state.job ? clone(state.job) : null, submitted: null }),
    startSearch: async (params) => { const id = 'arc-job-' + state.nextJob++; const candidates = [candidate(12008, id + '-a'), candidate(12011, id + '-b'), candidate(12019, id + '-c'), candidate(12021, id + '-d')]; const job = { job_id: id, state: 'completed', sequence: 1, query_digest: digest, context_digest: digest, cursor: 4, start_cursor: 0, candidates, progress: null, stop_reason: 'result_limit', error: null, resume_token: null, elapsed_ms: 1 }; state.job = job; return clone(job); },
    snapshot: () => Promise.resolve(clone(state.job)), cancelSearch: () => Promise.resolve(clone(state.job)),
  };
  window.review = {
    preview: ({ seed }) => Promise.resolve(candidateView(Number(seed))),
    retain: ({ candidate_id }) => Promise.resolve({ reference_id: 'arc-ref-' + String(candidate_id).replace(/[^a-z0-9-]/gi, '') }),
    release: () => Promise.resolve(),
    favorites: async ({ action, key, sample }) => { if (action === 'list') return clone(state.favourites); if (action === 'add' && sample) state.favourites.push(clone(sample)); if (action === 'remove') state.favourites = state.favourites.filter(item => String(item.backend?.referenceId || '') !== String(key)); return clone(state.favourites); },
    auxiliary: () => Promise.resolve({ initial_challenge_capacity: 3, enemy_groups: [], terrain: { display_effect_keys: [] }, special_rules: [] }),
    prepareCart: () => Promise.resolve(savePlan('cart', { added: [] })),
    update: () => Promise.resolve({ phase: 'current', canApply: true }), windowAction: () => Promise.resolve(),
    log: () => Promise.resolve(), copyLog: () => Promise.resolve(''), copyText: () => Promise.resolve(), openLink: () => Promise.resolve(),
    openSaveFolder: () => Promise.resolve(), openBackupFolder: () => Promise.resolve(), dataDirectory: () => Promise.resolve({ data_directory: 'D:/synthetic/data', restart_required: false }),
    exportFeedback: () => Promise.resolve({ path: 'D:/synthetic/feedback.txt' }),
    resignSave: async () => { record('save.prepare_resign', { save_id: saveReference.save_id }); return savePlan('resign', { source_path: 'D:/synthetic/SHARED.BIN', from_account: '20002', to_account: saveReference.account_id, rebound_scrolls: 2 }); },
    gameInstallation: () => Promise.resolve(null),
  };
  window.operations = {
    execute: async (command) => {
      const method = command?.method; const params = command?.params || {}; record(method, params);
      if (method === 'runtime.compatibility') return { compatibility: { present: true, reference_match: true, warning: false, accepted: false, plan: null, backup: null, differences: [], hard_blocks: [], game_version: '2.0.2.0', executable: 'D:/synthetic/Nioh3.exe' } };
      if (method === 'runtime.status') return { status: 'ready', read_only: true };
      if (method === 'runtime.menu_selection') return { menu_open: true, container: 'equipment', slot_index: 42, item_id: 171 };
      if (method === 'runtime.character_snapshot') return clone(character);
      if (method === 'save.discover') return { saves: [clone(saveReference)] };
      if (method === 'save.inventory') return clone(inventory());
      if (method === 'save.operations') return { operations: state.compactUncertain ? [{ operation_id: 'arc-uncertain', save_id: saveReference.save_id, commit_status: 'unknown', restore_status: 'not_needed' }] : [] };
      if (method === 'save.character') return clone(character);
      if (method === 'save.backups') return { backups: clone(backups) };
      if (method === 'save.prepare_restore') return savePlan('restore', { backup_id: params.backup_id });
      if (method === 'save.prepare_character_edit') return savePlan('character-edit', { added: (params.add || []).map((request, index) => ({ slot_index: 1600 + index, after: { ...equipment(1600 + index), item_id: request.item_id, level: request.level, plus: request.plus, rarity: request.rarity }, seeded: !!request.seed })) });
      if (method === 'save.discard') return { discarded: true };
      if (method === 'save.commit') throw new Error('E2E_BOUNDARY: save.commit must not be called');
      if (method === 'save.operation') return { operation_id: params.plan_id || 'arc-uncertain', save_id: saveReference.save_id, commit_status: 'not_committed', restore_status: 'not_needed' };
      if (method === 'save.recycle_backups') return { backups: clone(backups) };
      if (method === 'runtime.equipment_rules') return clone(rules(params.item_id));
      if (method === 'runtime.effect_values') return { effect_id: params.effect_id, rarity: params.rarity, level: params.level, star: false, values: [{ value: 50, roll_min: 1, roll_max: 1, top_fraction: 1 }, { value: 100, roll_min: 2, roll_max: 2, top_fraction: 0.5 }] };
      if (method === 'runtime.equipment_seeds') return seedResponse(params);
      if (method === 'runtime.inventory_snapshot') return { status: 'observed', read_only: true, rows: [], next_start: null };
      if (method === 'runtime.character_edit') throw new Error('E2E_BOUNDARY: runtime writes are forbidden');
      if (method === 'runtime.equipment_add_prepare' || method === 'runtime.equipment_add_execute') throw new Error('E2E_BOUNDARY: native writes are forbidden');
      return {};
    },
    snapshot: async (_role, jobId) => ({ job_id: jobId, kind: 'save.backups', state: 'completed', sequence: 1, cancellable: false, progress: null, result: null }),
    cancel: async (_role, jobId) => ({ job_id: jobId, kind: 'save.backups', state: 'cancelled', sequence: 1, cancellable: false, progress: null, result: null }),
    current: async () => ({ job: null, busy: false }),
    prepareInstall: async () => savePlan('install', { added: [] }),
    prepareCount: async () => savePlan('count', {}), prepareScrollEdit: async () => savePlan('scroll-edit', {}),
    prepareLiveAdd: async () => savePlan('live-add', {}), selectSave: async () => clone(saveReference), resignSave: async () => window.review.resignSave({ save_id: saveReference.save_id, snapshot_id: 'snapshot-' + state.snapshot }),
    generate: async () => ({}), searchNative: async () => ({}), captureGrace: async () => ({}), bindCachedSearch: async () => ({ cache_id: 'arc-cache' }),
  };
  window.nioh.resolveRecommendedLevel = window.nioh.resolveRecommendedLevel;
})();
`;

function routeButtons(page) {
  return page.locator("nav.nav > button");
}

async function routeInventory(page) {
  return page.locator("nav.nav > button").evaluateAll(buttons => buttons.map((button, index) => ({
    index, text: button.textContent?.trim() || "", aria: button.getAttribute("aria-label"), disabled: button.disabled,
  })));
}

async function clickRoute(page, name, selector) {
  const buttons = routeButtons(page);
  const routes = await routeInventory(page);
  for (let index = 0; index < routes.length; index++) {
    if (routes[index].disabled) continue;
    await buttons.nth(index).click();
    try {
      await page.locator(selector).first().waitFor({ state: "visible", timeout: 1500 });
      return { index, routes };
    } catch {
      // Route was another supported page; continue discovery instead of
      // assuming the nav index is stable across redesigns/locales.
    }
  }
  throw new Error(`route ${name} did not mount ${selector}; available=${JSON.stringify(routes)}`);
}

async function controlGeometry(page, selectors) {
  return page.evaluate((list) => list.map(selector => {
    const element = [...document.querySelectorAll(selector)].find(candidate => {
      const box = candidate.getBoundingClientRect();
      const style = getComputedStyle(candidate);
      return box.width > 0 && box.height > 0 && style.visibility !== "hidden" && style.display !== "none";
    }) || document.querySelector(selector);
    if (!element) return { selector, exists: false };
    const rect = element.getBoundingClientRect();
    const style = getComputedStyle(element);
    const points = [[rect.left + Math.min(3, rect.width / 2), rect.top + Math.min(3, rect.height / 2)],
      [rect.left + rect.width / 2, rect.top + rect.height / 2],
      [rect.right - Math.min(3, rect.width / 2), rect.bottom - Math.min(3, rect.height / 2)]];
    const unobscured = points.every(([x, y]) => {
      const top = document.elementFromPoint(x, y);
      return top === element || element.contains(top);
    });
    return { selector, exists: true, disabled: Boolean(element.disabled), visible: rect.width > 0 && rect.height > 0 && style.visibility !== "hidden" && style.display !== "none", rect: { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom, width: rect.width, height: rect.height }, unobscured, viewport: { width: innerWidth, height: innerHeight } };
  }), selectors);
}

async function reachability(page, stateName, selectors) {
  const initial = await controlGeometry(page, selectors);
  const results = [];
  for (const selector of selectors) {
    const index = await page.locator(selector).evaluateAll(elements => elements.findIndex(element => {
      const box = element.getBoundingClientRect();
      const style = getComputedStyle(element);
      return box.width > 0 && box.height > 0 && style.visibility !== "hidden" && style.display !== "none";
    }));
    if (index >= 0) {
      await page.locator(selector).nth(index).scrollIntoViewIfNeeded().catch(() => {});
      let trial = false;
      try { await page.locator(selector).nth(index).click({ trial: true, timeout: 1500 }); trial = true; } catch { /* geometry result carries the reason */ }
      const [result] = await controlGeometry(page, [selector]);
      result.trial = trial;
      results.push(result);
    } else {
      results.push({ selector, exists: false, trial: false });
    }
  }
  results.forEach((result, index) => { result.initial = initial[index]; });
  for (const result of results) {
    const passed = result.exists && result.visible && result.rect.width >= 1 && result.rect.height >= 1 && (result.disabled || result.unobscured || result.trial);
    check(`${stateName}: control exists and is reachable (${result.selector})`, passed, result, "geometry");
  }
  return results;
}

async function recoveryGeometry(page) {
  return page.evaluate(() => {
    const describe = (element) => {
      if (!element) return null;
      const rect = element.getBoundingClientRect();
      const style = getComputedStyle(element);
      const chain = [];
      for (let current = element; current && chain.length < 6; current = current.parentElement) {
        const box = current.getBoundingClientRect();
        const css = getComputedStyle(current);
        chain.push({ tag: current.tagName.toLowerCase(), className: typeof current.className === "string" ? current.className.split(/\s+/).slice(0, 3).join(".") : "", rect: { left: box.left, top: box.top, right: box.right, bottom: box.bottom, width: box.width, height: box.height }, overflowX: css.overflowX, overflowY: css.overflowY, display: css.display, flexGrow: css.flexGrow, flexShrink: css.flexShrink, flexBasis: css.flexBasis, minHeight: css.minHeight, maxHeight: css.maxHeight, scrollTop: current.scrollTop, scrollHeight: current.scrollHeight, clientHeight: current.clientHeight });
      }
      return { rect: { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom, width: rect.width, height: rect.height }, overflowX: style.overflowX, overflowY: style.overflowY, display: style.display, flexGrow: style.flexGrow, flexShrink: style.flexShrink, flexBasis: style.flexBasis, minHeight: style.minHeight, maxHeight: style.maxHeight, scrollTop: element.scrollTop, scrollHeight: element.scrollHeight, clientHeight: element.clientHeight, chain };
    };
    return {
      viewport: { width: innerWidth, height: innerHeight, dpr: devicePixelRatio },
      installMode: describe(document.querySelector(".install-mode.install-save")),
      savePicker: describe(document.querySelector(".install-mode.install-save .save-picker")),
      saveSelect: describe(document.querySelector(".install-mode.install-save .save-select")),
      uncertainOperation: describe(document.querySelector(".install-mode.install-save .uncertain-operation")),
      resultPane: describe(document.querySelector(".result-pane")),
    };
  });
}

async function searchPressureGeometry(page) {
  return page.evaluate(() => {
    const describe = (selector) => {
      const element = document.querySelector(selector);
      if (!element) return { selector, exists: false };
      const rect = element.getBoundingClientRect();
      const css = getComputedStyle(element);
      return { selector, exists: true, rect: { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom, width: rect.width, height: rect.height }, display: css.display, overflowX: css.overflowX, overflowY: css.overflowY, flexGrow: css.flexGrow, flexShrink: css.flexShrink, flexBasis: css.flexBasis, minWidth: css.minWidth, maxWidth: css.maxWidth, minHeight: css.minHeight, maxHeight: css.maxHeight, scrollTop: element.scrollTop, scrollHeight: element.scrollHeight, clientHeight: element.clientHeight, scrollWidth: element.scrollWidth, clientWidth: element.clientWidth };
    };
    return {
      viewport: { width: innerWidth, height: innerHeight, dpr: devicePixelRatio },
      main: describe(".search-page main"),
      selection: describe(".search-page .selection"),
      selectionHeader: describe(".search-page .selection>header"),
      primaryControls: describe(".search-page .primary-controls"),
      primaryHint: describe(".search-page .primary-controls .primary-hint"),
      selectedBody: describe(".search-page .selected-body"),
      searchDock: describe(".search-page .search-dock"),
    };
  });
}

async function auditBrandMetadata(page, locale, theme, matrix) {
  await page.waitForFunction(() => !!document.querySelector(".app-version")?.textContent?.trim(), null, { timeout: 10000 }).catch(() => {});
  const info = await page.evaluate(() => {
    const describe = (selector) => {
      const element = document.querySelector(selector);
      if (!element) return { selector, exists: false };
      const rect = element.getBoundingClientRect();
      const css = getComputedStyle(element);
      const range = document.createRange();
      range.selectNodeContents(element);
      const textRect = range.getBoundingClientRect();
      const marker = document.createElement("span");
      marker.style.cssText = "display:inline-block;width:0;height:0;padding:0;margin:0;border:0;vertical-align:baseline";
      element.append(marker);
      const baseline = marker.getBoundingClientRect().top;
      marker.remove();
      return { selector, exists: true, text: element.textContent?.trim() || "", visible: rect.width > 0 && rect.height > 0 && css.display !== "none" && css.visibility !== "hidden", rect: { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom, width: rect.width, height: rect.height }, textRect: { left: textRect.left, top: textRect.top, right: textRect.right, bottom: textRect.bottom, width: textRect.width, height: textRect.height }, baseline, fontFamily: css.fontFamily, fontSize: parseFloat(css.fontSize), lineHeight: parseFloat(css.lineHeight), display: css.display, visibility: css.visibility };
    };
    const navLabels = [...document.querySelectorAll("nav.nav > button:not(:disabled)")].map(button => ({ label: button.querySelector("span")?.textContent?.trim() || button.textContent?.trim() || "", aria: button.getAttribute("aria-label") || "" }));
    const brand = document.querySelector(".brand-text");
    return { brandText: brand?.textContent?.trim() || "", author: describe(".brand-text .credits"), version: describe(".brand-text .app-version"), build: describe(".brand-text .app-build"), small: describe(".brand-text small"), navLabels };
  });
  const prefix = `${locale}/${theme}/${matrix.label}`;
  const versionPresent = info.version.exists && info.version.visible && Boolean(info.version.text);
  const authorVersionPresent = info.author.exists && info.author.visible && versionPresent && Boolean(info.author.text);
  const compactBrand = matrix.width <= 1180;
  check(`${prefix}: brand preserves version and responsive author visibility`, versionPresent && (compactBrand || authorVersionPresent), info, "brand");
  const fontConsistent = versionPresent && info.author.exists && info.author.fontFamily === info.version.fontFamily && Math.abs(info.author.fontSize - info.version.fontSize) <= 0.25 && Math.abs(info.author.lineHeight - info.version.lineHeight) <= 0.5;
  check(`${prefix}: brand author/version font roles are consistent`, fontConsistent, { author: info.author, version: info.version }, "brand");
  const baselineAligned = authorVersionPresent ? Math.abs(info.author.baseline - info.version.baseline) <= 1 : compactBrand && versionPresent;
  check(`${prefix}: visible brand metadata shares a text baseline`, baselineAligned, { author: info.author.baseline, version: info.version.baseline, compactBrand, small: info.small.rect }, "brand");
  const buildHidden = !info.build.exists || !info.build.visible || !info.build.text;
  check(`${prefix}: build SHA is hidden from the brand`, buildHidden, { build: info.build, brandText: info.brandText }, "brand");
  const diagnostics = await page.evaluate(() => window.support?.diagnostics?.()).catch(error => ({ error: String(error) }));
  const sourceCommit = diagnostics?.packageVerification?.sourceCommit || diagnostics?.sourceCommit || "";
  check(`${prefix}: source commit remains available through diagnostics`, typeof sourceCommit === "string" && sourceCommit.length >= 7, { sourceCommit, diagnostics }, "identity");
  const labels = info.navLabels.map(entry => entry.label).filter(Boolean);
  check(`${prefix}: normal navigation labels are unique`, new Set(labels).size === labels.length, { labels, navLabels: info.navLabels }, "brand");
  return { ...info, diagnostics, sourceCommit };
}

async function capture(page, run, stateName, matrix, locale, theme, extra = {}) {
  await page.waitForTimeout(100);
  const tag = [run, locale, theme, matrix.label, stateName].map(safeName).join("-");
  const screenshot = resolve(output, `${tag}.png`);
  await page.screenshot({ path: screenshot, fullPage: false });
  const detector = await page.evaluate(detectTextFit);
  const viewport = await page.evaluate(() => ({ width: innerWidth, height: innerHeight, dpr: devicePixelRatio }));
  const item = { run, state: stateName, locale, theme, matrix: { ...matrix }, viewport, screenshot, detector, ...extra };
  evidence.states.push(item);
  evidence.screenshots.push(screenshot);
  if (detector.length) evidence.detectorFindings.push(item);
  const relevantFindings = detector.filter(finding => {
    if (extra.ignoreSelect && finding.k === "select") return false;
    if (extra.ignorePressureMetadata && /app-version|app-build/.test(finding.at || "")) return false;
    return true;
  });
  check(`${run}/${locale}/${theme}/${matrix.label}/${stateName}: detector is clear`, relevantFindings.length === 0, relevantFindings, "detector");
  return item;
}

async function waitForSave(page) {
  const picker = page.locator(".backup-page:visible .save-picker, .character-page:visible .save-picker").first();
  await picker.waitFor({ state: "visible", timeout: 15000 });
  await page.waitForFunction(() => !!document.querySelector('.backup-page:not([hidden]) .save-picker select option[value="arc-synthetic-save"], .character-page:not([hidden]) .save-picker select option[value="arc-synthetic-save"]'), null, { timeout: 5000 });
  await page.waitForTimeout(500);
}

async function dismissOrdinaryDialog(page, label) {
  const open = page.locator("dialog[open]");
  if (!await open.count()) return;
  const close = open.first().locator("header button").first();
  if (await close.count()) await close.click();
  else await open.first().press("Escape");
  await open.first().waitFor({ state: "hidden", timeout: 5000 });
  check(`${label}: ordinary dialog dismissed before next flow`, await page.locator("dialog[open]").count() === 0, null, "ownership");
}

async function runExpanded(page, matrix, locale, theme) {
  const run = "expanded";
  const callsBefore = () => page.evaluate(() => window.__arc.calls.map(call => call.method));
  const routeResult = {};
  try {
    routeResult.search = await clickRoute(page, "search", ".search-page");
    evidence.routes[locale] ||= {};
    evidence.routes[locale].search = routeResult.search;
    await capture(page, run, "search-default", matrix, locale, theme, { routes: routeResult });

    // Search produces four real fixture candidates; cart and history are then
    // populated through the normal UI flow so card geometry is meaningful.
    await page.locator(".search-actions button.primary-button").click();
    await page.waitForFunction(() => document.querySelectorAll('.result-detail .scroll').length > 0, null, { timeout: 10000 });
    const resultCards = page.locator(".result-detail .scroll");
    check(`${locale}/${theme}/${matrix.label}: search returned fixture candidates`, await resultCards.count() >= 1, await resultCards.count());
    // Reuse the existing pseudo-locale pressure idea without changing the
    // detector: non-select failures remain actionable, while native-select
    // truncation is recorded as the known stress boundary.
    const pressureGeometryBefore = await searchPressureGeometry(page);
    await page.evaluate(() => {
      const walk = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
      for (let node; (node = walk.nextNode());) {
        const text = node.textContent || "";
        if (text.trim().length > 1 && !node.parentElement?.closest("script,style,option,.app-version,.app-build")) node.textContent = text + text.slice(0, Math.ceil(text.length / 2));
      }
      for (const option of document.querySelectorAll("option")) option.textContent += option.textContent.slice(0, Math.ceil(option.textContent.length / 2));
    });
    const stressState = await capture(page, run, "text-stress-search", matrix, locale, theme, { ignoreSelect: true, ignorePressureMetadata: true, pressure: "visible text lengthened by 50%; .app-version/.app-build metadata excluded; native select findings remain recorded", pressureGeometryBefore, pressureGeometryAfter: await searchPressureGeometry(page) });
    const stressFailures = stressState.detector.filter(finding => finding.k !== "select" && !/app-version|app-build/.test(finding.at || ""));
    check(`${locale}/${theme}/${matrix.label}: text-stress has no non-select findings`, stressFailures.length === 0, stressFailures, "detector");
    await page.reload({ waitUntil: "domcontentloaded" });
    await page.locator("#root .shell").waitFor({ state: "visible", timeout: 15000 });
    await page.waitForTimeout(1000);
    await page.locator(".search-actions button.primary-button").waitFor({ state: "visible", timeout: 10000 });
    await page.waitForFunction(() => !document.querySelector(".search-actions button.primary-button")?.disabled, null, { timeout: 10000 });
    await page.locator(".search-actions button.primary-button").click();
    await page.waitForFunction(() => document.querySelectorAll('.result-detail .scroll').length > 0, null, { timeout: 10000 });
    const cartToggle = page.locator(".result-cart-actions .cart-toggle");
    if (await cartToggle.count()) await cartToggle.click();
    const nextResult = page.locator(".result-paging button[aria-label='下一张绘卷']");
    if (await nextResult.count() && !(await nextResult.isDisabled())) { await nextResult.click(); await page.waitForTimeout(200); const nextCart = page.locator(".result-cart-actions .cart-toggle"); if (await nextCart.count() && await nextCart.getAttribute("aria-pressed") !== "true") await nextCart.click(); }
    // A second search archives the first result page and yields the history modal.
    await page.locator(".search-actions button.primary-button").click();
    await page.waitForTimeout(600);
    // The button text is localized; the result header has one action button
    // before the sort select, so use structure rather than a Chinese label.
    const historyButton = page.locator(".result-pane > header > button").first();
    check(`${locale}/${theme}/${matrix.label}: history route is exposed after search`, await historyButton.count() === 1 && !(await historyButton.isDisabled()), { count: await historyButton.count(), disabled: await historyButton.count() ? await historyButton.isDisabled() : null });
    if (await historyButton.count() && !(await historyButton.isDisabled())) {
      await historyButton.click(); await page.locator("dialog[open] .history-pages").waitFor({ state: "visible", timeout: 5000 });
      await capture(page, run, "history-dialog", matrix, locale, theme);
      await reachability(page, "history-dialog", ["dialog[open] header button", "dialog[open] .history-pages .collection-actions button"]);
      const historyCards = page.locator("dialog[open] .history-pages .scroll");
      check(`${locale}/${theme}/${matrix.label}: history has a real card`, await historyCards.count() >= 1, await historyCards.count(), "geometry");
      await page.locator("dialog[open] header button").click();
    }
    const cartButton = page.locator(".compare-button").first();
    if (await cartButton.count() && !(await cartButton.isDisabled())) {
      await cartButton.click(); await page.locator("dialog[open] .cart-review").waitFor({ state: "visible", timeout: 5000 });
      await capture(page, run, "cart-dialog", matrix, locale, theme);
      await reachability(page, "cart-dialog", ["dialog[open] header button", "dialog[open] .cart-review .cart-check input", "dialog[open] .cart-review .collection-actions button"]);
      const cartCards = page.locator("dialog[open] .cart-review .scroll");
      check(`${locale}/${theme}/${matrix.label}: cart has populated cards`, await cartCards.count() >= 1, await cartCards.count(), "geometry");
      await page.locator("dialog[open] header button").click();
    }
    // Compact install-save mode has the same save picker as the full save
    // surfaces.  Seed one unresolved receipt to verify the recovery fence
    // remains visible inside the narrow install area, then resolve it through
    // the read-only receipt check before entering later save routes.
    await page.evaluate(() => { localStorage.setItem('arc-compact-uncertain', '1'); });
    await page.reload({ waitUntil: "domcontentloaded" });
    await page.locator("#root .shell").waitFor({ state: "visible", timeout: 15000 });
    await page.waitForTimeout(500);
    const saveMode = page.locator(".search-page:visible .install-choice input[type=radio]").nth(1);
    if (await saveMode.count()) {
      await saveMode.click();
      await page.locator(".search-page:visible .install-mode .save-picker").waitFor({ state: "visible", timeout: 5000 });
      await page.locator(".search-page:visible .uncertain-operation").waitFor({ state: "visible", timeout: 5000 });
      const recoveryState = await capture(page, run, "search-install-save-uncertain", matrix, locale, theme, { recoveryGeometryBefore: await recoveryGeometry(page) });
      await reachability(page, "search-install-save-uncertain", [".search-page .install-mode .save-picker select", ".search-page .uncertain-operation button"]);
      recoveryState.recoveryGeometryAfter = await recoveryGeometry(page);
      await page.evaluate(() => { localStorage.removeItem('arc-compact-uncertain'); window.__arc.compactUncertain = false; });
      const visibleUncertain = page.locator(".search-page .uncertain-operation").first();
      await visibleUncertain.locator("button").first().click();
      await visibleUncertain.waitFor({ state: "hidden", timeout: 5000 });
      check(`${locale}/${theme}/${matrix.label}: compact install-save recovery fence clears after receipt check`, await page.locator(".search-page .uncertain-operation").count() === 0, null, "ownership");
    } else {
      check(`${locale}/${theme}/${matrix.label}: compact install-save mode is available`, false, "save radio missing");
    }
  } catch (error) {
    check(`${locale}/${theme}/${matrix.label}: search/cart/history flow completed`, false, String(error));
  }

  try {
    await dismissOrdinaryDialog(page, `${locale}/${theme}/${matrix.label}/settings`);
    const settingsRoute = await page.locator(".topbar .icon-button.settings").click();
    await page.locator(".side-popup.settings-menu").waitFor({ state: "visible", timeout: 3000 });
    const settingsState = await capture(page, run, "settings-popup", matrix, locale, theme, { recoveryGeometrySettings: await recoveryGeometry(page) });
    await reachability(page, "settings-popup", [".side-popup.settings-menu [data-theme-choice]", ".side-popup.settings-menu [data-action='open-about']"]);
    check(`${locale}/${theme}/${matrix.label}: current theme choice is marked`, await page.locator(`.side-popup.settings-menu [data-theme-choice='${theme}']`).getAttribute("aria-checked") === "true");
    await page.locator(`.side-popup.settings-menu [data-theme-choice='${theme}']`).click();
    await page.keyboard.press("Escape");
    await page.locator(".side-popup.settings-menu").waitFor({ state: "hidden", timeout: 5000 });
  } catch (error) {
    check(`${locale}/${theme}/${matrix.label}: settings flow completed`, false, String(error));
  }

  try {
    await dismissOrdinaryDialog(page, `${locale}/${theme}/${matrix.label}/save-management`);
    const route = await clickRoute(page, "backups", ".backup-page");
    evidence.routes[locale].backups = route;
    await waitForSave(page);
    await page.locator(".backup-list table").waitFor({ state: "visible", timeout: 10000 });
    await capture(page, run, "save-management", matrix, locale, theme, { route });
    await reachability(page, "save-management", [".backup-toolbar button.backup-action.primary", ".backup-list tbody tr:first-child input[type=checkbox]", ".resign-panel > button"]);
    const selectedSaveBefore = await page.locator(".backup-page:visible .save-picker select").inputValue();
    check(`${locale}/${theme}/${matrix.label}: selected save ownership is stable`, selectedSaveBefore === "arc-synthetic-save", selectedSaveBefore, "ownership");

    const firstBackup = page.locator(".backup-list tbody tr:first-child input[type=checkbox]");
    await firstBackup.check();
    await page.locator("button.backup-action.primary").click();
    await page.locator(".backup-confirm").first().waitFor({ state: "visible", timeout: 5000 });
    await capture(page, run, "restore-review", matrix, locale, theme);
    await reachability(page, "restore-review", [".backup-confirm input[type=checkbox]", ".backup-confirm button"]);
    const commitsBeforeRestoreCancel = await callsBefore();
    // No direct restore cancel button exists in the product card; closing the
    // route is the documented dismissal boundary for an uncommitted restore.
    await clickRoute(page, "search", ".search-page");
    await page.waitForTimeout(200);
    const afterRestoreClose = await callsBefore();
    check(`${locale}/${theme}/${matrix.label}: restore review close dispatches no commit`, !afterRestoreClose.includes("save.commit"), { before: commitsBeforeRestoreCancel, after: afterRestoreClose }, "ownership");
    await clickRoute(page, "backups", ".backup-page"); await waitForSave(page);

    const resignButton = page.locator(".resign-panel > button").first();
    await resignButton.click();
    await page.locator(".resign-panel .backup-confirm").waitFor({ state: "visible", timeout: 5000 });
    await capture(page, run, "resign-review", matrix, locale, theme);
    await reachability(page, "resign-review", [".resign-panel .backup-confirm input[type=checkbox]", ".resign-panel .backup-confirm button:last-child"]);
    const resignCancel = page.locator(".resign-panel .backup-confirm button").last();
    check(`${locale}/${theme}/${matrix.label}: resign review exposes cancel`, await resignCancel.count() === 1);
    await resignCancel.click(); await page.waitForTimeout(250);
    const afterResignCancel = await callsBefore();
    check(`${locale}/${theme}/${matrix.label}: resign cancel dispatches no commit`, !afterResignCancel.includes("save.commit"), afterResignCancel, "ownership");
    check(`${locale}/${theme}/${matrix.label}: save selection survives resign cancel`, await page.locator(".backup-page:visible .save-picker select").inputValue() === "arc-synthetic-save", await page.locator(".backup-page:visible .save-picker select").inputValue(), "ownership");
  } catch (error) {
    check(`${locale}/${theme}/${matrix.label}: save-management flow completed`, false, String(error));
  }

  try {
    await dismissOrdinaryDialog(page, `${locale}/${theme}/${matrix.label}/equipment`);
    const route = await clickRoute(page, "equipment", ".equipment-page.character-page");
    evidence.routes[locale].equipment = route;
    await page.locator(".character-page").waitFor({ state: "visible", timeout: 15000 });
    await page.locator(".character-page:visible .character-modes button").nth(1).click();
    await waitForSave(page);
    await page.locator(".character-page:visible .character-table-wrap").waitFor({ state: "visible", timeout: 15000 });
    await page.waitForFunction(() => document.querySelectorAll('.character-table tbody tr[data-row^="equipment-"]').length >= 1500, null, { timeout: 15000 });
    const rowCount = await page.locator('.character-page:visible .character-table tbody tr[data-row^="equipment-"]').count();
    check(`${locale}/${theme}/${matrix.label}: populated equipment list has approximately 1,500 records`, rowCount >= 1500, rowCount, "data-volume");
    await capture(page, run, "equipment-list-1500", matrix, locale, theme, { rowCount });
    await reachability(page, "equipment-list-1500", [".character-sections [role=tab]", ".character-modes button", ".character-filters input", ".character-table thead", ".character-table tbody tr:first-child"]);
    const listReach = await page.locator(".character-page:visible .character-table-wrap").evaluate(wrap => {
      const first = wrap.querySelector("tbody tr:first-child"), last = wrap.querySelector("tbody tr:last-child");
      const original = wrap.scrollTop; wrap.scrollTop = wrap.scrollHeight;
      const wr = wrap.getBoundingClientRect(), lr = last?.getBoundingClientRect(), fr = first?.getBoundingClientRect();
      const result = { scrollHeight: wrap.scrollHeight, clientHeight: wrap.clientHeight, original, first: fr ? { top: fr.top, bottom: fr.bottom } : null, last: lr ? { top: lr.top, bottom: lr.bottom } : null, wrap: { top: wr.top, bottom: wr.bottom } };
      wrap.scrollTop = original; return result;
    });
    const lastReachable = !!listReach.last && listReach.last.top >= listReach.wrap.top - 1 && listReach.last.bottom <= listReach.wrap.bottom + 1;
    check(`${locale}/${theme}/${matrix.label}: last populated row is reachable after list scroll`, lastReachable, listReach, "geometry");

    await page.locator(".character-page:visible .character-sections [role=tab]").nth(1).click();
    await page.locator('.character-page:visible .character-list [data-row^="add-"]').first().waitFor({ state: "visible", timeout: 10000 });
    await page.locator('.character-page:visible .character-list [data-row^="add-"]').first().click();
    await page.locator(".character-page:visible .character-detail").waitFor({ state: "visible", timeout: 10000 });
    await page.locator(".character-page:visible .character-edit-modes button").nth(1).click();
    await page.waitForTimeout(300);
    await capture(page, run, "add-equipment-editor", matrix, locale, theme);
    await reachability(page, "add-equipment-editor", [".character-edit-modes button", ".character-detail .character-fields input", ".character-detail .character-actions button.primary"]);
    // The add editor has one primary action in this action row; its label is
    // localized, while the class is a stable product affordance.
    const addButton = page.locator(".character-page:visible .character-detail .character-actions button.primary").last();
    check(`${locale}/${theme}/${matrix.label}: add-equipment protected review entry point is visible`, await addButton.count() === 1, await addButton.allTextContents());
    check(`${locale}/${theme}/${matrix.label}: add-equipment queue action is enabled`, !(await addButton.isDisabled()), { disabled: await addButton.isDisabled(), text: await addButton.textContent() });
    await addButton.click({ force: true });
    const planButton = page.locator(".character-page:visible .character-queue button.primary").first();
    await planButton.waitFor({ state: "visible", timeout: 5000 });
    await planButton.click();
    await page.locator(".character-page:visible [data-action='discard-character-plan']").waitFor({ state: "visible", timeout: 5000 });
    await capture(page, run, "add-equipment-review", matrix, locale, theme);
    await reachability(page, "add-equipment-review", [".character-page [data-action='confirm-character-plan']", ".character-page [data-action='commit-character-plan']", ".character-page [data-action='discard-character-plan']"]);
    const beforeDiscard = await callsBefore();
    await page.locator(".character-page:visible [data-action='discard-character-plan']").click(); await page.waitForTimeout(300);
    const afterDiscard = await callsBefore();
    check(`${locale}/${theme}/${matrix.label}: abandoning add-equipment review dispatches no commit`, !afterDiscard.includes("save.commit"), { before: beforeDiscard, after: afterDiscard }, "ownership");
    check(`${locale}/${theme}/${matrix.label}: add-equipment review closes without losing save ownership`, await page.locator(".character-page:visible .save-picker select").inputValue() === "arc-synthetic-save", await page.locator(".character-page:visible .save-picker select").inputValue(), "ownership");
  } catch (error) {
    check(`${locale}/${theme}/${matrix.label}: equipment and add flow completed`, false, String(error));
  }
}

let server;
let browser;
try {
  evidence.source.head = execFileSync("git", ["rev-parse", "HEAD"], { cwd: repoRoot, encoding: "utf8" }).trim();
  evidence.source.scriptSha256 = createHash("sha256").update(await readFile(new URL(import.meta.url))).digest("hex");
  const buildPath = resolve(repoRoot, "apps/workshop/build.mjs");
  execFileSync(process.execPath, [buildPath], { cwd: repoRoot, stdio: "inherit", env: process.env });
  const builtPath = resolve(repoRoot, "deliverables/frontend-v2/search-ui-demo-v2/index.html");
  const builtHtml = await readFile(builtPath, "utf8");
  evidence.source.bundleSha256 = createHash("sha256").update(builtHtml).digest("hex");
  const html = builtHtml.replace("</head>", `<script>${bridge}</script></head>`);
  server = createServer((_request, response) => { response.writeHead(200, { "content-type": "text/html; charset=utf-8" }); response.end(html); });
  await new Promise(done => server.listen(0, "127.0.0.1", done));
  const serverUrl = `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch({ headless: true });
  for (const matrix of MATRIX) {
    for (const locale of RUN_LOCALES) {
      for (const theme of RUN_THEMES) {
        if (!RUN_MATRIX.split(",").includes(matrix.label)) continue;
        const context = await browser.newContext({ viewport: { width: matrix.width, height: matrix.height }, deviceScaleFactor: matrix.dpr });
        const page = await context.newPage();
        page.on("pageerror", error => evidence.failures.push({ name: `${locale}/${theme}/${matrix.label}: browser page error`, category: "browser", details: String(error) }));
        await page.addInitScript(({ locale: initLocale, theme: initTheme }) => { localStorage.setItem("nioh3-ui-locale", initLocale); if (initTheme === "dark") localStorage.setItem("nioh3-theme", "dark"); else localStorage.removeItem("nioh3-theme"); }, { locale, theme });
        try {
          console.log(`RUN ${locale} ${theme} ${matrix.label}`);
          await page.goto(serverUrl, { waitUntil: "domcontentloaded" });
          await page.locator("#root .shell").waitFor({ state: "visible", timeout: 15000 });
          await page.waitForTimeout(500);
          await auditBrandMetadata(page, locale, theme, matrix);
          await runExpanded(page, matrix, locale, theme);
        } catch (error) {
          check(`${locale}/${theme}/${matrix.label}: app mounted`, false, String(error), "harness");
        } finally {
          const bridgeRun = await page.evaluate(() => ({
            calls: window.__arc?.calls?.map(call => call.method) || [],
            commitCalled: typeof window.__arc?.commitCalled === "boolean" ? window.__arc.commitCalled : null,
          })).catch(error => ({ calls: [], commitCalled: null, error: String(error), unknown: true }));
          evidence.boundary.bridgeRuns.push({ locale, theme, matrix: matrix.label, ...bridgeRun });
          if (bridgeRun.commitCalled === true) evidence.boundary.commitCalled = true;
          const forbidden = bridgeRun.calls.filter(method => /^(?:save\.commit|runtime\.character_edit|runtime\.equipment_add_execute|runtime\.live_batch_execute)$/.test(method));
          evidence.boundary.forbiddenWriteMethods.push(...forbidden);
          check(`${locale}/${theme}/${matrix.label}: synthetic bridge observation completed`, !bridgeRun.error && bridgeRun.commitCalled !== null, bridgeRun.error || bridgeRun, "harness");
          check(`${locale}/${theme}/${matrix.label}: no protected commit/write dispatch`, forbidden.length === 0, forbidden, "ownership");
          await context.close();
        }
      }
    }
  }
} catch (error) {
  evidence.failures.push({ phase: "fatal", message: String(error), stack: error?.stack });
} finally {
  if (browser) await browser.close().catch(() => {});
  if (server) await new Promise(done => server.close(done));
  evidence.status = evidence.failures.length ? "failed" : "passed";
  evidence.pass = evidence.status === "passed";
  const jsonPath = resolve(output, "verify-arc-expanded.json");
  evidence.artifact = jsonPath;
  await writeFile(jsonPath, JSON.stringify(evidence, null, 2) + "\n", "utf8");
  const summary = { status: evidence.status, passed: evidence.checks.filter(item => item.passed).length, total: evidence.checks.length, detectorFindings: evidence.detectorFindings.length, failures: evidence.failures.length, evidence: jsonPath };
  console.log(JSON.stringify(summary, null, 2));
  if (!evidence.pass) process.exitCode = 1;
}
