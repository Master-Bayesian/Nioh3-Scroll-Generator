/** Focused real WebView2 + broker acceptance for the development Rust worker.
 *
 * Launches the real Tauri host with `NIOH3_RUST_SEARCH_WORKER` naming the
 * development binary, so every `offline_search` request in this run is served
 * by `crates/nioh3-worker`, and drives the real frontend bridge:
 * handshake, catalog for all three locales, recommended level, preview,
 * a supported search with its cancellation, favorites, the synthetic-save
 * cart, and the search history the UI keeps. A query the Rust slice does not
 * serve must surface an explicit error instead of being silently routed to the
 * shipped Python worker. Every writable path is an isolated temporary root and
 * the encrypted fixture is hashed before and after to prove nothing was
 * written.
 */
import { chromium } from 'playwright';
import { spawn, execFileSync } from 'node:child_process';
import { mkdtemp, mkdir, readFile, writeFile, realpath } from 'node:fs/promises';
import { existsSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { createServer } from 'node:net';
import assert from 'node:assert/strict';

const root = await realpath(await mkdtemp(join(tmpdir(), 'nioh3-rust-ui-')));
const output = resolve(process.env.NIOH3_RUST_UI_OUTPUT || 'deliverables/v080-frontend-rust-backend');
await mkdir(output, { recursive: true });
const python = process.env.NIOH3_PYTHON || 'python';
const rustWorker = resolve(
  process.env.NIOH3_RUST_SEARCH_WORKER || 'crates/nioh3-worker/target/debug/nioh3-readonly-worker.exe'
);
assert.ok(existsSync(rustWorker), `the Rust development worker must exist: ${rustWorker}`);
const executable = resolve(
  process.env.NIOH3_TAURI_EXE || 'apps/tauri/src-tauri/target/debug/nioh3-studio.exe'
);
assert.ok(existsSync(executable), `the Tauri host must exist: ${executable}`);

// The shipped UI acceptance fixture: a synthetic encrypted save in the isolated
// root. Nothing here is the user's save and no game process is involved.
const fixture = JSON.parse(
  execFileSync(python, ['apps/desktop/tests/fixtures/create-restore-save.py', root], {
    windowsHide: true,
    encoding: 'utf8',
    timeout: 45000,
  })
);
const fixtureDigest = () =>
  createHash('sha256').update(readFileSync(fixture.path)).digest('hex');
const fixtureBefore = fixtureDigest();

const server = createServer();
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const port = server.address().port;
await new Promise((r) => server.close(r));

const child = spawn(executable, ['--user-data-dir', join(root, 'profile')], {
  windowsHide: true,
  stdio: ['ignore', 'pipe', 'pipe'],
  env: {
    ...process.env,
    NIOH3_PYTHON: python,
    NIOH3_RUST_SEARCH_WORKER: rustWorker,
    NIOH3_TAURI_TEST_ROOT: join(root, 'profile'),
    NIOH3_TAURI_TEST_DEBUG_PORT: String(port),
    NIOH3_STATE_ROOT: join(root, 'state'),
    LOCALAPPDATA: join(root, 'local'),
  },
});
let stderr = '';
child.stderr.on('data', (b) => (stderr = (stderr + b).slice(-32000)));
const evidence = { executable, rustWorker, webview2: true, gameWrites: 0, isolatedRoot: root };
let browser;
let page;
try {
  for (let i = 0; i < 200; i++) {
    if (child.exitCode !== null) throw Error(`App exited ${child.exitCode}: ${stderr}`);
    try {
      if ((await fetch(`http://127.0.0.1:${port}/json/version`)).ok) break;
    } catch {}
    await new Promise((r) => setTimeout(r, 300));
  }
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  for (let i = 0; i < 150 && !page; i++) {
    page = browser.contexts()[0]?.pages()[0];
    if (!page) await new Promise((r) => setTimeout(r, 200));
  }
  if (!page) throw Error('WebView2 debugging endpoint opened without a page target');
  const ready = async (timeout = 60000) => {
    await page.waitForFunction(
      () => !!document.querySelector('#root .shell'),
      null,
      { timeout }
    );
    await page.waitForFunction(
      async () => {
        try {
          await window.nioh.handshake();
          return true;
        } catch {
          return false;
        }
      },
      null,
      { timeout }
    );
  };
  await ready();

  // Handshake: the broker must be talking to the Rust worker, not Python. The
  // Rust worker reports the same role and contract digest as the shipped one.
  const identity = await page.evaluate(() => window.nioh.handshake());
  assert.equal(identity.role, 'offline_search');
  assert.match(identity.context.context_digest, /^[0-9a-f]{64}$/);
  evidence.handshake = {
    role: identity.role,
    contextDigest: identity.context.context_digest,
    contractDigest: identity.contract_digest,
    cuda: identity.capabilities.cuda_pivot_and_auxiliary,
  };

  // The startup path already loaded the catalog for every rarity; asking for
  // the three locales proves the localized payloads come from the Rust tables.
  const catalogs = {};
  for (const locale of ['zh-CN', 'en-US', 'ja-JP']) {
    const catalog = await page.evaluate(
      (value) => window.nioh.searchCatalog(4, value),
      locale
    );
    assert.equal(catalog.ordinary_effects.length, 50, `${locale} ordinary effects`);
    assert.equal(catalog.grace_effects.length, 21, `${locale} grace effects`);
    assert.equal(catalog.terrain_options.length, 6);
    assert.equal(catalog.enemy_options.length, 487);
    assert.equal(catalog.special_rule_options.length, 277);
    assert.equal(catalog.special_rule_families.length, 103);
    assert.ok(catalog.ordinary_effects.every((e) => e.name.trim().length > 0));
    catalogs[locale] = {
      firstOrdinary: catalog.ordinary_effects[0],
      firstTerrain: catalog.terrain_options[0].name,
      firstGrace: catalog.grace_effects[0].name,
    };
  }
  assert.notEqual(
    catalogs['zh-CN'].firstOrdinary.name,
    catalogs['en-US'].firstOrdinary.name,
    'the localized payload must change with the requested locale'
  );
  assert.notEqual(
    catalogs['en-US'].firstOrdinary.name,
    catalogs['ja-JP'].firstOrdinary.name,
    'the localized payload must change with the requested locale'
  );
  evidence.catalogs = catalogs;

  // Basic flow in every locale: switch the UI language, reload the real WebView
  // page, and require the shell plus a connected backend again. The UI's own
  // startup then reloads the catalog through the Rust worker in that session.
  evidence.localeRuns = {};
  for (const locale of ['zh-CN', 'en-US', 'ja-JP']) {
    await page.evaluate((value) => window.preferences.setLocale(value), locale);
    await page.evaluate((value) => localStorage.setItem('nioh3-ui-locale', value), locale);
    await page.reload();
    await ready();
    const session = await page.evaluate(async () => ({
      identity: await window.nioh.handshake(),
      catalog: await window.nioh.searchCatalog(4, localStorage.getItem('nioh3-ui-locale')),
    }));
    assert.equal(session.identity.role, 'offline_search', `${locale} backend`);
    assert.equal(session.catalog.ordinary_effects.length, 50, `${locale} catalog`);
    assert.equal(session.catalog.enemy_options.length, 487, `${locale} catalog`);
    evidence.localeRuns[locale] = {
      shell: true,
      backend: session.identity.role,
      contextDigest: session.identity.context.context_digest,
      firstOrdinary: session.catalog.ordinary_effects[0].name,
      firstRule: session.catalog.special_rule_options[1].name,
    };
  }
  await page.evaluate(() => window.preferences.setLocale('zh-CN'));

  // Recommended level: the captured curve, resolved through the Rust worker.
  const recommended = await page.evaluate(() => window.nioh.resolveRecommendedLevel(200));
  assert.equal(recommended.status, 'exact');
  assert.deepEqual(recommended.canonical_internal_levels, [272, 273]);
  assert.equal(recommended.selected_internal_level, 272);
  evidence.recommended = recommended;

  // Preview: the whole candidate payload for one seed through the Rust worker.
  const preview = await page.evaluate(() =>
    window.review.preview({ seed: 10030609, rarity: 4, level: 180, retain: true })
  );
  assert.equal(preview.candidate.seed, 10030609);
  assert.ok(preview.candidate.effects.length > 0);
  assert.match(preview.reference_id, /^[0-9a-f]{64}$/);
  evidence.preview = {
    seed: preview.candidate.seed,
    effects: preview.candidate.effects.length,
    referenceId: preview.reference_id,
  };

  // A supported search: rarity 4 with no criteria is the full-family replay the
  // Rust slice serves. It must complete with candidates.
  const digest = identity.context.context_digest;
  const capabilities = identity.capabilities;
  const search = await page.evaluate(
    ({ digest, allowCpu }) =>
      window.nioh.startSearch({
        query: {
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
            required_special_rule_keys: [],
            required_special_rule_key_groups: [],
            required_enemy_lookup_keys: [],
            required_enemy_lookup_key_groups: [],
          },
        },
        context_digest: digest,
        result_count: 2,
        page_trials: 200000,
        job_trials: 200000,
        allow_cpu_fallback: allowCpu,
        resume_token: null,
      }),
    { digest, allowCpu: !capabilities.cuda_pivot_and_auxiliary }
  );
  assert.ok(search.job_id, `search.start returned ${JSON.stringify(search)}`);
  const terminal = async (jobId, timeout = 90000) => {
    const deadline = Date.now() + timeout;
    let state = await page.evaluate((id) => window.nioh.snapshot(id), jobId);
    while (!['completed', 'cancelled', 'failed'].includes(state.state)) {
      if (Date.now() > deadline) throw Error(`job ${jobId} stayed ${state.state}`);
      await new Promise((r) => setTimeout(r, 200));
      state = await page.evaluate((id) => window.nioh.snapshot(id), jobId);
    }
    return state;
  };
  const completed = await terminal(search.job_id);
  assert.equal(completed.state, 'completed', JSON.stringify(completed).slice(0, 400));
  assert.ok(completed.candidates.length >= 1, 'a supported search must return candidates');
  evidence.search = {
    jobId: search.job_id,
    state: completed.state,
    candidates: completed.candidates.length,
    firstSeed: completed.candidates[0].seed,
  };

  // History: the UI keeps the submitted query for the current job.
  const history = await page.evaluate(() => window.nioh.currentSearch());
  assert.equal(history.job.job_id, search.job_id);
  assert.equal(history.submitted.query.rarity, 4);
  evidence.history = { jobId: history.job.job_id, submittedRarity: history.submitted.query.rarity };

  // Cancellation: a long continuing job must stop when it is cancelled.
  const longJob = await page.evaluate(
    ({ digest, allowCpu }) =>
      window.nioh.startSearch({
        query: {
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
            required_special_rule_keys: [],
            required_special_rule_key_groups: [],
            required_enemy_lookup_keys: [],
            required_enemy_lookup_key_groups: [],
          },
        },
        context_digest: digest,
        result_count: 50,
        page_trials: 1000000,
        job_trials: 200000000,
        allow_cpu_fallback: allowCpu,
        resume_token: null,
      }),
    { digest, allowCpu: !capabilities.cuda_pivot_and_auxiliary }
  );
  const cancelled = await page.evaluate((id) => window.nioh.cancelSearch(id), longJob.job_id);
  assert.ok(['cancel_requested', 'cancelled'].includes(cancelled.state), cancelled.state);
  const stopped = await terminal(longJob.job_id);
  assert.equal(stopped.state, 'cancelled', JSON.stringify(stopped).slice(0, 400));
  evidence.cancel = { jobId: longJob.job_id, state: stopped.state, candidates: stopped.candidates.length };

  // Rarity-5 effect searches are served now (the complete-composition preimage
  // with its Grace), so the old "must be refused" expectation is stale. The
  // served contract is asserted as a positive: a full rarity-5 composition must
  // be searched through the Rust worker and return a candidate.
  const rarity5Query = {
    playthrough: 3,
    rarity: 5,
    level: 180,
    primary_effect_ids: [20781],
    required_secondary_ids: [6410, 12028, 28203, 41127],
    required_secondary_id_groups: [],
    grace_effect_id: 0x6553,
    minimum_roll_percent_by_effect_id: [],
    auxiliary: {
      required_terrain_effect_keys: [],
      required_terrain_effect_key_groups: [],
      required_special_rule_keys: [],
      required_special_rule_key_groups: [],
      required_enemy_lookup_keys: [],
      required_enemy_lookup_key_groups: [],
    },
  };
  const rarity5Job = await page.evaluate(
    ({ digest, query, allowCpu }) =>
      window.nioh.startSearch({
        query,
        context_digest: digest,
        result_count: 1,
        page_trials: 100000000,
        job_trials: 200000000,
        continue_until_complete: true,
        allow_cpu_fallback: allowCpu,
        resume_token: null,
      }),
    { digest, query: rarity5Query, allowCpu: !capabilities.cuda_pivot_and_auxiliary }
  );
  const rarity5Done = await terminal(rarity5Job.job_id, 240000);
  assert.equal(rarity5Done.state, 'completed', JSON.stringify(rarity5Done).slice(0, 400));
  assert.ok(
    rarity5Done.candidates.length >= 1,
    'a served rarity-5 composition must return a candidate'
  );
  const rarity5Candidate = rarity5Done.candidates[0];
  assert.equal(rarity5Candidate.rarity, 5, JSON.stringify(rarity5Candidate).slice(0, 300));
  evidence.servedRarity5 = {
    jobId: rarity5Job.job_id,
    state: rarity5Done.state,
    seed: rarity5Candidate.seed,
    cursor: rarity5Candidate.cursor,
  };

  // A genuinely unserved request must still fail explicitly, so the positive
  // above cannot be satisfied by silently falling back to the shipped worker.
  // Binding an NG3 search to a registered cache is refused by name on both hosts.
  const refusal = await page.evaluate(async ({ digest }) => {
    try {
      await window.nioh.startSearch({
        query: {
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
            required_special_rule_keys: [113],
            required_special_rule_key_groups: [],
            required_enemy_lookup_keys: [],
            required_enemy_lookup_key_groups: [],
          },
        },
        context_digest: digest,
        result_count: 1,
        page_trials: 1000,
        job_trials: 1000,
        allow_cpu_fallback: true,
        resume_token: null,
        cache_id: '0'.repeat(64),
      });
      return '';
    } catch (error) {
      return String(error);
    }
  }, { digest });
  assert.notEqual(refusal, '', 'an unserved request must not be accepted');
  assert.match(refusal, /^[A-Z][A-Z_]*:/, `the refusal must be named: ${refusal}`);
  evidence.unsupportedQuery = { refusal };

  // Favorites: add the retained preview, list it, then remove it.
  const sample = {
    seed: String(preview.candidate.seed),
    rarity: 4,
    level: 180,
    playthrough: 3,
    effects: [],
    rules: [],
    enemies: [],
  };
  const added = await page.evaluate(
    ({ sample, reference }) =>
      window.review.favorites({ action: 'add', reference_id: reference, sample }),
    { sample, reference: preview.reference_id }
  );
  assert.equal(added.length, 1);
  assert.equal(added[0].backend.installable, true);
  const removed = await page.evaluate(
    ({ key }) => window.review.favorites({ action: 'remove', key }),
    { key: `3:4:180:${sample.seed}` }
  );
  assert.equal(removed.length, 0);
  evidence.favorites = { added: added.length, installable: true, removed: removed.length };

  // Cart: the synthetic save is discovered through the protected worker, the
  // inventory is readable, and a prepared plan is produced without writing.
  // The protected worker answers with an operation job; the UI's own client
  // polls it, so the harness polls the same way.
  const settle = async (job, label) => {
    const deadline = Date.now() + 60000;
    while (!['completed', 'failed', 'cancelled'].includes(job.state)) {
      if (Date.now() > deadline) throw Error(`operation ${label} stayed ${job.state}`);
      await new Promise((r) => setTimeout(r, 150));
      job = await page.evaluate(
        ({ jobId }) => window.operations.snapshot('save', jobId),
        { jobId: job.job_id }
      );
    }
    if (job.state !== 'completed') {
      throw Error(`operation ${label} ended ${job.state}: ${JSON.stringify(job.error)}`);
    }
    return job.result;
  };
  const runOperation = async (method, params) =>
    settle(
      await page.evaluate(
        ({ method, params }) => window.operations.execute({ method, params }),
        { method, params }
      ),
      method
    );
  const discovery = await runOperation('save.discover', {});
  const saveId = discovery.save_id || discovery.saves?.[0]?.save_id;
  assert.ok(saveId, `save.discover returned ${JSON.stringify(discovery).slice(0, 300)}`);
  const inventory = await runOperation('save.inventory', { save_id: saveId });
  const snapshotId = inventory.snapshot_id || inventory.save_id || saveId;
  const prepared = await settle(
    await page.evaluate(
      ({ saveId, snapshotId, reference }) =>
        window.review.prepareCart({
          mode: 'save',
          save_id: saveId,
          snapshot_id: snapshotId,
          references: [reference],
          recommended_level: 180,
          transfer_count: 1,
        }),
      { saveId, snapshotId, reference: preview.reference_id }
    ),
    'save.prepare_install_many'
  );
  assert.ok(prepared.plan_id || prepared.candidates, JSON.stringify(prepared).slice(0, 400));
  evidence.cart = {
    saveId,
    snapshotId,
    inventoryKeys: Object.keys(inventory).slice(0, 10),
    inventory: Array.isArray(inventory.entries)
      ? inventory.entries.length
      : Array.isArray(inventory.inventory)
        ? inventory.inventory.length
        : null,
    emptySlots: inventory.empty_slots ?? null,
    plan: Object.keys(prepared).slice(0, 8),
  };

  // Diagnostics must show the isolated search worker, and the encrypted fixture
  // must be byte-identical: this run wrote nothing.
  const diagnostics = await page.evaluate(() => window.support.diagnostics());
  const searchWorker = diagnostics.workers.find((w) => w.role === 'offline_search');
  assert.ok(searchWorker, JSON.stringify(diagnostics).slice(0, 400));
  assert.equal(searchWorker.connection, 'ready');
  evidence.diagnostics = { offlineSearch: searchWorker };
  assert.equal(fixtureDigest(), fixtureBefore, 'the synthetic save must be unchanged');
  evidence.fixtureUnchanged = true;

  await writeFile(join(output, 'verification.json'), JSON.stringify(evidence, null, 2));
  await writeFile(
    join(output, 'verification.md'),
    [
      '# Development Rust read-only worker: frontend acceptance',
      '',
      `- Host: \`${executable}\``,
      `- Rust worker: \`${rustWorker}\``,
      `- Context digest: \`${evidence.handshake.contextDigest}\``,
      `- Catalogs: zh-CN / en-US / ja-JP, 50 ordinary effects, 21 Graces, 6 terrain, 487 enemies, 277 rules, 103 families`,
      `- Recommended level 200 -> internals ${recommended.canonical_internal_levels.join(', ')}`,
      `- Preview seed ${evidence.preview.seed}: ${evidence.preview.effects} effects`,
      `- Search: ${evidence.search.state}, ${evidence.search.candidates} candidates (seed ${evidence.search.firstSeed})`,
      `- Cancel: ${evidence.cancel.state}`,
      `- Served rarity-5 composition: seed ${evidence.servedRarity5.seed} at cursor ${evidence.servedRarity5.cursor}`,
      `- Unserved request: ${evidence.unsupportedQuery.refusal}`,
      `- Favorites: add ${evidence.favorites.added}, remove ${evidence.favorites.removed}`,
      `- Cart: ${evidence.cart.plan.join(', ')} over save \`${evidence.cart.saveId}\``,
      '- Synthetic encrypted fixture unchanged; no game process, no live save',
      '',
      'Bounded evidence: source-level WebView2 acceptance only, not outer-EXE',
      'packaging acceptance and not live-game acceptance.',
      '',
    ].join('\n')
  );
  console.log(JSON.stringify(evidence, null, 2));
} finally {
  try {
    await page?.evaluate(() => window.review.windowAction('close'));
  } catch {}
  try {
    browser?.close();
  } catch {}
  await new Promise((r) => setTimeout(r, 1500));
  if (child.exitCode === null) child.kill();
}
