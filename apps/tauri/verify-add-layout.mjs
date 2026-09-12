/** Real WebView2 addition acceptance; only an isolated synthetic save is writable. */
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { spawn, execFileSync } from 'node:child_process';
import { mkdtemp, mkdir, readFile, readdir, realpath, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { closeSession, isolatedEnvironment, pause } from './onefile-acceptance.mjs';
import { verifySidebarAlignment } from '../workshop/verify-sidebar-alignment.mjs';
import { verifySectionHelp } from '../workshop/verify-section-help.mjs';

const root = await realpath(await mkdtemp(join(tmpdir(), 'nioh3-add-layout-')));
const output = resolve(process.env.NIOH3_UI_OUTPUT || 'deliverables/frontend-v2/add-layout-acceptance');
await mkdir(output, { recursive: true });
const python = process.env.NIOH3_PYTHON || 'python';
const fixture = JSON.parse(execFileSync(python, [resolve('apps/desktop/tests/fixtures/create-save.py'),
  join(root, 'local', 'KoeiTecmo', 'NIOH3', 'Savedata')], { windowsHide: true, encoding: 'utf8', timeout: 45000 }));
const original = await readFile(fixture.path);
const { env, profile, port } = await isolatedEnvironment(root);
env.NIOH3_PYTHON = python;
const executable = resolve(process.env.NIOH3_TAURI_EXE || 'apps/tauri/src-tauri/target/debug/nioh3-studio.exe');
const child = spawn(executable, ['--user-data-dir', profile], {
  windowsHide: true, env, stdio: ['ignore', 'pipe', 'pipe'],
});
let stderr = '';
child.stderr.on('data', bytes => { stderr = (stderr + bytes).slice(-32000); });
const checks = [], layouts = [], errors = [];
let session;
const check = (name, condition) => { assert.ok(condition, name); checks.push(name); console.log(name); };
const backupRoot = join(profile, 'backups');
async function backupIds() {
  try { return (await readdir(backupRoot)).sort(); } catch (error) { if (error.code === 'ENOENT') return []; throw error; }
}
async function verifyNewBackup(previousIds, expectedBytes) {
  const fresh = (await backupIds()).filter(id => !previousIds.includes(id));
  assert.equal(fresh.length, 1, 'Each successful append creates exactly one automatic backup');
  const folder = join(backupRoot, fresh[0]);
  const manifest = JSON.parse(await readFile(join(folder, 'backup-manifest.json'), 'utf8'));
  const main = manifest.backup_files.find(file => file.source_role === 'main_save');
  assert(main, 'Automatic backup includes the main encrypted save');
  assert.deepEqual(await readFile(join(folder, main.backup_file)), expectedBytes,
    'Automatic backup preserves the complete pre-addition save');
  return fresh[0];
}
async function waitInventory(page, expectedSeeds) {
  await page.waitForFunction(seeds => {
    const actual = [...document.querySelectorAll('.inventory-list button span')]
      .map(element => Number(element.textContent.match(/\d+$/)?.[0])).sort((a, b) => a - b);
    return JSON.stringify(actual) === JSON.stringify([...seeds].sort((a, b) => a - b));
  }, expectedSeeds, { timeout: 30000 });
}
async function changeReactInput(locator, value) {
  await locator.evaluate((element, next) => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(element, next);
    element.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

try {
  for (let attempt = 0; attempt < 200; attempt++) {
    if (child.exitCode !== null) throw Error(`App exited ${child.exitCode}: ${stderr}`);
    try { if ((await fetch(`http://127.0.0.1:${port}/json/version`)).ok) break; } catch {}
    await pause(300);
  }
  const browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  let target;
  for (let attempt = 0; attempt < 150; attempt++) {
    target = browser.contexts()[0]?.pages()[0];
    if (target) break;
    await pause(200);
  }
  assert(target, 'WebView2 must expose the application page');
  session = { browser, page: target };
  const page = session.page;
  await page.getByText('后端已连接，请选择筛选条件。', { exact: true }).waitFor({ timeout: 45000 });
  const diagnostics = await page.evaluate(() => window.support.diagnostics());
  if (process.env.NIOH3_TAURI_EXE) assert.equal(diagnostics.packageVerification?.ok, true);
  page.on('pageerror', error => errors.push(error.message));
  page.setDefaultTimeout(15000);
  await page.evaluate(() => {
    window.__additionAcceptance = { requests: [], operations: [] };
    const prepare = window.review.prepareCart.bind(window.review);
    window.review.prepareCart = params => {
      window.__additionAcceptance.requests.push(structuredClone(params));
      return prepare(params);
    };
    const execute = window.operations.execute.bind(window.operations);
    window.operations.execute = command => {
      window.__additionAcceptance.operations.push(command.method);
      return execute(command);
    };
  });
  await page.getByRole('radio', { name: '回标题界面后添加到存档', exact: true }).check();
  const picker = page.locator('.install-mode .save-picker');
  await page.waitForFunction(() => {
    const select = document.querySelector('.install-mode .save-picker select');
    return select && select.value && !select.disabled;
  });
  await waitInventory(page, [36526331]);
  const knownSeed = page.getByRole('textbox', { name: '已知绘卷ID', exact: true });
  await knownSeed.fill('76634363');
  await page.getByRole('button', { name: '查看', exact: true }).click();
  await page.locator('.result-detail .scroll').waitFor();
  check('Direct addition is available with an empty cart',
    await page.getByRole('button', { name: '添加当前绘卷', exact: true }).isEnabled() &&
    await page.getByRole('button', { name: '查看购物车（0）', exact: true }).isDisabled());
  check('The outside cart-only addition action is removed',
    await page.getByRole('button', { name: '选择购物车中的绘卷添加', exact: true }).count() === 0);

  const cdp = await page.context().newCDPSession(page);
  const layoutFailures = [];
  for (const [label, requestedWidth, requestedHeight] of [['default', null, null], ['1600x1000', 1600, 1000], ['1280x800', 1280, 800]]) {
    if (requestedWidth) {
      await cdp.send('Emulation.setDeviceMetricsOverride', { width: requestedWidth, height: requestedHeight, deviceScaleFactor: 1, mobile: false });
      await page.waitForFunction(([w, h]) => innerWidth === w && innerHeight === h, [requestedWidth, requestedHeight]);
    }
    const sidebar = await verifySidebarAlignment(page, { outputDirectory: join(output, label) }).catch(error => { layoutFailures.push(error.message); return null; });
    const geometry = await page.evaluate(() => {
      const rect = element => { const r = element.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height, right: r.right, bottom: r.bottom }; };
      const pane = document.querySelector('.result-pane');
      const add = document.querySelector('.result-cart-actions .cart-toggle');
      const view = document.querySelector('.result-cart-actions .compare-button');
      return { viewport: { width: innerWidth, height: innerHeight, devicePixelRatio, scale: visualViewport?.scale }, pane: rect(pane),
        scrollHeight: pane.scrollHeight, clientHeight: pane.clientHeight,
        add: rect(add), view: rect(view),
        controls: [...document.querySelectorAll('.install-mode input, .install-mode select, .install-mode button')]
          .map(element => ({ label: element.getAttribute('aria-label') || element.textContent || element.type, ...rect(element) })) };
    });
    const { width, height } = geometry.viewport;
    try {
    assert(Math.abs((geometry.add.y + geometry.add.height / 2) - (geometry.view.y + geometry.view.height / 2)) <= 1,
      `Cart add and view controls share one row at ${width}x${height}`);
    assert(geometry.scrollHeight <= geometry.clientHeight + 1,
      `Result pane needs no scrolling at ${width}x${height}: ${JSON.stringify(geometry)}`);
    for (const control of geometry.controls) {
      assert(control.width > 0 && control.height > 0 && control.x >= -1 && control.y >= -1 &&
        control.right <= width + 1 && control.bottom <= height + 1,
      `Addition control fits viewport at ${width}x${height}: ${JSON.stringify(control)}`);
    }
    const tools = await picker.locator('button').evaluateAll(elements => elements.map(element => element.getBoundingClientRect().y));
    assert(Math.max(...tools) - Math.min(...tools) <= 1, `Save utility buttons share one row at ${width}x${height}`);
    check(`Cart row, compact save controls, sidebar, and no result scrolling at ${label}`, true);
    } catch (error) { layoutFailures.push({ label, error: String(error) }); }
    await page.screenshot({ path: join(output, `addition-${label}.png`) });
    layouts.push({ label, width, height, geometry, sidebar });
    await writeFile(join(output, 'layout-measurements.json'), JSON.stringify({ layouts, failures: layoutFailures }, null, 2));
    console.log(`Addition geometry ${label}: ${geometry.viewport.width}x${geometry.viewport.height}, scroll ${geometry.scrollHeight}/${geometry.clientHeight}`);
  }
  assert.deepEqual(layoutFailures, [], 'All default and compact addition layouts fit');
  const sectionHelp = await verifySectionHelp(page, { outputDirectory: join(output, 'section-help') });

  const beforeDirect = await readFile(fixture.path), beforeDirectBackups = await backupIds();
  await page.getByRole('button', { name: '添加当前绘卷', exact: true }).click();
  const direct = page.locator('.current-add-review');
  await direct.getByRole('button', { name: '确认添加所选 1 张', exact: true }).waitFor();
  assert.equal(await direct.locator('.current-add-summary strong').innerText(), '76634363');
  assert.deepEqual(await readFile(fixture.path), beforeDirect);
  assert.deepEqual(await backupIds(), beforeDirectBackups);
  check('Direct addition automatically prepares one retained candidate without writing', true);
  const firstRequest = await page.evaluate(() => window.__additionAcceptance.requests[0]);
  assert.equal(firstRequest.mode, 'save');
  assert.equal(firstRequest.references.length, 1);
  assert.equal(firstRequest.transfer_count, 4294967295);

  // Simulate a background selection/form change while the modal owns its frozen sample.
  await changeReactInput(page.getByRole('spinbutton', { name: '转手次数', exact: true }), '2');
  await changeReactInput(knownSeed, '10030565');
  await page.getByRole('button', { name: '查看', exact: true }).evaluate(button => button.click());
  await page.waitForFunction(() => document.querySelector('.result-detail .scroll footer strong')?.textContent === '10030565');
  assert.equal(await direct.locator('.current-add-summary strong').innerText(), '76634363');
  assert.match(await direct.innerText(), /转手次数\s*-1/);
  const confirmDirect = direct.getByRole('button', { name: '确认添加所选 1 张', exact: true });
  assert.equal(await confirmDirect.isDisabled(), true);
  const title = direct.getByRole('checkbox', { name: '游戏已回到标题界面', exact: true });
  await title.check(); await title.uncheck();
  assert.equal(await confirmDirect.isDisabled(), true);
  await title.check();
  check('Frozen seed and transfer settings remain unchanged; title confirmation gates commit', await confirmDirect.isEnabled());
  await confirmDirect.click();
  await direct.getByText('已添加到存档。', { exact: true }).waitFor({ timeout: 30000 });
  await waitInventory(page, [36526331, 76634363]);
  const directBackup = await verifyNewBackup(beforeDirectBackups, beforeDirect);
  await page.getByRole('button', { name: '关闭窗口', exact: true }).click();
  check('Direct commit adds only its frozen seed and leaves the cart empty',
    await page.getByRole('button', { name: '查看购物车（0）', exact: true }).isDisabled());

  await page.getByRole('spinbutton', { name: '转手次数', exact: true }).fill('-1');
  await page.getByRole('button', { name: '清空全部', exact: true }).click();
  if (process.env.NIOH3_PARITY_ALLOW_CPU === '1') {
    await page.getByRole('button', { name: '设置', exact: true }).click();
    await page.getByRole('checkbox', { name: '允许使用 CPU 搜索', exact: true }).check();
    await page.getByRole('button', { name: '关闭侧边菜单', exact: true }).click();
  }
  await page.locator('.primary-button').click();
  await page.waitForFunction(() => document.querySelectorAll('.number-rail button').length === 25 &&
    !document.querySelector('.primary-button').disabled, null, { timeout: 60000 });
  const firstCartSeed = Number(await page.locator('.result-detail .scroll footer strong').innerText());
  await page.getByRole('button', { name: '加入购物车', exact: true }).click();
  await page.getByRole('button', { name: '查看购物车（1）', exact: true }).waitFor();
  await page.getByRole('slider', { name: '滑动切换绘卷', exact: true }).fill('2');
  const chosenCartSeed = Number(await page.locator('.result-detail .scroll footer strong').innerText());
  assert.equal(new Set([firstCartSeed, chosenCartSeed, 76634363, 36526331]).size, 4, 'Fixture and selected search seeds must differ');
  await page.getByRole('button', { name: '加入购物车', exact: true }).click();
  await page.getByRole('button', { name: '查看购物车（2）', exact: true }).waitFor();
  const priorJob = await page.evaluate(async () => (await window.nioh.currentSearch()).job.job_id);
  await page.getByRole('button', { name: '下一批 →', exact: true }).click();
  await page.waitForFunction(async id => {
    const state = await window.nioh.currentSearch();
    return state.job?.job_id !== id && state.job?.state === 'completed' && state.job?.candidates.length === 25;
  }, priorJob, { timeout: 60000 });
  await page.getByRole('button', { name: '查看购物车（2）', exact: true }).click();
  const cart = page.locator('.cart-review');
  await cart.getByRole('checkbox', { name: `勾选绘卷${firstCartSeed}`, exact: true }).uncheck();
  const beforeCart = await readFile(fixture.path), beforeCartBackups = await backupIds();
  await cart.getByRole('button', { name: '核对添加', exact: true }).click();
  const confirmCart = cart.getByRole('button', { name: '确认添加所选 1 张', exact: true });
  await confirmCart.waitFor();
  assert.deepEqual(await readFile(fixture.path), beforeCart);
  assert.deepEqual(await backupIds(), beforeCartBackups);
  assert.equal(await confirmCart.isDisabled(), true);
  await cart.getByRole('checkbox', { name: '游戏已回到标题界面', exact: true }).check();
  await confirmCart.click();
  await cart.getByText('已添加到存档。', { exact: true }).waitFor({ timeout: 30000 });
  await waitInventory(page, [36526331, 76634363, chosenCartSeed]);
  const cartBackup = await verifyNewBackup(beforeCartBackups, beforeCart);
  check('Cart subset survives a new search page and adds exactly one selected seed with backup', true);
  await page.getByRole('button', { name: '关闭窗口', exact: true }).click();
  await page.getByRole('button', { name: '绘卷编辑', exact: true }).click();
  await page.screenshot({ path: join(output, 'three-record-inventory.png') });
  await page.getByRole('button', { name: '绘卷搜索', exact: true }).click();
  await cdp.send('Emulation.setDeviceMetricsOverride', { width: 1455, height: 909, deviceScaleFactor: 1, mobile: false });
  for (const language of ['English', '日本語', '简体中文']) {
    await page.locator('.language-button').click();
    await page.getByRole('button', { name: language, exact: true }).click();
    const localizedLayout = await page.locator('.result-pane').evaluate(pane => ({
      height: pane.clientHeight, content: pane.scrollHeight,
      overflowing: [...pane.querySelectorAll('.install-mode button, .install-choice')]
        .filter(element => element.scrollWidth > element.clientWidth + 1)
        .map(element => element.textContent),
    }));
    assert(localizedLayout.content <= localizedLayout.height + 1, `${language} addition area must fit at default size`);
    assert.deepEqual(localizedLayout.overflowing, [], `${language} addition labels must fit their controls`);
    await page.screenshot({ path: join(output, `addition-${language === 'English' ? 'en' : language === '日本語' ? 'ja' : 'zh'}.png`) });
  }
  check('Chinese, English and Japanese addition controls fit the default viewport', true);
  const calls = await page.evaluate(() => window.__additionAcceptance);
  assert.equal(calls.requests.length, 2, 'Each addition prepares once');
  assert(calls.requests.every(request => request.mode === 'save' && request.references.length === 1));
  assert(!calls.operations.some(method => method.startsWith('runtime.')), 'Acceptance must not issue game-memory operations');
  assert.deepEqual(errors, [], 'No renderer errors');
  await writeFile(join(output, 'verification.json'), JSON.stringify({ executable, checks, layouts, sectionHelp,
    directSeed: 76634363, cartSeed: chosenCartSeed, excludedCartSeed: firstCartSeed, finalInventoryCount: 3,
    automaticBackups: [directBackup, cartBackup], gameWrites: 0, errors,
    scope: 'Real frontend and backend against one isolated encrypted synthetic save; no clipboard commands or game-memory operations.' }, null, 2));
} catch (error) {
  await writeFile(join(output, 'failure.txt'), `${error.stack || error}\n${stderr}\n${session ? await session.page.locator('body').innerText().catch(() => '') : ''}`);
  if (session) await session.page.screenshot({ path: join(output, 'failure.png') }).catch(() => {});
  throw error;
} finally {
  await closeSession(session, child);
}
