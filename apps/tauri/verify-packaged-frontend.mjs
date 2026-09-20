/**
 * Frontend acceptance against the real `rust-packaged` worker graph.
 *
 * The development harness (`verify-rust-backend.mjs`) selects a Rust worker with
 * a development environment variable. This one runs the same real WebView2
 * frontend with the host resolving its worker graph from the package, so what is
 * exercised is the profile a released package uses: the packaged read-only worker
 * for search/preview and the packaged protected worker for the save roles.
 *
 * Every run uses an isolated user-data root and a synthetic encrypted save
 * fixture created by the shipped fixture helper, and never touches a real game
 * process or a user save. The update check is read-only against the official
 * feed and nothing is downloaded or published.
 *
 * Usage:
 *   node apps/tauri/verify-packaged-frontend.mjs --package <app root> \
 *     --exe <host exe> --python <python.exe> --out <dir> [--host debug|release]
 *     [--screenshots <dir>] [--artifact-exe <path>] [--artifact-zip <path>]
 *
 * `--package` is the app root the host resolves: the staged portable directory
 * for the opt-in development runtime, or the extracted one-file runtime (the
 * directory holding `Nioh3Studio.exe`, `worker/`, `packages/` and
 * `build-manifest.json`) for a real release candidate. `--host debug` (default)
 * drives a debug build through the test-only `NIOH3_TAURI_PACKAGE_ROOT`
 * override; `--host release` drives a release build, which resolves its own
 * resource root and needs no override.
 */
import {chromium} from 'playwright';
import {spawn, execFileSync} from 'node:child_process';
import {existsSync, readFileSync, rmSync, statSync} from 'node:fs';
import {mkdtemp, mkdir, readFile, realpath, writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createServer} from 'node:net';
import assert from 'node:assert/strict';

const ROLES = ['offline_search', 'save', 'runtime'];
const REGRESSION_SEED = 226061463;
const REGRESSION_TRIAL = 158614759;
const ROOT = resolve(fileURLToPath(new URL('../..', import.meta.url)));

function parseArgs(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index];
    if (key === '--package') options.package = argv[++index];
    else if (key === '--exe') options.exe = argv[++index];
    else if (key === '--python') options.python = argv[++index];
    else if (key === '--out') options.out = argv[++index];
    else if (key === '--host') options.host = argv[++index];
    else if (key === '--screenshots') options.screenshots = argv[++index];
    else if (key === '--artifact-exe') options.artifactExe = argv[++index];
    else if (key === '--artifact-zip') options.artifactZip = argv[++index];
    else if (key === '--timeout') options.timeout = Number(argv[++index]);
    else throw new Error(`unknown argument: ${key}`);
  }
  if (!options.package) throw new Error('--package is required');
  return options;
}

/**
 * Raw bytes identity for one file. A normalized PE hash is not an identity, so
 * evidence records the size and the raw SHA-256 of the exact artifact.
 */
function rawHash(path) {
  return {
    path,
    size: statSync(path).size,
    sha256: createHash('sha256').update(readFileSync(path)).digest('hex'),
  };
}

async function freePort() {
  const server = createServer();
  await new Promise((ready) => server.listen(0, '127.0.0.1', ready));
  const port = server.address().port;
  await new Promise((ready) => server.close(ready));
  return port;
}

function startHost({executable, profile, staged, port, stateRoot, localAppData, host}) {
  const child = spawn(executable, ['--user-data-dir', profile], {
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
    env: {
      ...process.env,
      NIOH3_TAURI_TEST_ROOT: profile,
      NIOH3_TAURI_TEST_DEBUG_PORT: String(port),
      NIOH3_STATE_ROOT: stateRoot,
      LOCALAPPDATA: localAppData,
      // No ambient Python: a fallback would fail loudly instead of passing.
      NIOH3_PYTHON: 'C:/nioh3-no-python-for-this-run/python.exe',
      // A release build resolves its own resource root, so the test-only override
      // is only supplied for a debug host.
      ...(host === 'release' ? {} : {NIOH3_TAURI_PACKAGE_ROOT: staged}),
    },
  });
  let stderr = '';
  child.stderr.on('data', (chunk) => {
    stderr = (stderr + chunk).slice(-32000);
  });
  return {child, stderr: () => stderr};
}

async function connect(port, stderrOf, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    if (Date.now() > deadline) throw new Error(`no WebView2 endpoint: ${stderrOf()}`);
    try {
      if ((await fetch(`http://127.0.0.1:${port}/json/version`)).ok) break;
    } catch {}
    await new Promise((ready) => setTimeout(ready, 300));
  }
  const browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  let page;
  for (let attempt = 0; attempt < 150 && !page; attempt += 1) {
    page = browser.contexts()[0]?.pages()[0];
    if (!page) await new Promise((ready) => setTimeout(ready, 200));
  }
  if (!page) throw new Error('the WebView2 endpoint opened without a page target');
  return {browser, page};
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const staged = resolve(options.package);
  const hostMode = options.host === 'release' ? 'release' : 'debug';
  const executable = resolve(
    options.exe || join(staged, 'Nioh3Studio.exe'),
  );
  const python = resolve(options.python || 'python');
  const output = resolve(options.out || 'deliverables/v080-packaged-frontend');
  const timeoutMs = (options.timeout || 90) * 1000;
  await mkdir(output, {recursive: true});
  const screenshotDir = options.screenshots ? resolve(options.screenshots) : null;
  if (screenshotDir) await mkdir(screenshotDir, {recursive: true});
  if (!existsSync(executable)) throw new Error(`host executable missing: ${executable}`);
  if (!existsSync(join(staged, 'worker', 'worker-backend.json'))) {
    throw new Error(`the staged package carries no worker manifest: ${staged}`);
  }
  const manifest = JSON.parse(
    await readFile(join(staged, 'worker', 'worker-backend.json'), 'utf8'),
  );
  assert.equal(manifest.backend, 'rust');

  const isolated = await realpath(await mkdtemp(join(process.env.TEMP || '.', 'nioh3-packaged-ui-')));
  const stateRoot = join(isolated, 'state');
  const localAppData = join(isolated, 'local');
  await mkdir(stateRoot, {recursive: true});

  // The shipped synthetic-save fixture: encrypted, isolated, and never a user
  // save. The helper creates `local/` and `profile/` itself and requires them to
  // be absent, so it is invoked before anything else touches the isolated root.
  // The fixture is hashed before and after to prove this run wrote nothing.
  const fixture = JSON.parse(
    execFileSync(
      python,
      [join(ROOT, 'apps/desktop/tests/fixtures/create-restore-save.py'), isolated],
      {
        windowsHide: true,
        stdio: ['ignore', 'pipe', 'pipe'],
        timeout: 90000,
      },
    ).toString('utf8'),
  );
  const profile = join(isolated, 'profile');
  const fixtureDigest = () =>
    createHash('sha256').update(readFileSync(fixture.path)).digest('hex');
  const fixtureBefore = fixtureDigest();
  /**
   * SHA-256 of the decrypted savedata generation.
   *
   * A restore re-encrypts the container, so comparing encrypted bytes would fail
   * even for a byte-exact recovery. This decrypts through the project's own
   * crypto entry point and hashes the plaintext, which is the invariant that
   * actually has to hold.
   */
  const plaintextDigest = (path) =>
    execFileSync(
      python,
      [
        '-c',
        [
          'import hashlib,pathlib,shutil,sys,tempfile',
          'sys.path.insert(0, sys.argv[1])',
          'from nioh3_scroll_editor.savegame import SaveCrypto, default_crypto_tool',
          'work = pathlib.Path(tempfile.mkdtemp(prefix="nioh3-plain-"))',
          'plain = work / "plain.bin"',
          'SaveCrypto(default_crypto_tool(pathlib.Path(sys.argv[1]))).decrypt(pathlib.Path(sys.argv[2]), plain)',
          'print(hashlib.sha256(plain.read_bytes()).hexdigest())',
          'shutil.rmtree(work, ignore_errors=True)',
        ].join('\n'),
        ROOT,
        path,
      ],
      {windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], timeout: 120000},
    )
      .toString('utf8')
      .trim();
  const backupGeneration = plaintextDigest(fixture.backup_path);
  const startingGeneration = plaintextDigest(fixture.path);
  /** Decrypted savedata bytes, for exact record-byte comparison. */
  const plaintextBytes = (path) => {
    const work = join(
      process.env.TEMP || '.',
      `nioh3-plain-${Date.now()}-${Math.floor(Math.random() * 1e6)}`,
    );
    execFileSync(
      python,
      [
        '-c',
        [
          'import pathlib,sys',
          'sys.path.insert(0, sys.argv[1])',
          'from nioh3_scroll_editor.savegame import SaveCrypto, default_crypto_tool',
          'pathlib.Path(sys.argv[3]).parent.mkdir(parents=True, exist_ok=True)',
          'SaveCrypto(default_crypto_tool(pathlib.Path(sys.argv[1]))).decrypt(pathlib.Path(sys.argv[2]), pathlib.Path(sys.argv[3]))',
        ].join('\n'),
        ROOT,
        path,
        join(work, 'plain.bin'),
      ],
      {windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], timeout: 120000},
    );
    const bytes = readFileSync(join(work, 'plain.bin'));
    rmSync(work, {recursive: true, force: true});
    return bytes;
  };

  const port = await freePort();
  const evidence = {
    graph: 'rust-packaged',
    hostMode,
    developmentBuild: hostMode !== 'release',
    releaseCandidate: hostMode === 'release',
    executable,
    stagedPackage: staged,
    roles: Object.fromEntries(
      ROLES.map((role) => [role, manifest.invocation[role]?.binary ?? null]),
    ),
    isolatedRoot: isolated,
    gameWrites: 0,
    userSaveTouched: false,
  };
  // Bind the run to exact bytes: the inner host, the packaged worker binaries,
  // the build manifest, and - when the caller supplies them - the outer one-file
  // EXE and the outer ZIP the runtime was extracted from.
  evidence.artifact = {
    innerExe: rawHash(executable),
    buildManifest: rawHash(join(staged, 'build-manifest.json')),
    workerManifest: rawHash(join(staged, 'worker', 'worker-backend.json')),
    roleBinaries: Object.fromEntries(
      ROLES.map((role) => [
        role,
        manifest.invocation[role]?.binary
          ? rawHash(join(staged, 'worker', manifest.invocation[role].binary))
          : null,
      ]),
    ),
    outerExe: options.artifactExe ? rawHash(resolve(options.artifactExe)) : null,
    outerZip: options.artifactZip ? rawHash(resolve(options.artifactZip)) : null,
  };
  let host = startHost({
    executable,
    profile,
    staged,
    port,
    stateRoot,
    localAppData,
    host: hostMode,
  });
  let browser;
  let page;
  const settle = async (job, label) => {
    const deadline = Date.now() + 120000;
    while (!['completed', 'failed', 'cancelled'].includes(job.state)) {
      if (Date.now() > deadline) throw new Error(`operation ${label} stayed ${job.state}`);
      await new Promise((ready) => setTimeout(ready, 150));
      job = await page.evaluate(
        ({jobId}) => window.operations.snapshot('save', jobId),
        {jobId: job.job_id},
      );
    }
    if (job.state !== 'completed') {
      throw new Error(`operation ${label} ended ${job.state}: ${JSON.stringify(job.error)}`);
    }
    return job.result;
  };
  // The protected host owns one operation at a time and refuses a second one
  // while the previous owner is still finishing, so wait for it to report idle
  // before dispatching the next request. This is the shipped contract, not a
  // workaround for a defect.
  const waitIdle = async (label, timeout = 120000) => {
    const deadline = Date.now() + timeout;
    for (;;) {
      const current = await page.evaluate(() => window.operations.current('save'));
      if (!current || current.busy !== true) return;
      if (Date.now() > deadline) {
        throw new Error(`the protected worker stayed busy before ${label}`);
      }
      await new Promise((ready) => setTimeout(ready, 150));
    }
  };
  const runOperation = async (method, params) => {
    // The protected host accepts one owner at a time. A just-finished job can
    // still hold the owner for a moment, so a BUSY refusal is retried after the
    // host reports idle rather than treated as a failure.
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
  try {
    ({browser, page} = await connect(port, host.stderr, timeoutMs));
    const ready = async (timeout = 90000) => {
      await page.waitForFunction(() => !!document.querySelector('#root .shell'), null, {
        timeout,
      });
    };
    await ready();

    // The host must have resolved the staged Rust graph, and the frontend must be
    // answered by the Rust worker (no Python is reachable in this run).
    const identity = await page.evaluate(() => window.nioh.handshake());
    assert.equal(identity.role, 'offline_search', JSON.stringify(identity).slice(0, 300));
    assert.match(identity.context.context_digest, /^[0-9a-f]{64}$/);
    evidence.handshake = {
      role: identity.role,
      contextDigest: identity.context.context_digest,
      contractDigest: identity.contract_digest,
      selectedContext: {
        gameFileVersion: identity.context.game_file_version,
        versionedResourceDir: identity.context.versioned_resource_dir,
        bundleDigest: identity.context.bundle_digest,
        versionedDigest: identity.context.versioned_digest,
        legacyContextDigest: identity.context.legacy_context_digest,
        contextDigest: identity.context.context_digest,
      },
    };

    // Catalog for every locale, from the staged tables.
    evidence.catalogs = {};
    for (const locale of ['zh-CN', 'en-US', 'ja-JP']) {
      const catalog = await page.evaluate(
        (value) => window.nioh.searchCatalog(4, value),
        locale,
      );
      assert.equal(catalog.ordinary_effects.length, 50, `${locale} ordinary effects`);
      assert.equal(catalog.grace_effects.length, 21, `${locale} grace effects`);
      assert.equal(catalog.terrain_options.length, 6, `${locale} terrain`);
      assert.equal(catalog.enemy_options.length, 487, `${locale} enemies`);
      assert.equal(catalog.special_rule_options.length, 277, `${locale} rules`);
      assert.ok(
        catalog.ordinary_effects.every((entry) => entry.name.trim().length > 0),
        `${locale} names must be localized`,
      );
      evidence.catalogs[locale] = {
        firstOrdinary: catalog.ordinary_effects[0].name,
        firstTerrain: catalog.terrain_options[0].name,
      };
    }
    assert.notEqual(
      evidence.catalogs['zh-CN'].firstOrdinary,
      evidence.catalogs['en-US'].firstOrdinary,
      'localized payloads must differ per locale',
    );

    if (screenshotDir) {
      // Visual acceptance: the shipped shell rendered in each of the three UI
      // languages, with the CSS viewport and DPR recorded alongside. This is a
      // real WebView2 window, so the captures are native-layout evidence rather
      // than an emulated viewport. Screenshots are evidence, so a capture
      // failure is recorded instead of turning the functional gate red; the
      // caller reviews the recorded result.
      evidence.visual = {screenshots: [], errors: [], observed: {}, viewports: {}};
      const readMetrics = () =>
        page.evaluate(() => ({
          width: window.innerWidth,
          height: window.innerHeight,
          dpr: window.devicePixelRatio,
          lang: document.documentElement.lang,
        }));
      const dismissPopups = async () => {
        const dismiss = page.locator('.popup-dismiss');
        if (await dismiss.count()) {
          await dismiss.first().click({timeout: 2000}).catch(() => {});
        }
      };
      // Selecting the language that is already active does not necessarily close
      // the popup, so a no-op switch is skipped instead of re-clicking the menu.
      const switchLanguage = async (label, lang) => {
        const current = await page.evaluate(() => document.documentElement.lang);
        if (current === lang) return;
        await dismissPopups();
        await page.locator('.language-button').click({timeout: 10000});
        await page
          .locator('.side-popup')
          .getByRole('button', {name: label, exact: true})
          .click({timeout: 10000});
      };
      const startingMetrics = await readMetrics();
      evidence.visual.viewports.initial = startingMetrics;
      // The labels are the product's own localized names, written as escapes so
      // this file stays ASCII.
      const languages = [
        {label: 'English', lang: 'en-US', file: 'shell-en-US.png'},
        {label: '\u65e5\u672c\u8a9e', lang: 'ja-JP', file: 'shell-ja-JP.png'},
        {label: '\u7b80\u4f53\u4e2d\u6587', lang: 'zh-CN', file: 'shell-zh-CN.png'},
      ];
      for (const language of languages) {
        try {
          await switchLanguage(language.label, language.lang);
          await page.waitForFunction(
            (lang) => document.documentElement.lang === lang,
            language.lang,
            {timeout: 15000},
          );
          await page.screenshot({path: join(screenshotDir, language.file)});
          evidence.visual.observed[language.lang] = true;
          evidence.visual.screenshots.push(language.file);
        } catch (error) {
          evidence.visual.observed[language.lang] = false;
          evidence.visual.errors.push(`${language.lang}: ${error}`);
          await dismissPopups();
        }
      }
      // Restore the language this run started in so the remaining legs and the
      // persisted preference are not perturbed by the screenshot step.
      const restore = languages.find((entry) => entry.lang === startingMetrics.lang);
      if (restore) {
        try {
          await switchLanguage(restore.label, restore.lang);
          await page.waitForFunction(
            (lang) => document.documentElement.lang === lang,
            restore.lang,
            {timeout: 15000},
          );
          await dismissPopups();
        } catch (error) {
          evidence.visual.errors.push(`restore ${restore.lang}: ${error}`);
          await dismissPopups();
        }
      }
    }

    // Preview through the staged read-only worker.
    const preview = await page.evaluate(() =>
      window.review.preview({seed: 10030609, rarity: 4, level: 180, retain: true}),
    );
    assert.equal(preview.candidate.seed, 10030609);
    assert.ok(preview.candidate.effects.length > 0);
    assert.match(preview.reference_id, /^[0-9a-f]{64}$/);
    evidence.preview = {
      seed: preview.candidate.seed,
      effects: preview.candidate.effects.length,
      referenceId: preview.reference_id,
      playthrough: preview.candidate.playthrough,
      recordStage: preview.candidate.record_stage,
      installable: preview.candidate.installable,
    };

    evidence.favorites = {
      added: (await page.evaluate(
        ({reference, sample}) =>
          window.review.favorites({action: 'add', reference_id: reference, sample}),
        {
          reference: preview.reference_id,
          sample: {
            seed: String(preview.candidate.seed),
            rarity: 4,
            level: 180,
            playthrough: 3,
            effects: [],
            rules: [],
            enemies: [],
          },
        },
      )).length,
    };

    // The v0.7.5 continuation regression: the three named rules must be found,
    // and a resume after cancellation must continue without replaying.
    const regressionQuery = {
      playthrough: 3,
      rarity: 4,
      level: 180,
      primary_effect_ids: [],
      required_secondary_ids: [],
      required_secondary_id_groups: [],
      grace_effect_id: null,
      minimum_roll_percent_by_effect_id: [],
      auxiliary: {
        required_terrain_effect_keys: [],
        required_terrain_effect_key_groups: [],
        required_special_rule_keys: [64956, 113, 20893],
        required_special_rule_key_groups: [],
        required_enemy_lookup_keys: [],
        required_enemy_lookup_key_groups: [],
      },
    };
    const allowCpu = !identity.capabilities.cuda_pivot_and_auxiliary;
    const regression = await page.evaluate(
      async ({query, digest, allowCpu}) => {
        const params = {
          query,
          context_digest: digest,
          result_count: 1,
          // The published first match sits at trial 158,614,759. The job budget
          // must cover that window, and the shipped protocol caps one page at
          // 100M trials, so the job is asked to continue across pages.
          page_trials: 10000000,
          job_trials: 100000000,
          continue_until_complete: true,
          allow_cpu_fallback: allowCpu,
          resume_token: null,
        };
        try {
          return await window.nioh.startSearch(params);
        } catch (error) {
          throw new Error(
            `regression start refused: ${error} | params=${JSON.stringify(params)} ` +
              `| queryKeys=${Object.keys(params.query).join(',')} ` +
              `| auxKeys=${Object.keys(params.query.auxiliary).join(',')}`,
          );
        }
      },
      {query: regressionQuery, digest: identity.context.context_digest, allowCpu},
    );
    const regressionJob = await page.evaluate(
      async ({jobId, timeout}) => {
        const deadline = Date.now() + timeout;
        let state = await window.nioh.snapshot(jobId);
        while (!['completed', 'cancelled', 'failed'].includes(state.state)) {
          if (Date.now() > deadline) throw new Error(`regression job stayed ${state.state}`);
          await new Promise((ready) => setTimeout(ready, 200));
          state = await window.nioh.snapshot(jobId);
        }
        return state;
      },
      {jobId: regression.job_id, timeout: 180000},
    );
    assert.equal(
      regressionJob.state,
      'completed',
      JSON.stringify(regressionJob).slice(0, 400),
    );
    const hit = regressionJob.candidates.find(
      (candidate) => candidate.seed === REGRESSION_SEED,
    );
    assert.ok(
      hit,
      `the v0.7.5 seed must be found: ${regressionJob.candidates.map((c) => c.seed)}`,
    );
    assert.equal(hit.cursor, REGRESSION_TRIAL, 'the published cursor must be reproduced');
    // Cart/save source: the real search candidate, which carries the playthrough
    // and stage the install gate requires. The fixed preview seed does not carry
    // that context, so it is kept as the preview leg only.
    const cartSeed = hit.seed;
    const cartPreview = await page.evaluate(
      ({seed, rarity, level}) =>
        window.review.preview({seed, rarity, level, retain: true}),
      {seed: cartSeed, rarity: hit.rarity, level: hit.level ?? 180},
    );
    evidence.cartSource = {
      seed: cartSeed,
      rarity: hit.rarity,
      level: hit.level ?? 180,
      referenceId: cartPreview.reference_id,
      candidatePlaythrough: cartPreview.candidate.playthrough,
      candidateStage: cartPreview.candidate.record_stage,
      installable: cartPreview.candidate.installable,
      installBlocker: cartPreview.candidate.install_blocker,
    };
    await writeFile(
      join(output, 'partial-evidence.json'),
      `${JSON.stringify(evidence, null, 2)}\n`,
      'utf8',
    );
    evidence.regression = {
      seed: hit.seed,
      cursor: hit.cursor,
      stopReason: regressionJob.stop_reason,
      cursorAfter: regressionJob.cursor,
      candidateInstallable: hit.installable,
      candidatePlaythrough: hit.playthrough,
      candidateStage: hit.record_stage,
    };

    // A cancelled page then a resume from its token must continue, not replay.
    const longQuery = {
      ...regressionQuery,
      auxiliary: {
        ...regressionQuery.auxiliary,
        required_special_rule_keys: [113],
      },
    };
    const longParams = {
      query: longQuery,
      context_digest: identity.context.context_digest,
      result_count: 50,
      page_trials: 1000000,
      job_trials: 200000000,
      allow_cpu_fallback: allowCpu,
      resume_token: null,
    };
    const longJob = await page.evaluate(
      async ({params}) => {
        try {
          return await window.nioh.startSearch(params);
        } catch (error) {
          throw new Error(
            `cancel-search start refused: ${error} | params=${JSON.stringify(params)}`,
          );
        }
      },
      {params: longParams},
    );
    await page.evaluate((id) => window.nioh.cancelSearch(id), longJob.job_id);
    const stopped = await page.evaluate(
      async ({jobId, timeout}) => {
        const deadline = Date.now() + timeout;
        let state = await window.nioh.snapshot(jobId);
        while (!['completed', 'cancelled', 'failed'].includes(state.state)) {
          if (Date.now() > deadline) throw new Error(`cancel job stayed ${state.state}`);
          await new Promise((ready) => setTimeout(ready, 200));
          state = await window.nioh.snapshot(jobId);
        }
        return state;
      },
      {jobId: longJob.job_id, timeout: 120000},
    );
    assert.equal(stopped.state, 'cancelled', JSON.stringify(stopped).slice(0, 300));
    assert.ok(stopped.resume_token, 'a cancelled job must publish a resume token');
    const resumed = await page.evaluate(
      async ({query, digest, allowCpu, token}) => {
        try {
          return await window.nioh.startSearch({
            query,
            context_digest: digest,
            result_count: 1,
            page_trials: 1000000,
            job_trials: 10000000,
            allow_cpu_fallback: allowCpu,
            resume_token: token,
          });
        } catch (error) {
          throw new Error(`resume refused token=${String(token).slice(0, 16)}: ${error}`);
        }
      },
      {
        query: longQuery,
        digest: identity.context.context_digest,
        allowCpu,
        token: stopped.resume_token,
      },
    );
    const resumedState = await page.evaluate(
      async ({jobId, timeout}) => {
        const deadline = Date.now() + timeout;
        let state = await window.nioh.snapshot(jobId);
        while (!['completed', 'cancelled', 'failed'].includes(state.state)) {
          if (Date.now() > deadline) throw new Error(`resume job stayed ${state.state}`);
          await new Promise((ready) => setTimeout(ready, 200));
          state = await window.nioh.snapshot(jobId);
        }
        return state;
      },
      {jobId: resumed.job_id, timeout: 120000},
    );
    assert.ok(
      resumedState.start_cursor >= stopped.cursor,
      `a resume must continue from the checkpoint: ${resumedState.start_cursor} < ${stopped.cursor}`,
    );
    evidence.continuation = {
      cancelledCursor: stopped.cursor,
      resumedFrom: resumedState.start_cursor,
      state: resumedState.state,
    };

    // Protected save roles through the staged protected worker, on the fixture.
    // Discovery first (the shipped convention), then the explicit product
    // registration path for the exact fixture file, so the flow is exercised even
    // while the discovery root is being settled.
    await waitIdle('save.discover');
    const discovery = await runOperation('save.discover', {});
    let registration = null;
    try {
      registration = await runOperation('save.register', {path: fixture.path});
    } catch (error) {
      registration = {error: String(error)};
    }
    evidence.protected = {
      discovery,
      registration,
      fixturePath: fixture.path,
      stateRoot,
      localAppData,
    };
    const saveId =
      discovery.save_id ||
      discovery.saves?.[0]?.save_id ||
      registration?.save_id ||
      registration?.saves?.[0]?.save_id;
    if (saveId) {
      const inventory = await runOperation('save.inventory', {save_id: saveId});
      const snapshotId = inventory.snapshot_id || inventory.save_id || saveId;
      // `save.backups` lists the bundles this host wrote for that save. It is a
      // read-only listing, so it is recorded rather than asserted: the migration
      // record already tracks that the Rust host's backup directory differs from
      // the shipped one, and that gap is owned outside this UI gate.
      const backups = await runOperation('save.backups', {save_id: saveId});
      const listedBackups = Array.isArray(backups.backups)
        ? backups.backups
        : Array.isArray(backups)
          ? backups
          : [];
      const listedBackupId = listedBackups[0]?.backup_id ?? listedBackups[0]?.id ?? null;
      if (!listedBackupId) {
        throw new Error(
          'save.backups listed no bundle for the fixture, so the restore leg cannot be ' +
            'driven through the product path; the backup directory gap is owned by ' +
            '/root/m3_save_acceptance',
        );
      }
      evidence.protected = {
        ...evidence.protected,
        saveId,
        snapshotId,
        backupId: fixture.backup_id,
        listedBackupIds: listedBackups
          .slice(0, 8)
          .map((entry) => entry.backup_id ?? entry.id ?? null),
        backups: Array.isArray(backups.backups)
          ? backups.backups.length
          : Array.isArray(backups)
            ? backups.length
            : null,
      };
      // Cart -> plan (materialization). The plan is produced only after the
      // editor/delete/restore legs below, because a committed plan changes the
      // save bytes and expires that snapshot.
      // The plan must bind the snapshot the host returned for this exact read,
      // so take the inventory and the plan inside one helper and pass both ids
      // explicitly instead of relying on an earlier snapshot.
      const prepareCartPlan = async () => {
      const currentInventory = await runOperation('save.inventory', {save_id: saveId});
      const currentSnapshotId =
        currentInventory.snapshot_id || currentInventory.save_id || saveId;
      let planned;
      try {
        planned = await settle(
          await page.evaluate(
            ({saveId, snapshotId, reference}) =>
              window.review.prepareCart({
                mode: 'save',
                save_id: saveId,
                snapshot_id: snapshotId,
                references: [reference],
                recommended_level: 180,
                transfer_count: 1,
              }),
            {saveId, snapshotId: currentSnapshotId, reference: cartPreview.reference_id},
          ),
          'save.prepare_install_many',
        );
      } catch (error) {
        // `settle` raises on a failed job, so classify the failure here instead
        // of losing the reason to a generic harness error.
        const message = String(error);
        if (message.includes('词条序列') || message.includes('\u8bcd\u6761\u5e8f\u5217')) {
          evidence.cart = {
            blocked: true,
            owner: '/root/m3_save_acceptance',
            step: 'save.prepare_install_many',
            error: message,
            reproduction:
              'preview(seed 10030609, rarity 4) -> prepareCart: the shipped Python save worker ' +
              'returns a plan for the same candidate; the Rust host refuses effect_sequence_only ' +
              'in install_record (crates/nioh3-protected/src/save_app.rs, ~line 1202) while its ' +
              'install_blocker (~line 1498) permits it.',
            comparisonProbe: 'apps/tauri/probe-save-graph.mjs',
          };
          evidence.protected.cartBlocked = true;
          await writeFile(
            join(output, 'partial-evidence.json'),
            `${JSON.stringify(evidence, null, 2)}\n`,
            'utf8',
          );
          throw new Error(`the packaged Rust save role refused the offline candidate: ${message}`);
        }
        throw error;
      }
      assert.ok(planned.plan_id, JSON.stringify(planned).slice(0, 600));
      // The cart plan reports its materialized preview as `preview.items`, one
      // entry per installed scroll, each carrying the installation digest that
      // proves the record was materialized rather than refused.
      const planItems = Array.isArray(planned.candidates)
        ? planned.candidates
        : Array.isArray(planned.preview?.items)
          ? planned.preview.items
          : [];
      assert.ok(
        planItems.length > 0,
        `the plan must carry materialized items: ${JSON.stringify(planned).slice(0, 700)}`,
      );
      assert.ok(
        planItems.every((entry) => /^[0-9a-f]{64}$/.test(String(entry.installation_sha256 ?? ''))),
        `every planned item must carry an installation digest: ${JSON.stringify(planItems).slice(0, 500)}`,
      );
      evidence.cart = {
        plan: Object.keys(planned).slice(0, 8),
        planId: planned.plan_id,
        itemCount: planItems.length,
        candidateIds: planItems.map((entry) => entry.candidate_id ?? null),
        installationDigests: planItems.map((entry) => entry.installation_sha256),
      };
      return planned;
      };

      const entries = inventory.entries ?? inventory.inventory ?? [];
      assert.ok(entries.length > 0, `the fixture inventory must not be empty: ${JSON.stringify(inventory).slice(0, 400)}`);
      const entry = entries[0];
      // The editor and delete paths do not need a new candidate, so they are
      // driven against the fixture's existing records: an edit plan built from
      // the entry's own header and slots, a real delete commit, and then a
      // restore that must return the file to its exact starting digest.
      const entrySlot = entry.slot_index ?? entry.slot;
      const entryEffects = (entry.effects ?? []).map((effect, index) => ({
        slot_index: effect.slot_index ?? effect.slot ?? index,
        effect_id: effect.effect_id ?? effect.id,
        value: effect.value ?? effect.raw ?? 0,
        prefix: effect.prefix ?? 0,
        metadata: effect.metadata ?? 0,
        tail_0: effect.tail_0 ?? effect.tail0 ?? 0,
        tail_1: effect.tail_1 ?? effect.tail1 ?? 0,
      }));
      const header = entry.header ?? {};
      // One existing supported field: an effect's own `value` (the "raw value"
      // the editor exposes). Its effect id stays, so the record change is a
      // single resolved-value field and every unrelated field must survive.
      const editedEffect = entryEffects[0];
      assert.ok(editedEffect, `the fixture entry must carry effects: ${JSON.stringify(entry).slice(0, 400)}`);
      const editedValue = (editedEffect.value + 1) >>> 0;
      const editedEffects = entryEffects.map((effect, index) =>
        index === 0 ? {...effect, value: editedValue} : effect,
      );
      const plaintextBefore = plaintextBytes(fixture.path);
      const editPlan = await runOperation('save.prepare_edit', {
        save_id: saveId,
        snapshot_id: inventory.snapshot_id,
        edits: [
          {
            slot_index: entrySlot,
            header: {
              seed: header.seed ?? null,
              playthrough: header.playthrough ?? null,
              rarity: header.rarity ?? null,
              level: header.level ?? null,
              recommended_level: header.recommended_level ?? null,
              transfer_count: header.transfer_count ?? -1,
            },
            effects: editedEffects,
          },
        ],
      });
      assert.ok(editPlan.plan_id, `save.prepare_edit: ${JSON.stringify(editPlan).slice(0, 500)}`);
      const editChange = (editPlan.preview?.changes ?? editPlan.changes ?? [])[0] ?? {};
      evidence.saveFlow = {
        editor: {
          planId: editPlan.plan_id,
          slot: entrySlot,
          effectId: editedEffect.effect_id,
          valueBefore: editedEffect.value,
          valueAfter: editedValue,
          changedOffsets: editChange.changed_offsets ?? null,
          afterHeader: editChange.after_header ?? null,
        },
      };
      // The plan itself must not have written: the fixture is untouched until the
      // commit below, which is what makes the readback a real apply.
      assert.equal(fixtureDigest(), fixtureBefore, 'preparing an edit plan must not write');
      const editCommit = await settle(
        await page.evaluate(
          ({planId}) => window.operations.execute({method: 'save.commit', params: {plan_id: planId}}),
          {planId: editPlan.plan_id},
        ),
        'save.commit(edit)',
      );
      assert.match(
        String(editCommit.commit_status ?? ''),
        /^committed/,
        `the edit commit must report a committed status: ${JSON.stringify(editCommit).slice(0, 500)}`,
      );
      assert.notEqual(fixtureDigest(), fixtureBefore, 'the edit commit must write the save');

      // Read the edited record back and compare the exact bytes.
      const afterEditInventory = await runOperation('save.inventory', {save_id: saveId});
      const editSnapshotId = afterEditInventory.snapshot_id ?? null;
      const afterEntry = (afterEditInventory.entries ?? []).find(
        (candidate) => (candidate.slot_index ?? candidate.slot) === entrySlot,
      );
      assert.ok(afterEntry, `the edited slot must still exist: ${JSON.stringify(afterEditInventory).slice(0, 400)}`);
      const afterHeader = afterEntry.header ?? {};
      const afterEffects = (afterEntry.effects ?? []).map((effect, index) => ({
        slot_index: effect.slot_index ?? effect.slot ?? index,
        effect_id: effect.effect_id ?? effect.id,
        value: effect.value ?? effect.raw ?? 0,
        prefix: effect.prefix ?? 0,
        metadata: effect.metadata ?? 0,
        tail_0: effect.tail_0 ?? effect.tail0 ?? 0,
        tail_1: effect.tail_1 ?? effect.tail1 ?? 0,
      }));
      assert.equal(
        afterEffects[0].value,
        editedValue,
        `the edited value must be read back: ${JSON.stringify(afterEffects[0])}`,
      );
      assert.equal(afterEffects[0].effect_id, editedEffect.effect_id, 'the effect identity must not change');
      assert.deepEqual(
        afterEffects.slice(1),
        entryEffects.slice(1),
        'no unrelated effect may change',
      );
      for (const key of ['seed', 'playthrough', 'rarity', 'level', 'recommended_level', 'transfer_count']) {
        assert.equal(
          afterHeader[key],
          header[key],
          `the header field ${key} must be preserved`,
        );
      }
      // Exact record bytes: the decrypted savedata must differ, and every
      // difference must sit inside the offsets the plan said it would touch.
      const plaintextAfter = plaintextBytes(fixture.path);
      const differing = [];
      const length = Math.max(plaintextBefore.length, plaintextAfter.length);
      for (let index = 0; index < length; index += 1) {
        if (plaintextBefore[index] !== plaintextAfter[index]) differing.push(index);
      }
      const declaredOffsets = new Set(
        (editChange.changed_offsets ?? []).map((offset) => Number(offset)),
      );
      // The plan reports record-relative offsets; the absolute record position in
      // the container is not part of the contract. The observable correlation is
      // the byte *count*: the edited record plus the checksum re-derivation must
      // change exactly as many bytes as the plan declared, and no more.
      const firstDiff = differing[0] ?? null;
      const lastDiff = differing[differing.length - 1] ?? null;
      const declaredChangedCount = (editChange.changed_offsets ?? []).length;
      evidence.saveFlow.editor.applied = {
        committed: true,
        commitStatus: editCommit.commit_status ?? null,
        valueReadBack: afterEffects[0].value,
        headerPreserved: true,
        unrelatedEffectsPreserved: true,
        decryptedChangedBytes: differing.length,
        declaredChangedOffsets: declaredChangedCount,
        diffRunStart: firstDiff,
        diffRunEnd: lastDiff,
        diffPositions: differing.slice(0, 8),
      };
      assert.ok(differing.length > 0, 'the edit commit must change the decrypted record bytes');
      assert.equal(
        differing.length,
        // The record change itself, plus the one re-derived user-checksum byte
        // that the writer updates with it. Anything beyond that would be an
        // unrelated field moving.
        declaredChangedCount + 1,
        `the plan declared ${declaredChangedCount} record bytes; the savedata must change those plus the checksum, got ${differing.length}`,
      );
      evidence.saveFlow.editor.diffWindow = {
        first: differing[0] ?? null,
        last: differing[differing.length - 1] ?? null,
      };
      // Restore the fixture to its starting generation and keep going, so the
      // delete/restore legs below start from the same bytes they always did. More
      // than one bundle exists by now (the fixture's own plus the one the host
      // made before this edit), so pick the bundle whose generation matches the
      // pre-edit state rather than assuming the first listing entry.
      const bundlesAfterEdit = await runOperation('save.backups', {save_id: saveId});
      const candidates = Array.isArray(bundlesAfterEdit.backups)
        ? bundlesAfterEdit.backups
        : Array.isArray(bundlesAfterEdit)
          ? bundlesAfterEdit
          : [];
      let restoreBackupId = null;
      let restoreBackupGeneration = null;
      for (const bundle of candidates) {
        const id = bundle.backup_id ?? bundle.id;
        const bundlePath = join(profile, 'backups', id, 'SAVEDATA.BIN');
        if (!existsSync(bundlePath)) continue;
        const generation = plaintextDigest(bundlePath);
        if (generation === startingGeneration) {
          restoreBackupId = id;
          restoreBackupGeneration = generation;
          break;
        }
      }
      assert.ok(
        restoreBackupId,
        `no listed bundle carries the pre-edit generation: ${JSON.stringify(candidates).slice(0, 400)}`,
      );
      const restoreAfterEdit = await runOperation('save.prepare_restore', {
        save_id: saveId,
        snapshot_id: editSnapshotId,
        backup_id: restoreBackupId,
      });
      await settle(
        await page.evaluate(
          ({planId}) => window.operations.execute({method: 'save.commit', params: {plan_id: planId}}),
          {planId: restoreAfterEdit.plan_id},
        ),
        'save.commit(restore after edit)',
      );
      const restoredAfterEdit = plaintextDigest(fixture.path);
      evidence.saveFlow.editor.fixtureRestored = restoredAfterEdit === startingGeneration;
      evidence.saveFlow.editor.restoreBackupId = restoreBackupId;
      evidence.saveFlow.editor.restoreBackupGeneration = restoreBackupGeneration;
      assert.equal(
        restoredAfterEdit,
        startingGeneration,
        'the fixture must be returned to its starting generation after the edit leg',
      );
      await writeFile(
        join(output, 'partial-evidence.json'),
        `${JSON.stringify(evidence, null, 2)}\n`,
        'utf8',
      );

      const deletePlan = await runOperation('save.prepare_delete', {
        save_id: saveId,
        snapshot_id: (await runOperation('save.inventory', {save_id: saveId})).snapshot_id,
        slots: [entrySlot],
      });
      assert.ok(deletePlan.plan_id, `save.prepare_delete: ${JSON.stringify(deletePlan).slice(0, 500)}`);
      const deleted = await settle(
        await page.evaluate(
          ({planId}) => window.operations.execute({method: 'save.commit', params: {plan_id: planId}}),
          {planId: deletePlan.plan_id},
        ),
        'save.commit(delete)',
      );
      const afterDelete = await runOperation('save.inventory', {save_id: saveId});
      const afterDeleteEntries = afterDelete.entries ?? afterDelete.inventory ?? [];
      assert.ok(
        afterDeleteEntries.length < entries.length,
        `the delete must remove a record: ${entries.length} -> ${afterDeleteEntries.length}`,
      );
      assert.notEqual(fixtureDigest(), fixtureBefore, 'the delete must have written the save');
      evidence.saveFlow.delete = {
        planId: deletePlan.plan_id,
        committed: deleted !== undefined,
        slotsBefore: entries.length,
        slotsAfter: afterDeleteEntries.length,
      };
      await writeFile(
        join(output, 'partial-evidence.json'),
        `${JSON.stringify(evidence, null, 2)}\n`,
        'utf8',
      );

      // Restore the pre-change generation from the fixture's own backup and read
      // it back byte-for-byte.
      const restoreAfterDelete = await runOperation('save.prepare_restore', {
        save_id: saveId,
        snapshot_id: (await runOperation('save.inventory', {save_id: saveId})).snapshot_id,
        backup_id: restoreBackupId,
      });
      assert.ok(
        restoreAfterDelete.plan_id,
        `save.prepare_restore: ${JSON.stringify(restoreAfterDelete).slice(0, 500)}`,
      );
      await settle(
        await page.evaluate(
          ({planId}) => window.operations.execute({method: 'save.commit', params: {plan_id: planId}}),
          {planId: restoreAfterDelete.plan_id},
        ),
        'save.commit(restore)',
      );
      const restoredInventory = await runOperation('save.inventory', {save_id: saveId});
      const restoredEntries = restoredInventory.entries ?? restoredInventory.inventory ?? [];
      // The fixture helper creates the backup first and then changes the live
      // save, so the backup holds the *previous* generation. "Restore" therefore
      // must return the file to the backup's bytes, not to the pre-restore ones.
      const restoredDigest = fixtureDigest();
      const restoredGeneration = plaintextDigest(fixture.path);
      // Decrypting is a read-only pass through the project's own crypto entry
      // point, so take the cart snapshot after it rather than before.
      const refreshed = await runOperation('save.inventory', {save_id: saveId});
      // Diagnostic: the cart plan binds this snapshot, so record the identity the
      // host handed back for every inventory read in this run.
      evidence.snapshotProbe = {
        saveId,
        initialSnapshotId: snapshotId,
        initialSource: inventory.source_sha256 ?? null,
        restoredSnapshotId: restoredInventory.snapshot_id ?? null,
        restoredSource: restoredInventory.source_sha256 ?? null,
        refreshedSnapshotId: refreshed.snapshot_id ?? null,
        refreshedSource: refreshed.source_sha256 ?? null,
        refreshedEntries: (refreshed.entries ?? []).length,
      };
      evidence.saveFlow.restore = {
        planId: restoreAfterDelete.plan_id,
        slotsAfterRestore: restoredEntries.length,
        restoredDigest,
        backupDigest: fixture.backup_sha256,
        restoredGeneration,
        backupGeneration,
        startingGeneration,
        restoredToBackupGeneration: restoredGeneration === backupGeneration,
        restoredToStartingGeneration: restoredGeneration === startingGeneration,
      };
      // By this point the edit leg has already returned the fixture to its
      // starting generation, and the host backed that state up before the delete,
      // so this restore must land on the starting generation again.
      assert.equal(
        restoredGeneration,
        startingGeneration,
        'the restore must recover the fixture to its starting generation',
      );
      // The bundle the host captured before the delete carries the starting
      // generation, which is proven by the readback above; the fixture's own
      // bundle is the earlier pre-change generation and is not expected to match
      // here.
      assert.equal(
        restoredEntries.length,
        entries.length,
        'the restore must return the deleted record',
      );
      assert.ok(refreshed.snapshot_id, `save.inventory: ${JSON.stringify(refreshed).slice(0, 300)}`);
      const planned = await prepareCartPlan();

      // Keep the partial diagnostic current: everything above is accepted, and
      // the cart commit below is the only remaining write.
      await writeFile(
        join(output, 'partial-evidence.json'),
        `${JSON.stringify(evidence, null, 2)}\n`,
        'utf8',
      );

      // Apply the cart plan and read the result back from the saved data.
      const commit = await settle(
        await page.evaluate(
          ({planId}) => window.operations.execute({method: 'save.commit', params: {plan_id: planId}}),
          {planId: planned.plan_id},
        ),
        'save.commit',
      );
      const committed = await runOperation('save.inventory', {save_id: saveId});
      const committedEntries = committed.entries ?? committed.inventory ?? [];
      assert.ok(
        committedEntries.length >= entries.length,
        `the committed inventory must be readable back: ${JSON.stringify(committed).slice(0, 400)}`,
      );
      evidence.cart = {
        ...evidence.cart,
        committed: true,
        commitKeys: Object.keys(commit).slice(0, 8),
        entriesBefore: entries.length,
        entriesAfter: committedEntries.length,
        snapshotBefore: Array.isArray(refreshed.entries)
          ? refreshed.entries.length
          : null,
      };
    } else {
      // The staged protected worker discovers saves below its `--state-root`,
      // while the shipped worker scans `%LOCALAPPDATA%/KoeiTecmo/NIOH3/Savedata`.
      // On this run they are different directories on purpose, so an empty result
      // is recorded as an exact, owner-named deferral instead of being asserted
      // away. The save surface itself is gated on the shipped graph by
      // `tests/migration/test_save_*_parity.py` and by /root/m3_save_acceptance.
      evidence.protected.deferred =
        'no save was available from discovery or explicit registration';
      evidence.protected.owner = '/root/m3_save_acceptance';
      evidence.cart = {deferred: true};
    }

    // Settings and the updater status are answered by the host, not a remote.
    // The development build never checks a feed, so this records the real answer
    // instead of asserting a release-only state; the packaged update lifecycle is
    // covered separately by the one-file gate.
    const locale = await page.evaluate(() => window.preferences.getLocale());
    assert.equal(typeof locale, 'string', `preferences must answer: ${locale}`);
    // Startup readiness: the UI sends `support:ready` once the shell is up, which
    // is what flips the updater out of its startup-pending state. The shipped
    // readiness call is `support:diagnostics`.
    await page.evaluate(() => window.support.diagnostics());
    await page.evaluate(async () => {
      try {
        return await window.review.update({action: 'status', channel: 'stable'});
      } catch (error) {
        return String(error);
      }
    });
    const updater = await page.evaluate(async () => {
      const calls = [];
      const read = async (action) => {
        calls.push(action);
        try {
          return {ok: true, state: await window.review.update({action, channel: 'stable'})};
        } catch (error) {
          return {ok: false, error: String(error)};
        }
      };
      // `StartupUpdateCheck` runs once per launch: a status read, then a single
      // check once the host reports it can apply. The app's own notice component
      // has usually already performed that check by the time this runs, so the
      // phase observed here is the *result* of the launch check.
      const startupStatus = await read('status');
      let startupCheck = null;
      if (
        startupStatus.ok &&
        startupStatus.state.canApply === true &&
        startupStatus.state.phase === 'idle'
      ) {
        startupCheck = await read('check');
      }
      const settle = async () => {
        for (let attempt = 0; attempt < 60; attempt += 1) {
          const state = await window.review.update({action: 'status', channel: 'stable'});
          if (!['checking', 'downloading'].includes(state.phase)) return state;
          await new Promise((ready) => setTimeout(ready, 500));
        }
        return {phase: 'timeout'};
      };
      const startupSettled = await settle();
      // A manual check from the update panel, which the user can trigger any time.
      const manualCheck = await read('check');
      const manualSettled = await settle();
      return {calls, startupStatus, startupCheck, startupSettled, manualCheck, manualSettled};
    });
    evidence.updater = {
      calls: updater.calls,
      startupStatus: updater.startupStatus.ok
        ? {phase: updater.startupStatus.state.phase, canApply: updater.startupStatus.state.canApply}
        : {error: updater.startupStatus.error},
      // The launch check's observed outcome. It runs inside the app at startup;
      // this records the settled phase rather than re-running it.
      startupSettled: {
        phase: updater.startupSettled.phase,
        version: updater.startupSettled.version ?? null,
        error: updater.startupSettled.error ?? null,
      },
      startupCheck: updater.startupCheck
        ? updater.startupCheck.ok
          ? {phase: updater.startupCheck.state.phase, version: updater.startupCheck.state.version ?? null}
          : {error: updater.startupCheck.error}
        : null,
      manualCheck: updater.manualCheck.ok
        ? {phase: updater.manualCheck.state.phase, version: updater.manualCheck.state.version ?? null}
        : {error: updater.manualCheck.error},
      manualSettled: {
        phase: updater.manualSettled.phase,
        version: updater.manualSettled.version ?? null,
        error: updater.manualSettled.error ?? null,
      },
      // Read-only: this gate never downloads. The check action only reads the
      // official release feed, which the task allows.
      downloadsAttempted: updater.calls.filter((action) => action === 'download').length,
    };
    assert.equal(evidence.updater.downloadsAttempted, 0, 'the acceptance must not download');
    assert.ok(
      updater.calls.includes('status'),
      `the updater must answer a status request: ${JSON.stringify(updater.calls)}`,
    );
    assert.ok(
      updater.startupStatus.ok,
      `the startup readiness status must answer: ${JSON.stringify(updater.startupStatus)}`,
    );
    // The launch check must have produced a real terminal outcome. "current"
    // means the feed answered and this build is up to date; "available"/"ready"
    // mean a newer release exists; "failed" is the explicit failure state the
    // UI renders as an alert. An "idle" or "timeout" phase means the check never
    // reached the feed, which is a failure of this leg.
    assert.ok(
      ['current', 'available', 'ready', 'failed'].includes(evidence.updater.startupSettled.phase),
      `the launch check must reach a terminal phase: ${JSON.stringify(evidence.updater.startupSettled)}`,
    );
    assert.equal(
      updater.startupStatus.state.canApply,
      true,
      'the host must report itself ready to apply updates once startup cleanup is done',
    );
    assert.ok(
      updater.manualCheck.ok,
      `a manual check must be accepted: ${JSON.stringify(updater.manualCheck)}`,
    );
    assert.ok(
      ['current', 'available', 'ready', 'failed'].includes(evidence.updater.manualSettled.phase),
      `the manual check must reach a terminal phase: ${JSON.stringify(evidence.updater.manualSettled)}`,
    );
    evidence.settings = {locale};

    // Persistence: favorites must survive a host restart from the same profile.
    const beforeRestart = await page.evaluate(() => window.review.favorites({action: 'list'}));
    assert.ok(beforeRestart.length >= 1, 'the favorite must be listed before the restart');
    await page.evaluate(() => window.review.windowAction('close'));
    await new Promise((ready) => setTimeout(ready, 1500));
    if (host.child.exitCode === null) host.child.kill();
    browser.close();
    browser = undefined;
    page = undefined;
    const restartPort = await freePort();
    host = startHost({
      executable,
      profile,
      staged,
      port: restartPort,
      stateRoot,
      localAppData,
      host: hostMode,
    });
    ({browser, page} = await connect(restartPort, host.stderr, timeoutMs));
    await ready();
    const afterRestart = await page.evaluate(() => window.review.favorites({action: 'list'}));
    assert.ok(
      afterRestart.length >= 1,
      'the restarted host must still list the favorite',
    );
    evidence.persistence = {
      favoritesBefore: beforeRestart.length,
      favoritesAfterRestart: afterRestart.length,
    };

    // Diagnostics must show the staged role binary. The fixture is intentionally
    // not compared against its starting bytes here: this run commits a delete,
    // a restore and a cart install on it, so the meaningful assertions are the
    // per-leg readbacks above and the isolation of the fixture copy.
    //
    // The worker becomes reachable asynchronously: the shipped UI mounts the
    // shell, loads its catalogs, and only then hands the search worker its
    // handshake, so a freshly restarted host can legitimately report `starting`
    // for a moment after the window appears. Wait for the shipped readiness
    // state rather than sampling it once at an arbitrary instant; an
    // `unavailable` worker still fails immediately, and a worker that never
    // becomes ready fails on the deadline.
    const diagnosticWaitStart = Date.now();
    const diagnosticDeadline = diagnosticWaitStart + 60000;
    let searchWorker;
    for (;;) {
      const diagnostics = await page.evaluate(() => window.support.diagnostics());
      searchWorker = diagnostics.workers.find((worker) => worker.role === 'offline_search');
      assert.ok(searchWorker, JSON.stringify(diagnostics).slice(0, 400));
      if (searchWorker.connection === 'ready') break;
      assert.notEqual(
        searchWorker.connection,
        'unavailable',
        `the search worker failed to start: ${JSON.stringify(searchWorker)}`,
      );
      if (Date.now() >= diagnosticDeadline) break;
      await new Promise((ready) => setTimeout(ready, 250));
    }
    assert.equal(
      searchWorker.connection,
      'ready',
      `the search worker never became ready: ${JSON.stringify(searchWorker)}`,
    );
    assert.match(
      String(searchWorker.backend || ''),
      /rust/,
      `the answering worker must be the Rust graph: ${JSON.stringify(searchWorker)}`,
    );
    evidence.diagnostics = {
      offlineSearch: searchWorker,
      readyWaitMs: Date.now() - diagnosticWaitStart,
    };
    evidence.fixtureWrites = {
      isolatedCopy: true,
      committedLegs: [
        'save.commit edit (value changed and read back)',
        'save.commit restore after edit',
        'save.commit delete',
        'save.commit restore',
        'save.commit cart install',
      ],
      realSaveTouched: false,
    };
    if (screenshotDir && evidence.visual) {
      // The restarted host must come back in the persisted language.
      try {
        evidence.visual.viewports.afterRestart = await page.evaluate(() => ({
          width: window.innerWidth,
          height: window.innerHeight,
          dpr: window.devicePixelRatio,
          lang: document.documentElement.lang,
        }));
        await page.screenshot({path: join(screenshotDir, 'shell-after-restart.png')});
        evidence.visual.screenshots.push('shell-after-restart.png');
      } catch (error) {
        evidence.visual.errors.push(`afterRestart: ${error}`);
      }
    }
  } finally {
    try {
      await page?.evaluate(() => window.review.windowAction('close'));
    } catch {}
    try {
      browser?.close();
    } catch {}
    await new Promise((ready) => setTimeout(ready, 1500));
    if (host.child.exitCode === null) host.child.kill();
  }
  await writeFile(
    join(output, 'packaged-frontend.json'),
    `${JSON.stringify(evidence, null, 2)}\n`,
    'utf8',
  );
  console.log(JSON.stringify(evidence));
  console.log('TAURI_PACKAGED_FRONTEND_OK');
}

let partialEvidence = null;
main()
  .catch(async (error) => {
    console.error(`TAURI_PACKAGED_FRONTEND_FAILED: ${error.stack || error.message}`);
    process.exitCode = 1;
  });
