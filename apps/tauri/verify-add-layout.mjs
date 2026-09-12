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
import { addKnownEnemyVariantsToCart, verifyCollectionLayout } from '../workshop/verify-collection-layout.mjs';

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
let nativeWindow, viewportMatrix, settingsSwitches, previewLineSpacing, collections, modalDismissal;
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
async function viewportSnapshot(page) {
  return page.evaluate(() => ({
    width: innerWidth,
    height: innerHeight,
    devicePixelRatio,
    outerWidth,
    outerHeight,
    screen: { width: screen.width, height: screen.height, availWidth: screen.availWidth, availHeight: screen.availHeight },
  }));
}
async function waitForViewportChange(page, previous) {
  await page.waitForFunction(value => innerWidth !== value.width || innerHeight !== value.height, previous, { timeout: 10000 });
  let last = await viewportSnapshot(page), stable = 0;
  for (let attempt = 0; attempt < 30 && stable < 3; attempt++) {
    await pause(100);
    const next = await viewportSnapshot(page);
    stable = next.width === last.width && next.height === last.height ? stable + 1 : 0;
    last = next;
  }
  return last;
}
async function verifyViewportReachability(page, label) {
  await page.evaluate(() => scrollTo(0, 0));
  const geometry = await page.evaluate(currentLabel => {
    const rectangle = element => {
      const value = element.getBoundingClientRect();
      return { left: value.left + scrollX, right: value.right + scrollX,
        top: value.top + scrollY, bottom: value.bottom + scrollY, width: value.width, height: value.height };
    };
    const root = document.documentElement, body = document.body, shell = document.querySelector('.shell');
    const documentWidth = Math.max(root.scrollWidth, body.scrollWidth);
    const documentHeight = Math.max(root.scrollHeight, body.scrollHeight);
    const scrollableAncestor = (element, axis) => {
      for (let ancestor = element.parentElement; ancestor; ancestor = ancestor.parentElement) {
        const style = getComputedStyle(ancestor);
        const overflow = axis === 'x' ? style.overflowX : style.overflowY;
        const scrollSize = axis === 'x' ? ancestor.scrollWidth : ancestor.scrollHeight;
        const clientSize = axis === 'x' ? ancestor.clientWidth : ancestor.clientHeight;
        if (['auto', 'scroll'].includes(overflow) && scrollSize > clientSize + 1) return true;
      }
      return false;
    };
    const containers = [...document.querySelectorAll('main, .result-pane, .editor-host, .backup-page')]
      .filter(element => getComputedStyle(element).display !== 'none')
      .map(element => {
        const maximumScrollTop = Math.max(0, element.scrollHeight - element.clientHeight);
        const previousScrollTop = element.scrollTop;
        element.scrollTop = maximumScrollTop;
        const reachedVerticalEnd = Math.abs(element.scrollTop - maximumScrollTop) <= 1;
        element.scrollTop = previousScrollTop;
        return { className: element.className, scrollWidth: element.scrollWidth, clientWidth: element.clientWidth,
          scrollHeight: element.scrollHeight, clientHeight: element.clientHeight,
          overflowX: getComputedStyle(element).overflowX, overflowY: getComputedStyle(element).overflowY,
          reachedVerticalEnd };
      });
    const controls = [...document.querySelectorAll('button, input, select, summary')]
      .filter(element => {
        const style = getComputedStyle(element), box = element.getBoundingClientRect();
        return style.display !== 'none' && style.visibility !== 'hidden' && box.width > 0 && box.height > 0;
      }).map(element => ({ label: element.getAttribute('aria-label') || element.textContent?.trim() || element.getAttribute('type'),
        rectangle: rectangle(element), reachableX: scrollableAncestor(element, 'x'), reachableY: scrollableAncestor(element, 'y') }));
    const unreachableControls = controls.filter(value =>
      ((value.rectangle.left < -1 || value.rectangle.right > documentWidth + 1) && !value.reachableX) ||
      ((value.rectangle.top < -1 || value.rectangle.bottom > documentHeight + 1) && !value.reachableY));
    return { label: currentLabel,
      viewport: { width: innerWidth, height: innerHeight, devicePixelRatio, scale: visualViewport?.scale },
      document: { width: documentWidth, height: documentHeight,
        bodyOverflowX: getComputedStyle(body).overflowX, bodyOverflowY: getComputedStyle(body).overflowY },
      shell: rectangle(shell), containers, unreachableControls };
  }, label);
  assert(geometry.document.width <= geometry.viewport.width + 1,
    `${label} must not escape horizontally: ${JSON.stringify(geometry)}`);
  assert(geometry.shell.left >= -1 && geometry.shell.right <= geometry.document.width + 1 &&
    geometry.shell.top >= -1 && geometry.shell.bottom <= geometry.document.height + 1,
  `${label} shell must remain inside the reachable document`);
  assert.deepEqual(geometry.unreachableControls, [],
    `${label} controls remain reachable through document or panel scrolling`);
  if (geometry.document.height > geometry.viewport.height + 1)
    assert(['auto', 'scroll'].includes(geometry.document.bodyOverflowY), `${label} exposes a vertical document scrollbar`);
  for (const container of geometry.containers) {
    if (container.scrollHeight > container.clientHeight + 1)
      assert(['auto', 'scroll'].includes(container.overflowY) && container.reachedVerticalEnd,
        `${label} ${container.className} exposes and reaches its vertical overflow`);
    assert(container.scrollWidth <= container.clientWidth + 1 || ['auto', 'scroll'].includes(container.overflowX),
      `${label} ${container.className} does not clip horizontal overflow`);
  }
  await page.evaluate(() => scrollTo(0, Math.max(document.documentElement.scrollHeight, document.body.scrollHeight)));
  const bottomReachable = await page.evaluate(() => {
    const shell = document.querySelector('.shell').getBoundingClientRect();
    return shell.bottom <= innerHeight + 1;
  });
  assert(bottomReachable, `${label} bottom edge is reachable through the document scrollbar`);
  await page.evaluate(() => scrollTo(0, 0));
  return geometry;
}
async function dialogPoints(dialog) {
  const rectangle = await dialog.boundingBox();
  assert(rectangle, 'Open dialog must have a rendered rectangle');
  assert(rectangle.x > 4 && rectangle.y > 4, 'Dialog must leave a testable backdrop inside the application');
  return { rectangle, outside: { x: 2, y: 2 } };
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
  if (process.env.NIOH3_TAURI_EXE && process.env.NIOH3_UI_SOURCE_BUILD !== '1')
    assert.equal(diagnostics.packageVerification?.ok, true);
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

  const maximize = page.getByRole('button', { name: '最大化或还原', exact: true });
  const nativeStart = await viewportSnapshot(page);
  await maximize.click();
  const nativeMaximized = await waitForViewportChange(page, nativeStart);
  const nativeMaximizedGeometry = await verifyViewportReachability(page, 'native-maximized');
  await maximize.click();
  const nativeRestored = await waitForViewportChange(page, nativeMaximized);
  const nativeRestoredGeometry = await verifyViewportReachability(page, 'native-restored');
  const workAreaTolerance = 2;
  assert(Math.abs(nativeMaximized.outerWidth - nativeMaximized.screen.availWidth) <= workAreaTolerance &&
    Math.abs(nativeMaximized.outerHeight - nativeMaximized.screen.availHeight) <= workAreaTolerance,
  `Native maximize must occupy the available work area: ${JSON.stringify({ nativeStart, nativeMaximized })}`);
  assert.equal(nativeRestored.width, nativeStart.width, 'Native restore returns the exact pre-maximize CSS width');
  assert.equal(nativeRestored.height, nativeStart.height, 'Native restore returns the exact pre-maximize CSS height');
  nativeWindow = { start: nativeStart, maximized: nativeMaximized, restored: nativeRestored,
    maximizedGeometry: nativeMaximizedGeometry, restoredGeometry: nativeRestoredGeometry };
  check('Actual native maximize and restore preserve a reachable viewport', true);
  // Capture only after all native geometry measurements to avoid screenshot-induced fractional-DPI shifts.
  await maximize.click();
  const screenshotMaximized = await waitForViewportChange(page, nativeRestored);
  await page.screenshot({ path: join(output, 'native-maximized.png') });
  await maximize.click();
  await waitForViewportChange(page, screenshotMaximized);
  await page.screenshot({ path: join(output, 'native-restored.png') });

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

  await cdp.send('Emulation.clearDeviceMetricsOverride');
  await page.waitForFunction(([w, h]) => innerWidth === w && innerHeight === h,
    [nativeRestored.width, nativeRestored.height]);
  viewportMatrix = [];
  for (const [label, physicalWidth, physicalHeight, scale, width, height] of [
    ['1600x1000-at-100', 1600, 1000, 1, 1600, 1000],
    ['1920x1080-at-125', 1920, 1080, 1.25, 1536, 864],
    ['1920x1080-at-150', 1920, 1080, 1.5, 1280, 720],
    ['1600x900-at-150', 1600, 900, 1.5, 1067, 600],
    ['responsive-at-150', 1536, 900, 1.5, 1024, 600],
  ]) {
    await cdp.send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });
    await pause(200);
    console.log(`Requested effective viewport ${label}:`, await viewportSnapshot(page));
    await page.waitForFunction(([w, h]) => Math.abs(innerWidth - w) <= 1 && Math.abs(innerHeight - h) <= 1,
      [width, height]);
    const geometry = await verifyViewportReachability(page, label);
    viewportMatrix.push({ label,
      requested: { physicalWidth, physicalHeight, scale, effectiveCssWidth: width, effectiveCssHeight: height },
      observed: await viewportSnapshot(page),
      geometry });
    await page.screenshot({ path: join(output, `viewport-${label}.png`) });
  }
  await writeFile(join(output, 'viewport-matrix.json'), JSON.stringify(viewportMatrix, null, 2));
  await cdp.send('Emulation.clearDeviceMetricsOverride');
  await page.waitForFunction(([w, h]) => innerWidth === w && innerHeight === h,
    [nativeRestored.width, nativeRestored.height]);
  check('Effective 100%, 125%, and 150% viewport matrices keep all overflow reachable', true);

  await page.getByRole('button', { name: '设置', exact: true }).click();
  const showIdsSwitch = page.getByRole('switch', { name: '显示词条与敌人 ID', exact: true });
  check('Only the two Settings booleans are exposed as switches', await page.getByRole('switch').count() === 2 &&
    await page.getByRole('checkbox', { name: '勾选绘卷', exact: false }).count() === 0);
  await showIdsSwitch.focus();
  await page.keyboard.press('Space');
  assert.equal(await showIdsSwitch.isChecked(), true, 'Space toggles the Settings switch');
  settingsSwitches = await page.locator('.settings-switch').evaluateAll(labels => labels.map(label => {
    const text = label.querySelector('span').getBoundingClientRect();
    const input = label.querySelector('input'), control = input.getBoundingClientRect();
    const style = getComputedStyle(input), thumb = getComputedStyle(input, '::before');
    return { name: label.textContent.trim(), role: input.getAttribute('role'), checked: input.checked,
      active: document.activeElement === input,
      label: { left: text.left, right: text.right, centerY: text.top + text.height / 2 },
      control: { left: control.left, right: control.right, width: control.width, height: control.height,
        centerY: control.top + control.height / 2 },
      style: { appearance: style.appearance, borderRadius: style.borderRadius, outlineStyle: style.outlineStyle,
        outlineWidth: style.outlineWidth, boxShadow: style.boxShadow,
        thumbWidth: thumb.width, thumbHeight: thumb.height, thumbTransform: thumb.transform } };
  }));
  for (const value of settingsSwitches) {
    assert.equal(value.role, 'switch');
    assert(value.label.right < value.control.left && Math.abs(value.label.centerY - value.control.centerY) <= 1,
      `Settings text stays left and aligned with its switch: ${JSON.stringify(value)}`);
    assert(value.control.width >= 40 && value.control.height >= 22 && value.style.appearance === 'none' &&
      value.style.borderRadius !== '0px' && value.style.thumbWidth === value.style.thumbHeight,
    `Settings boolean has iOS-style track and thumb geometry: ${JSON.stringify(value)}`);
  }
  const focusedSwitch = settingsSwitches.find(value => value.active);
  assert(focusedSwitch && focusedSwitch.name.includes('显示词条') &&
    ((focusedSwitch.style.outlineStyle !== 'none' && parseFloat(focusedSwitch.style.outlineWidth) >= 1.5) ||
      focusedSwitch.style.boxShadow !== 'none'),
    'Focused Settings switch has a visible focus ring');
  previewLineSpacing = await page.locator('.result-scroll .effect-line').evaluateAll(rows => rows.map((row, index) => {
    const rectangle = element => {
      const value = element?.getBoundingClientRect();
      return value ? { top: value.top, bottom: value.bottom, height: value.height } : null;
    };
    return { row: rectangle(row), name: rectangle(row.querySelector('div > span')),
      id: rectangle(row.querySelector('small')), next: rectangle(rows[index + 1]) };
  }));
  for (const value of previewLineSpacing.filter(value => value.id)) {
    assert(value.id.top > value.name.top && value.id.bottom <= value.row.bottom + 1 &&
      (!value.next || value.id.bottom <= value.next.top + 1),
    `Preview effect name and ID remain legible without crossing rows: ${JSON.stringify(value)}`);
  }
  await page.screenshot({ path: join(output, 'settings-switches.png') });
  await page.getByRole('button', { name: '关闭侧边菜单', exact: true }).click();
  await page.getByRole('button', { name: '设置', exact: true }).click();
  assert.equal(await showIdsSwitch.isChecked(), true, 'Settings switch state persists when the panel reopens');
  await showIdsSwitch.focus();
  await page.keyboard.press('Space');
  await page.getByRole('button', { name: '关闭侧边菜单', exact: true }).click();
  check('Settings switches keep keyboard, focus, and panel-state behavior', true);

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

  const knownVariants = await addKnownEnemyVariantsToCart(page, { addToFavorites: true });
  await page.getByRole('button', { name: /查看购物车（\d+）/ }).click();
  check('Cart selection remains a checkbox rather than a Settings-style switch',
    await page.locator('.cart-review input[type="checkbox"]').count() >= 3 &&
    await page.locator('.cart-review [role="switch"]').count() === 0);
  await page.screenshot({ path: join(output, 'collection-cart.png') });
  const cartLayout = await verifyCollectionLayout(page, {
    container: '.cart-review', outputDirectory: join(output, 'collection-cart'), label: 'cart', minimumCards: 3,
  });
  await page.getByRole('button', { name: '关闭窗口', exact: true }).click();

  await page.getByRole('button', { name: '收藏夹', exact: true }).click();
  await page.screenshot({ path: join(output, 'collection-favorites.png') });
  const favoritesLayout = await verifyCollectionLayout(page, {
    container: '.favorites-review', outputDirectory: join(output, 'collection-favorites'),
    label: 'favorites', minimumCards: 3, verifyPreview: false,
  });
  const activeDialog = page.locator('dialog[open]');
  await activeDialog.evaluate(element => { element.scrollTop = 0; });
  const favoriteHeading = activeDialog.locator(':scope > header > h2');
  await favoriteHeading.click();
  assert.equal(await activeDialog.count(), 1, 'Interacting with modal content does not dismiss it');
  const favoritePoints = await dialogPoints(activeDialog);
  const heading = await favoriteHeading.boundingBox();
  assert(heading, 'Favorites heading must remain reachable');
  await page.mouse.move(heading.x + heading.width / 2, heading.y + heading.height / 2);
  await page.mouse.down();
  await page.mouse.move(favoritePoints.outside.x, favoritePoints.outside.y);
  await page.mouse.up();
  assert.equal(await activeDialog.count(), 1, 'A press begun inside and released on the backdrop does not dismiss the modal');
  await page.mouse.click(favoritePoints.outside.x, favoritePoints.outside.y);
  await page.locator('dialog[open]').waitFor({ state: 'detached' });
  check('Favorites dismisses on a complete backdrop click without false inside-release dismissal', true);

  await page.getByRole('button', { name: '历史', exact: true }).click();
  await page.screenshot({ path: join(output, 'collection-history.png') });
  const historyLayout = await verifyCollectionLayout(page, {
    container: '.history-pages', outputDirectory: join(output, 'collection-history'),
    label: 'history', minimumCards: 2, verifyPreview: false,
  });
  await page.getByRole('button', { name: '关闭窗口', exact: true }).click();
  collections = { knownVariants, cart: cartLayout, favorites: favoritesLayout, history: historyLayout };
  check('Cart, favorites, and history cards keep equal geometry, complete enemy labels, and reachable action rows', true);

  await page.getByRole('button', { name: '绘卷编辑', exact: true }).click();
  await page.getByRole('button', { name: '选择购物车中绘卷种子', exact: true }).click();
  const seedDialog = page.locator('dialog.seed-cart-dialog[open]');
  const seedDialogPoints = await dialogPoints(seedDialog);
  await page.mouse.click(seedDialogPoints.outside.x, seedDialogPoints.outside.y);
  await seedDialog.waitFor({ state: 'detached' });
  modalDismissal = { favorites: 'backdrop dismissed; inside-to-outside press preserved',
    seedCart: 'backdrop dismissed', pointerSequence: 'pointerdown and pointerup must both be on backdrop' };
  check('The editor seed-selection dialog also dismisses from its backdrop', true);

  const calls = await page.evaluate(() => window.__additionAcceptance);
  assert.equal(calls.requests.length, 2, 'Each addition prepares once');
  assert(calls.requests.every(request => request.mode === 'save' && request.references.length === 1));
  assert(!calls.operations.some(method => method.startsWith('runtime.')), 'Acceptance must not issue game-memory operations');
  assert.deepEqual(errors, [], 'No renderer errors');
  await writeFile(join(output, 'verification.json'), JSON.stringify({ executable, checks, layouts, sectionHelp,
    nativeWindow, viewportMatrix, settingsSwitches, previewLineSpacing, collections, modalDismissal,
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
