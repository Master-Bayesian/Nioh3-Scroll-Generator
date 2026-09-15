/**
 * Focused comparison probe for the cart -> plan leg of the save path.
 *
 * It runs one candidate through preview -> prepareCart with the save role served
 * by whichever worker graph the environment selects, and prints the outcome. It
 * exists so a Rust-only refusal can be compared against the shipped worker on
 * the same candidate instead of being asserted from one run.
 *
 * Usage:
 *   node apps/tauri/probe-save-graph.mjs --exe <host> [--search-worker <exe>] [--out <json>]
 */
import {chromium} from 'playwright';
import {spawn, execFileSync} from 'node:child_process';
import {mkdtemp, mkdir, writeFile} from 'node:fs/promises';
import {join, resolve} from 'node:path';
import {createServer} from 'node:net';

function parseArgs(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index];
    if (key === '--exe') options.exe = argv[++index];
    else if (key === '--search-worker') options.searchWorker = argv[++index];
    else if (key === '--package') options.package = argv[++index];
    else if (key === '--python') options.python = argv[++index];
    else if (key === '--out') options.out = argv[++index];
    else if (key === '--timeout') options.timeout = Number(argv[++index]);
    else throw new Error(`unknown argument: ${key}`);
  }
  if (!options.exe) throw new Error('--exe is required');
  return options;
}

async function freePort() {
  const server = createServer();
  await new Promise((ready) => server.listen(0, '127.0.0.1', ready));
  const port = server.address().port;
  await new Promise((ready) => server.close(ready));
  return port;
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const executable = resolve(options.exe);
  const python = resolve(
    options.python || 'F:/Nioh3_ScrollEditor/.codex_tmp/v2-build-env/Scripts/python.exe',
  );
  const timeoutMs = (options.timeout || 90) * 1000;
  const isolated = await mkdtemp(join(process.env.TEMP || '.', 'nioh3-save-graph-'));
  const profile = join(isolated, 'profile');
  const stateRoot = join(isolated, 'state');
  await mkdir(stateRoot, {recursive: true});
  execFileSync(
    python,
    [resolve('apps/desktop/tests/fixtures/create-restore-save.py'), isolated],
    {windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], timeout: 90000},
  );
  const port = await freePort();
  const environment = {
    ...process.env,
    NIOH3_TAURI_TEST_ROOT: profile,
    NIOH3_TAURI_TEST_DEBUG_PORT: String(port),
    NIOH3_STATE_ROOT: stateRoot,
    LOCALAPPDATA: join(isolated, 'local'),
    NIOH3_PYTHON: python,
  };
  if (options.searchWorker) {
    environment.NIOH3_RUST_SEARCH_WORKER = resolve(options.searchWorker);
  }
  if (options.package) {
    environment.NIOH3_TAURI_PACKAGE_ROOT = resolve(options.package);
  }
  const child = spawn(executable, ['--user-data-dir', profile], {
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
    env: environment,
  });
  let stderr = '';
  child.stderr.on('data', (chunk) => {
    stderr = (stderr + chunk).slice(-8000);
  });
  let browser;
  let page;
  const result = {searchWorker: options.searchWorker ?? null, package: options.package ?? null};
  try {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      if (child.exitCode !== null) throw new Error(`app exited: ${stderr}`);
      try {
        if ((await fetch(`http://127.0.0.1:${port}/json/version`)).ok) break;
      } catch {}
      if (Date.now() > deadline) throw new Error(`no endpoint: ${stderr}`);
      await new Promise((ready) => setTimeout(ready, 300));
    }
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
    for (let attempt = 0; attempt < 150 && !page; attempt += 1) {
      page = browser.contexts()[0]?.pages()[0];
      if (!page) await new Promise((ready) => setTimeout(ready, 200));
    }
    await page.waitForFunction(() => !!document.querySelector('#root .shell'), null, {
      timeout: timeoutMs,
    });
    const identity = await page.evaluate(() => window.nioh.handshake());
    const preview = await page.evaluate(() =>
      window.review.preview({seed: 10030609, rarity: 4, level: 180, retain: true}),
    );
    result.preview = {
      playthrough: preview.candidate.playthrough,
      recordStage: preview.candidate.record_stage,
      installable: preview.candidate.installable,
      installBlocker: preview.candidate.install_blocker,
    };
    const diagnostics = await page.evaluate(() => window.support.diagnostics());
    result.workers = diagnostics.workers.map((worker) => ({
      role: worker.role,
      backend: worker.backend ?? null,
      connection: worker.connection,
    }));
    const settle = async (job, label) => {
      const jobDeadline = Date.now() + 120000;
      while (!['completed', 'failed', 'cancelled'].includes(job.state)) {
        if (Date.now() > jobDeadline) throw new Error(`${label} stayed ${job.state}`);
        await new Promise((ready) => setTimeout(ready, 150));
        job = await page.evaluate(
          ({jobId}) => window.operations.snapshot('save', jobId),
          {jobId: job.job_id},
        );
      }
      return job;
    };
    const waitIdle = async (label, limit = 60000) => {
      const idleDeadline = Date.now() + limit;
      for (;;) {
        const current = await page.evaluate(() => window.operations.current('save'));
        if (!current || current.busy !== true) return;
        if (Date.now() > idleDeadline) throw new Error(`worker stayed busy before ${label}`);
        await new Promise((ready) => setTimeout(ready, 150));
      }
    };
    const runOperation = async (method, params) => {
      for (let attempt = 0; ; attempt += 1) {
        await waitIdle(method);
        try {
          return await settle(
            await page.evaluate(
              ({method, params}) => window.operations.execute({method, params}),
              {method, params},
            ),
            method,
          );
        } catch (error) {
          if (attempt >= 40 || !String(error).includes('BUSY')) throw error;
          await new Promise((ready) => setTimeout(ready, 200));
        }
      }
    };
    const discovery = await runOperation('save.discover', {});
    const saveId = discovery.result?.save_id || discovery.result?.saves?.[0]?.save_id;
    result.discovered = saveId ?? null;
    if (saveId) {
      const inventory = await runOperation('save.inventory', {save_id: saveId});
      const snapshotId =
        inventory.result?.snapshot_id || inventory.result?.save_id || saveId;
      result.inventory = {
        entries: (inventory.result?.entries ?? inventory.result?.inventory ?? []).length,
      };
      try {
        const planned = await page.evaluate(
          ({saveId, snapshotId, reference}) =>
            window.review.prepareCart({
              mode: 'save',
              save_id: saveId,
              snapshot_id: snapshotId,
              references: [reference],
              recommended_level: 180,
              transfer_count: 1,
            }),
          {saveId, snapshotId, reference: preview.reference_id},
        );
        const settled = await settle(planned, 'save.prepare_install_many');
        result.plan = settled.result
          ? {ok: true, keys: Object.keys(settled.result).slice(0, 8)}
          : {ok: false, error: settled.error, state: settled.state};
      } catch (error) {
        result.plan = {ok: false, error: String(error)};
      }
    }
    result.handshakeRole = identity.role;
  } finally {
    try {
      browser?.close();
    } catch {}
    await new Promise((ready) => setTimeout(ready, 1000));
    if (child.exitCode === null) child.kill();
  }
  if (options.out) {
    await writeFile(resolve(options.out), `${JSON.stringify(result, null, 2)}\n`, 'utf8');
  }
  console.log(JSON.stringify(result));
}

main().catch((error) => {
  console.error(`PROBE_FAILED: ${error.stack || error.message}`);
  process.exitCode = 1;
});
