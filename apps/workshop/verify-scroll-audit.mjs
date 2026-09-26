/**
 * Focused UI acceptance for the read-only scroll generation audit.
 *
 * This verifier intentionally runs the renderer against a deterministic IPC
 * mock. It proves request identity, replay evidence labelling and stale-result
 * isolation; it does not claim a live-game or packaged-app result.
 */
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdir, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { chromium } from 'playwright';
import { build } from 'esbuild';

const output = resolve(
  process.env.NIOH3_UI_OUTPUT ||
    'D:/Nioh3_v080_deliverables/deliverables/v081-integration-continuation-20260921/scroll-audit-ui',
);
await mkdir(output, { recursive: true });

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
const script = bundle.outputFiles.find((file) => file.path.endsWith('.js')).text
  .replaceAll('</script', '<\\/script');
const style = bundle.outputFiles.find((file) => file.path.endsWith('.css')).text;

const bridgeMock = `
const digest = (value) => value.padEnd(64, '0').slice(0, 64);
const recordType = 58884;
const projection = (playthrough, seed, level, rarity, delta = 0, type = recordType) => ({
  record_type: type, playthrough, seed, level, rarity,
  effects: Array.from({ length: 7 }, (_, slot_index) => ({ slot_index, prefix: slot_index === 5 || slot_index === 6 ? 0 : 100 + slot_index, effect_id: slot_index === 5 || slot_index === 6 ? 4294967295 : 10436 + slot_index, value: slot_index === 5 || slot_index === 6 ? 0 : delta + slot_index, metadata: slot_index === 5 || slot_index === 6 ? 0 : 200 + slot_index, tail_0: 0, tail_1: 0 })),
});
const auditRow = (options) => {
  const { slot_index, offset, sha, type = recordType, playthrough, seed, level = 180, rarity, reasons, attempted, matched, phase, delta = 0, phaseResults = [] } = options;
  const observed = projection(playthrough, seed, level, rarity, delta, type);
  const phases = phaseResults.map((entry) => ({
    phase: entry.phase,
    matched: entry.matched,
    expected_projection: projection(playthrough, seed, level, rarity, entry.delta || delta, type),
    mismatches: entry.mismatches || [],
  }));
  return {
    coverage_scope: 'generated_effect_projection', slot_index, record_offset: offset, record_sha256: sha,
    record_type: type, playthrough, seed, level, rarity, status: 'insufficient_data', reasons,
    replay_evidence: {
      attempted, matched, matched_phase: phase,
      compared_fields: ['record_type','playthrough','seed','level','rarity','effects[*].(prefix,effect_id,value,metadata,tail_0,tail_1)'],
      ignored_fields: ['recommended_level','generation_serial','transfer_count','completion_salt','account_id','inventory_key','challenge_count','unmodelled_history'],
      observed_projection: observed, phase_results: phases,
    },
  };
};
const auditRows = () => [
  auditRow({ slot_index: 0, offset: 1535182, sha: 'fcd3af45cac9cf260a381052bbd81dc4350f5d8e1398899e23b8ec5728913de8', playthrough: 3, seed: 226061463, rarity: 3, reasons: ['normal_input_domain_unproven','replay_match'], attempted: true, matched: true, phase: 'R3', phaseResults: [{ phase: 'R3', matched: true }] }),
  auditRow({ slot_index: 1, offset: 1535414, sha: '14f9c5907937aea804c952ba578fee1d9432c75143d96c7633d5cc900b79c809', playthrough: 3, seed: 387276918, rarity: 4, reasons: ['normal_input_domain_unproven','replay_match'], attempted: true, matched: true, phase: 'stage_one', phaseResults: [{ phase: 'stage_one', matched: true }, { phase: 'final', matched: false, mismatches: ['effects[3]'] }] }),
  auditRow({ slot_index: 2, offset: 1535646, sha: 'fb1f86096ca702639dbf5fa6cdfa30f2aab6aa9e6f9d0579598a9d2abf1c5878', playthrough: 3, seed: 10030700, rarity: 4, reasons: ['normal_input_domain_unproven','replay_match'], attempted: true, matched: true, phase: 'final', phaseResults: [{ phase: 'stage_one', matched: false, mismatches: ['effects[1]'] }, { phase: 'final', matched: true }] }),
  auditRow({ slot_index: 3, offset: 1535878, sha: 'd3a498f88e8f15634d63cf1c7f9d0a288b46e99fd2575197c9856ff5b01c1e64', playthrough: 3, seed: 226061463, rarity: 3, reasons: ['normal_input_domain_unproven','replay_mismatch'], attempted: true, matched: false, phase: null, delta: 1, phaseResults: [{ phase: 'R3', matched: false, mismatches: ['effects[0]'] }] }),
  auditRow({ slot_index: 4, offset: 1536110, sha: '9f1dae3298504e5a895c8d57128081da46af25f2f836fc455f35848e815ffa94', playthrough: 3, seed: 226061463, rarity: 5, reasons: ['normal_input_domain_unproven','unsupported_rarity'], attempted: false, matched: false, phase: null }),
  auditRow({ slot_index: 5, offset: 1536342, sha: 'a30c98ad06093456655692f65542cf71c309b7f2498a61474032562f49fca62b', type: 7810, playthrough: 1, seed: 226061463, rarity: 3, reasons: ['normal_input_domain_unproven','unsupported_record_type'], attempted: false, matched: false, phase: null }),
];
const saveId = '34ce90ce46e92efe7154e11c6ac743b42a9eaa2e8639e67a172b590c6a2c33fe';
const snapshotId = 'd498dc16ebd58c714ebd8241f57655d3';
const sourceSha = '478bcdd130bf407d3124ebb0a7605425eab9ae2d996aa670bd53d97bd3c252fa';
const makeAudit = (saveId, snapshotId) => ({ save_id: saveId, snapshot_id: snapshotId, source_sha256: sourceSha, status: 'insufficient_data', coverage_scope: 'generated_effect_projection', context: { product_version: '0.8.0', game_profile: 'pc-v2.00.02-v2.01', game_file_version: '2.0.2.0', versioned_resource_dir: 'r4_finalizer/pc_v2_02/resource_v1', bundle_digest: 'd1fb81af3bfc8e577239c2facdb097c320c9ab8b0a88e9fea792b1819c5584b2', versioned_digest: '09fa65803a0c058880f4d38900290b152eab89febb03615ce2576f4b020b358b', resources_digest: '411866d772e8e2450c1f4becd4c5e79761600f7766ab0659438bcbde21997048', algorithm_version: 'scroll-generation-v0.7-native-completion-1', policy_version: 'operation-policy-v1', context_digest: '13844ac4a7fbf55622bf5474e103897f2c8d65ab182460d146aa157651db5ea9', legacy_context_digest: '9fe76d2b7fd6bc33db4896f1af2a44d6ef28bf1964eedc1300f8dde482561faf', production_authority: true, seed_accelerator_abi: 2, seed_accelerator_build_id: 'sha256:7826ab5226d82e8825eff32c81cdf6650129e46d6919cb04a58b7ea08065acf7' }, rows: auditRows() });
window.__audit = { calls: [], mode: 'ok', queue: [], snapshotVersion: 1, discoverReady: false, saveId, snapshotId, auditJob: 0 };
const saveReference = () => ({ save_id: window.__audit.saveId, path: 'fixture.sav', account_id: 'acct', save_slot: 0 });
const inventory = () => ({ save_id: window.__audit.saveId, snapshot_id: window.__audit.snapshotId, source_sha256: sourceSha, account_id: 'acct', empty_slots: 0, entries: [{ slot_index: 0, header: { playthrough: 3, level: 180, recommended_level: 180, seed: 226061463, rarity: 3, transfer_count: 0 }, effects: Array.from({ length: 7 }, (_, slot_index) => ({ slot_index, effect_id: slot_index + 1, value: 1, prefix: 0, metadata: 0, tail_0: 0, tail_1: 0 })), derived: { initial_challenge_capacity: 1, remaining_challenge_attempts: 1, recommended_displayed_level: 180, recommended_raw_was_clamped: false } }] });
window.__audit.execute = (command) => {
  window.__audit.calls.push({ method: command.method, params: command.params });
  if (command.method === 'save.discover') return Promise.resolve({ saves: window.__audit.discoverReady ? [saveReference()] : [] });
  if (command.method === 'save.inventory') return Promise.resolve(inventory());
  if (command.method === 'save.operations') return Promise.resolve({ operations: [] });
  if (command.method === 'save.audit_scrolls') {
    const jobId = 'audit-' + (++window.__audit.auditJob);
    if (window.__audit.mode === 'error') return Promise.reject(new Error('stale snapshot: refresh the save'));
    const job = { job_id: jobId, kind: 'save.audit_scrolls', state: 'running', sequence: 1, cancellable: false, progress: null, result: null, error: null };
    if (window.__audit.mode === 'defer') window.__audit.queue.push({ jobId, saveId: command.params.save_id, snapshotId: command.params.snapshot_id });
    return Promise.resolve(job);
  }
  return Promise.resolve({});
};
window.__audit.snapshot = (role, jobId) => {
  const item = window.__audit.queue.find((entry) => entry.jobId === jobId);
  if (window.__audit.mode === 'defer' && item) return new Promise((resolve) => { item.resolve = resolve; });
  return Promise.resolve({ job_id: jobId, kind: 'save.audit_scrolls', state: 'completed', sequence: 2, cancellable: false, progress: null, result: makeAudit(window.__audit.saveId, window.__audit.snapshotId), error: null });
};
window.nioh = { handshake: () => Promise.resolve({}), searchCatalog: () => Promise.resolve({}), resolveRecommendedLevel: () => Promise.resolve(180), startSearch: () => Promise.resolve({}), currentSearch: () => Promise.resolve({}), snapshot: () => Promise.resolve({}), cancelSearch: () => Promise.resolve({}), restartWorker: () => Promise.resolve({}) };
window.operations = { execute: (command) => window.__audit.execute(command), current: () => Promise.resolve({ job: null, busy: false }), snapshot: (role, jobId) => window.__audit.snapshot(role, jobId), cancel: () => Promise.resolve({}), selectSave: () => Promise.resolve(saveReference()), prepareInstall: () => Promise.resolve({}), prepareLiveAdd: () => Promise.resolve({}), generate: () => Promise.resolve({}), searchNative: () => Promise.resolve({}), captureGrace: () => Promise.resolve({}), bindCachedSearch: () => Promise.resolve({}), prepareCount: () => Promise.resolve({}) };
window.preferences = { getLocale: () => Promise.resolve('zh-CN'), setLocale: () => Promise.resolve() };
window.support = { diagnostics: () => Promise.resolve({ version: '0.8.1' }), exportDiagnostics: () => Promise.resolve() };
window.review = { update: () => Promise.resolve({ phase: 'current', canApply: true }), windowAction: () => Promise.resolve(), favorites: () => Promise.resolve([]), log: () => Promise.resolve(), copyLog: () => Promise.resolve(''), openLink: () => Promise.resolve(), copyText: () => Promise.resolve(), release: () => Promise.resolve(), retain: () => Promise.resolve(), preview: () => Promise.resolve({}), auxiliary: () => Promise.resolve({ initial_challenge_capacity: 1, enemy_groups: [], terrain: { display_effect_keys: [] }, special_rules: [] }), dataDirectory: () => Promise.resolve(''), openSaveFolder: () => Promise.resolve(), openBackupFolder: () => Promise.resolve(), prepareCart: () => Promise.resolve() };
`;
const html = `<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Scroll audit acceptance</title><style>${style}</style></head><body><div id="root"></div><script>window.addEventListener('error', (event) => console.error('BRIDGE_RUNTIME_ERROR:' + event.message));</script><script>${bridgeMock}</script><script>${script}</script></body></html>`;
await writeFile(join(output, 'index.html'), html, 'utf8');
const server = createServer((request, response) => { response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' }); response.end(html); });
await new Promise((listening) => server.listen(0, '127.0.0.1', listening));
const origin = `http://127.0.0.1:${server.address().port}/`;
const checks = [];
const errors = [];
const check = (name, condition) => { assert.ok(condition, name); checks.push(name); console.log(name); };
const calls = (page) => page.evaluate(() => window.__audit.calls);
const selectLocale = async (page, label) => {
  await page.locator('.language-button').click();
  await page.locator('.side-popup button', { hasText: label }).first().click();
  await page.locator('.side-popup').waitFor({ state: 'detached' });
};

const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto(origin);
  await page.getByRole('button', { name: '绘卷编辑', exact: true }).click();
  await page.getByRole('heading', { name: '绘卷生成核对（实验）', exact: true }).waitFor();
  const audit = page.locator('[data-testid="scroll-generation-audit"]');
  check('Audit is disabled before a save snapshot is loaded', await audit.getByRole('button', { name: '生成核对', exact: true }).isDisabled());
  await page.evaluate(() => { window.__audit.discoverReady = true; });
  await page.getByRole('button', { name: '重新检测', exact: true }).click();
  await page.getByRole('combobox', { name: '自动检测的存档' }).waitFor();
  await audit.getByRole('button', { name: '生成核对', exact: true }).waitFor({ state: 'attached' });
  await audit.getByRole('button', { name: '生成核对', exact: true }).waitFor({ state: 'visible' });
  await page.waitForFunction(() => !document.querySelector('[data-testid="scroll-generation-audit"] button')?.hasAttribute('disabled'));
  check('Audit becomes enabled for the loaded snapshot', !(await audit.getByRole('button', { name: '生成核对', exact: true }).isDisabled()));
  await audit.getByRole('button', { name: '生成核对', exact: true }).click();
  await audit.getByText('正在核对…', { exact: true }).waitFor();
  const auditCall = (await calls(page)).find((call) => call.method === 'save.audit_scrolls');
  check('Audit sends the selected save and snapshot identity', auditCall?.params.save_id === '34ce90ce46e92efe7154e11c6ac743b42a9eaa2e8639e67a172b590c6a2c33fe' && auditCall?.params.snapshot_id === 'd498dc16ebd58c714ebd8241f57655d3');
  await audit.getByText('生成核对匹配', { exact: true }).first().waitFor();
  await page.waitForFunction(() => !document.querySelector('[data-testid="scroll-generation-audit"] .scroll-audit-run')?.hasAttribute('disabled'));
  check('Audit action re-enables after completion', !(await audit.getByRole('button', { name: '生成核对', exact: true }).isDisabled()));
  check('Matched replay evidence is labelled without a legality verdict', await audit.getByText('生成核对匹配', { exact: true }).first().isVisible() && !(await audit.innerText()).match(/合法|非法/));
  check('Mismatch and unsupported evidence remain distinguishable', await audit.getByText('生成核对未匹配', { exact: true }).isVisible() && await audit.getByText('不支持', { exact: true }).first().isVisible());
  check('Diagnostic disclaimer and insufficient-data status are visible', (await audit.innerText()).includes('匹配不代表完整规则判定') && (await audit.innerText()).includes('insufficient_data'));
  check('Known reason codes have human-readable primary labels', (await audit.innerText()).includes('正常取得条件尚未验证') && (await audit.innerText()).includes('稀有度暂不支持') && (await audit.innerText()).includes('重放不匹配'));
  check('Raw reason codes stay available as secondary evidence', await audit.locator('.scroll-audit-reasons-raw').count() === 6);
  check('No write control is exposed by the audit panel', await audit.locator('button').evaluateAll((nodes) => nodes.every((node) => node.textContent.trim() === '生成核对')));
  const successBody = await page.locator('body').innerText();
  check('Successful result has no accidental TypeError text', !successBody.includes('TypeError') && !successBody.includes('Cannot read properties'));
  check('Success fixture exposes the six representative backend rows', await audit.locator('.scroll-audit-row').count() === 6);
  check('Technical disclosure summaries keep a readable width', await audit.locator('.scroll-audit-technical summary').evaluateAll((nodes) => nodes.every((node) => node.getBoundingClientRect().width >= 80)));
  check('Result list is scrollable when rows exceed its bounded panel', await audit.locator('.scroll-audit-rows').evaluate((node) => node.scrollHeight > node.clientHeight));
  await page.screenshot({ path: join(output, 'scroll-audit-success-zh-CN.png'), fullPage: true });
  await audit.locator('.scroll-audit-row').last().scrollIntoViewIfNeeded();
  check('The final result row is reachable in the bounded list', await audit.locator('.scroll-audit-row').last().isVisible());
  await page.screenshot({ path: join(output, 'scroll-audit-success-bottom-zh-CN.png'), fullPage: true });
  await audit.locator('.scroll-audit-rows').evaluate((node) => { node.scrollTop = 0; });
  for (const [label, heading, runLabel, disclaimer, reason, locale] of [
    ['English', 'Scroll generation check (experimental)', 'Generation check', 'A match is diagnostic only; it is not a complete rule verdict, and the save is not modified.', 'Normal acquisition conditions are not verified', 'en-US'],
    ['日本語', '絵巻生成チェック（実験）', '生成チェック', '一致は診断情報であり、完全なルール判定を意味しません。セーブは変更しません。', '通常取得条件は未検証です', 'ja-JP'],
  ]) {
    await selectLocale(page, label);
    await page.getByRole('heading', { name: heading, exact: true }).waitFor();
    check(`${locale} audit heading and action are translated`,
      await audit.getByRole('heading', { name: heading, exact: true }).isVisible() &&
      await audit.getByRole('button', { name: runLabel, exact: true }).isVisible());
    check(`${locale} diagnostic disclaimer is translated`, await audit.getByText(disclaimer, { exact: true }).isVisible());
    const localizedAuditText = await audit.innerText();
    check(`${locale} known reason is translated`, localizedAuditText.includes(reason));
    check(`${locale} successful result table remains visible`, await audit.locator('.scroll-audit-row').count() === 6);
    await page.screenshot({ path: join(output, `scroll-audit-success-${locale}.png`), fullPage: true });
  }
  await selectLocale(page, '简体中文');

  await page.evaluate(() => { window.__audit.mode = 'defer'; });
  await audit.getByRole('button', { name: '生成核对', exact: true }).click();
  await audit.getByText('正在核对…', { exact: true }).waitFor();
  await page.waitForFunction(() => window.__audit.queue.some((entry) => typeof entry.resolve === 'function'));
  await page.getByRole('button', { name: '绘卷搜索', exact: true }).click();
  await page.evaluate(() => window.__audit.queue.splice(0).forEach((entry) => entry.resolve({ job_id: entry.jobId, kind: 'save.audit_scrolls', state: 'completed', sequence: 2, cancellable: false, progress: null, result: makeAudit(window.__audit.saveId, window.__audit.snapshotId), error: null })));
  await page.waitForTimeout(200);
  check('Late audit response after navigation paints no stale result', await page.locator('[data-testid="scroll-generation-audit"] .scroll-audit-result').count() === 0);
  check('Existing editor/search navigation remains responsive', await page.getByRole('heading', { name: '绘卷搜索', exact: true }).isVisible());

  await page.getByRole('button', { name: '绘卷编辑', exact: true }).click();
  await page.getByRole('heading', { name: '绘卷生成核对（实验）', exact: true }).waitFor();
  await page.evaluate(() => { window.__audit.mode = 'error'; });
  await audit.getByRole('button', { name: '生成核对', exact: true }).click();
  await audit.getByRole('alert').waitFor();
  check('Backend failure advises a fresh save read', (await audit.getByRole('alert').innerText()).includes('重新读取存档后重试'));

  await page.screenshot({ path: join(output, 'scroll-audit-error-zh-CN.png'), fullPage: true });
  check('Error result contains no accidental TypeError text', !(await page.locator('body').innerText()).includes('TypeError') && !(await page.locator('body').innerText()).includes('Cannot read properties'));
  await writeFile(join(output, 'verification.json'), JSON.stringify({ checks, errors, calls: await calls(page) }, null, 2), 'utf8');
  check('No runtime error was raised during the run', errors.length === 0);
  console.log(`${checks.length} scroll audit checks passed`);
} finally {
  await browser.close();
  await new Promise((closed) => server.close(closed));
}
