/** Regression acceptance for the reported Japanese seed, favorites and startup notice. */
import { _electron as electron } from 'playwright';
import { resolve, join } from 'node:path';
import { mkdir, writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
const output = resolve('deliverables/frontend-v2/v071-ui');
await mkdir(output, { recursive: true });
const app = await electron.launch({
  ...(process.env.NIOH3_PORTABLE_EXE ? { executablePath: resolve(process.env.NIOH3_PORTABLE_EXE), args: [] } : { args: [resolve('apps/desktop/dist/main.cjs')] }),
  env: { ...process.env, NIOH3_REVIEW_UI: '1', NIOH3_ELECTRON_TEST: '1' }
});
try {
  const p = await app.firstWindow();
  await p.getByText('后端已连接，请选择筛选条件。', { exact: true }).waitFor({ timeout: 30000 });
  await p.getByRole('textbox', { name: '已知绘卷ID', exact: true }).fill('76634363');
  await p.getByRole('button', { name: '查看', exact: true }).click();
  await p.locator('.result-detail .scroll').waitFor();
  await p.locator('.language-button').click();
  await p.locator('.side-popup').getByRole('button', { name: '日本語', exact: true }).click();
  const card = p.locator('.result-detail .scroll');
  assert.doesNotMatch(await card.innerText(), /RUBY|\^(?:20|21|FE|FF)~/);
  assert.match(await card.innerText(), /マガツヒの恩寵/);
  const star = p.locator('.result-tools .favorite-button');
  await star.waitFor();
  assert.equal(await star.locator('svg path').getAttribute('fill'), 'none');
  await star.click();
  await p.waitForFunction(() => document.querySelector('.result-tools .favorite-button')?.getAttribute('aria-pressed') === 'true');
  assert.equal(await star.locator('svg path').getAttribute('fill'), 'currentColor');
  await star.click();
  const geometry = [];
  for (const zoom of [1, 1.25, 1.5]) {
    await app.evaluate(({ BrowserWindow }, zoom) => {
      const w = BrowserWindow.getAllWindows()[0];
      w.setContentSize(1600, 1000); w.webContents.setZoomFactor(zoom); w.showInactive();
    }, zoom);
    const measured = await card.locator('.effect-line').evaluateAll(lines => lines.map(e => {
      const r = e.getBoundingClientRect(); const text = e.querySelector('div>span')?.getBoundingClientRect();
      return { top: r.top, bottom: r.bottom, textTop: text?.top, textBottom: text?.bottom };
    }));
    for (const line of measured) if (line.textTop !== undefined) {
      assert.ok(line.textTop >= line.top - 2 && line.textBottom <= line.bottom + 2, 'Japanese name stays in its row');
    }
    geometry.push({ zoom, measured });
    await p.screenshot({ path: join(output, `japanese-${zoom}.png`), timeout: 15000 });
  }
  // Isolate only the release transport; no real future release or game write.
  await app.evaluate(({ ipcMain }) => {
    globalThis.updateChecks = 0;
    let phase = 'idle';
    ipcMain.removeHandler('review:update');
    ipcMain.handle('review:update', (_, { action }) => {
      if (action === 'check') { globalThis.updateChecks++; phase = 'available'; }
      return { phase, version: '0.7.2', canApply: true };
    });
  });
  await p.reload();
  await p.locator('.update-notice').waitFor({ timeout: 15000 });
  assert.equal(await app.evaluate(() => globalThis.updateChecks), 1);
  await p.locator('.update-notice').click();
  await p.locator('.update-panel').waitFor();
  await writeFile(join(output, 'verification.json'), JSON.stringify({ seed: 76634363, geometry, automaticCheck: true, starStates: ['outline', 'filled'], gameWrites: 0 }, null, 2));
  console.log('V071_JAPANESE_STARS_AUTO_UPDATE_OK');
} finally { await app.close(); }
