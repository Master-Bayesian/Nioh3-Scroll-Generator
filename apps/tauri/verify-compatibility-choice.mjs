/** Actual outer EXE/WebView2; scripted consent, native window close, no game/save writes. */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { isolatedEnvironment, inspectOnefile, closeSession, connect } from './onefile-acceptance.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));
const [exe, output, syntheticGame] = process.argv.slice(2).map(value => resolve(value));
assert.ok(exe && output && syntheticGame, 'supply outer EXE, evidence directory and synthetic VERSIONINFO EXE');
await mkdir(output, { recursive: true });
const identity = await inspectOnefile(exe);
const translations = JSON.parse(await readFile(join(root, 'apps/workshop/ui-locales.json'), 'utf8'));
const words = (key, locale) => locale === 'zh-CN' ? key : translations.ui[key][locale === 'en-US' ? 0 : 1];
const report = {
  ok: false,
  scope: 'actual outer EXE/native WebView2; scripted compatibility plans and backups; real title-bar shutdown; synthetic VERSIONINFO; no real game/save writes',
  exe: { path: exe, sha256: identity.sha256, payloadSha256: identity.payloadSha256 },
  sessions: [],
};

function fixture() {
  window.__choice = { calls: [], forbidden: [], serial: 0, verified: true, reference: false, accepted: false, events: 0 };
  window.addEventListener('nioh3:compatibility-accepted', () => window.__choice.events++);
  window.operations.execute = async command => {
    const f = window.__choice;
    f.calls.push(command);
    if (command.method !== 'runtime.compatibility') {
      f.forbidden.push(command.method);
      throw Error('E2E_BOUNDARY: unexpected operation ' + command.method);
    }
    const action = command.params.action;
    if (action === 'prepare') {
      f.accepted = false;
      f.plan = f.verified && !f.reference ? {
        plan_id: (++f.serial).toString(16).padStart(64, '0'),
        bypassed_checks: ['executable_sha256'], allowed_features: ['live_character', 'native_generation'],
        required_checks: ['process_identity', 'code_and_layout', 'ownership_and_bounds', 'verified_backup', 'single_writer', 'recovery_receipts', 'readback'], audit_path: null,
      } : null;
    }
    if (action === 'accept') {
      if (!f.verified || !f.plan || command.params.plan_id !== f.plan.plan_id || !command.params.confirmed || !command.params.backup_confirmed) throw Error('COMPATIBILITY_PLAN_MISMATCH');
      f.accepted = true;
    }
    if (action === 'cancel') { f.plan = null; f.accepted = false; }
    return { compatibility: {
      present: true, warning: !f.reference, reference_match: f.reference, accepted: f.accepted,
      game_version: '2.0.2.0', executable: 'D:/fixture/Nioh3.exe',
      operation_scoped_features: ['live_scroll_add', 'live_equipment_add'],
      differences: f.reference ? [] : [{ code: 'executable_sha256', expected: 'a'.repeat(64), actual: 'b'.repeat(64) }],
      hard_blocks: f.verified ? [] : [{ code: 'backup_unverified', detail: 'Fixture backup not verified' }],
      plan: f.reference ? null : f.plan,
      backup: f.reference ? null : { attempted: true, verified: f.verified, paths: f.verified ? ['D:/fixture-backup/SAVEDATA.BIN'] : [], error: f.verified ? null : 'Fixture backup not verified' },
    } };
  };
}

const cases = [['zh-CN', 'footer'], ['en-US', 'x'], ['ja-JP', 'escape'], ['zh-CN', 'outside']];
try {
  for (const [locale, exit] of cases) {
    const isolation = await mkdtemp('D:/Nioh3_v080_deliverables/tmp/compatibility-choice-native-');
    const { env, port } = await isolatedEnvironment(isolation);
    await mkdir(env.NIOH3_TAURI_TEST_ROOT, { recursive: true });
    await writeFile(join(env.NIOH3_TAURI_TEST_ROOT, 'game-install.json'), JSON.stringify({ schema: 'nioh3-game-install/v1', executable: syntheticGame }));
    let child, session;
    const evidence = { locale, exit, isolation, checks: [], screenshots: [] };
    report.sessions.push(evidence);
    const check = (name, result) => { assert.ok(result, name); evidence.checks.push(name); };
    try {
      child = spawn(exe, ['--user-data-dir', env.NIOH3_TAURI_TEST_ROOT], { env, windowsHide: true, stdio: 'ignore' });
      session = await connect(port, child);
      const page = session.page;
      evidence.packageVerification = session.diagnostics.packageVerification;
      check('clean exact package verified', evidence.packageVerification.ok && evidence.packageVerification.sourceDirty === false);
      check('synthetic selected game identity', session.diagnostics.gameInstallation.source === 'selected' && session.diagnostics.gameInstallation.file_version === '2.0.2.0');
      report.sourceCommit ??= evidence.packageVerification.sourceCommit;
      assert.equal(evidence.packageVerification.sourceCommit, report.sourceCommit);
      await page.locator('.nav nav > button').nth(0).click();
      await page.locator('.language-button').click();
      await page.locator('.side-popup button').filter({ hasText: ({ 'zh-CN': '简体中文', 'en-US': 'English', 'ja-JP': '日本語' })[locale] }).click();
      await page.evaluate(fixture);
      const dialog = page.locator('.compatibility-dialog');
      const accept = page.locator('[data-action=compatibility-accept]');
      const risk = page.locator('[data-action=compatibility-risk]');
      const backed = page.locator('[data-action=compatibility-backup]');
      const ready = async () => { await dialog.waitFor(); await page.waitForFunction(() => document.querySelector('.compatibility-body')?.getAttribute('aria-busy') === 'false'); };
      const open = async () => { await page.evaluate(() => window.dispatchEvent(new Event('nioh3:compatibility-required'))); await ready(); };
      const screenshot = async name => { const path = join(output, locale + '-' + exit + '-' + name + '.png'); await page.screenshot({ path }); evidence.screenshots.push(path); };
      await open();
      evidence.viewport = await page.evaluate(() => ({ width: innerWidth, height: innerHeight, dpr: devicePixelRatio }));
      check('explicit localized mode/exit choices', await accept.innerText() === words('进入兼容模式', locale) && await page.locator('[data-action=compatibility-exit]').innerText() === words('关闭工具', locale));
      check('X has localized exit label', await page.locator('[data-action=compatibility-close]').getAttribute('aria-label') === words('关闭工具', locale));
      check('fresh unchecked risk and backup', !await risk.isChecked() && !await backed.isChecked() && await accept.isDisabled());
      check('independent live-add scope retained', await dialog.locator('[data-section=operation-scoped-features]').count() === 1 && await page.evaluate(() => !window.__choice.plan.allowed_features.includes('live_equipment_add')));
      check('footer choices fit native viewport', await accept.evaluate(e => { const r = e.getBoundingClientRect(); return r.top >= 0 && r.bottom <= innerHeight && r.right <= innerWidth; }));
      await screenshot('choices');
      await risk.check(); check('risk alone cannot accept', await accept.isDisabled()); await backed.check();
      await accept.click(); await dialog.waitFor({ state: 'detached' });
      check('confirmed current plan enters mode', await page.evaluate(() => window.__choice.accepted && window.__choice.events === 1));
      await open(); check('reopen prepares fresh unchecked plan', !await risk.isChecked() && !await backed.isChecked() && await accept.isDisabled());
      await page.evaluate(() => { window.__choice.verified = false; });
      await page.locator('[data-action=compatibility-prepare]').click(); await ready();
      check('unverified backup cannot be bypassed', await accept.isDisabled() && await risk.isDisabled() && await backed.isDisabled());
      await screenshot('backup-refusal');
      await page.evaluate(() => { window.__choice.verified = true; window.__choice.reference = true; });
      await page.locator('[data-action=compatibility-prepare]').click(); await ready();
      check('exact reference alone permits return without consent', await accept.count() === 0 && await page.locator('[data-action=compatibility-return]').isEnabled());
      await page.locator('[data-action=compatibility-return]').click(); await dialog.waitFor({ state: 'detached' });
      check('reference return does not accept a plan', await page.evaluate(() => window.__choice.events === 1));
      await page.evaluate(() => { window.__choice.reference = false; }); await open();
      if (exit === 'footer') {
        const before = evidence.viewport;
        await page.evaluate(() => window.review.windowAction('maximize'));
        await page.waitForFunction(width => innerWidth > width, before.width);
        evidence.maximized = await page.evaluate(() => ({ width: innerWidth, height: innerHeight, dpr: devicePixelRatio }));
        await screenshot('maximized');
        await page.evaluate(() => window.review.windowAction('maximize'));
        await page.waitForFunction(({ width, height }) => Math.abs(innerWidth - width) <= 1 && Math.abs(innerHeight - height) <= 1, before);
        check('native maximize/restore retains usable geometry', await accept.isVisible());
        await page.evaluate(() => { window.__choice.nativeClose = window.review.windowAction.bind(window.review); window.review.windowAction = async () => { throw Error('WINDOW_CLOSE_FAILED: scripted refusal'); }; });
        await page.locator('[data-action=compatibility-exit]').click();
        await page.waitForFunction(() => document.querySelector('.compatibility-dialog .notice')?.textContent.includes('WINDOW_CLOSE_FAILED'));
        check('close failure remains modal and retryable', await dialog.isVisible() && await page.locator('[data-action=compatibility-exit]').isEnabled() && await accept.isDisabled());
        await page.locator('[data-action=compatibility-prepare]').click(); await ready();
        await page.evaluate(() => { window.__choice.closeCalls = 0; window.review.windowAction = async action => { window.__choice.closeCalls++; await new Promise(resolve => { window.__choice.releaseClose = resolve; }); await window.__choice.nativeClose(action); }; });
        await page.locator('[data-action=compatibility-exit]').click();
        await page.waitForFunction(() => typeof window.__choice.releaseClose === 'function');
        check('pending shutdown keeps all modal actions disabled', await dialog.isVisible() && await accept.isDisabled() && await page.locator('[data-action=compatibility-prepare]').isDisabled());
        const count = await page.evaluate(() => window.__choice.calls.length);
        await page.keyboard.press('Escape'); await page.mouse.click(3, 3);
        await page.evaluate(() => window.dispatchEvent(new Event('nioh3:compatibility-required')));
        check('repeated exit/reopen cannot duplicate close', await page.evaluate(count => window.__choice.closeCalls === 1 && window.__choice.calls.length === count, count));
        await screenshot('pending-shutdown');
        evidence.fixture = await page.evaluate(() => ({ calls: window.__choice.calls, forbidden: window.__choice.forbidden, events: window.__choice.events }));
        await page.evaluate(() => window.__choice.releaseClose());
      } else {
        evidence.fixture = await page.evaluate(() => ({ calls: window.__choice.calls, forbidden: window.__choice.forbidden, events: window.__choice.events }));
        if (exit === 'x') await page.locator('[data-action=compatibility-close]').click();
        else if (exit === 'escape') await page.keyboard.press('Escape');
        else await page.mouse.click(3, 3);
      }
      assert.deepEqual(evidence.fixture.forbidden, []);
      let timer;
      const exited = child.exitCode !== null ? Promise.resolve([child.exitCode]) : once(child, 'exit');
      try {
        const [code] = await Promise.race([exited, new Promise((_, reject) => { timer = setTimeout(() => reject(Error('Native tool did not close within its existing grace period')), 30000); })]);
        check('normal native close exits the owned tool successfully', code === 0);
      } finally { clearTimeout(timer); }
      console.log('COMPATIBILITY_CHOICE_NATIVE_OK: ' + locale + ' / ' + exit);
    } finally { await closeSession(session, child); }
  }
  assert.equal((await inspectOnefile(exe)).sha256, identity.sha256);
  report.ok = true;
} catch (error) { report.error = String(error?.stack || error); throw error; }
finally { await writeFile(join(output, 'compatibility-choice-native.json'), JSON.stringify(report, null, 2) + '\n'); }
