/** One native-debug integration acceptance for the read-only equipment page.
 *
 * Scope, deliberately narrow: launch the rebuilt DEBUG Tauri host plus the
 * protected worker against an isolated D:-rooted profile, open the equipment
 * page through the real sidebar, click the page's own refresh control exactly
 * once, and check the one returned `runtime.inventory_snapshot` envelope against
 * what the page rendered. The read is a game-memory read, so no save is opened,
 * discovered, registered or written; the only bridge call in the whole run is
 * that single refresh.
 *
 * The verifier never builds, never retries, never pages, never falls back to
 * Cheat Engine, and never touches anything but the Studio/worker children it
 * spawned itself. Output stays on the D: delivery volume.
 *
 * Enumerated failure modes live in the integration `PREPARE.md`
 * ("Verifier: how it can fail", written before this file).
 */
import { chromium } from 'playwright';
import { spawn } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { mkdir, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { join, resolve } from 'node:path';
import { createServer } from 'node:net';
import assert from 'node:assert/strict';

const DEFAULT_WORKSPACE = 'D:/Nioh3_v080_deliverables/tmp/v081-equipment-browser-verify';
const DEFAULT_OUTPUT =
  'D:/Nioh3_v080_deliverables/deliverables/v081-equipment-browser-20260921/integration/verify';
const DEFAULT_EXE = 'D:/Nioh3_v080_deliverables/build-cache/tauri-target/debug/nioh3-studio.exe';

/** Output and profile must never land in the checkout or the C: system temp. */
function onDeliveryVolume(value, label) {
  const absolute = resolve(value);
  assert.match(absolute, /^[Dd]:[\\/]/, `${label} must be on the D: delivery volume, got ${absolute}`);
  return absolute;
}

const workspace = onDeliveryVolume(process.env.NIOH3_EQUIPMENT_WORKSPACE || DEFAULT_WORKSPACE, 'workspace');
const output = onDeliveryVolume(process.env.NIOH3_EQUIPMENT_OUTPUT || DEFAULT_OUTPUT, 'output');
const executable = onDeliveryVolume(process.env.NIOH3_TAURI_EXE || DEFAULT_EXE, 'NIOH3_TAURI_EXE');

assert.ok(existsSync(executable), `the rebuilt debug host must exist: ${executable}`);
const protectedWorker = process.env.NIOH3_RUST_PROTECTED_WORKER;
assert.ok(
  protectedWorker && existsSync(protectedWorker),
  'NIOH3_RUST_PROTECTED_WORKER must name the rebuilt protected worker; without it the read would fall back to the Python worker',
);

await mkdir(join(workspace, 'profile'), { recursive: true });
await mkdir(join(workspace, 'state'), { recursive: true });
await mkdir(join(workspace, 'local'), { recursive: true });
await mkdir(output, { recursive: true });

const server = createServer();
await new Promise((done) => server.listen(0, '127.0.0.1', done));
const port = server.address().port;
await new Promise((done) => server.close(done));

const evidence = {
  scenario: 'equipment-browser-native-readonly',
  status: 'partial',
  pass: false,
  partial: true,
  debugOnly: true,
  packaged: false,
  gameAccess: 'read-only',
  runtimeCallCount: 0,
  expectations: {
    method: 'runtime.inventory_snapshot',
    params: { start: 0, limit: 64 },
    envelope: { status: 'observed', read_only: true, consistency: 'reread_equal' },
    priorSample: { item_id: '0xF6E8', level_raw: 180, plus_raw: 20 },
  },
  identities: {},
  screenshots: [],
  blocked: null,
  failures: [],
};

const child = spawn(executable, ['--user-data-dir', join(workspace, 'profile')], {
  windowsHide: true,
  stdio: ['ignore', 'pipe', 'pipe'],
  env: {
    ...process.env,
    NIOH3_TAURI_TEST_ROOT: join(workspace, 'profile'),
    NIOH3_TAURI_TEST_DEBUG_PORT: String(port),
    NIOH3_STATE_ROOT: join(workspace, 'state'),
    LOCALAPPDATA: join(workspace, 'local'),
  },
});
let stderr = '';
child.stderr.on('data', (data) => (stderr = (stderr + data).slice(-32000)));

let browser;
let page;

/** Thrown when the one read is refused by genuinely external game availability. */
class BlockedStop extends Error {}

async function writeEvidence() {
  await writeFile(join(output, 'verify-equipment-browser.json'), JSON.stringify(evidence, null, 2));
  await writeFile(
    join(output, 'verify-equipment-browser.md'),
    [
      '# Equipment browser: native-debug read-only acceptance',
      '',
      `- Status: ${evidence.status} (pass=${evidence.pass}, partial=${evidence.partial})`,
      `- Host: \`${executable}\``,
      `- Protected worker: \`${protectedWorker}\``,
      `- Bridge calls: ${evidence.runtimeCallCount} (expected exactly 1)`,
      `- Envelope: ${evidence.envelope ? JSON.stringify(evidence.envelopeSummary) : 'none'}`,
      `- Prior UI sample (0xF6E8 / 180 / +20): ${evidence.samplePresent ?? 'not reached'}`,
      `- Screenshots: ${evidence.screenshots.map((shot) => shot.path.split(/[\\/]/).pop()).join(', ') || 'none'}`,
      evidence.blocked ? `- Blocked: ${evidence.blocked.code} ${evidence.blocked.detail}` : '',
      '',
      'Bounded evidence: native WebView2 source acceptance on a debug host reading',
      'running-game memory; not packaged acceptance, not a write, and not a',
      'logical-identity claim for any item.',
      '',
    ].filter(Boolean).join('\n'),
  );
}

try {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    if (child.exitCode !== null) throw Error(`App exited ${child.exitCode}: ${stderr}`);
    try {
      if ((await fetch(`http://127.0.0.1:${port}/json/version`)).ok) break;
    } catch {}
    await new Promise((done) => setTimeout(done, 300));
  }
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  for (let attempt = 0; attempt < 150 && !page; attempt += 1) {
    page = browser.contexts()[0]?.pages()[0];
    if (!page) await new Promise((done) => setTimeout(done, 200));
  }
  assert.ok(page, 'WebView2 opened without a page target');
  // Bounded page diagnostics: a blank shell must name its own cause instead of
  // only timing out.
  const pageErrors = [];
  page.on('pageerror', (error) => pageErrors.push(`pageerror: ${error.message}`));
  page.on('console', (message) => {
    if (message.type() === 'error') pageErrors.push(`console: ${message.text()}`);
  });
  try {
    await page.waitForFunction(() => !!document.querySelector('#root .shell'), null, { timeout: 60000 });
  } catch (error) {
    evidence.pageDiagnostics = await page
      .evaluate(() => ({
        href: location.href,
        title: document.title,
        lang: document.documentElement.lang,
        rootChildren: document.getElementById('root')?.children.length ?? null,
        rootHtmlLength: document.getElementById('root')?.innerHTML.length ?? null,
        bodyTextLength: document.body?.innerText?.length ?? null,
      }))
      .catch(() => null);
    evidence.pageErrors = pageErrors.slice(0, 20);
    evidence.stderrTail = stderr.slice(-4000);
    throw error;
  }

  // Record every bridge command while still forwarding to the real backend.
  await page.evaluate(() => {
    window.__equipmentVerify = { calls: [], envelopes: [], errors: [] };
    const original = window.operations.execute.bind(window.operations);
    window.operations.execute = async (command) => {
      const record = { method: command?.method, params: command?.params };
      window.__equipmentVerify.calls.push(record);
      try {
        const result = await original(command);
        if (command?.method === 'runtime.inventory_snapshot') {
          record.envelope = result;
          window.__equipmentVerify.envelopes.push(result);
        }
        return result;
      } catch (error) {
        record.error = String(error?.message || error);
        window.__equipmentVerify.errors.push(record.error);
        throw error;
      }
    };
  });

  const recorded = () => page.evaluate(() => window.__equipmentVerify);

  // Open the page through the real sidebar; nothing else is clicked.
  await page.locator('.nav nav > button:nth-child(5)').click();
  await page.locator('section.equipment-page').waitFor({ timeout: 20000 });
  const title = (await page.locator('header.topbar h1').innerText()).trim();
  assert.equal(title, '装备浏览', 'the shell title must name the equipment page');
  evidence.identities.context = {
    title,
    lang: await page.evaluate(() => document.documentElement.lang),
    devicePixelRatio: await page.evaluate(() => window.devicePixelRatio),
    windowSize: await page.evaluate(() => ({ width: window.innerWidth, height: window.innerHeight })),
  };
  const beforeRead = await recorded();
  assert.equal(beforeRead.calls.length, 0, 'opening the page must not read by itself');

  // Exactly one explicit refresh, then wait for the page to finish painting it.
  await page.locator('button.equipment-load').click();
  await page.locator('dl.equipment-meta').waitFor({ timeout: 30000 });
  await page.waitForFunction(() => {
    const button = document.querySelector('button.equipment-load');
    return button && !button.disabled;
  });

  const afterRead = await recorded();
  evidence.runtimeCallCount = afterRead.calls.length;
  evidence.methods = afterRead.calls.map((call) => call.method);
  assert.deepEqual(
    afterRead.calls.map((call) => call.method),
    ['runtime.inventory_snapshot'],
    'the run must make exactly one operations.execute call',
  );
  assert.deepEqual(afterRead.calls[0].params, { start: 0, limit: 64 }, 'refresh params must be start 0 / limit 64');
  if (afterRead.errors.length) {
    const detail = afterRead.errors[0];
    // Only a genuine external game-availability refusal may be "blocked".
    // A private/unknown method, a contract rejection or an unexpected envelope
    // is an implementation failure and must return nonzero so it cannot read as
    // a valid unavailable case.
    const implementationFailure = /PRIVATE_OR_UNKNOWN_OPERATION|SCHEMA|CONTRACT|MALFORMED|INVALID|UNEXPECTED|DENIED|NOT_ALLOWED|ENVELOPE/i.test(detail);
    const externalUnavailable =
      !implementationFailure &&
      /GAME_EXECUTABLE_(NOT_FOUND|UNREADABLE|AMBIGUOUS)|GAME_VERSION_|PROCESS_|NOT_RUNNING|UNSUPPORTED|UNAVAILABLE/i.test(detail);
    if (externalUnavailable) {
      evidence.blocked = { code: 'game-unavailable', detail };
      evidence.status = 'blocked';
      evidence.pass = false;
      evidence.partial = false;
      await writeEvidence();
      console.log(JSON.stringify(evidence, null, 2));
      throw new BlockedStop(detail);
    }
    evidence.blocked = { code: implementationFailure ? 'implementation-failure' : 'unclassified-error', detail };
    throw new Error(`bridge error (${evidence.blocked.code}): ${detail}`);
  }

  const envelope = afterRead.envelopes[0];
  assert.ok(envelope, 'the refresh must return one envelope');
  await writeFile(join(output, 'runtime-inventory-snapshot.json'), JSON.stringify(envelope, null, 2));
  assert.equal(envelope.status, 'observed', 'envelope status');
  assert.equal(envelope.read_only, true, 'envelope read_only');
  assert.equal(envelope.consistency, 'reread_equal', 'envelope consistency');
  assert.equal(envelope.start, 0, 'envelope start');
  assert.equal(envelope.limit, 64, 'envelope limit');
  assert.ok(Array.isArray(envelope.rows), 'envelope rows must be an array');
  assert.ok(envelope.rows.length <= envelope.limit, 'rows must not exceed the requested limit');
  evidence.envelope = {
    status: envelope.status,
    read_only: envelope.read_only,
    consistency: envelope.consistency,
    start: envelope.start,
    limit: envelope.limit,
    game_version: envelope.game_version,
    observed_at: envelope.observed_at,
    observed_slot_count: envelope.observed_slot_count,
    next_start: envelope.next_start,
    rows: envelope.rows.length,
    process: envelope.process,
  };
  evidence.envelopeSummary = evidence.envelope;

  // What the page rendered for that one envelope.
  const renderedRows = await page.locator('table.equipment-table tbody tr').count();
  assert.equal(renderedRows, envelope.rows.length, 'the table must render every returned row');
  const meta = await page.evaluate(() =>
    Object.fromEntries(
      [...document.querySelectorAll('dl.equipment-meta > div')].map((row) => [
        row.querySelector('dt')?.textContent?.trim(),
        row.querySelector('dd')?.textContent?.trim(),
      ]),
    ),
  );
  assert.equal(meta['本页行数'], String(envelope.rows.length), 'meta row count');
  assert.equal(meta['原始槽位总数'], String(envelope.observed_slot_count), 'meta observed slot count');
  assert.equal(meta['只读'], '是', 'meta read-only');
  assert.equal(meta['一致性'], '二次读取一致', 'meta consistency');
  evidence.meta = meta;

  // First row selection drives the raw detail region.
  await page.locator('table.equipment-table tbody tr:first-child td:first-child > button').click();
  await page.locator('section.equipment-detail[aria-label="原始字段"]').waitFor({ timeout: 10000 });
  assert.equal(await page.locator('table.equipment-table tbody tr.selected').count(), 1, 'one selected row');
  const raw = await page.evaluate(() =>
    Object.fromEntries(
      [...document.querySelectorAll('dl.equipment-raw > div')].map((row) => [
        row.querySelector('dt')?.textContent?.trim(),
        row.querySelector('dd')?.textContent?.trim(),
      ]),
    ),
  );
  evidence.selectedRow = { raw };

  const sample = envelope.rows.find(
    (row) => row.item_id === 0xf6e8 && row.level_raw === 180 && row.plus_raw === 20,
  );
  evidence.samplePresent = Boolean(sample);
  evidence.sample = sample ? { slot: sample.slot, item_id: '0xF6E8', level_raw: sample.level_raw, plus_raw: sample.plus_raw } : null;

  // One native-window screenshot, then the three locales on the same snapshot.
  const shot = async (locale, kind, emulated = false) => {
    const path = join(output, `equipment-${kind}-${locale}.png`);
    await page.screenshot({ path });
    evidence.screenshots.push({
      path,
      locale,
      kind,
      emulated,
      viewport: await page.evaluate(() => ({ width: window.innerWidth, height: window.innerHeight })),
      dpr: await page.evaluate(() => window.devicePixelRatio),
    });
  };
  await shot('zh-CN', 'native');

  const localeLabels = { 'en-US': 'English', 'ja-JP': '日本語' };
  const localeTitles = { 'en-US': 'Equipment browser', 'ja-JP': '装備ブラウザ' };
  for (const locale of ['en-US', 'ja-JP']) {
    await page.locator('.language-button').click();
    await page.locator('.side-popup').getByRole('button', { name: localeLabels[locale], exact: true }).click();
    await page.waitForFunction((value) => document.documentElement.lang === value, locale);
    await page.waitForFunction(
      (value) => document.querySelector('header.topbar h1')?.textContent?.trim() === value,
      localeTitles[locale],
    );
    await shot(locale, 'native');
  }

  // Narrower viewport, honestly labelled: a live WebView2 target may refuse it.
  try {
    await page.setViewportSize({ width: 900, height: 900 });
    await shot('ja-JP', 'emulated-900', true);
  } catch (error) {
    evidence.screenshots.push({
      path: null,
      locale: 'ja-JP',
      kind: 'emulated-900',
      emulated: true,
      applied: false,
      reason: String(error?.message || error),
    });
  }

  await page.locator('.language-button').click();
  await page.locator('.side-popup').getByRole('button', { name: '简体中文', exact: true }).click();
  await page.waitForFunction(() => document.documentElement.lang === 'zh-CN');

  const finalCalls = await recorded();
  assert.equal(finalCalls.calls.length, 1, 'locale switches must not read again');
  evidence.runtimeCallCount = finalCalls.calls.length;

  const digest = (path) => createHash('sha256').update(readFileSync(path)).digest('hex');
  evidence.identities.host = { path: executable, sha256: digest(executable) };
  evidence.identities.protectedWorker = { path: protectedWorker, sha256: digest(protectedWorker) };
  evidence.identities.gameVersion = envelope.game_version;
  evidence.identities.process = envelope.process;

  evidence.status = evidence.samplePresent ? 'pass' : 'partial';
  evidence.pass = evidence.samplePresent;
  evidence.partial = !evidence.samplePresent;
  evidence.note = evidence.samplePresent
    ? 'Raw 0xF6E8 / level 180 / plus 20 is present, matching the earlier UI-confirmed sample. No logical-identity claim.'
    : 'The earlier 0xF6E8 / 180 / +20 sample is absent from this page; that alone is not a parser or read failure.';
  await writeEvidence();
  console.log(JSON.stringify(evidence, null, 2));
} catch (error) {
  if (error instanceof BlockedStop) {
    // The structured blocked result is already written; stop, no retry.
  } else {
    evidence.failures.push(String(error?.message || error));
    evidence.status = 'fail';
    evidence.pass = false;
    evidence.partial = false;
    if (page) {
      await page.screenshot({ path: join(output, 'equipment-failure.png') }).catch(() => {});
    }
    await writeEvidence().catch(() => {});
    console.log(JSON.stringify(evidence, null, 2));
    process.exitCode = 1;
  }
} finally {
  try {
    await page?.evaluate(() => window.review.windowAction('close'));
  } catch {}
  try {
    browser?.close();
  } catch {}
  await new Promise((done) => setTimeout(done, 1500));
  if (child.exitCode === null) child.kill();
}
