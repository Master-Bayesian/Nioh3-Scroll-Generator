/**
 * Focused read-only equipment-browser acceptance.
 *
 * Runs the real component in headless Chromium against a mocked IPC bridge, so
 * no game process, save file or native build is involved. Live-game and packaged
 * acceptance stay with the separate final integration owner.
 */
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { chromium } from 'playwright';
import { build } from 'esbuild';
import { verifySidebarAlignment } from './verify-sidebar-alignment.mjs';
import { verifySectionHelp } from './verify-section-help.mjs';

const output = resolve(
  process.env.NIOH3_UI_OUTPUT ||
    'D:/Nioh3_v080_deliverables/deliverables/v081-equipment-browser-20260921/frontend/browser',
);
await mkdir(output, { recursive: true });

// This is the reviewed backend output, not a hand-written vendor catalog.
// Keeping the fixture outside the source tree makes the harness prove the
// renderer contract against the exact bytes and response accepted by the
// protected adapter.
const catalogFixturePath = resolve(
  process.env.NIOH3_LOCAL_CATALOG_RESPONSE_FIXTURE ||
    'D:/Nioh3_v080_deliverables/deliverables/v081-local-catalog-20260921/backend/valid_little_endian-responses.json',
);
const catalogCommandFixturePath = resolve(
  process.env.NIOH3_LOCAL_CATALOG_COMMAND_FIXTURE ||
    'D:/Nioh3_v080_deliverables/deliverables/v081-local-catalog-20260921/backend/valid_little_endian-commands.json',
);
const catalogResponses = JSON.parse(await readFile(catalogFixturePath, 'utf8'));
const catalogCommands = JSON.parse(await readFile(catalogCommandFixturePath, 'utf8'));
const catalogFixture = catalogResponses.find((entry) => entry.id === 'import')?.result;
const catalogCommand = catalogCommands.import;
assert.ok(catalogFixture && catalogCommand, 'Reviewed local-catalog fixture is complete');
const catalogBytes = Buffer.from(catalogCommand.params.content_base64, 'base64');

const bundle = await build({
  entryPoints: ['apps/workshop/main.tsx'],
  bundle: true,
  write: false,
  outdir: 'out',
  format: 'iife',
  platform: 'browser',
  jsx: 'transform',
  jsxFactory: 'localizedElement',
  tsconfigRaw: { compilerOptions: { jsx: 'react', jsxFactory: 'localizedElement' } },
  inject: ['apps/workshop/presentation-jsx.ts'],
  external: ['game-reference.png'],
  define: { 'process.env.NODE_ENV': '"production"' },
});
const script = bundle.outputFiles
  .find((file) => file.path.endsWith('.js'))
  .text.replaceAll('</script', '<\\/script');
const style = bundle.outputFiles.find((file) => file.path.endsWith('.css')).text;

const bridgeMock = `
const localCatalogFixture = ${JSON.stringify(catalogFixture)};
const localCatalogCommand = ${JSON.stringify(catalogCommand)};
window.__catalog = { calls: [], queue: [], mode: 'ok', fixture: localCatalogFixture, command: localCatalogCommand };
const slots = [
  { slot: 0, id: 0x1234, raw_value: 80 },
  { slot: 1, id: 0xFFFF, raw_value: 0 },
  { slot: 2, id: 0x5678, raw_value: 4 },
  { slot: 3, id: 0x7B14, raw_value: 0 },
  { slot: 4, id: 0xFFFF, raw_value: 0 },
  { slot: 5, id: 0xFFFF, raw_value: 0 },
  { slot: 6, id: 0xFFFF, raw_value: 0 },
];
window.__eq = {
  calls: [], queue: [], mode: 'ok', total: 128,
  process: { pid: 4242, creation_filetime: '133000000000000000' },
  makeRow(slot) {
    return {
      slot,
      item_id: slot === 0 ? 0xF6E8 : slot === 1 ? 0x0000 : 0xFF00 + slot,
      level_raw: slot === 0 ? 180 : 42,
      plus_raw: slot === 0 ? 20 : 0,
      quantity_raw: 1,
      rarity_raw: slot === 0 ? 4 : 0,
      record_sha256: (slot.toString(16).padStart(8, '0') + '0'.repeat(56)).slice(0, 64),
      effects: slots,
    };
  },
  snapshot(start, limit) {
    const end = Math.min(start + limit, this.total);
    return {
      status: 'observed', game_version: '2.0.2.0',
      observed_at: '2026-09-21T21:18:25Z',
      process: { ...this.process },
      start, limit, observed_slot_count: this.total,
      next_start: end < this.total ? end : null,
      rows: Array.from({ length: Math.max(0, end - start) }, (_, index) => this.makeRow(start + index)),
      consistency: 'reread_equal', read_only: true,
    };
  },
};
window.__eq.execute = (command) => {
  window.__eq.calls.push({ method: command.method, params: command.params });
  if (command.method === 'catalog.import_names') {
    window.__catalog.calls.push({ method: command.method, params: command.params });
    if (window.__catalog.mode === 'error')
      return Promise.reject(new Error('CATALOG_IMPORT_REJECTED: malformed items_little_endian.json'));
    if (window.__catalog.mode === 'defer')
      return new Promise((resolve, reject) =>
        window.__catalog.queue.push({ resolve, reject, response: window.__catalog.fixture }));
    return Promise.resolve(window.__catalog.fixture);
  }
  if (command.method !== 'runtime.inventory_snapshot') return Promise.resolve({});
  if (window.__eq.mode === 'error')
    return Promise.reject(new Error('No running Nioh3 process was found in this session.'));
  const start = (command.params && command.params.start) || 0;
  const limit = (command.params && command.params.limit) || 64;
  if (window.__eq.mode === 'defer')
    return new Promise((resolve, reject) =>
      window.__eq.queue.push({ resolve, reject, response: window.__eq.snapshot(start, limit) }));
  return Promise.resolve(window.__eq.snapshot(start, limit));
};
const canned = () => Promise.resolve({});
window.nioh = {
  handshake: canned, searchCatalog: () => Promise.resolve({}), resolveRecommendedLevel: canned,
  startSearch: canned, currentSearch: canned, snapshot: canned, cancelSearch: canned, restartWorker: canned,
};
window.operations = {
  execute: (command) => window.__eq.execute(command), current: canned, snapshot: canned, cancel: canned,
  selectSave: () => Promise.resolve(null), prepareInstall: canned, prepareLiveAdd: canned, generate: canned,
  searchNative: canned, captureGrace: canned, bindCachedSearch: canned, prepareCount: canned,
};
window.preferences = { getLocale: () => Promise.resolve('zh-CN'), setLocale: canned };
window.support = { diagnostics: () => Promise.resolve({ version: '0.8.1' }), exportDiagnostics: canned };
window.review = {
  update: () => Promise.resolve({ phase: 'current', canApply: true }), windowAction: canned,
  favorites: () => Promise.resolve([]), log: canned, copyLog: () => Promise.resolve(''),
  openLink: canned, copyText: canned, release: canned, retain: canned, preview: canned,
  auxiliary: canned, dataDirectory: canned, openSaveFolder: canned, openBackupFolder: canned,
  prepareCart: canned,
};
`;

const html = `<!doctype html><html lang="zh-CN"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><title>Nioh3 equipment browser acceptance</title>
<style>${style}</style></head><body><div id="root"></div>
<script>${bridgeMock}</script>
<script>${script}</script></body></html>`;
await writeFile(join(output, 'index.html'), html, 'utf8');

const server = createServer((request, response) => {
  response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
  response.end(html);
});
await new Promise((listening) => server.listen(0, '127.0.0.1', listening));
const origin = `http://127.0.0.1:${server.address().port}/`;

const checks = [];
const errors = [];
const check = (name, condition) => {
  assert.ok(condition, name);
  checks.push(name);
  console.log(name);
};
const snapshotCalls = (page) =>
  page.evaluate(() => window.__eq.calls.filter((call) => call.method === 'runtime.inventory_snapshot'));
const catalogCalls = (page) =>
  page.evaluate(() => window.__catalog.calls.filter((call) => call.method === 'catalog.import_names'));
const rows = (page) => page.locator('.equipment-table tbody tr');
const selectLocale = async (page, label) => {
  await page.locator('.language-button').click();
  await page.locator('.side-popup button', { hasText: label }).first().click();
  await page.locator('.side-popup').waitFor({ state: 'detached' });
};

const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1600, height: 1000 } });
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto(origin);
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await page.getByRole('heading', { name: '绘卷搜索', exact: true }).waitFor();

  check('No IPC read happens on startup', (await snapshotCalls(page)).length === 0);
  check('Sidebar keeps every existing entry plus the new one',
    await page.locator('.nav nav > button').count() === 6);
  check('Equipment entry precedes the disabled coming-soon entry',
    (await page.locator('.nav nav > button').nth(4).innerText()).includes('装备') &&
    await page.locator('.nav nav > button').nth(5).isDisabled());

  await page.getByRole('button', { name: '装备', exact: true }).click();
  await page.getByRole('heading', { name: '装备浏览', exact: true }).waitFor();
  check('Navigation performs no IPC read', (await snapshotCalls(page)).length === 0);
  check('Unloaded state is explicit',
    await page.getByText('尚未加载。请点击加载装备按钮读取当前背包。', { exact: true }).isVisible());
  check('Read-only description is present',
    await page.getByText('只读实验功能：读取运行中的游戏内存，不写入存档。', { exact: true }).isVisible());
  check('No write, edit or generate control exists',
    await page.locator('.equipment-page button').evaluateAll((nodes) =>
      nodes.every((node) => ['加载装备', '刷新', '上一页', '下一页'].includes(node.textContent.trim()) ||
        /^[0-9]+$/.test(node.textContent.trim()))));

  await page.getByRole('button', { name: '加载装备', exact: true }).click();
  await rows(page).first().waitFor();
  const first = await snapshotCalls(page);
  check('Load sends one page request with the 0/64 defaults',
    first.length === 1 && first[0].params.start === 0 && first[0].params.limit === 64);
  check('A full page of 64 raw slots is shown', await rows(page).count() === 64);
  const tableText = await page.locator('.equipment-table').innerText();
  check('Raw fields are labelled and shown without scaling',
    tableText.includes('0xF6E8') && tableText.includes('等级（原始）') && tableText.includes('180') &&
    tableText.includes('强化（原始）') && tableText.includes('20'));
  check('Unavailable item name is stated honestly', tableText.includes('名称未收录'));
  check('Slot index is labelled', tableText.includes('槽位'));
  check('Raw slot count is not claimed as occupancy',
    await page.getByText('槽位序号为读取到的原始槽位，不代表玩家背包已占用数量或认证容量。', { exact: true }).isVisible());
  check('A zero raw id is not presented as a proven empty slot',
    (await rows(page).nth(1).innerText()).includes('0x0000'));

  const catalogInput = page.locator('.equipment-catalog-file input[type="file"]');
  await page.locator('.equipment-catalog-version').fill(catalogCommand.params.declared_version);
  await catalogInput.setInputFiles({
    name: 'items_little_endian.json',
    mimeType: 'application/json',
    buffer: catalogBytes,
  });
  await page.getByText('Birdflight Cross Spear', { exact: true }).first().waitFor();
  const importedCatalogCalls = await catalogCalls(page);
  check('Catalog import sends the reviewed raw bytes without normalization',
    importedCatalogCalls.length === 1 &&
    importedCatalogCalls[0].params.content_base64 === catalogCommand.params.content_base64 &&
    importedCatalogCalls[0].params.source_label === 'items_little_endian.json' &&
    importedCatalogCalls[0].params.role === 'save_active_items' &&
    importedCatalogCalls[0].params.declared_version === catalogCommand.params.declared_version &&
    importedCatalogCalls[0].params.locale === 'zh-CN');
  check('Catalog maps the exact little-endian ID in the reviewed response',
    (await rows(page).first().innerText()).includes('0xF6E8') &&
    (await rows(page).first().innerText()).includes('Birdflight Cross Spear') &&
    (await page.locator('.equipment-catalog-disclosure').innerText()).includes('items_little_endian.json'));
  check('Catalog disclosure exposes source context without compatibility claims',
    (await page.locator('.equipment-catalog-disclosure').innerText()).includes(`声明版本 ${catalogCommand.params.declared_version}`) &&
    (await page.locator('.equipment-catalog-disclosure').innerText()).includes('不代表游戏兼容性或合法性'));
  check('Catalog result contains no unsafe rows or conflicts',
    catalogFixture.conflicts.length === 0 &&
    catalogFixture.rows.every((row) => row.displayable ?
      row.namespace === 'save_item_u16_le_bytes' && row.id === row.display_id &&
      row.high_word === null && row.state === 'accepted' && row.quarantine_reason === null : true));

  await page.evaluate(() => { window.__catalog.mode = 'error'; });
  await catalogInput.setInputFiles({
    name: 'items_little_endian.json',
    mimeType: 'application/json',
    buffer: Buffer.from('{"bad":"source"}', 'utf8'),
  });
  await page.locator('.equipment-catalog-import .equipment-error').waitFor();
  check('Rejected catalog import preserves the previous active catalog',
    (await page.locator('.equipment-catalog-disclosure').innerText()).includes('items_little_endian.json') &&
    (await rows(page).first().innerText()).includes('Birdflight Cross Spear'));
  await page.evaluate(() => { window.__catalog.mode = 'ok'; });

  await page.getByRole('button', { name: '移除本地目录', exact: true }).click();
  await page.locator('.equipment-catalog-disclosure').waitFor({ state: 'detached' });
  await page.waitForTimeout(50);
  check('Explicit clear removes active names and persisted storage',
    await page.locator('.equipment-catalog-disclosure').count() === 0 &&
    (await rows(page).first().innerText()).includes('名称未收录') &&
    await page.evaluate(() => localStorage.getItem('nioh3-equipment-local-name-catalog-v1') === null));

  await page.evaluate(() => localStorage.setItem(
    'nioh3-equipment-local-name-catalog-v1',
    JSON.stringify({ entries: [[0xF6E8, 'Injected invalid name']] }),
  ));
  await page.reload();
  await page.getByRole('button', { name: '装备', exact: true }).click();
  await page.getByRole('button', { name: '加载装备', exact: true }).click();
  await rows(page).first().waitFor();
  check('Invalid stored catalog shape fails closed without an arbitrary label',
    await page.locator('.equipment-catalog-disclosure').count() === 0 &&
    !(await rows(page).first().innerText()).includes('Injected invalid name') &&
    (await rows(page).first().innerText()).includes('名称未收录'));

  await catalogInput.setInputFiles({
    name: 'items_little_endian.json',
    mimeType: 'application/json',
    buffer: catalogBytes,
  });
  await page.getByText('Birdflight Cross Spear', { exact: true }).first().waitFor();
  await page.reload();
  await page.getByRole('button', { name: '装备', exact: true }).click();
  await page.getByRole('heading', { name: '装备浏览', exact: true }).waitFor();
  await page.getByRole('button', { name: '加载装备', exact: true }).click();
  await rows(page).first().waitFor();
  await page.getByText('Birdflight Cross Spear', { exact: true }).first().waitFor();
  check('Restart restores the one persisted active catalog',
    await page.locator('.equipment-catalog-disclosure').count() === 1 &&
    (await page.locator('.equipment-catalog-disclosure').innerText()).includes('items_little_endian.json'));

  await page.evaluate(() => { window.__catalog.mode = 'defer'; });
  await catalogInput.setInputFiles({
    name: 'items_little_endian.json',
    mimeType: 'application/json',
    buffer: catalogBytes,
  });
  await page.getByRole('button', { name: '移除本地目录', exact: true }).click();
  await page.locator('.equipment-catalog-disclosure').waitFor({ state: 'detached' });
  await page.waitForTimeout(50);
  await page.evaluate(() => {
    const queued = window.__catalog.queue.splice(0);
    queued.forEach((entry) => entry.resolve(entry.response));
    window.__catalog.mode = 'ok';
  });
  await page.waitForTimeout(100);
  check('Late catalog response after remove cannot restore a cleared catalog',
    await page.locator('.equipment-catalog-disclosure').count() === 0 &&
    (await rows(page).first().innerText()).includes('名称未收录'));

  await page.evaluate(() => { window.__catalog.mode = 'defer'; });
  await catalogInput.setInputFiles({
    name: 'items_little_endian.json',
    mimeType: 'application/json',
    buffer: catalogBytes,
  });
  await page.getByRole('button', { name: '绘卷搜索', exact: true }).click();
  await page.getByRole('heading', { name: '绘卷搜索', exact: true }).waitFor();
  await page.evaluate(() => {
    const queued = window.__catalog.queue.splice(0);
    queued.forEach((entry) => entry.resolve(entry.response));
    window.__catalog.mode = 'ok';
  });
  await page.waitForTimeout(100);
  await page.getByRole('button', { name: '装备', exact: true }).click();
  check('Unmounted catalog response paints nothing',
    await page.locator('.equipment-catalog-disclosure').count() === 0);
  await page.getByRole('button', { name: '加载装备', exact: true }).click();
  await rows(page).first().waitFor();

  const searchInput = page.locator('.equipment-search input');
  await searchInput.fill('f6e8');
  check('Page-local search filters the current page only',
    await searchInput.inputValue() === 'f6e8' &&
    await rows(page).count() === 1 &&
    (await rows(page).first().innerText()).includes('0xF6E8'));
  await searchInput.fill('');
  check('Clearing the page-local search restores the page',
    await searchInput.inputValue() === '' && await rows(page).count() === 64);

  await rows(page).first().locator('button').click();
  const detail = await page.locator('.equipment-raw').innerText();
  check('Selected row exposes raw basic fields',
    detail.includes('0xF6E8') && detail.includes('180') && detail.includes('20'));
  const effects = page.locator('.equipment-effects tbody tr');
  check('Seven effect slots are listed', await effects.count() === 7);
  check('Sentinel effect slot renders a dash instead of a bogus id',
    (await effects.nth(1).innerText()).includes('—') &&
    !(await effects.nth(1).innerText()).includes('0xFFFF'));
  check('Unknown effect keeps its raw hex id',
    (await effects.nth(3).innerText()).includes('0x7B14'));

  // Failure mode recorded before the implementation: a pending page request
  // must not leave the old table/detail looking current or selectable.
  await page.evaluate(() => { window.__eq.mode = 'defer'; });
  await page.getByRole('button', { name: '下一页', exact: true }).click();
  check('Loading a new page clears the old table before the response',
    await page.locator('.equipment-table').count() === 0 &&
    await page.locator('.equipment-meta').count() === 0);
  check('Loading a new page clears the old selection before the response',
    await page.getByText('选择一行查看原始字段。', { exact: true }).isVisible());
  await page.evaluate(() => {
    const queued = window.__eq.queue.splice(0);
    queued.forEach((entry) => entry.resolve(entry.response));
    window.__eq.mode = 'ok';
  });
  await rows(page).first().waitFor();
  const paged = await snapshotCalls(page);
  check('Next page requests the following 64 slots',
    paged.at(-1)?.params.start === 64 && paged.at(-1)?.params.limit === 64 &&
    paged.length > 1);
  check('Next page clears the previous selection',
    await page.getByText('选择一行查看原始字段。', { exact: true }).isVisible());
  check('Last page disables the next control',
    await page.getByRole('button', { name: '下一页', exact: true }).isDisabled());
  await page.getByRole('button', { name: '上一页', exact: true }).click();
  await rows(page).first().waitFor();
  check('Previous page returns to the first 64 slots',
    (await snapshotCalls(page)).at(-1)?.params.start === 0);

  // A page response from a restarted process must not be silently merged into
  // the old session. The verifier then proves that an explicit refresh starts
  // a new session successfully instead of making refresh itself fail.
  await page.locator('.equipment-load').click();
  await rows(page).first().waitFor();
  await page.evaluate(() => { window.__eq.mode = 'defer'; });
  await page.getByRole('button', { name: '下一页', exact: true }).click();
  await page.evaluate(() => {
    window.__eq.process = { pid: 4343, creation_filetime: '133000000000000001' };
    const queued = window.__eq.queue.splice(0);
    queued.forEach((entry) => entry.resolve({
      ...entry.response,
      process: { ...window.__eq.process },
    }));
    window.__eq.mode = 'ok';
  });
  await page.locator('.equipment-error').waitFor();
  check('A changed process identity rejects the mixed page',
    (await page.locator('.equipment-error').innerText()).includes('请点击加载装备按钮重新加载装备') &&
    await page.locator('.equipment-table').count() === 0 &&
    await page.locator('.equipment-detail .equipment-raw').count() === 0);
  await page.getByRole('button', { name: '加载装备', exact: true }).click();
  await rows(page).first().waitFor();
  check('Explicit refresh establishes the new process session',
    (await snapshotCalls(page)).at(-1).params.start === 0 &&
    await page.locator('.equipment-error').count() === 0);

  // The container count is part of the page address space. A count change is
  // a second independent mixed-session failure, not a reason to claim a page.
  await page.evaluate(() => { window.__eq.mode = 'defer'; });
  await page.getByRole('button', { name: '下一页', exact: true }).click();
  await page.evaluate(() => {
    window.__eq.total = 192;
    const queued = window.__eq.queue.splice(0);
    queued.forEach((entry) => entry.resolve({
      ...entry.response,
      observed_slot_count: window.__eq.total,
      next_start: 128,
    }));
    window.__eq.mode = 'ok';
  });
  await page.locator('.equipment-error').waitFor();
  check('A changed container count rejects the mixed page',
    (await page.locator('.equipment-error').innerText()).includes('请点击加载装备按钮重新加载装备') &&
    await page.locator('.equipment-table').count() === 0);
  await page.getByRole('button', { name: '加载装备', exact: true }).click();
  await rows(page).first().waitFor();
  check('Refresh also accepts the changed container as a new session',
    (await snapshotCalls(page)).at(-1).params.start === 0 &&
    await page.locator('.equipment-error').count() === 0);

  await rows(page).first().locator('button').click();
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await rows(page).first().waitFor();
  check('Refresh resets the selection',
    await page.getByText('选择一行查看原始字段。', { exact: true }).isVisible());
  check('Refresh re-reads from the first page',
    (await snapshotCalls(page)).at(-1).params.start === 0);

  await page.evaluate(() => { window.__eq.mode = 'error'; });
  await page.getByRole('button', { name: '刷新', exact: true }).click();
  await page.locator('.equipment-error').waitFor();
  check('Error clears the table instead of leaving a stale snapshot',
    await page.locator('.equipment-table').count() === 0 &&
    await page.locator('.equipment-meta').count() === 0);
  check('Error uses the existing localized message',
    (await page.locator('.equipment-error').innerText()).includes('请先启动游戏并进入角色存档。'));

  await page.evaluate(() => { window.__eq.mode = 'defer'; });
  await page.locator('.equipment-load').click();
  await page.getByRole('button', { name: '绘卷搜索', exact: true }).click();
  await page.getByRole('heading', { name: '绘卷搜索', exact: true }).waitFor();
  await page.evaluate(() => {
    const queued = window.__eq.queue.splice(0);
    queued.forEach((entry) => entry.resolve(window.__eq.snapshot(0, 64)));
  });
  await page.waitForTimeout(200);
  check('A late response after the page change paints nothing',
    await page.locator('.equipment-table').count() === 0);
  check('Existing scroll page still renders', await page.locator('.search-page').isVisible());

  const sidebar = await verifySidebarAlignment(page, { outputDirectory: join(output, 'sidebar') });
  check('Sidebar keeps every control measurable after the new entry',
    Object.keys(sidebar.expanded).length === 9);
  const sections = await verifySectionHelp(page, { outputDirectory: join(output, 'section-help') });
  check('Existing filter sections keep their help popovers', sections.length === 5);

  await page.getByRole('button', { name: '装备', exact: true }).click();
  check('Returning shows the unloaded state without a stale snapshot',
    await page.getByText('尚未加载。请点击加载装备按钮读取当前背包。', { exact: true }).isVisible() &&
    await page.locator('.equipment-table').count() === 0);
  await page.evaluate(() => { window.__eq.mode = 'ok'; });
  await page.getByRole('button', { name: '加载装备', exact: true }).click();
  await rows(page).first().waitFor();
  await rows(page).first().locator('button').click();
  await catalogInput.setInputFiles({
    name: 'items_little_endian.json',
    mimeType: 'application/json',
    buffer: catalogBytes,
  });
  await page.getByText('Birdflight Cross Spear', { exact: true }).first().waitFor();
  await page.screenshot({ path: join(output, 'equipment-detail.png'), fullPage: true });

  for (const [label, heading, searchLabel, locale] of [
    ['English', 'Equipment browser', 'Search this page (name or hex ID)', 'en-US'],
    ['日本語', '装備ブラウザ', 'このページ内を検索（名前または16進ID）', 'ja-JP'],
  ]) {
    await selectLocale(page, label);
    await page.getByRole('heading', { name: heading, exact: true }).waitFor();
    check(`${locale} page title is translated`,
      await page.getByRole('heading', { name: heading, exact: true }).isVisible());
    check(`${locale} page-local search label is translated`,
      await page.getByText(searchLabel, { exact: true }).first().isVisible());
    check(`${locale} local catalog controls are translated`,
      await page.getByText(locale === 'en-US' ? 'Local name catalog (optional)' : 'ローカル名カタログ（任意）', { exact: true }).isVisible());
    check(`${locale} navigation labels stay inside the sidebar`, await page.evaluate(() => {
      const nav = document.querySelector('.shell > .nav');
      const limit = nav.getBoundingClientRect().right;
      return [...document.querySelectorAll('.nav nav > button')].every((button) =>
        [...button.querySelectorAll('span')].every((span) =>
          span.getBoundingClientRect().right <= limit + 0.5));
    }));
    await page.screenshot({ path: join(output, `equipment-${locale}.png`), fullPage: true });
  }
  await page.setViewportSize({ width: 1280, height: 800 });
  check('No horizontal overflow at a constrained viewport',
    await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth));
  await selectLocale(page, '简体中文');
  await page.getByRole('heading', { name: '装备浏览', exact: true }).waitFor();
  await page.screenshot({ path: join(output, 'equipment-zh-CN.png'), fullPage: true });
  await rows(page).first().locator('button').click();
  await page.locator('.equipment-detail').screenshot({ path: join(output, 'equipment-raw-fields.png') });
  check('Raw-field detail is captured with its effect slots',
    await page.locator('.equipment-effects tbody tr').count() === 7);

  check('No runtime error was raised during the run', errors.length === 0);
  await writeFile(
    join(output, 'verification.json'),
    JSON.stringify({ checks, errors, calls: await snapshotCalls(page) }, null, 2),
    'utf8',
  );
  console.log(`${checks.length} equipment browser checks passed`);
} finally {
  await browser.close();
  await new Promise((closed) => server.close(closed));
}
