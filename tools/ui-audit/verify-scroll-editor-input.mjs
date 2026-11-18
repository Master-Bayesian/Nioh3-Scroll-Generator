/**
 * Browser acceptance for the released scroll editor's effect picker and draft
 * ownership. The fixture emulates transactions against two synthetic saves.
 */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { join, resolve } from 'node:path';
import { chromium } from 'playwright';

const root = resolve(import.meta.dirname, '../..');
const output = process.env.NIOH3_UI_OUTPUT;
assert.ok(output, 'Set NIOH3_UI_OUTPUT to an evidence directory under the project build root.');
await mkdir(output, { recursive: true });
execFileSync(process.execPath, ['apps/workshop/build.mjs'], { cwd: root, stdio: 'inherit' });
const bundle = await readFile(join(root, 'deliverables/frontend-v2/search-ui-demo-v2/index.html'), 'utf8');
const browser = await chromium.launch();
const server = createServer((_req, res) => res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' }).end(bundle));
await new Promise(ready => server.listen(0, '127.0.0.1', ready));
const viewport = { width: 1440, height: 900 };
const evidence = {
  browser: browser.version(),
  viewport,
  dpr: 1,
  bundleSha256: createHash('sha256').update(bundle).digest('hex'),
  fixture: 'Two in-browser synthetic save references, without a native backend.',
  results: [],
};

function bridge({ locale, theme }) {
  localStorage.setItem('nioh3-ui-locale', locale);
  localStorage.setItem('nioh3-theme', theme);
  localStorage.setItem('nioh3-scroll-edit-mode', 'live');
  const saves = ['fixture-a', 'fixture-b'].map((save_id, index) => ({
    save_id,
    path: 'D:/synthetic/' + save_id + '/SAVEDATA.BIN',
    account_id: String(10001 + index),
    save_slot: index,
  }));
  const calls = [];
  window.maintenanceCalls = calls;
  const countEdit = state => ({ count_edit: { state, operation_id: 'synthetic-scroll-edit', plan_digest: 'reviewed' } });
  function inventory(save_id) {
    const save = saves.find(item => item.save_id === save_id);
    const number = save.save_slot;
    return {
      save_id,
      account_id: save.account_id,
      // Deliberately identical seed, slot, digest and snapshot token.
      snapshot_id: 'synthetic-snapshot',
      source_sha256: 'a'.repeat(64),
      empty_slots: 500,
      entries: [{
        slot_index: 0,
        record_hex: String(number + 1).repeat(400),
        header: { playthrough: 3, level: 170 + number, recommended_level: 350, seed: 12000, rarity: 4, transfer_count: 0 },
        effects: Array.from({ length: 7 }, (_, slot_index) => ({
          slot_index, effect_id: slot_index ? 0xffffffff : 15, value: slot_index ? 0 : 50,
          prefix: 0, metadata: 0, tail_0: 0, tail_1: 0,
        })),
        derived: { initial_challenge_capacity: 3, recommended_raw_level: 350, recommended_displayed_level: 350 },
      }],
    };
  }
  window.preferences = { getLocale: async () => locale, setLocale: async () => {} };
  window.support = { diagnostics: async () => ({ version: '0.8.12' }) };
  window.nioh = {
    currentSearch: async () => ({ job: null, submitted: null }),
    handshake: async () => ({ backend: 'synthetic' }),
  };
  window.review = {
    auxiliary: async () => ({ initial_challenge_capacity: 3, enemy_groups: [], terrain: { display_effect_keys: [] }, special_rules: [] }),
    favorites: async () => [],
    update: async () => ({ phase: 'current' }),
    log: async () => {},
  };
  window.operations = {
    current: async () => ({ job: null, busy: false }),
    execute: async ({ method, params }) => {
      calls.push({ method, params });
      if (method === 'runtime.compatibility') return { compatibility: {
        present: true, reference_match: true, warning: false, accepted: false, plan: null,
        backup: null, differences: [], hard_blocks: [], game_version: '2.0.2.0',
        executable: 'D:/synthetic/Nioh3.exe',
      } };
      if (method === 'save.discover') return { saves };
      if (method === 'save.inventory') return inventory(params.save_id);
      if (method === 'save.operations') return { operations: [] };
      if (method === 'runtime.count_execute') return countEdit('verified');
      if (method === 'save.commit') throw new Error('Unexpected save mutation');
      return {};
    },
    prepareScrollEdit: async params => {
      calls.push({ method: 'runtime.count_prepare', params });
      return countEdit('prepared');
    },
  };
}

const selections = [
  { locale: 'zh-CN', theme: 'dark', effect: '词条 1', review: '核对修改', apply: '确认写入游戏' },
  { locale: 'en-US', theme: 'light', effect: 'Effect 1', review: 'Review edits', apply: 'Write to game' },
  { locale: 'ja-JP', theme: 'dark', effect: '効果 1', review: '変更を確認', apply: 'ゲームに書き込む' },
];

try {
  for (const selection of selections) {
    const { locale, theme } = selection;
    const context = await browser.newContext({ viewport, deviceScaleFactor: 1 });
    await context.addInitScript(bridge, { locale, theme });
    await context.tracing.start({ screenshots: true, snapshots: true });
    const page = await context.newPage();
    page.setDefaultTimeout(8_000);
    page.on('pageerror', error => console.error('Fixture page error:', error.message));
    page.on('console', message => { if (message.type() === 'error') console.error('Fixture console:', message.text()); });
    const checked = { locale, theme, checks: [] };
    evidence.results.push(checked);
    try {
      await page.goto('http://127.0.0.1:' + server.address().port);
      await page.locator('nav.nav > button').nth(1).click();
      await page.locator('.save-picker select').selectOption('fixture-a');
      const section = page.locator('details.editor-effects');
      const summary = page.locator('details.editor-effects > summary');
      const openEffects = async () => {
        await summary.waitFor();
        if (await section.getAttribute('open') === null) await summary.click();
      };
      await openEffects();
      const picker = section.locator('.effect-picker').first();
      const input = picker.getByRole('combobox', { name: selection.effect, exact: true });
      const baseline = await input.inputValue();
      await input.focus();
      const options = picker.getByRole('option');
      await options.nth(2).waitFor();
      await input.press('ArrowDown');
      await input.press('ArrowDown');
      await input.press('ArrowDown');
      const activeId = await input.getAttribute('aria-activedescendant');
      assert.equal(activeId, await options.nth(2).getAttribute('id'));
      assert.equal(await options.nth(2).evaluate(el => getComputedStyle(el).outlineStyle), 'solid');
      const chosen = (await options.nth(2).textContent()).trim();
      assert.notEqual(chosen, baseline);
      assert.equal(await input.evaluate(el => el === document.activeElement), true);
      await input.dispatchEvent('keydown', { key: 'Enter', code: 'Enter', isComposing: true, keyCode: 229 });
      assert.equal(await input.getAttribute('aria-expanded'), 'true');
      assert.equal(await input.getAttribute('aria-activedescendant'), activeId);
      checked.checks.push('composition-enter-preserves-the-active-option-and-popup');
      await page.screenshot({ path: join(output, locale + '-keyboard-focus.png') });
      await input.press('Enter');
      assert.equal(await input.getAttribute('aria-expanded'), 'false');
      assert.equal(await input.inputValue(), chosen);
      checked.checks.push('arrows-and-enter-select-the-highlighted-option');

      await input.click();
      await options.first().waitFor();
      await input.press('Escape');
      assert.equal(await input.inputValue(), chosen);
      await input.click();
      await input.press('Tab');
      assert.equal(await picker.getByRole('listbox').count(), 0);
      assert.equal(await input.inputValue(), chosen);
      checked.checks.push('escape-and-tab-dismiss-without-changing-the-draft');

      await input.click();
      await options.first().waitFor();
      await options.nth(2).click();
      assert.equal(await input.getAttribute('aria-expanded'), 'false');
      assert.equal(await input.inputValue(), chosen);
      checked.checks.push('pointer-selection-still-works');

      await page.getByRole('button', { name: selection.review, exact: true }).click();
      await page.getByRole('button', { name: selection.apply, exact: true }).click();
      await page.getByRole('button', { name: selection.apply, exact: true }).waitFor({ state: 'hidden' });
      const mode = page.locator('.editor-write-mode button');
      await mode.nth(1).click();
      assert.equal(await input.inputValue(), baseline);
      await mode.nth(0).click();
      assert.equal(await input.inputValue(), chosen);
      checked.checks.push('verified-live-overlay-stays-out-of-the-save-file-editor');

      await page.locator('.save-picker select').selectOption('fixture-b');
      await openEffects();
      assert.equal(await input.inputValue(), baseline);
      await page.locator('.save-picker select').selectOption('fixture-a');
      await openEffects();
      assert.equal(await input.inputValue(), baseline);
      const calls = await page.evaluate(() => window.maintenanceCalls);
      assert.deepEqual(calls.filter(call => call.method === 'save.inventory').map(call => call.params.save_id),
        ['fixture-a', 'fixture-b', 'fixture-a']);
      assert.deepEqual(calls.filter(call => call.method === 'runtime.count_prepare').map(call => call.params.save_id), ['fixture-a']);
      assert.equal(calls.filter(call => call.method === 'runtime.count_execute').length, 1);
      assert.equal(calls.some(call => call.method === 'save.commit'), false);
      checked.checks.push('same-seed-snapshot-and-slot-cannot-reuse-another-save-or-stale-overlay');
    } catch (error) {
      checked.error = String(error);
      await page.screenshot({ path: join(output, locale + '-failure.png') }).catch(() => {});
      throw error;
    } finally {
      await context.tracing.stop({ path: join(output, locale + '-trace.zip') });
      await context.close();
    }
  }
} finally {
  await browser.close();
  await new Promise(done => server.close(done));
  await writeFile(join(output, 'scroll-editor.json'), JSON.stringify(evidence, null, 2) + '\n');
}
console.log('Scroll editor acceptance passed for ' + evidence.results.map(result => result.locale).join(', '));
