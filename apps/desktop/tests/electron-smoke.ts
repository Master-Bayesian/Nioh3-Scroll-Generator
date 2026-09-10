/** Run the built desktop, including its sandboxed preload and real Python child. */
import { _electron as electron } from 'playwright';
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import {version} from '../../../package.json';

const output = resolve('deliverables/frontend-v2');
await mkdir(output, { recursive: true });
const desktop = await electron.launch({
  ...(process.env.NIOH3_PORTABLE_EXE ? { executablePath: process.env.NIOH3_PORTABLE_EXE, args: [] } : { args: [resolve('apps/desktop/dist/main.cjs')] }),
  env: { ...process.env, NIOH3_ELECTRON_TEST: '1', NIOH3_LEGACY_WORKBENCH: '1', NIOH3_REVIEW_UI: '' } as Record<string, string>,
});
try {
  assert.match(await desktop.evaluate(({ app }) => app.getPath('userData')), /nioh3-v2-electron-/);
  if (process.env.NIOH3_PORTABLE_EXE) {
    assert.equal(await desktop.evaluate(({ app }) => app.isPackaged), true);
    // A real package includes default_app.asar; successful startup verifies its
    // physical bytes through original-fs before any worker is constructed.
  }
  const page = await desktop.firstWindow();
  await desktop.evaluate(({BrowserWindow})=>BrowserWindow.getAllWindows()[0].webContents.setBackgroundThrottling(false));
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(String(error)));
  await page.locator('#language-select').selectOption('en-US');
  await page.getByText('Ready — offline search', { exact: true }).waitFor();
  await writeFile(resolve(output, 'electron-initial-snapshot.txt'), await page.locator('body').ariaSnapshot());
  const isolation = await page.evaluate(() => ({ require: typeof (window as unknown as { require?: unknown }).require,
    process: typeof (window as unknown as { process?: unknown }).process, methods: Object.keys(window.nioh).sort() }));
  assert.equal(isolation.require, 'undefined'); assert.equal(isolation.process, 'undefined');
  assert.deepEqual(isolation.methods, ['cancelSearch', 'currentSearch', 'handshake', 'resolveRecommendedLevel', 'restartWorker', 'searchCatalog', 'snapshot', 'startSearch']);
  const level = await page.evaluate(() => window.nioh.resolveRecommendedLevel(350));
  assert.equal(level.status, 'exact');
  assert.deepEqual(level.canonical_internal_levels, [585, 586]);
  assert.equal(level.selected_internal_level, 585);
  assert.equal((await page.evaluate(() => window.nioh.resolveRecommendedLevel(141))).status, 'out_of_range');
  const runtime = await page.evaluate(() => window.operations.execute({ method: 'runtime.status', params: {} })) as { safe_to_shutdown: boolean };
  assert.equal(runtime.safe_to_shutdown, true);
  assert.equal(await page.evaluate(() => typeof window.operations.prepareLiveAdd), 'function');
  assert.equal(await page.evaluate(async () => {
    try { await window.operations.execute({ method: 'runtime.live_add_prepare', params: {} } as any); return false; }
    catch { return true; }
  }), true);
  const diagnostic = await page.evaluate(() => window.support.diagnostics());
  assert.equal(diagnostic.schema, 'nioh3-v2-diagnostics/v1');
  assert.equal(diagnostic.version, version);
  assert.equal(diagnostic.workers.find(worker => worker.role === 'runtime')?.connection, 'ready');
  assert.equal(/SAVEDATA|record_hex|account_id|[A-Z]:\\\\/.test(JSON.stringify(diagnostic)), false);
  if (process.env.NIOH3_PORTABLE_EXE) assert.ok(diagnostic.packageVerification?.fileCount);
  const denied = await page.evaluate(async () => {
    try { await window.operations.execute({ method: 'save.template', params: {} } as any); return false; }
    catch { return true; }
  });
  assert.equal(denied, true);
  const initialDraft = await page.locator('#query').inputValue();
  await page.locator('#language-select').selectOption('ja-JP');
  await page.waitForFunction(() => document.documentElement.lang === 'ja-JP');
  await page.reload();
  await page.waitForFunction(() => document.documentElement.lang === 'ja-JP');
  assert.equal(await page.locator('#language-select').inputValue(), 'ja-JP');
  for (const locale of ['zh-CN', 'ja-JP', 'en-US']) {
    await page.locator('#language-select').selectOption(locale);
    await page.waitForFunction(value => document.documentElement.lang === value, locale);
    assert.equal(await page.locator('#query').inputValue(), initialDraft);
    assert.equal(await page.evaluate(() => window.preferences.getLocale()), locale);
    const catalog = await page.evaluate(value => window.nioh.searchCatalog(3, value as 'zh-CN' | 'ja-JP' | 'en-US'), locale);
    const expected = { 'zh-CN': '刚力符', 'ja-JP': '剛力符', 'en-US': 'Power Talisman' }[locale]!;
    assert.ok(catalog.special_rule_options);
    const names = catalog.special_rule_options.filter(row => row.variant.qualifier_kind === 'item');
    assert.ok(names.some(row => row.variant.qualifier_key === 0x729d && row.name.includes(expected)));
    assert.ok(names.every(row => !row.name.includes('^') && !row.name.includes('~RUBY')));
  }
  const hello = await page.evaluate(() => window.nioh.handshake());
  if (!hello.capabilities.cuda_pivot_and_auxiliary && !hello.capabilities.directcompute_effect_filter) {
    await page.getByRole('checkbox').check();
  }
  await page.getByRole('button', { name: 'Start search', exact: true }).click();
  await page.waitForFunction(() => document.querySelector('[data-testid="job-state"]')?.getAttribute('data-state') === 'completed');
  assert.equal(await page.locator('tbody tr').count(), 2);
  const submitted = await page.locator('[data-testid="job-state"]').innerText();
  // A renderer reload must retain the existing job/candidates and submitted query.
  const previousJob = await page.evaluate(async () => (await window.nioh.currentSearch()).job!.job_id);
  await page.reload();
  await page.getByText('Ready — offline search', { exact: true }).waitFor();
  assert.equal(await page.locator('tbody tr').count(), 2);
  assert.equal((await page.evaluate(() => window.nioh.currentSearch())).job!.job_id, previousJob);
  await page.locator('#language-select').selectOption('ja-JP');
  await page.waitForFunction(() => document.documentElement.lang === 'ja-JP');
  assert.equal(await page.locator('tbody tr').count(), 2);
  await page.locator('#language-select').selectOption('en-US');
  await page.waitForFunction(() => document.documentElement.lang === 'en-US');
  assert.equal(await page.locator('[data-testid="job-state"]').innerText(), submitted);
  await page.getByLabel('Search query JSON').fill('{"draft":"intentionally invalid"}');
  await page.getByRole('button', { name: 'Resume submitted query', exact: true }).click();
  await page.waitForFunction(() => document.querySelector('[data-testid="job-state"]')?.getAttribute('data-state') === 'completed');
  assert.equal(await page.locator('tbody tr').count(), 2);
  // A never-shown packaged window may have no compositor frame on Windows.
  if(process.env.NIOH3_CAPTURE_UI==='1'){
    await desktop.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0].showInactive());
    await page.screenshot({ path: resolve(output, 'electron-search-smoke.png'), fullPage: true, timeout:15000 });
  }
  await writeFile(resolve(output, 'electron-result-snapshot.txt'), await page.locator('body').ariaSnapshot());
  assert.deepEqual(errors, []);
  await writeFile(resolve(output, 'electron-smoke.json'), JSON.stringify({ passed: true, isolation, rendererErrors: errors, evidence: 'Electron renderer → preload → broker → framed Python worker → real offline search; no game or save writes' }, null, 2));
  console.log('ELECTRON_SEARCH_SMOKE_OK');
} finally { await desktop.close(); }
