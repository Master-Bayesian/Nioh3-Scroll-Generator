/**
 * Focused browser acceptance for seeded equipment addition.
 *
 * Mounts the production workshop bundle in Chromium with a scripted desktop
 * bridge. It exercises the real UI and SaveSession request shape, but it does
 * not launch a Tauri host, game, or save file. The run stops after plan
 * preparation; save.commit is forbidden by the mock.
 */
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import { execFileSync } from 'node:child_process';
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { chromium } from 'playwright';
import { build } from 'esbuild';
import { dirname, join, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const defaultOutput = 'D:/Nioh3_v080_deliverables/deliverables/codex-v083-handover-20260929/ui';
const defaultProfileRoot = 'D:/Nioh3_v080_deliverables/tmp/codex-v083-seed-ui';
const runLabel = process.env.NIOH3_UI_RUN_LABEL || 'final';
assert.match(runLabel, /^[a-z0-9-]+$/, 'NIOH3_UI_RUN_LABEL must be lowercase and path-safe');

function onDeliveryVolume(value, label) {
  const absolute = resolve(value);
  assert.match(absolute, /^[Dd]:[\\/]/, `${label} must be on D:, got ${absolute}`);
  return absolute;
}

const output = onDeliveryVolume(process.env.NIOH3_UI_OUTPUT || defaultOutput, 'output');
const profileRoot = onDeliveryVolume(process.env.NIOH3_UI_PROFILE_ROOT || defaultProfileRoot, 'profile root');
const profilePath = resolve(profileRoot, `${runLabel}-${process.pid}`);
assert.ok(profilePath.startsWith(profileRoot + sep), 'browser profile must stay below the task profile root');
await mkdir(output, { recursive: true });
await mkdir(profileRoot, { recursive: true });

const evidence = {
  scenario: 'v083-seeded-equipment-addition-ui',
  runLabel,
  status: 'running',
  pass: false,
  boundary: {
    browser: 'headless Chromium against the production workshop bundle',
    bridge: 'scripted window.operations mock; runtime.equipment_seeds and save.prepare_character_edit are recorded fixtures',
    nativeHost: false,
    packaged: false,
    gameProcess: false,
    saveFileAccess: false,
    commitCalled: false,
    viewportCss: { width: 1440, height: 960 },
    deviceScaleFactor: 1,
  },
  source: {},
  checks: [],
  localeRuns: {},
  screenshots: [],
  failures: [],
};

function record(name, passed, details = null, category = 'product') {
  evidence.checks.push({ name, category, passed: Boolean(passed), details });
  if (!passed) evidence.failures.push({ name, category, details });
}
function requireCheck(name, passed, details = null, category = 'product') {
  record(name, passed, details, category);
  assert.ok(passed, `${name}: ${JSON.stringify(details)}`);
}
function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

const bridge = `
const saveId = 'seed-ui-save';
const sourceSha = 'a'.repeat(64);
const saveReference = { save_id: saveId, path: 'D:/synthetic/SAVEDATA00.BIN', account_id: '10001', save_slot: 0 };
const emptyInventory = { save_id: saveId, snapshot_id: 'snapshot-1', source_sha256: sourceSha, account_id: '10001', empty_slots: 10, entries: [] };
const generation = { difficulty: 3, difficulties: [
  { difficulty: 2, progress: [2, 1, 1, 0] },
  { difficulty: 3, progress: [3, 2, 1, 0] },
] };
const seedResponse = (params = {}) => ({
  item_id: params.item_id ?? 171,
  rarity: params.rarity ?? 4,
  level: params.level ?? 180,
  difficulty: params.difficulty ?? 2,
  seeds: 65536,
  empty: 0,
  matches: 2,
  outcomes: [
    { seed: 1111, effects: [{ effect_id: 15, value: 12, roll: 1, star: false, role: 'random' }] },
    { seed: 2222, effects: [{ effect_id: 540, value: 34, roll: 2, star: true, role: 'random' }] },
  ],
});
window.__seed = { calls: [], pending: [], nextMode: 'success', liveMode: window.__seedLiveMode || 'ready', seedResponse };
const liveCharacter = {
  source: 'runtime', game_version: '2.0.2.0', process_id: 4242,
  currencies: { amrita: 0, gold: 0 }, equipment_slots: 0, equipment: [], items: [],
};
const saveCharacter = {
  save_id: saveId, source_sha256: sourceSha, currencies: { amrita: 0, gold: 0 },
  equipment_slots: 0, equipment: [], items: [], generation,
};
const rules = (itemId) => ({
  item_id: itemId, known: true, roles: ['random'],
  random_pool: [
    { effect_id: 15, star: false, min: 1, max: 100, group: 1, masks: [0, 0] },
    { effect_id: 540, star: false, min: 1, max: 100, group: 2, masks: [0, 0] },
  ],
});
const previewItem = {
  slot_index: 42, item_id: 171, appearance_id: 171, quantity: 1, level: 180,
  level_before_forge: 180, plus: 9, familiarity: 0, inventory_key: 42, seed: 2222,
  rarity: 4, effects: [{ index: 0, effect_id: 540, value: 34, star: true }],
};
const canned = () => Promise.resolve({});
window.__seed.execute = async (command) => {
  const method = command?.method;
  const params = command?.params ?? {};
  window.__seed.calls.push({ method, params: structuredClone(params) });
  if (method === 'runtime.character_snapshot') {
    if (window.__seed.liveMode === 'missing') throw new Error('OPERATION_REJECTED: no running process matches Nioh3.exe');
    if (window.__seed.liveMode === 'unloaded') throw new Error('OPERATION_REJECTED: character layout: no character is loaded');
    if (window.__seed.liveMode === 'foreign') throw new Error("OPERATION_REJECTED: character layout: the player object's vtable does not match this build");
    return structuredClone(liveCharacter);
  }
  if (method === 'save.discover') return { saves: [structuredClone(saveReference)] };
  if (method === 'save.inventory') return structuredClone(emptyInventory);
  if (method === 'save.operations') return { operations: [] };
  if (method === 'save.character') return structuredClone(saveCharacter);
  if (method === 'runtime.equipment_rules') return rules(params.item_id);
  if (method === 'runtime.effect_values') return { effect_id: params.effect_id, rarity: params.rarity, level: params.level, star: null, values: [] };
  if (method === 'runtime.equipment_seeds') {
    const mode = window.__seed.nextMode;
    window.__seed.nextMode = 'success';
    if (mode === 'defer') return new Promise((resolve, reject) => window.__seed.pending.push({ resolve, reject, params: structuredClone(params) }));
    if (mode === 'error') throw new Error('The character has not played this difficulty.');
    if (mode === 'no-match') return { ...seedResponse(params), empty: 0, matches: 0, outcomes: [] };
    if (mode === 'all-empty') return { ...seedResponse(params), empty: 65536, matches: 0, outcomes: [] };
    return seedResponse(params);
  }
  if (method === 'save.prepare_character_edit') return {
    plan_id: 'seed-ui-plan', save_id: saveId, kind: 'edit', source_sha256: sourceSha,
    expires_in_seconds: 600, preview: { added: [{ slot_index: 42, after: structuredClone(previewItem), seeded: true }] },
  };
  if (method === 'save.discard') return { discarded: true };
  if (method === 'save.commit') throw new Error('E2E_BOUNDARY: save.commit must not be called');
  if (method === 'runtime.character_edit') throw new Error('E2E_BOUNDARY: live writes are forbidden');
  return {};
};
window.nioh = {
  handshake: canned, searchCatalog: canned, resolveRecommendedLevel: canned,
  startSearch: canned, currentSearch: canned, snapshot: canned, cancelSearch: canned, restartWorker: canned,
  checkFeasibility: canned,
};
window.operations = {
  execute: (command) => window.__seed.execute(command), current: canned, snapshot: canned, cancel: canned,
  selectSave: () => Promise.resolve(structuredClone(saveReference)), prepareInstall: canned, prepareLiveAdd: canned, generate: canned,
  searchNative: canned, captureGrace: canned, bindCachedSearch: canned, prepareCount: canned,
};
window.preferences = {
  getLocale: () => Promise.resolve(localStorage.getItem('nioh3-ui-locale') || 'zh-CN'),
  setLocale: canned,
};
window.support = { diagnostics: () => Promise.resolve({ version: '0.8.3-test' }), exportDiagnostics: canned };
window.review = {
  update: () => Promise.resolve({ phase: 'current', canApply: true }), windowAction: canned,
  favorites: () => Promise.resolve([]), log: canned, copyLog: () => Promise.resolve(''),
  openLink: canned, copyText: canned, release: canned, retain: canned, preview: canned,
  auxiliary: canned, dataDirectory: canned, openSaveFolder: canned, openBackupFolder: canned,
  prepareCart: canned, exportFeedback: canned,
};
`;

let server;
let context;
let serverUrl;
try {
  const built = await build({
    entryPoints: [join(repoRoot, 'apps/workshop/main.tsx')],
    bundle: true,
    write: false,
    outdir: 'out',
    format: 'iife',
    platform: 'browser',
    jsx: 'transform',
    jsxFactory: 'localizedElement',
    tsconfigRaw: { compilerOptions: { jsx: 'react', jsxFactory: 'localizedElement' } },
    inject: [join(repoRoot, 'apps/workshop/presentation-jsx.ts')],
    external: ['game-reference.png'],
    define: { 'process.env.NODE_ENV': '"production"' },
  });
  const bundle = built.outputFiles.find((file) => file.path.endsWith('.js'))?.text;
  const style = built.outputFiles.find((file) => file.path.endsWith('.css'))?.text;
  assert.ok(bundle && style, 'workshop bundle and stylesheet are available');
  const html = `<!doctype html><html lang="zh-CN"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><title>Seed equipment UI E2E</title>
<style>${style}</style></head><body><div id="root"></div><script>${bridge}</script>
<script>${bundle.replaceAll('</script', '<\\/script')}</script></body></html>`;
  await writeFile(join(output, `seed-ui-${runLabel}.html`), html, 'utf8');
  server = createServer((_request, response) => {
    response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
    response.end(html);
  });
  await new Promise((done) => server.listen(0, '127.0.0.1', done));
  serverUrl = `http://127.0.0.1:${server.address().port}`;

  const browserErrors = [];
  context = await chromium.launchPersistentContext(profilePath, {
    headless: true,
    viewport: { width: 1440, height: 960 },
    deviceScaleFactor: 1,
  });
  async function openEditor(locale, liveMode = 'ready') {
    const page = await context.newPage();
    page.on('pageerror', (error) => browserErrors.push(`${locale}: ${error.message}`));
    await page.addInitScript(({ locale, liveMode }) => {
      localStorage.setItem('nioh3-ui-locale', locale);
      window.__seedLiveMode = liveMode;
    }, { locale, liveMode });
    await page.goto(serverUrl);
    await page.waitForSelector('#root .shell', { timeout: 30000 });
    await page.locator('.nav nav > button').nth(4).click();
    await page.locator('.character-page').waitFor({ timeout: 15000 });
    await page.waitForFunction(() => window.__seed.calls.some((call) => call.method === 'runtime.character_snapshot'));
    return page;
  }

  async function enterSaveAdd(page, locale) {
    await page.locator('.character-sections button').nth(1).click();
    await page.locator('.character-add-live').waitFor();
    const liveCopy = await page.locator('.character-add-live').innerText();
    record(`${locale}: live add routes to save flow without seed controls`,
      (await page.locator('.seed-panel').count()) === 0 && liveCopy.length > 0,
      { liveCopy });
    const liveSeedCalls = await page.evaluate(() => window.__seed.calls.filter((call) => call.method === 'runtime.equipment_seeds').length);
    record(`${locale}: no seed search is sent from live add mode`, liveSeedCalls === 0, { liveSeedCalls });
    await page.locator('.character-add-live button.primary').click();
    await page.waitForFunction(() => window.__seed.calls.some((call) => call.method === 'save.character'));
    await page.locator('[data-row="add-171"]').click();
    await page.waitForFunction(() => window.__seed.calls.some((call) => call.method === 'runtime.equipment_rules'));
    await page.waitForFunction(() => {
      const button = document.querySelector('.seed-panel .character-actions button.primary');
      return button && !button.disabled;
    });
    return page;
  }

  async function prepareWanted(page) {
    const fieldInputs = page.locator('.character-fields input');
    await fieldInputs.nth(0).fill('180');
    await fieldInputs.nth(1).fill('9');
    await fieldInputs.nth(2).fill('4');
    await page.locator('.character-fields select').selectOption('2');
    const picker = page.locator('.seed-want .effect-picker input');
    await picker.focus();
    await page.locator('.seed-want .effect-picker-list').waitFor();
    const options = await page.locator('.seed-want .effect-picker-list button').count();
    requireCheck('wanted alternatives fixture has one grouped choice after the empty option', options === 2, { options }, 'harness');
    const alternativeLabel = await page.locator('.seed-want .effect-picker-list button').nth(1).innerText();
    await page.locator('.seed-want .effect-picker-list button').nth(1).click();
    return { alternativeLabel };
  }

  const head = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repoRoot, encoding: 'utf8' }).trim();
  const files = [
    'apps/workshop/CharacterEditor.tsx', 'apps/workshop/BackupManager.tsx',
    'apps/workshop/Editor.tsx', 'apps/workshop/CartActions.tsx', 'apps/workshop/public-errors.ts',
    'apps/workshop/ui-translations.tsv', 'apps/workshop/ui-locales.json',
  ];
  evidence.source = {
    head,
    branch: execFileSync('git', ['branch', '--show-current'], { cwd: repoRoot, encoding: 'utf8' }).trim(),
    worktreeSha256: Object.fromEntries(await Promise.all(files.map(async (file) => [
      file, createHash('sha256').update(await readFile(join(repoRoot, file))).digest('hex'),
    ]))),
  };

  for (const locale of ['zh-CN', 'en-US', 'ja-JP']) {
    const page = await openEditor(locale);
    await enterSaveAdd(page, locale);
    requireCheck(`${locale}: legal add exposes the seed panel`, (await page.locator('.seed-panel').count()) === 1);
    const routeHint = await page.locator('.character-detail > .equipment-notes').first().innerText();
    const routeScope = {
      'zh-CN': /发放物品.*不包含敌人或地区掉落加成/,
      'en-US': /item.grant.*excludes enemy and region drop bonuses/i,
      'ja-JP': /アイテム付与.*敵や地域のドロップ補正は含みません/,
    };
    record(`${locale}: legal add explains the item-grant route boundary`, routeScope[locale].test(routeHint), { routeHint });
    const scopeScreenshot = join(output, `seed-route-${runLabel}-${locale}.png`);
    await page.locator('.character-detail > .equipment-notes').first().scrollIntoViewIfNeeded();
    await page.screenshot({ path: scopeScreenshot, fullPage: false });
    evidence.screenshots.push(scopeScreenshot);
    await page.locator('.character-edit-modes button').nth(1).click();
    record(`${locale}: modded add retains manual effects and hides seed-only controls`,
      (await page.locator('.seed-panel').count()) === 0 && (await page.locator('.character-effects').count()) === 1);
    await page.locator('.character-edit-modes button').nth(0).click();
    await page.locator('.seed-panel').waitFor();

    const { alternativeLabel } = await prepareWanted(page);
    await page.locator('.seed-panel .character-actions button.primary').click();
    await page.waitForFunction(() => document.querySelectorAll('.seed-outcomes button[role="option"]').length === 2);
    const seedCall = (await page.evaluate(() => window.__seed.calls.filter((call) => call.method === 'runtime.equipment_seeds').at(-1)));
    const expectedSearch = {
      item_id: 171, rarity: 4, level: 180, plus: 9, difficulty: 2,
      progress: [2, 1, 1, 0], want: [[15, 540]], limit: 40,
    };
    record(`${locale}: search submits selected difficulty progress and grouped wanted alternatives`,
      JSON.stringify(seedCall?.params) === JSON.stringify(expectedSearch), { expected: expectedSearch, actual: seedCall?.params });
    const second = page.locator('.seed-outcomes button[role="option"]').nth(1);
    await second.click();
    record(`${locale}: chosen result is the second returned seed`, (await second.getAttribute('aria-selected')) === 'true');
    await page.locator('.seed-panel .character-actions button:nth-child(2)').click();
    await page.locator('.character-queue').waitFor();
    await page.locator('.character-queue button.primary').click();
    await page.waitForFunction(() => window.__seed.calls.some((call) => call.method === 'save.prepare_character_edit'));

    const prepareCall = await page.evaluate(() => window.__seed.calls.find((call) => call.method === 'save.prepare_character_edit'));
    const actualAdd = prepareCall?.params?.add?.[0];
    const expectedAdd = { item_id: 171, level: 180, plus: 9, rarity: 4, seed: 2222, difficulty: 2 };
    record(`${locale}: exact chosen seed is carried into save plan preparation`,
      JSON.stringify(actualAdd) === JSON.stringify(expectedAdd), { expected: expectedAdd, actual: actualAdd });
    record(`${locale}: submission uses save plan only and never commits or writes live`,
      !await page.evaluate(() => window.__seed.calls.some((call) => ['save.commit', 'runtime.character_edit'].includes(call.method))),
      { methods: (await page.evaluate(() => window.__seed.calls)).map((call) => call.method) });
    const prepared = await page.locator('.character-plan').first().innerText();
    const hint = await page.locator('.character-hint').innerText();
    const planNotice = (await page.locator('.notice[role="status"]').allInnerTexts()).join('\n');
    const instructionText = `${prepared}\n${hint}\n${planNotice}`;
    const localizedGuard = !/(?:游戏必须关闭|必须关闭游戏|the game must be closed|the game needs to be closed|close the game completely|close nioh 3 completely|ゲームを終了する必要があります|ゲームは終了している必要があります|ゲームを閉じておく必要があります|ゲームを完全に終了してください)/i.test(instructionText);
    const hasPlanNotice = /已生成修改计划|Change plan ready|変更計画を作成しました/.test(planNotice);
    record(`${locale}: save-add instructions do not require closing the game`, localizedGuard && hasPlanNotice, { plan: prepared, hint, planNotice, hasPlanNotice });
    const viewport = await page.evaluate(() => ({ width: innerWidth, height: innerHeight, dpr: devicePixelRatio, lang: document.documentElement.lang }));
    record(`${locale}: requested UI locale is active`, viewport.lang === locale, viewport);
    const screenshot = join(output, `seed-flow-${runLabel}-${locale}.png`);
    await page.locator('.character-plan').first().scrollIntoViewIfNeeded();
    await page.screenshot({ path: screenshot, fullPage: false });
    evidence.screenshots.push(screenshot);
    evidence.localeRuns[locale] = { viewport, alternativeLabel, searchParams: seedCall?.params, submittedAdd: actualAdd, screenshot };
    await page.close();
  }

  for (const locale of ['zh-CN', 'en-US', 'ja-JP']) {
    for (const liveMode of ['missing', 'unloaded']) {
      const page = await openEditor(locale, liveMode);
      await page.locator('.character-page .notice').waitFor();
      const notice = await page.locator('.character-page .notice').innerText();
      record(`${locale}: ${liveMode} character is informational and cannot be edited`,
        await page.locator('.character-page .notice-info').count() === 1 &&
        await page.locator('.character-page .notice[role="alert"]').count() === 0 &&
        await page.locator('.character-currencies').count() === 0, { notice });
      const screenshot = join(output, `character-${liveMode}-${runLabel}-${locale}.png`);
      await page.screenshot({ path: screenshot, fullPage: false });
      evidence.screenshots.push(screenshot);
      if (liveMode === 'unloaded') {
        await page.evaluate(() => { window.__seed.liveMode = 'ready'; });
        await page.locator('.character-toolbar > button').first().click();
        await page.locator('.character-currencies').waitFor();
        await page.evaluate(() => { window.__seed.liveMode = 'missing'; });
        await page.locator('.character-toolbar > button').first().click();
        await page.waitForFunction(() => !document.querySelector('.character-toolbar > button').disabled);
        record(`${locale}: unavailable reload clears the former live snapshot`,
          await page.locator('.character-currencies').count() === 0 && await page.locator('.character-page .notice-info').count() === 1);
      }
      await page.locator('.character-modes button').nth(1).click();
      await page.waitForFunction(() => window.__seed.calls.some(call => call.method === 'save.character'));
      await page.locator('.character-currencies').waitFor();
      record(`${locale}: ${liveMode} game does not prevent save-file mode`,
        await page.locator('.character-page .notice[role="alert"]').count() === 0);
      await page.close();
    }
  }
  const foreignPage = await openEditor('zh-CN', 'foreign');
  await foreignPage.locator('.character-page .notice').waitFor();
  record('genuine character layout mismatch remains a diagnostic error',
    await foreignPage.locator('.character-page .notice-error').count() === 1 &&
    await foreignPage.locator('.character-currencies').count() === 0);
  await foreignPage.close();

  const edgePage = await openEditor('zh-CN');
  await enterSaveAdd(edgePage, 'zh-CN-edge');
  await prepareWanted(edgePage);
  const staleInput = edgePage.locator('.character-fields input').nth(0);
  await edgePage.evaluate(() => { window.__seed.nextMode = 'defer'; });
  await edgePage.locator('.seed-panel .character-actions button.primary').click();
  await edgePage.waitForFunction(() => window.__seed.pending.length === 1);
  const pendingParams = await edgePage.evaluate(() => window.__seed.pending[0].params);
  await staleInput.fill('179');
  await edgePage.evaluate(() => {
    const pending = window.__seed.pending.shift();
    pending.resolve(window.__seed.seedResponse(pending.params));
  });
  await edgePage.waitForFunction(() => {
    const button = document.querySelector('.seed-panel .character-actions button.primary');
    return button && !button.disabled;
  });
  const staleSummaryCount = await edgePage.locator('.seed-summary').count();
  const staleOutcomeCount = await edgePage.locator('.seed-outcomes button[role="option"]').count();
  record('late seed response is discarded after search inputs change',
    staleSummaryCount === 0 && staleOutcomeCount === 0,
    { searched: pendingParams, currentLevel: await staleInput.inputValue(), staleSummaryCount, staleOutcomeCount });

  await staleInput.fill('180');
  await edgePage.evaluate(() => { window.__seed.nextMode = 'no-match'; });
  await edgePage.locator('.seed-panel .character-actions button.primary').click();
  await edgePage.locator('.seed-summary').waitFor();
  const noMatchSummary = await edgePage.locator('.seed-summary').innerText();
  const noMatchAdd = edgePage.locator('.seed-panel .character-actions button:nth-child(2)');
  record('zero-match response explains no legal seed and cannot be selected',
    noMatchSummary.includes('当前生成路线无匹配') &&
    (await edgePage.locator('.seed-outcomes button[role="option"]').count()) === 0 && await noMatchAdd.isDisabled(),
    { summary: noMatchSummary, addDisabled: await noMatchAdd.isDisabled() });

  await edgePage.evaluate(() => { window.__seed.nextMode = 'all-empty'; });
  await edgePage.locator('.seed-panel .character-actions button.primary').click();
  await edgePage.locator('.seed-summary').waitFor();
  const allEmptySummary = await edgePage.locator('.seed-summary').innerText();
  record('all-empty generator result has a distinct explanation and no selectable outcome',
    allEmptySummary.includes('当前生成路线下') && allEmptySummary !== noMatchSummary &&
    (await edgePage.locator('.seed-outcomes button[role="option"]').count()) === 0,
    { allEmptySummary, noMatchSummary });

  await edgePage.evaluate(() => { window.__seed.nextMode = 'error'; });
  await edgePage.locator('.seed-panel .character-actions button.primary').click();
  await edgePage.locator('.notice[role="alert"]').waitFor();
  const errorText = await edgePage.locator('.notice[role="alert"] p').innerText();
  record('backend search error is presented in localized player language',
    errorText.includes('这个角色还没有玩过所选难度') && !errorText.includes('The character has not played'),
    { errorText });
  await edgePage.evaluate(() => { window.__seed.nextMode = 'success'; });
  await edgePage.locator('.seed-panel .character-actions button.primary').click();
  await edgePage.waitForFunction(() => document.querySelectorAll('.seed-outcomes button[role="option"]').length === 2);
  record('search can be retried after a backend error', (await edgePage.locator('.seed-outcomes button[role="option"]').count()) === 2);
  const edgeShot = join(output, `seed-flow-${runLabel}-edge-zh-CN.png`);
  await edgePage.locator('.seed-panel').scrollIntoViewIfNeeded();
  await edgePage.screenshot({ path: edgeShot, fullPage: false });
  evidence.screenshots.push(edgeShot);
  await edgePage.close();

  evidence.browserErrors = browserErrors;
  record('no browser page errors', browserErrors.length === 0, browserErrors);
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  const category = error?.name === 'AssertionError' || error?.name === 'TimeoutError' ? 'product' : 'harness';
  evidence.failures.push({ phase: 'fatal', category, message, stack: error instanceof Error ? error.stack : '' });
} finally {
  evidence.boundary.commitCalled = evidence.checks.some((entry) => entry.name.includes('commit') && entry.passed === false);
  evidence.status = evidence.failures.length ? 'failed' : 'passed';
  evidence.pass = evidence.status === 'passed';
  if (context) await context.close().catch(() => {});
  if (server) await new Promise((done) => server.close(done));
  const profileBase = resolve(profileRoot) + sep;
  if (profilePath.startsWith(profileBase)) await rm(profilePath, { recursive: true, force: true });
  const jsonPath = join(output, `verify-seed-equipment-${runLabel}.json`);
  const markdownPath = join(output, `verify-seed-equipment-${runLabel}.md`);
  await writeFile(jsonPath, JSON.stringify(evidence, null, 2) + '\n', 'utf8');
  const failed = evidence.checks.filter((entry) => !entry.passed);
  const productChecks = evidence.checks.filter((entry) => entry.category === 'product');
  const harnessChecks = evidence.checks.filter((entry) => entry.category === 'harness');
  const productFailures = evidence.failures.filter((entry) => entry.category === 'product');
  const harnessFailures = evidence.failures.filter((entry) => entry.category === 'harness');
  await writeFile(markdownPath, [
    '# v0.8.3 seeded equipment UI browser acceptance',
    '',
    `- Status: ${evidence.status}`,
    `- Source HEAD: \`${evidence.source.head ?? 'not captured'}\` (${evidence.source.branch ?? 'unknown'})`,
    '- Browser: headless Chromium; CSS viewport 1440 × 960, DPR 1.',
    '- Bridge: scripted `window.operations` mock. The UI issued `runtime.equipment_seeds` and prepared `save.prepare_character_edit` payloads against fixtures.',
    '- Boundary: no native Tauri host, packaged application, game process, save-file access, save commit, or live write.',
    `- Total reached checks: ${evidence.checks.filter((entry) => entry.passed).length}/${evidence.checks.length} passed; ${failed.length} check failures.`,
    `- Product checks: ${productChecks.filter((entry) => entry.passed).length}/${productChecks.length} passed; ${productFailures.length} product failures.`,
    `- Harness checks: ${harnessChecks.filter((entry) => entry.passed).length}/${harnessChecks.length} passed; ${harnessFailures.length} harness failures.`,
    `- Screenshots: ${evidence.screenshots.map((path) => `\`${path.split(/[\\/]/).pop()}\``).join(', ') || 'none'}.`,
    '',
    failed.length ? '## Failed checks\n\n' + failed.map((entry) => `- [${entry.category}] ${entry.name}: ${JSON.stringify(entry.details)}`).join('\n') : 'All recorded browser checks passed.',
    evidence.failures.some((entry) => entry.phase === 'fatal') ? `Harness stopped early: ${evidence.failures.filter((entry) => entry.phase === 'fatal').length} fatal error(s); later scenarios may be unexecuted.` : 'The full configured scenario sequence reached its end.',
    '',
    `Detailed trace: \`${jsonPath}\`.`,
    '',
  ].join('\n'), 'utf8');
  console.log(`Seed UI browser acceptance: ${evidence.status}; ${evidence.checks.filter((entry) => entry.passed).length}/${evidence.checks.length} checks passed.`);
  console.log(`Evidence: ${jsonPath}`);
  if (!evidence.pass) process.exitCode = 1;
}
