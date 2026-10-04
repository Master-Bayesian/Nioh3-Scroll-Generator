/**
 * Production UI acceptance for character drafts and cursor following.
 *
 * Mounts the production workshop bundle in Chromium with a scripted desktop
 * bridge, or drives the matching outer one-file in native WebView2. Both paths
 * use read-only snapshot/cursor fixtures; commits and live writes are forbidden.
 */
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import { execFileSync, spawn } from 'node:child_process';
import { parseArgs } from 'node:util';
import { isolatedEnvironment, inspectOnefile, closeSession, pause } from '../tauri/onefile-acceptance.mjs';
const { values: options } = parseArgs({ options: { exe: { type: 'string' }, onefile: { type: 'boolean', default: false }, out: { type: 'string' }, game: { type: 'string' } } });
assert.ok(!options.exe || options.onefile, 'Native acceptance requires an outer one-file candidate');
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { chromium } from 'playwright';
import { build } from 'esbuild';
import { dirname, join, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const defaultOutput = 'D:/Nioh3_v080_deliverables/deliverables/codex-release-candidate-20261003/evidence/character-draft';
const defaultProfileRoot = 'D:/Nioh3_v080_deliverables/tmp/codex-test12-draft-ui';
const runLabel = process.env.NIOH3_UI_RUN_LABEL || 'final';
assert.match(runLabel, /^[a-z0-9-]+$/, 'NIOH3_UI_RUN_LABEL must be lowercase and path-safe');

function onDeliveryVolume(value, label) {
  const absolute = resolve(value);
  assert.match(absolute, /^[Dd]:[\\/]/, `${label} must be on D:, got ${absolute}`);
  return absolute;
}

const output = onDeliveryVolume(options.out || process.env.NIOH3_UI_OUTPUT || defaultOutput, 'output');
const profileRoot = onDeliveryVolume(process.env.NIOH3_UI_PROFILE_ROOT || defaultProfileRoot, 'profile root');
const profilePath = resolve(profileRoot, `${runLabel}-${process.pid}`);
assert.ok(profilePath.startsWith(profileRoot + sep), 'browser profile must stay below the task profile root');
await mkdir(output, { recursive: true });
await mkdir(profileRoot, { recursive: true });

const evidence = {
  scenario: 'test12-character-draft-ui',
  runLabel,
  status: 'running',
  pass: false,
  boundary: {
    browser: options.exe ? 'native WebView2 against the actual candidate frontend' : 'headless Chromium against the production workshop bundle',
    bridge: 'scripted read-only runtime snapshot, legal values, and inventory cursor; all writes forbidden',
    nativeHost: Boolean(options.exe),
    packaged: options.onefile,
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
const equipment = (slot) => ({
  slot_index: slot, item_id: 171, appearance_id: 171, quantity: 1,
  level: 180, level_before_forge: 180, plus: 0, rarity: 4, familiarity: 0,
  inventory_key: slot, seed: 2222, record_sha256: 'b'.repeat(64),
  effects: [{ index: 0, effect_id: 15, value: 100, star: false }, { index: 1, effect_id: 540, value: 100, star: false }],
});
window.__seed.cursor = { menu_open: true, container: 'equipment', slot_index: 42, item_id: 171 };
const liveCharacter = {
  source: 'runtime', game_version: '2.0.2.0', process_id: 4242,
  currencies: { amrita: 0, gold: 0 }, equipment_slots: 4, equipment: [equipment(42), equipment(43), { ...equipment(44), item_id: 0xbd6b, type_class: 54 }, { ...equipment(45), item_id: 0xfffe, type_class: 54 }],
  items: [{ container: 'held', slot_index: 3, item_id: 8001, quantity: 5, limit: 99, record_sha256: 'c'.repeat(64) }],
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
  if (method === 'runtime.compatibility') return { compatibility: { present: true, reference_match: true, warning: false, accepted: false, plan: null, backup: null, differences: [], hard_blocks: [], game_version: '2.0.2.0', executable: 'D:/synthetic/Nioh3.exe' } };
  if (method === 'runtime.menu_selection') return structuredClone(window.__seed.cursor);
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
  if (method === 'runtime.equipment_rules') return { ...rules(params.item_id), roles: ['random', 'random'] };
  if (method === 'runtime.effect_values') return { effect_id: params.effect_id, rarity: params.rarity, level: params.level, star: false, values: [{ value: 50, roll_min: 1, roll_max: 1, top_fraction: 1 }, { value: 100, roll_min: 2, roll_max: 2, top_fraction: 0.5 }] };
  if (method === 'runtime.equipment_seeds') {
    const mode = window.__seed.nextMode;
    window.__seed.nextMode = 'success';
    if (mode === 'defer') return new Promise((resolve, reject) => window.__seed.pending.push({ resolve, reject, params: structuredClone(params) }));
    if (mode === 'error') throw new Error('The character has not played this difficulty.');
    if (mode === 'no-match') return { ...seedResponse(params), empty: 0, matches: 0, outcomes: [] };
    if (mode === 'all-empty') return { ...seedResponse(params), empty: 65536, matches: 0, outcomes: [] };
    return seedResponse(params);
  }
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
let child;
let nativePage;
let session;
try {
  if (!options.exe) {
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
  await writeFile(join(output, `character-draft-${runLabel}.html`), html, 'utf8');
  server = createServer((_request, response) => {
    response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
    response.end(html);
  });
  await new Promise((done) => server.listen(0, '127.0.0.1', done));
  serverUrl = `http://127.0.0.1:${server.address().port}`;

  context = await chromium.launchPersistentContext(profilePath, {
    headless: true,
    viewport: { width: 1440, height: 960 },
    deviceScaleFactor: 1,
  });
  } else {
    const identity = await inspectOnefile(options.exe);
    evidence.packageIdentity = { executable: resolve(options.exe), sha256: identity.sha256, payloadSha256: identity.payloadSha256 };
    const { env, port } = await isolatedEnvironment(profilePath);
    if (options.game) {
      await mkdir(env.NIOH3_TAURI_TEST_ROOT, { recursive: true });
      await writeFile(join(env.NIOH3_TAURI_TEST_ROOT, 'game-install.json'), JSON.stringify({ schema: 'nioh3-game-install/v1', executable: resolve(options.game) }));
    }
    child = spawn(resolve(options.exe), ['--user-data-dir', env.NIOH3_TAURI_TEST_ROOT], { env, windowsHide: true, stdio: 'ignore' });
    for (let attempt = 0; attempt < 150; attempt++) {
      if (child.exitCode !== null) throw new Error('Candidate exited before CDP was ready: ' + child.exitCode);
      try { if ((await fetch('http://127.0.0.1:' + port + '/json/version')).ok) break; } catch {}
      await pause(300);
    }
    context = await chromium.connectOverCDP('http://127.0.0.1:' + port);
    nativePage = context.contexts()[0].pages()[0];
    session = { browser: context, page: nativePage };
    await nativePage.locator('.shell').waitFor();
    const viewport = await nativePage.evaluate(() => ({ width: innerWidth, height: innerHeight, dpr: devicePixelRatio }));
    evidence.boundary.viewportCss = { width: viewport.width, height: viewport.height };
    evidence.boundary.deviceScaleFactor = viewport.dpr;
    evidence.packageDiagnostics = await nativePage.evaluate(() => window.support.diagnostics());
    requireCheck('native manifest matches frozen source', evidence.packageDiagnostics.packageVerification.sourceCommit === execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repoRoot, encoding: 'utf8' }).trim(), evidence.packageDiagnostics.packageVerification.sourceCommit, 'harness');
    requireCheck('native package hashes verified', evidence.packageDiagnostics.packageVerification.ok, null, 'harness');
    await nativePage.evaluate(() => { window.__nativeWindowAction = window.review.windowAction.bind(window.review); });
    await nativePage.evaluate(bridge);
    await nativePage.evaluate(() => { window.review.windowAction = window.__nativeWindowAction; });
  }
  const browserErrors = [];
  async function openEditor(locale, liveMode = 'ready') {
    const page = nativePage || await context.newPage();
    if (nativePage) {
      await page.evaluate(() => { window.__seed.cursor = { menu_open: true, container: 'equipment', slot_index: 42, item_id: 171 }; });
      await page.locator('.nav nav > button').nth(0).click();
    }
    page.on('pageerror', (error) => browserErrors.push(`${locale}: ${error.message}`));
    await page.addInitScript(({ locale, liveMode }) => {
      localStorage.setItem('nioh3-ui-locale', locale);
      window.__seedLiveMode = liveMode;
    }, { locale, liveMode });
    if (nativePage) {
      const titles = { 'zh-CN': '简体中文', 'en-US': 'English', 'ja-JP': '日本語' };
      await page.locator('.language-button').click();
      await page.locator('.side-popup button').filter({ hasText: titles[locale] }).click();
    } else await page.goto(serverUrl);
    await page.waitForSelector('#root .shell', { timeout: 30000 });
    await page.locator('.nav nav > button').nth(4).click();
    await page.locator('.character-page').waitFor({ timeout: 15000 });
    await page.waitForFunction(() => window.__seed.calls.some((call) => call.method === 'runtime.character_snapshot'));
    return page;
  }


  evidence.source = {
    head: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repoRoot, encoding: 'utf8' }).trim(),
    characterEditorSha256: createHash('sha256').update(await readFile(join(repoRoot, 'apps/workshop/CharacterEditor.tsx'))).digest('hex'),
  };
  const words = {
    'zh-CN': { revert: '还原', max: '最高', allMax: '全部取合法最高', changed: '有未应用的修改', unchanged: '尚未修改' },
    'en-US': { revert: 'Revert', max: 'Highest', allMax: 'Set all to legal maximum', changed: 'Unsaved changes', unchanged: 'Unchanged' },
    'ja-JP': { revert: '元に戻す', max: '最高', allMax: 'すべて合法な最大値にする', changed: '未適用の変更あり', unchanged: '変更なし' },
  };
  async function move(page, slot) {
    const count = await page.evaluate(slot => {
      window.__seed.cursor.slot_index = slot;
      return window.__seed.calls.filter(call => call.method === 'runtime.menu_selection').length;
    }, slot);
    await page.waitForFunction(count => window.__seed.calls.filter(call => call.method === 'runtime.menu_selection').length >= count + 2, count);
  }
  for (const locale of (runLabel === 'before' ? ['zh-CN'] : ['zh-CN', 'en-US', 'ja-JP'])) {
    const page = await openEditor(locale);
    const w = words[locale];
    const revert = page.locator('.character-detail .character-actions button').filter({ hasText: new RegExp('^' + w.revert + '$') });
    await page.locator('[data-row="equipment-42"]').click();
    await page.locator('.character-value button').first().waitFor();
    record(locale + ': initial draft has no revert action', await revert.count() === 0);
    await page.locator('.character-value button').first().click();
    record(locale + ': already-highest click preserves unchanged draft', await revert.count() === 0);
    await move(page, 43);
    record(locale + ': following continues after an already-highest click', await page.locator('[data-row="equipment-43"]').getAttribute('class') === 'selected', await page.locator('.character-hint').innerText());
    if (runLabel !== 'before') {
      await page.locator('[data-row="equipment-42"]').click();
      await page.evaluate(() => { window.__seed.cursor.slot_index = 42; });
      await page.locator('.character-value button').first().waitFor();
      await page.locator('.character-detail .character-actions > button').first().click();
      record(locale + ': bulk maximum preserves unchanged draft', await revert.count() === 0);
      record(locale + ': unchanged status is localized', await page.locator('[data-draft-state="unchanged"]').innerText() === w.unchanged);
      const value = page.locator('.character-value input').first();
      await value.fill('50');
      await page.locator('.character-value button').first().click();
      record(locale + ': returning to the source maximum clears the edit', await revert.count() === 0 && await value.inputValue() === '100');
      await value.fill('50');
      record(locale + ': visible edit has localized unapplied status', await page.locator('[data-draft-state="changed"]').innerText() === w.changed);
      await move(page, 43);
      record(locale + ': actual edit pauses following and preserves the input', await value.inputValue() === '50' && await page.locator('[data-row="equipment-42"]').getAttribute('class') === 'selected' && await page.locator('.character-follow-note').count() === 1);
      const shot = join(output, 'character-draft-' + runLabel + '-' + locale + '.png');
      await page.screenshot({ path: shot });
      evidence.screenshots.push(shot);
      await revert.click();
      await move(page, 43);
      record(locale + ': revert clears status and resumes following', await page.locator('[data-row="equipment-43"]').getAttribute('class') === 'selected' && await page.locator('[data-draft-state="unchanged"]').count() === 1);
      // Existing legal-write conflict handling is checked before any mock mutation.
      await page.locator('.character-edit-modes button').nth(1).click();
      await page.locator('.character-effects .effect-picker input').nth(1).focus();
      await page.locator('.character-effects .effect-picker-list button').nth(1).click();
      await page.locator('.character-edit-modes button').nth(0).click();
      record(locale + ': conflicting draft is explained before writing', await page.locator('.character-findings li').count() > 0, await page.locator('.character-findings').innerText());
      await page.locator('.character-detail .character-actions button.primary').click();
      const refused = await page.evaluate(() => window.__seed.calls.every(call => call.method !== 'runtime.character_edit'));
      record(locale + ': legal conflict refusal does not dispatch a write', refused);
      await revert.click();
      // Verified open state with a temporarily absent cursor must not claim a closed menu.
      const selectionBeforeWaiting = await page.locator('[data-row^="equipment-"].selected').getAttribute('data-row');
      await page.evaluate(() => { window.__seed.cursor = { menu_open: true, slot_index: null }; });
      const waiting = { 'zh-CN': '持有物品菜单已打开，请在游戏中选中一件物品', 'en-US': 'The inventory menu is open. Select an item in the game.', 'ja-JP': '所持品メニューは開いています。ゲーム内でアイテムを選択してください。' };
      await page.waitForFunction(expected => document.querySelector('.character-follow-note')?.textContent === expected, waiting[locale]);
      record(locale + ': open menu without cursor waits without clearing the draft', await page.locator('[data-row^="equipment-"].selected').getAttribute('data-row') === selectionBeforeWaiting && await page.locator('[data-draft-state="unchanged"]').count() === 1);
      await page.evaluate(() => { window.__seed.cursor = { menu_open: true, container: 'equipment', slot_index: 44, item_id: 0xbd6b }; });
      await move(page, 44);
      const soulName = { 'zh-CN': '魑魅的魂核', 'en-US': 'Sudama Soul Core', 'ja-JP': '魑魅の魂核' };
      record(locale + ': Sudama core is selected with its localized name', await page.locator('[data-row="equipment-44"]').getAttribute('class') === 'selected' && (await page.locator('[data-row="equipment-44"]').innerText()).includes(soulName[locale]));
      const namedShot = join(output, 'sudama-soul-core-' + runLabel + '-' + locale + '.png'); await page.screenshot({ path: namedShot }); evidence.screenshots.push(namedShot);
      await page.evaluate(() => { window.__seed.cursor = { menu_open: true, container: 'equipment', slot_index: 45, item_id: 0xfffe }; }); await move(page, 45);
      record(locale + ': unlisted soul core keeps the exact ID and can be selected', await page.locator('[data-row="equipment-45"]').getAttribute('class') === 'selected' && (await page.locator('[data-row="equipment-45"]').innerText()).includes('0xFFFE'));
      record(locale + ': unknown name does not block existing-record fields', await page.locator('.character-value input').first().isEnabled());
      const unknownShot = join(output, 'unknown-soul-core-' + runLabel + '-' + locale + '.png'); await page.screenshot({ path: unknownShot }); evidence.screenshots.push(unknownShot);
      await page.evaluate(() => { window.__seed.cursor = { menu_open: true, container: 'equipment', slot_index: 42, item_id: 171 }; }); await move(page, 42);
      // Quantity edits share the same explicit status, while currencies remain independent.
      await page.evaluate(() => { window.__seed.cursor.menu_open = false; });
      await page.locator('.character-tabs button').nth(1).click();
      await page.locator('[data-row="items-8001"]').click();
      await page.locator('.character-detail .character-fields input').fill('6');
      record(locale + ': quantity edits show the localized status', await page.locator('[data-draft-state="changed"]').innerText() === w.changed);
      await revert.click();
      record(locale + ': quantity revert clears the status', await page.locator('[data-draft-state="unchanged"]').innerText() === w.unchanged);
    }
    const mutations = await page.evaluate(() => window.__seed.calls.filter(call => /commit|prepare_character_edit|character_edit|add_equipment/.test(call.method)));
    record(locale + ': no mutation requested', mutations.length === 0, mutations, 'harness');
    if (!nativePage) await page.close();
  }
  if (nativePage && runLabel !== 'before') {
    await nativePage.evaluate(() => window.review.windowAction('maximize'));
    await pause(300);
    await nativePage.locator('[data-row="items-8001"]').click();
    await nativePage.locator('.character-detail .character-fields input').fill('6');
    const geometry = await nativePage.locator('[data-draft-state="changed"]').evaluate(element => {
      const rect = element.getBoundingClientRect();
      return { left: rect.left, right: rect.right, top: rect.top, bottom: rect.bottom, width: innerWidth, height: innerHeight, dpr: devicePixelRatio };
    });
    record('native maximized draft status remains visible', geometry.left >= 0 && geometry.right <= geometry.width && geometry.top >= 0 && geometry.bottom <= geometry.height, geometry);
    const shot = join(output, 'character-draft-' + runLabel + '-maximized.png');
    await nativePage.screenshot({ path: shot });
    evidence.screenshots.push(shot);
    await nativePage.evaluate(() => window.review.windowAction('maximize'));
  }
  evidence.browserErrors = browserErrors;
  record('no browser page errors', browserErrors.length === 0, browserErrors);
} catch (error) {
  evidence.failures.push({ phase: 'fatal', message: error.message, stack: error.stack });
} finally {
  evidence.status = evidence.failures.length ? 'failed' : 'passed';
  evidence.pass = evidence.status === 'passed';
  if (nativePage || child) {
    try { await closeSession(session, child); } catch (error) { evidence.failures.push({ phase: 'close', message: error.message }); }
    evidence.status = evidence.failures.length ? 'failed' : 'passed';
    evidence.pass = evidence.status === 'passed';
  } else if (context) await context.close();
  if (server) await new Promise(done => server.close(done));
  if (!nativePage && profilePath.startsWith(resolve(profileRoot) + sep)) await rm(profilePath, { recursive: true, force: true });
  const jsonPath = join(output, 'verify-character-draft-' + runLabel + '.json');
  await writeFile(jsonPath, JSON.stringify(evidence, null, 2) + '\n', 'utf8');
  console.log(JSON.stringify({ status: evidence.status, passed: evidence.checks.filter(x => x.passed).length, total: evidence.checks.length, failures: evidence.failures, evidence: jsonPath }, null, 2));
  if (!evidence.pass) process.exitCode = 1;
}
