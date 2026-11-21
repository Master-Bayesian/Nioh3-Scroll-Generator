/** Exercise the required-set search through a real WebView2 and protected worker. */
import {selectLoadoutScene} from './loadout-scene-control.mjs';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { chromium } from 'playwright';

const [rootPath, targetPath, backend = 'card', sourceMode = 'epoch'] = process.argv.slice(2);
assert(['epoch','first-use'].includes(sourceMode));
assert(rootPath && targetPath && ['card', 'cpu'].includes(backend),
  'Usage: node verify-required-set-loadout.mjs FIXTURE BUILD_TARGET [card|cpu]');
const root = resolve(rootPath), target = resolve(targetPath);
const output = await mkdtemp(join(root, 'native-' + backend + '-'));
console.log('Evidence directory ' + output);
const fixture = JSON.parse(await readFile(join(root, 'FIXTURE.json'), 'utf8'));
const baseline = JSON.parse(await readFile(fixture.standalone_reference, 'utf8'));
const fixturePath = join(root, fixture.fixture);
const host = join(target, 'debug/nioh3-studio.exe');
const worker = join(target, 'debug/nioh3-protected-worker.exe');
const search = join(target, 'debug/nioh3-readonly-worker.exe');
const hash = async file => createHash('sha256').update(await readFile(file)).digest('hex');
assert.equal(await hash(fixturePath), fixture.source_sha256);
const server = createServer();
await new Promise(done => server.listen(0, '127.0.0.1', done));
const { port } = server.address();
await new Promise(done => server.close(done));
const endpoint = 'http://127.0.0.1:' + port;
const profile = join(output, 'profile');
const child = spawn(host, ['--user-data-dir', profile], {
  windowsHide: true,
  stdio: ['ignore', 'pipe', 'pipe'],
  env: {
    ...process.env,
    LOCALAPPDATA: join(root, 'local'),
    NIOH3_TAURI_TEST_ROOT: profile,
    NIOH3_TAURI_TEST_DEBUG_PORT: String(port),
    NIOH3_STATE_ROOT: join(output, 'state'),
    NIOH3_RUST_PROTECTED_WORKER: worker,
    NIOH3_RUST_PROTECTED_DATA_ROOT: join(root, 'host-data'),
    NIOH3_RUST_PROTECTED_CONTRACT_DIR: resolve('packages/contracts'),
    NIOH3_RUST_PROTECTED_GAME_FILE_VERSION: '2.0.2.0',
    NIOH3_RUST_SEARCH_WORKER: search,
    NIOH3_RUST_SEARCH_DATA_ROOT: join(root, 'host-data'),
    NIOH3_RUST_SEARCH_GAME_FILE_VERSION: '2.0.2.0',
    NIOH3_LOADOUT_GPU: backend === 'card' ? '1' : '0',
    NIOH3_E2E_NO_GAME: '1',
    NIOH3_E2E_CAPTURE_ROOT: root,
    ...(sourceMode === 'first-use'
      ? {NIOH3_E2E_FIRST_USE_MEMORY: join(root, 'memory.json')}
      : {NIOH3_E2E_SOURCE_EPOCH: join(root, 'EPOCH_INPUT.json')}),
  },
});
let stderr = '';
child.stderr.on('data', bytes => { stderr = (stderr + bytes).slice(-24_000); });
const evidence = { passed: false, backend, context: fixture.observation_scope ?? (sourceMode === 'first-use' ? 'empty profile / ordinary reader on retained bytes, controlled unarmed inputs' : 'retained replay'), gameAccess: 'none', saveWrites: 0, pageErrors: [] };
let browser, page;
try {
  const until = Date.now() + 45_000;
  while (Date.now() < until) {
    if (child.exitCode !== null) throw Error('Tauri exited with ' + child.exitCode);
    try { if ((await fetch(endpoint + '/json/version')).ok) break; } catch { /* Browser startup. */ }
    await delay(250);
  }
  browser = await chromium.connectOverCDP(endpoint, { timeout: 10000 });
  page = browser.contexts()[0].pages()[0];
  page.on('pageerror', error => evidence.pageErrors.push(String(error)));
  await page.waitForFunction(() => !!window.operations, { timeout: 15000 });
  // Observe accepted IPC. Each call still executes in the original Tauri broker.
  await page.evaluate(() => {
    const log = { calls: [], jobs: {}, cancelled: [] };
    window.__loadoutE2e = log;
    const ops = window.operations;
    const execute = ops.execute.bind(ops);
    const snapshot = ops.snapshot.bind(ops);
    const cancel = ops.cancel.bind(ops);
    ops.execute = async command => {
      const entry = { method: command.method, params: command.params };
      log.calls.push(entry);
      try {
        const reply = await execute(command);
        if (reply?.job_id) {
          entry.jobId = reply.job_id;
          log.jobs[reply.job_id] = reply;
        }
        return reply;
      } catch (error) {
        entry.error = String(error);
        throw error;
      }
    };
    ops.snapshot = async (role, id) => {
      const job = await snapshot(role, id);
      log.jobs[id] = job;
      return job;
    };
    ops.cancel = async (role, id) => {
      const job = await cancel(role, id);
      log.cancelled.push(id);
      log.jobs[id] = job;
      return job;
    };
  });
  await page.getByRole('button', { name: '配装优化', exact: true }).click();
  await selectLoadoutScene(page, JSON.parse(await readFile(join(root, 'host-data', 'loadout', fixture.scene_id ?? 'kusarigama-strong', 'bundle.json'), 'utf8')));
  const context = page.getByTestId('loadout-context');
  await page.waitForFunction(() => {
    const state = document.querySelector('[data-testid="loadout-discovery-panel"]')?.dataset.state;
    return document.querySelector('[data-testid="loadout-context"]') || state === 'Failed' || state === 'Absent';
  }, null, { timeout: 30000 });
  if (!await context.count()) throw Error('Loadout discovery did not produce an attack');
  await context.locator('strong').getByText('已读取角色状态', { exact: true }).waitFor({ timeout: 60000 });
  const logs = () => page.evaluate(() => window.__loadoutE2e);
  const jobFor = async method => {
    const log = await logs();
    const entry = log.calls.findLast(row => row.method === method && row.jobId);
    return entry ? log.jobs[entry.jobId] : null;
  };
  const start = page.getByTestId('loadout-search-start');
  assert(await start.isDisabled(), 'Set-free search was available');
  const setsJob = await jobFor('save.loadout_sets');
  assert(setsJob?.result?.sets.some(set => set.key === 0xB2C1), 'Missing native set metadata for 0xB2C1');
  const contextJob = await jobFor('save.loadout_context');
  evidence.contextReceipt = { id: contextJob.result.context_id, source: contextJob.result.source,
    sourceSha: contextJob.result.source_sha256, attrs: contextJob.result.attrs };
  assert.equal(evidence.contextReceipt.source, 'captured');
  assert.equal(evidence.contextReceipt.sourceSha.toLowerCase(), fixture.source_sha256);

  await page.getByRole('combobox', { name: '添加套装条件', exact: true }).selectOption(String(0xB2C1));
  const tier = page.getByRole('combobox', { name: '生效件数', exact: true });
  await tier.selectOption(String(fixture.required_pieces ?? 5));
  await page.screenshot({ path: join(output, 'query.png') });
  await start.click();
  const result = page.getByTestId('loadout-result-coverage');
  await result.waitFor({ timeout: 120000 });
  assert.equal(await result.getAttribute('data-coverage'), 'complete');
  const run = await jobFor('save.loadout_search');
  assert.equal(run.state, 'completed');
  const score = run.result;
  assert.equal(score.backend, backend);
  assert.equal(score.full_size, baseline.progress.requested);
  assert.equal(score.excluded_by_set_bound, baseline.progress.excluded_by_set_bound);
  assert.equal(score.searched, baseline.progress.completed);
  assert.equal(score.answered, baseline.progress.answered);
  assert.equal(score.no_path + score.refused + score.source_uncomputed, 0);
  const equipped = fixture.equipped ?? {total: 629, physical: 610, elemental: 19};
  assert.equal(score.current.damage, equipped.total);
  assert.equal(score.current.damage_details.physical, equipped.physical);
  assert.equal(score.current.damage_details.elemental, equipped.elemental);
  if (equipped.contacts) assert.deepEqual(score.current.damage_details.contacts, equipped.contacts);
  assert.equal(score.best.length, 20);
  const expected = baseline.ranked.slice(0, score.best.length);
  const assignments = rows => rows.map(item => [item.slot, item.slot_index ?? item.save_slot]);
  for (const [index, row] of score.best.entries()) {
    assert.equal(row.damage, expected[index].damage, 'Damage at row ' + index);
    assert.deepEqual(assignments(row.items), assignments(expected[index].assignments), 'Slots at row ' + index);
  }
  evidence.search = { total: score.full_size, excluded: score.excluded_by_set_bound,
    answered: score.answered, rankRowsMatched: score.best.length, seconds: score.seconds,
    current: score.current.damage_details, winner: score.best[0] };
  console.log('Verified ' + score.answered + ' scored outfits and ' + score.best.length + ' published ranks on ' + backend);
  await page.screenshot({ path: join(output, 'result.png') });
  if (!fixture.ranking_only) {
    evidence.locales = ['zh-CN'];
    if (backend === 'card') {
      for (const [locale, label] of [['en-US', 'English'], ['ja-JP', '日本語'], ['zh-CN', '简体中文']]) {
        await page.locator('.language-button').click();
        await page.locator('.side-popup').getByRole('button', { name: label, exact: true }).click();
        await page.waitForFunction(value => document.documentElement.lang === value, locale);
        assert.equal(await result.getAttribute('data-coverage'), 'complete');
        assert(await page.getByTestId('loadout-search-start').isVisible());
        if (locale === 'en-US') {
          const scope = await page.getByTestId('loadout-source-model-scope').innerText();
          assert.match(scope, /reconstructed inputs/);
          assert.doesNotMatch(scope, /\p{Script=Han}/u);
          assert.match(await page.getByTestId('loadout-weapon-domain').innerText(), /12 owned and 12 included/);
          assert(await page.getByRole('option', { name: 'Strong attack (first hit)', exact: true }).count());
        }
        if (locale !== 'zh-CN') {
          await page.screenshot({ path: join(output, 'result-' + locale + '.png') });
          evidence.locales.push(locale);
        }
      }
    }

    await page.locator('.loadout-list > li.open').first()
      .getByRole('button', { name: '穿上这套（写入存档）', exact: true }).click();
    const plan = page.locator('.loadout-page .character-save-plan');
    await plan.waitFor();
    assert(await plan.getByRole('button', { name: '写入存档', exact: true }).isDisabled());
    const preview = await jobFor('save.prepare_character_edit');
    assert(preview?.result?.preview?.worn?.length > 0, 'Missing verified wardrobe preview');
    const selected = new Map(score.best[0].items.map(item => [item.slot_index, item.slot]));
    assert(preview.result.preview.worn.filter(line => line.after !== null).every(line => selected.get(line.slot_index) === line.after));
    evidence.preview = preview.result.preview.worn;
    await page.screenshot({ path: join(output, 'wear-preview.png') });
    await plan.getByRole('button', { name: '放弃', exact: true }).click();
    await plan.waitFor({ state: 'hidden' });

    await tier.selectOption('2');
    await start.click();
    await page.waitForFunction(() => {
      const jobs = Object.values(window.__loadoutE2e.jobs);
      return jobs.some(job => job.kind === 'save.loadout_search' && job.state === 'running'
        && job.progress?.stage === 'evaluation');
    }, { timeout: 30000 });
    await page.getByTestId('loadout-search-cancel').click();
    await page.getByTestId('loadout-search-cancelled').waitFor({ timeout: 60000 });
    assert.equal(await result.count(), 0);
    const cancelled = await jobFor('save.loadout_search');
    assert.equal(cancelled.state, 'failed');
    assert.equal(cancelled.result, null);
    assert.match(cancelled.error.message, /LOADOUT_SEARCH_CANCELLED/);
    evidence.cancelled = { terminal: cancelled.state, error: cancelled.error, progress: cancelled.progress };
    await page.screenshot({ path: join(output, 'cancelled.png') });
    const runProtected = async command => {
      const job = await page.evaluate(cmd => window.operations.execute(cmd), command);
      assert(job?.job_id, 'Expected a protected job for ' + command.method);
      const deadline = Date.now() + 30000;
      while (Date.now() < deadline) {
        const state = await page.evaluate(id => window.operations.snapshot('save', id), job.job_id);
        if (state.state === 'failed') throw Error(state.error?.message);
        if (state.state === 'completed') return state.result;
        await delay(100);
      }
      throw Error('Timed out waiting for ' + command.method);
    };
    const binding = contextJob.result;
    const source = { save_id: binding.save_id, source_sha256: binding.source_sha256,
      context_id: binding.context_id };
    const swapSearch = await runProtected({
      method: 'save.loadout_search',
      params: { ...source, bundle: binding.bundle, client_request_id: crypto.randomUUID(),
        items: [[193, 0], [363, 0], [37, 9], [216, 9], [335, 9]].map(([slot_index, slot]) => ({ slot_index, slot })),
        sets: [{ options: [{ key: 0xF4CB, pieces: 2 }], at_least: 1 }], limits: {} },
    });
    assert.equal(swapSearch.backend, backend);
    assert.equal(swapSearch.complete, true);
    assert.equal(swapSearch.answered, 10);
    const swapped = swapSearch.best.find(item => item.move_previous_weapon_to_slot === 1);
    assert(swapped?.items.some(item => item.slot === 0 && item.slot_index === 193));
    const swapPlan = await runProtected({
      method: 'save.prepare_character_edit',
      params: { ...source, wear: { school: 'ninja', move_previous_weapon_to_slot: 1,
        items: swapped.items.map(({ slot, slot_index }) => ({ slot, slot_index })) } },
    });
    const moved = swapPlan.preview.worn;
    assert(moved.some(line => line.slot_index === 193 && line.before === 1 && line.after === 0));
    assert(moved.some(line => line.slot_index === 153 && line.before === 0 && line.after === 1));
    await runProtected({ method: 'save.discard', params: { plan_id: swapPlan.plan_id } });
    evidence.spareSwap = { assigned: swapped.items, moved, candidateDamage: swapped.damage };
  }
  const log = await logs();
  evidence.ipcMethods = log.calls.map(row => row.method);
  assert(!evidence.ipcMethods.includes('save.commit'));
  if (!fixture.ranking_only) assert(evidence.ipcMethods.includes('save.discard'));
  assert.equal(await hash(fixturePath), fixture.source_sha256, 'Acceptance changed the source file');
  assert.deepEqual(evidence.pageErrors, []);
  evidence.geometry = await page.evaluate(() => ({ width: innerWidth, height: innerHeight, dpr: devicePixelRatio }));
  evidence.hashes = { host: await hash(host), worker: await hash(worker), fixture: await hash(fixturePath) };
  evidence.passed = true;
  console.log(JSON.stringify({ passed: true, backend, answered: score.answered, winner: score.best[0].damage,
    rankRowsMatched: score.best.length, current: score.current.damage, seconds: score.seconds }));
} catch (error) {
  evidence.error = String(error);
  if (page) {
    evidence.trace = await page.evaluate(async () => ({
      calls: window.__loadoutE2e?.calls,
      jobs: window.__loadoutE2e?.jobs,
      current: await Promise.race([
        window.operations.current('save').catch(error => ({ error: String(error) })),
        new Promise(resolve => setTimeout(() => resolve('timeout'), 3000)),
      ]),
    })).catch(() => null);
    evidence.visibleText = await page.locator('body').innerText({ timeout: 4000 }).catch(() => '');
    await page.screenshot({ path: join(output, 'failure.png') }).catch(() => {});
  }
  throw error;
} finally {
  evidence.stderr = stderr;
  await writeFile(join(output, 'NATIVE_E2E.json'), JSON.stringify(evidence, null, 2) + '\n');
  await browser?.close().catch(() => {});
  if (child.exitCode === null) child.kill();
}
