/** Actual outer-EXE acceptance for the reported missing enemy roster. */
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdir, mkdtemp, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {executable, inspectOnefile, isolatedEnvironment, connect, closeSession, pause} from './onefile-acceptance.mjs';

const exe = executable();
const outer = await inspectOnefile(exe);
const output = resolve(process.env.NIOH3_ENEMY_UI_OUTPUT || 'deliverables/frontend-v2/tauri-acceptance/enemy-preview');
await mkdir(output, {recursive: true});
const root = await mkdtemp(join(tmpdir(), 'nioh3-enemy-preview-'));
const environment = await isolatedEnvironment(root);
const child = spawn(exe, ['--user-data-dir', environment.profile], {
  env: environment.env, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'],
});
let stderr = '', session, failure;
child.stderr.on('data', value => { stderr += value; });
const report = {
  schema: 'nioh3-native-enemy-preview/v1', pass: false,
  executable: exe, sourceExeSha256: outer.sha256, payloadSha256: outer.payloadSha256,
  scope: 'Actual outer EXE, WebView2 and unchanged packaged read-only previews; isolated profile; no game/user-save writes',
  checks: [], screenshots: [], gameWrites: 0,
};
const check = (name, passed, details = null) => {
  report.checks.push({name, passed: !!passed, details});
  assert(passed, name + ': ' + JSON.stringify(details));
};
try {
  session = await connect(environment.port, child);
  const {page} = session;
  report.sourceCommit = session.diagnostics.packageVerification.sourceCommit;
  check('package source identity is present', /^[0-9a-f]{40}$/.test(report.sourceCommit));
  if (process.env.NIOH3_EXPECTED_SOURCE_COMMIT) {
    check('exact expected source commit', report.sourceCommit === process.env.NIOH3_EXPECTED_SOURCE_COMMIT);
  }
  await page.evaluate(() => {
    const state = window.__enemyPreviewAcceptance = {previews: [], operations: []};
    const preview = window.review.preview;
    window.review.preview = async params => {
      const reply = await preview(params);
      state.previews.push({params: structuredClone(params), reply: structuredClone(reply)});
      return reply;
    };
    const execute = window.operations.execute;
    window.operations.execute = async command => {
      state.operations.push(structuredClone(command));
      if (/^(?:save\.(?:prepare|commit|restore|resign|delete)|runtime\.(?:equipment_add|character_edit|player_edit|live_batch))/.test(command.method)) {
        throw Error('WRITE_FORBIDDEN_ENEMY_PREVIEW_ACCEPTANCE');
      }
      return execute(command);
    };
  });
  const input = page.getByRole('textbox', {name: '已知绘卷ID', exact: true});
  const view = page.getByRole('button', {name: '查看', exact: true});
  const card = page.locator('.result-detail .scroll');
  const enemies = card.locator('.enemy-lines');
  const preview = async seed => {
    await input.fill(String(seed)); await view.click();
    await page.waitForFunction(() => document.querySelector('.direct-seed')?.dataset.previewState === 'current');
    check('displayed ID ' + seed, (await card.locator('footer strong').innerText()).includes(String(seed)));
  };
  await page.getByRole('combobox', {name: '周目', exact: true}).selectOption('3');
  await page.getByRole('combobox', {name: '稀有度', exact: true}).selectOption('4');
  await page.getByRole('spinbutton', {name: '绘卷等级', exact: true}).fill('180');
  await page.getByRole('spinbutton', {name: '推荐等级', exact: true}).fill('330');
  await preview(141974638);
  const names = await enemies.locator(':scope > span').allTextContents();
  check('the reported game enemies are visible', names.length === 3 && ['烟烟罗', '河童', '赤河童'].every(name => names.some(text => text.includes(name))), names);
  check('unknown state does not produce possessed badges', await enemies.locator('.enemy-state-marker').count() === 0);
  await preview(155277563);
  check('ordinary detailed preview is preserved', await enemies.getAttribute('data-enemy-preview') === 'detailed' && await enemies.locator('.enemy-occurrence').count() > 0);
  await preview(141974638);
  const languages = [
    ['简体中文', 'zh-CN', '地狱附身状态暂未确认。'],
    ['English', 'en-US', 'Crucible Wraith status has not been confirmed.'],
    ['日本語', 'ja-JP', '地獄憑き状態は未確認です。'],
  ];
  for (const theme of ['light', 'dark']) for (const [label, lang, note] of languages) {
    if (await page.evaluate(() => document.documentElement.lang) !== lang) {
      await page.locator('.language-button').click();
      await page.locator('.side-popup').getByRole('button', {name: label, exact: true}).click();
      await page.waitForFunction(value => document.documentElement.lang === value, lang);
    }
    await page.evaluate(value => { document.documentElement.dataset.theme = value; }, theme);
    for (const geometry of ['normal', 'maximized']) {
      if (geometry === 'maximized') { await page.evaluate(() => window.review.windowAction('maximize')); await pause(300); }
      check('known roster ' + theme + '/' + lang + '/' + geometry,
        await enemies.getAttribute('data-enemy-preview') === 'base_only' && await enemies.locator(':scope > span').count() === 3 && await enemies.locator('.enemy-preview-note').innerText() === note);
      check('document fits ' + theme + '/' + lang + '/' + geometry,
        await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1));
      const file = `enemies-${theme}-${lang}-${geometry}.png`;
      await page.screenshot({path: join(output, file), animations: 'disabled'});
      report.screenshots.push(file);
      if (geometry === 'maximized') { await page.evaluate(() => window.review.windowAction('maximize')); await pause(300); }
    }
  }
  report.requests = await page.evaluate(() => window.__enemyPreviewAcceptance);
  check('only read-only preview requests were sent', report.requests.previews.length === 3 && report.requests.previews.every(item => !item.params.retain));
  check('no protected mutation was dispatched', !report.requests.operations.some(command => /^(?:save\.(?:prepare|commit|restore|resign|delete)|runtime\.(?:equipment_add|character_edit|player_edit|live_batch))/.test(command.method)));
  report.pass = true;
} catch (error) {
  failure = error; report.error = error.stack || String(error);
  if (session) await session.page.screenshot({path: join(output, 'failure.png')}).catch(() => {});
} finally {
  try { await closeSession(session, child); }
  catch (error) { failure ||= error; report.pass = false; report.cleanupError = String(error); }
  await writeFile(join(output, 'report.json'), JSON.stringify(report, null, 2) + '\n');
  await writeFile(join(output, 'stderr.log'), stderr);
  console.log(JSON.stringify({pass: report.pass, checks: report.checks.length, screenshots: report.screenshots.length, output}));
}
if (failure) throw failure;
