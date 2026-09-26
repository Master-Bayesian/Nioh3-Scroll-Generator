/**
 * Bounded native-debug continuation acceptance for the v0.8.1 read-only slices.
 *
 * Failure cases are intentionally listed before the harness:
 * 1. Missing/stale host or worker must fail closed; this helper never falls
 *    back to the Python worker and never builds.
 * 2. The retained synthetic audit fixture must be present and its known
 *    post-test mutation must reverse to the recorded source_sha_before. The
 *    retained evidence file and fixture are never modified.
 * 3. A missing CDP target, blank shell, missing page, missing bridge, or
 *    unexpected process exit is a harness failure, not a blocked game result.
 * 4. The local catalog flow must issue one inline catalog.import_names call with
 *    the exact bytes/metadata, show one accepted row, reject malformed input
 *    without replacing the active catalog, persist across reload, and clear.
 * 5. The scroll flow may issue only save.discover/save.inventory/
 *    save.audit_scrolls plus their snapshot polling. It must not call a runtime
 *    method or an equipment refresh (which would read a real game process).
 * 6. The audit must complete through the real UI, retain its exact terminal
 *    result, report every row as insufficient_data, and leave the copied save
 *    byte-identical. Missing job results, wrong identity/hash, or a visible
 *    legality verdict fail closed.
 * 7. Native screenshots, locale switches, and maximize/restore measurements
 *    are evidence only; they do not promote this debug run to packaged/game
 *    acceptance.
 *
 * Output and all writable fixture/profile data stay on D:. Only the spawned
 * Studio process is terminated in cleanup.
 */
import { chromium } from 'playwright';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createServer } from 'node:net';
import { existsSync, readFileSync } from 'node:fs';
import { mkdir, mkdtemp, readFile, readdir, copyFile, writeFile } from 'node:fs/promises';
import { join, resolve, dirname } from 'node:path';
import assert from 'node:assert/strict';

const DELIVERY_ROOT = 'D:/Nioh3_v080_deliverables';
const DEFAULT_TARGET = `${DELIVERY_ROOT}/build-cache/tauri-target`;
const DEFAULT_HOST = `${DEFAULT_TARGET}/debug/nioh3-studio.exe`;
const DEFAULT_PROTECTED = `${DEFAULT_TARGET}/debug/nioh3-protected-worker.exe`;
const DEFAULT_SEARCH = `${DEFAULT_TARGET}/debug/nioh3-readonly-worker.exe`;
const RETAINED_AUDIT_DIR = `${DELIVERY_ROOT}/deliverables/v081-integration-continuation-20260921/scroll-audit`;
const RETAINED_AUDIT = join(RETAINED_AUDIT_DIR, 'e2e-audit-result.json');
const NATIVE_OUTPUT = join(RETAINED_AUDIT_DIR, 'native');
const BUILD_COMMAND =
  'CARGO_TARGET_DIR=D:\\Nioh3_v080_deliverables\\build-cache\\tauri-target; ' +
  'cargo build --locked --manifest-path crates/nioh3-protected/Cargo.toml --bin nioh3-protected-worker; ' +
  'TAURI_CONFIG=<D-rooted frontendDist file list> cargo build --locked --manifest-path apps/tauri/src-tauri/Cargo.toml';

function onDeliveryVolume(value, label) {
  const absolute = resolve(value);
  assert.match(absolute, /^[Dd]:[\\/]/, `${label} must be on D:, got ${absolute}`);
  return absolute;
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

async function hashFile(path) {
  return sha256(await readFile(path));
}

async function findFile(root, wanted) {
  const entries = await readdir(root, { withFileTypes: true });
  for (const entry of entries) {
    const path = join(root, entry.name);
    if (entry.isFile() && entry.name.toLowerCase() === wanted.toLowerCase()) return path;
    if (entry.isDirectory()) {
      const nested = await findFile(path, wanted);
      if (nested) return nested;
    }
  }
  return null;
}

const host = onDeliveryVolume(process.env.NIOH3_TAURI_EXE || DEFAULT_HOST, 'NIOH3_TAURI_EXE');
const protectedWorker = onDeliveryVolume(
  process.env.NIOH3_RUST_PROTECTED_WORKER || DEFAULT_PROTECTED,
  'NIOH3_RUST_PROTECTED_WORKER',
);
const searchWorker = onDeliveryVolume(
  process.env.NIOH3_RUST_SEARCH_WORKER || DEFAULT_SEARCH,
  'NIOH3_RUST_SEARCH_WORKER',
);
const output = onDeliveryVolume(process.env.NIOH3_V081_NATIVE_OUTPUT || NATIVE_OUTPUT, 'output');
const scrollOnly = process.env.NIOH3_V081_SCROLL_ONLY === '1';
assert.ok(existsSync(host), `debug host is missing: ${host}`);
assert.ok(existsSync(protectedWorker), `protected worker is missing: ${protectedWorker}`);
assert.ok(existsSync(searchWorker), `search worker is missing: ${searchWorker}`);
assert.ok(existsSync(RETAINED_AUDIT), `retained audit evidence is missing: ${RETAINED_AUDIT}`);

const retained = JSON.parse(await readFile(RETAINED_AUDIT, 'utf8'));
assert.match(retained.source_sha_before, /^[0-9a-f]{64}$/i, 'retained source_sha_before');
assert.match(retained.source_sha_after_snapshot_expiry, /^[0-9a-f]{64}$/i, 'retained mutated fixture hash');
const retainedFixture = await findFile(RETAINED_AUDIT_DIR, 'SAVEDATA.BIN');
assert.ok(retainedFixture, 'retained synthetic SAVEDATA.BIN is missing');
assert.equal(await hashFile(retainedFixture), retained.source_sha_after_snapshot_expiry.toLowerCase(), 'retained fixture hash must match its recorded post-test mutation');

await mkdir(output, { recursive: true });
const runRoot = await mkdtemp(join(output, 'run-'));
const profileRoot = join(runRoot, 'profile');
const stateRoot = join(runRoot, 'state');
const localRoot = join(runRoot, 'local');
const savePath = join(localRoot, 'KoeiTecmo', 'NIOH3', 'Savedata', '76561198000000123', 'SAVEDATA00', 'SAVEDATA.BIN');
await mkdir(dirname(savePath), { recursive: true });
const copied = Buffer.from(await readFile(retainedFixture));
copied[copied.length - 1] ^= 1;
assert.equal(sha256(copied), retained.source_sha_before.toLowerCase(), 'restored fixture copy must match source_sha_before');
await writeFile(savePath, copied);

const server = createServer();
await new Promise((done) => server.listen(0, '127.0.0.1', done));
const port = server.address().port;
await new Promise((done) => server.close(done));

const evidence = {
  scenario: 'v081-native-debug-catalog-and-scroll-audit',
  status: 'fail',
  pass: false,
  debugOnly: true,
  packaged: false,
  gameAccess: 'none',
  runtimeCalls: [],
  bridgeCalls: [],
  screenshots: [],
  geometry: {},
  catalog: {},
  scrollAudit: {},
  identities: { host: { path: host }, protectedWorker: { path: protectedWorker }, searchWorker: { path: searchWorker } },
  fixture: {
    source: retainedFixture,
    copiedPath: savePath,
    sourceShaBefore: retained.source_sha_before.toLowerCase(),
    copiedShaBeforeAudit: sha256(copied),
  },
  build: { command: BUILD_COMMAND, targetDir: process.env.CARGO_TARGET_DIR || DEFAULT_TARGET },
  failures: [],
};
if (scrollOnly) {
  const priorEvidencePath = join(output, 'native-v081-evidence.json');
  if (existsSync(priorEvidencePath)) {
    try {
      const prior = JSON.parse(readFileSync(priorEvidencePath, 'utf8'));
      evidence.catalog = prior.catalog || {};
      evidence.geometry = prior.geometry || {};
      evidence.screenshots = Array.isArray(prior.screenshots)
        ? prior.screenshots.filter((shot) => String(shot.path || '').includes('catalog-'))
        : [];
    } catch (error) {
      evidence.failures.push(`prior catalog evidence unreadable: ${String(error?.message || error)}`);
    }
  } else {
    evidence.failures.push(`prior catalog evidence missing: ${priorEvidencePath}`);
  }
}

const child = spawn(host, ['--user-data-dir', profileRoot], {
  windowsHide: true,
  stdio: ['ignore', 'pipe', 'pipe'],
  env: {
    ...process.env,
    NIOH3_TAURI_TEST_ROOT: profileRoot,
    NIOH3_TAURI_TEST_DEBUG_PORT: String(port),
    NIOH3_STATE_ROOT: stateRoot,
    LOCALAPPDATA: localRoot,
    NIOH3_RUST_SEARCH_WORKER: searchWorker,
    NIOH3_RUST_PROTECTED_WORKER: protectedWorker,
    NIOH3_RUST_SEARCH_GAME_FILE_VERSION: '2.0.2.0',
    NIOH3_RUST_PROTECTED_GAME_FILE_VERSION: '2.0.2.0',
  },
});
let stderr = '';
child.stderr.on('data', (data) => (stderr = (stderr + data).slice(-32000)));
let browser;
let page;

async function writeEvidence() {
  evidence.fixture.copiedShaAfterAudit = await hashFile(savePath);
  evidence.identities.host.sha256 = await hashFile(host);
  evidence.identities.protectedWorker.sha256 = await hashFile(protectedWorker);
  evidence.identities.searchWorker.sha256 = await hashFile(searchWorker);
  const sourceFiles = [
    'apps/workshop/LocalCatalogImport.tsx',
    'apps/workshop/EquipmentBrowser.tsx',
    'apps/workshop/ScrollGenerationAudit.tsx',
    'apps/workshop/main.tsx',
    'apps/workshop/style.css',
    'apps/tauri/bridge.ts',
    'apps/desktop/src/operations-api.ts',
    'apps/tauri/src-tauri/src/broker.rs',
    'packages/contracts/protected-request.schema.json',
    'packages/contracts/protected-response.schema.json',
  ];
  evidence.identities.sourceSha256 = {};
  for (const file of sourceFiles) evidence.identities.sourceSha256[file] = await hashFile(resolve(file));
  evidence.stderrTail = stderr.slice(-4000);
  await writeFile(join(output, 'native-v081-evidence.json'), JSON.stringify(evidence, null, 2));
  await writeFile(
    join(output, 'native-v081-evidence.md'),
    [
      '# v0.8.1 native-debug continuation',
      '',
      `- Status: ${evidence.status} (pass=${evidence.pass})`,
      `- Host: \`${host}\` (${evidence.identities.host.sha256 || 'not hashed'})`,
      `- Protected worker: \`${protectedWorker}\` (${evidence.identities.protectedWorker.sha256 || 'not hashed'})`,
      `- Search worker: \`${searchWorker}\` (${evidence.identities.searchWorker.sha256 || 'not hashed'})`,
      `- Catalog bridge methods: ${evidence.catalog.methods || 'none'}`,
      `- Scroll audit rows: ${evidence.scrollAudit.rowCount ?? 'not reached'}`,
      `- Copied save SHA before/after: ${evidence.fixture.copiedShaBeforeAudit} / ${evidence.fixture.copiedShaAfterAudit}`,
      `- Screenshots: ${evidence.screenshots.map((shot) => shot.path.split(/[\\/]/).pop()).join(', ') || 'none'}`,
      '',
      'Bounded source/debug evidence only. This is not packaged, live-game, write, legality, or save/reload acceptance.',
      '',
    ].join('\n'),
  );
}

function pushCall(record) {
  evidence.bridgeCalls.push(record);
  if (record.method?.startsWith('runtime.')) evidence.runtimeCalls.push(record);
}

try {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    if (child.exitCode !== null) throw new Error(`Studio exited ${child.exitCode}: ${stderr}`);
    try {
      if ((await fetch(`http://127.0.0.1:${port}/json/version`)).ok) break;
    } catch {}
    await new Promise((done) => setTimeout(done, 300));
    if (attempt === 199) throw new Error('CDP endpoint did not open');
  }
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  for (let attempt = 0; attempt < 150 && !page; attempt += 1) {
    page = browser.contexts()[0]?.pages()[0];
    if (!page) await new Promise((done) => setTimeout(done, 200));
  }
  assert.ok(page, 'WebView2 opened without a page target');
  page.on('pageerror', (error) => evidence.failures.push(`pageerror: ${error.message}`));
  await page.waitForFunction(() => !!document.querySelector('#root .shell'), null, { timeout: 60000 });
  const installProbe = async () => page.evaluate(() => {
    window.__v081Verify = { calls: [], snapshots: [], auxiliary: [] };
    const execute = window.operations.execute.bind(window.operations);
    const snapshot = window.operations.snapshot.bind(window.operations);
    const auxiliary = window.review.auxiliary.bind(window.review);
    window.operations.execute = async (command) => {
      const record = { method: command?.method, params: command?.params };
      window.__v081Verify.calls.push(record);
      try {
        const result = await execute(command);
        record.result = result;
        return result;
      } catch (error) {
        record.error = String(error?.message || error);
        throw error;
      }
    };
    window.operations.snapshot = async (role, jobId) => {
      const result = await snapshot(role, jobId);
      window.__v081Verify.snapshots.push({ role, jobId, result });
      return result;
    };
    window.review.auxiliary = async (params) => {
      const record = {
        params,
        startedAt: performance.now(),
        completedAt: null,
        auditDisabledAtStart: document.querySelector('[data-testid="scroll-generation-audit"] .scroll-audit-run')?.hasAttribute('disabled') ?? null,
        auditDisabledAtComplete: null,
        error: null,
      };
      window.__v081Verify.auxiliary.push(record);
      try {
        return await auxiliary(params);
      } catch (error) {
        record.error = String(error?.message || error);
        throw error;
      } finally {
        record.completedAt = performance.now();
        record.auditDisabledAtComplete = document.querySelector('[data-testid="scroll-generation-audit"] .scroll-audit-run')?.hasAttribute('disabled') ?? null;
      }
    };
  });
  await installProbe();
  const bridgeState = () => page.evaluate(() => window.__v081Verify);
  const screenshot = async (name, locale) => {
    const path = join(output, name);
    await page.screenshot({ path });
    evidence.screenshots.push({ path, locale, viewport: await page.evaluate(() => ({ width: innerWidth, height: innerHeight })), dpr: await page.evaluate(() => devicePixelRatio) });
  };
  const localeLabels = { 'en-US': 'English', 'ja-JP': '日本語' };
  const localeTitles = { 'en-US': 'Equipment browser', 'ja-JP': '装備ブラウザ' };

  if (!scrollOnly) {
  // --- A: local catalog, real bridge, no game refresh ---
  await page.locator('.nav nav > button:nth-child(5)').click();
  await page.locator('section.equipment-page').waitFor({ timeout: 20000 });
  assert.equal((await page.locator('header.topbar h1').innerText()).trim(), '装备浏览');
  assert.equal((await bridgeState()).calls.length, 0, 'equipment page must not auto-read');
  await page.locator('.equipment-catalog-version').fill('v081-local-test');
  const validBytes = Buffer.from('{"E8F6":{"name":"Birdflight Cross Spear","type":null}}', 'utf8');
  const validSha = sha256(validBytes);
  const input = page.locator('.equipment-catalog-file input[type=file]');
  await input.setInputFiles({ name: 'items_little_endian.json', mimeType: 'application/json', buffer: validBytes });
  await page.locator('.equipment-catalog-disclosure').waitFor({ timeout: 20000 });
  let state = await bridgeState();
  const catalogCall = state.calls.find((call) => call.method === 'catalog.import_names');
  assert.ok(catalogCall, 'catalog.import_names must use the real bridge');
  assert.equal(catalogCall.params.role, 'save_active_items');
  assert.equal(catalogCall.params.source_label, 'items_little_endian.json');
  assert.equal(catalogCall.params.declared_version, 'v081-local-test');
  assert.equal(catalogCall.params.locale, 'zh-CN');
  assert.equal(Buffer.from(catalogCall.params.content_base64, 'base64').toString('utf8'), validBytes.toString('utf8'));
  assert.equal(String(catalogCall.result.source.sha256).toLowerCase(), validSha);
  assert.equal(catalogCall.result.source.bytes, validBytes.length);
  assert.equal(catalogCall.result.counts.display_rows, 1);
  assert.equal(catalogCall.result.rows[0].id, 0xf6e8);
  assert.equal(catalogCall.result.rows[0].display_id, 0xf6e8);
  const stored = await page.evaluate(() => JSON.parse(localStorage.getItem('nioh3-equipment-local-name-catalog-v1') || 'null'));
  assert.equal(String(stored.source.sha256).toLowerCase(), validSha, 'accepted catalog must persist its source hash');
  assert.equal(stored.entries[0][0], 0xf6e8);
  evidence.catalog = { methods: state.calls.map((call) => call.method).join(', '), source: catalogCall.result.source, counts: catalogCall.result.counts, acceptedId: catalogCall.result.rows[0].display_id, stored: true };
  await screenshot('catalog-zh-CN.png', 'zh-CN');

  // A malformed selected file must retain the accepted active catalog.
  const malformed = Buffer.from('{"E8F6":{"name":"broken"}', 'utf8');
  await input.setInputFiles({ name: 'items_little_endian.json', mimeType: 'application/json', buffer: malformed });
  await page.locator('.equipment-catalog-import .equipment-error').waitFor({ timeout: 20000 });
  state = await bridgeState();
  assert.equal(state.calls.filter((call) => call.method === 'catalog.import_names').length, 2);
  const retainedAfterReject = await page.evaluate(() => JSON.parse(localStorage.getItem('nioh3-equipment-local-name-catalog-v1') || 'null'));
  assert.equal(String(retainedAfterReject.source.sha256).toLowerCase(), validSha, 'rejected import must preserve active catalog');
  assert.equal(await page.locator('.equipment-catalog-disclosure').count(), 1);
  evidence.catalog.rejectedPreserved = true;

  // Reload proves browser persistence, then capture the two other native locales.
  await page.reload();
  // A full WebView reload creates a fresh JS context; reinstall the real bridge
  // probe before the post-reload search/audit flow.
  await installProbe();
  await page.locator('.nav nav > button:nth-child(5)').click();
  await page.waitForFunction(() => !!document.querySelector('section.equipment-page'), null, { timeout: 30000 });
  await page.locator('.equipment-catalog-disclosure').waitFor();
  const persistedAfterReload = await page.evaluate(() => JSON.parse(localStorage.getItem('nioh3-equipment-local-name-catalog-v1') || 'null'));
  assert.equal(String(persistedAfterReload?.source?.sha256 || '').toLowerCase(), validSha);
  evidence.catalog.persistedAfterReload = true;
  for (const locale of ['en-US', 'ja-JP']) {
    await page.locator('.language-button').click();
    await page.locator('.side-popup').getByRole('button', { name: localeLabels[locale], exact: true }).click();
    await page.waitForFunction((value) => document.documentElement.lang === value, locale);
    await page.waitForFunction((value) => document.querySelector('header.topbar h1')?.textContent?.trim() === value, localeTitles[locale]);
    await screenshot(`catalog-${locale}.png`, locale);
  }
  await page.locator('.language-button').click();
  await page.locator('.side-popup').getByRole('button', { name: '简体中文', exact: true }).click();
  await page.waitForFunction(() => document.documentElement.lang === 'zh-CN');
  await page.getByRole('button', { name: '移除本地目录', exact: true }).click();
  await page.waitForFunction(() => !localStorage.getItem('nioh3-equipment-local-name-catalog-v1'));
  evidence.catalog.cleared = true;

  const normalGeometry = await page.evaluate(() => ({ width: innerWidth, height: innerHeight, dpr: devicePixelRatio }));
  await page.evaluate(() => window.review.windowAction('maximize'));
  await new Promise((done) => setTimeout(done, 700));
  const maximizedGeometry = await page.evaluate(() => ({ width: innerWidth, height: innerHeight, dpr: devicePixelRatio }));
  await page.evaluate(() => window.review.windowAction('maximize'));
  await new Promise((done) => setTimeout(done, 700));
  const restoredGeometry = await page.evaluate(() => ({ width: innerWidth, height: innerHeight, dpr: devicePixelRatio }));
  evidence.geometry = { normal: normalGeometry, maximized: maximizedGeometry, restored: restoredGeometry };
  }

  // --- B: one D-rooted synthetic save through the real audit UI ---
  await page.locator('.nav nav > button:nth-child(1)').click();
  await page.locator('select[aria-label="周目"]').selectOption('4');
  const savePicker = page.locator('.native-search-source .save-picker');
  await savePicker.waitFor({ timeout: 20000 });
  const saveSelect = savePicker.locator('select[aria-label="自动检测的存档"]');
  await page.waitForFunction(() => {
    const select = document.querySelector('.native-search-source .save-picker select[aria-label="自动检测的存档"]');
    return select && select.value && select.options.length > 1;
  }, null, { timeout: 90000 });
  const selectedSaveId = await saveSelect.inputValue();
  assert.ok(selectedSaveId, 'synthetic save must be auto-selected');
  await page.locator('.nav nav > button:nth-child(2)').click();
  const auditButton = page.locator('[data-testid="scroll-generation-audit"] .scroll-audit-run');
  await auditButton.waitFor({ state: 'visible', timeout: 30000 });
  await page.waitForFunction(() => Array.isArray(window.__v081Verify?.auxiliary) && window.__v081Verify.auxiliary.length > 0, null, { timeout: 30000 });
  const auxiliaryCalls = await page.evaluate(() => window.__v081Verify.auxiliary);
  assert.ok(auxiliaryCalls.some((call) => call.auditDisabledAtStart === true), 'audit must be disabled when auxiliary read starts');
  const auxiliaryInFlight = auxiliaryCalls.find((call) => call.completedAt === null);
  if (auxiliaryInFlight) assert.equal(auxiliaryInFlight.auditDisabledAtStart, true, 'audit must remain disabled while auxiliary read is in flight');
  await page.waitForFunction(() => window.__v081Verify?.auxiliary?.every((call) => call.completedAt !== null), null, { timeout: 30000 });
  await page.waitForFunction(() => !document.querySelector('[data-testid="scroll-generation-audit"] .scroll-audit-run')?.hasAttribute('disabled'), null, { timeout: 30000 });
  const auxiliaryGuard = await page.evaluate(() => ({ calls: window.__v081Verify.auxiliary, auditEnabledAfterSettle: !document.querySelector('[data-testid="scroll-generation-audit"] .scroll-audit-run')?.hasAttribute('disabled') }));
  assert.ok(auxiliaryGuard.calls.length > 0 && auxiliaryGuard.calls.every((call) => call.completedAt !== null), 'auxiliary read must settle before audit starts');
  assert.equal(auxiliaryGuard.auditEnabledAfterSettle, true, 'audit must become actionable after auxiliary read settles');
  const beforeAudit = await hashFile(savePath);
  assert.equal(beforeAudit, retained.source_sha_before.toLowerCase());
  await page.locator('[data-testid="scroll-generation-audit"] .scroll-audit-run').click();
  await page.locator('[data-testid="scroll-generation-audit"] .scroll-audit-result').waitFor({ timeout: 90000 });
  assert.match(await page.locator('.scroll-audit-summary').innerText(), /insufficient_data/);
  const afterState = await bridgeState();
  evidence.bridgeCalls = afterState.calls;
  evidence.runtimeCalls = afterState.calls.filter((call) => call.method?.startsWith('runtime.'));
  const auditSnapshots = afterState.snapshots.filter((entry) => entry.result?.kind === 'save.audit_scrolls' && entry.result?.state === 'completed');
  assert.ok(auditSnapshots.length > 0, 'terminal save.audit_scrolls snapshot must be captured');
  const terminal = auditSnapshots[auditSnapshots.length - 1].result;
  const audit = terminal.result;
  assert.equal(audit.save_id, selectedSaveId);
  assert.equal(audit.status, 'insufficient_data');
  assert.ok(Array.isArray(audit.rows) && audit.rows.length > 0);
  assert.ok(audit.rows.every((row) => row.status === 'insufficient_data'));
  assert.ok(audit.rows.every((row) => row.reasons.includes('normal_input_domain_unproven')));
  assert.equal(audit.source_sha256, beforeAudit);
  const afterAudit = await hashFile(savePath);
  assert.equal(afterAudit, beforeAudit, 'audit must not mutate the copied save');
  evidence.scrollAudit = { saveId: audit.save_id, snapshotId: audit.snapshot_id, status: audit.status, sourceSha256: audit.source_sha256, rowCount: audit.rows.length, rows: audit.rows, bridgeMethods: afterState.calls.map((call) => call.method), auxiliaryGuard };
  assert.ok(!evidence.bridgeCalls.some((call) => call.method?.startsWith('runtime.')), 'native continuation must not call runtime/game methods');
  await writeFile(join(output, 'scroll-audit-e2e-audit-result.json'), JSON.stringify(audit, null, 2));
  await writeFile(join(output, 'e2e-audit-result.json'), JSON.stringify(audit, null, 2));
  await screenshot('scroll-audit-zh-CN.png', 'zh-CN');
  for (const locale of ['en-US', 'ja-JP']) {
    await page.locator('.language-button').click();
    await page.locator('.side-popup').getByRole('button', { name: localeLabels[locale], exact: true }).click();
    await page.waitForFunction((value) => document.documentElement.lang === value, locale);
    await screenshot(`scroll-audit-${locale}.png`, locale);
  }
  evidence.status = 'pass';
  evidence.pass = true;
  evidence.note = 'Native WebView2 debug-only evidence; catalog import and scroll audit are read-only and no game/runtime call was made.';
} catch (error) {
  evidence.failures.push(String(error?.stack || error?.message || error));
  if (page) {
    try {
      const debugState = await page.evaluate(() => ({
        calls: window.__v081Verify?.calls || [],
        snapshots: window.__v081Verify?.snapshots || [],
        url: location.href,
        bodyText: document.body?.innerText?.slice(0, 6000) || '',
      }));
      evidence.bridgeCalls = debugState.calls;
      evidence.runtimeCalls = debugState.calls.filter((call) => call.method?.startsWith('runtime.'));
      evidence.debugPage = debugState;
    } catch (debugError) {
      evidence.failures.push(`debug capture: ${String(debugError?.message || debugError)}`);
    }
  }
  evidence.status = 'fail';
  evidence.pass = false;
  if (page) await page.screenshot({ path: join(output, 'native-v081-failure.png') }).catch(() => {});
  process.exitCode = 1;
} finally {
  await writeEvidence().catch((error) => evidence.failures.push(`write evidence: ${String(error)}`));
  try { await page?.evaluate(() => window.review.windowAction('close')); } catch {}
  try { await browser?.close(); } catch {}
  await new Promise((done) => setTimeout(done, 1200));
  if (child.exitCode === null) child.kill();
}

console.log(JSON.stringify(evidence, null, 2));
