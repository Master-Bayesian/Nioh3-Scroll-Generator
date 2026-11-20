/** Final-EXE numerical acceptance. Requires Windows, live game and an isolated
 * approved save/profile fixture. Never redirects workers, schemas or scene data.
 * Usage: node apps/tauri/verify-packaged-loadout-search.mjs CONFIG.json OUTPUT
 * See tools/acceptance/PACKAGED_LOADOUT_HOST.md for the exact config and receipts.
 */
import assert from 'node:assert/strict';
import {spawn, execFileSync} from 'node:child_process';
import {mkdir, readFile, writeFile, copyFile, readdir, cp, lstat} from 'node:fs/promises';
import {basename, dirname, join, resolve, sep} from 'node:path';
import {arch, release} from 'node:os';
import {chromium} from 'playwright';
import {digest, inspectOnefile, isolatedEnvironment, pause, closeSession} from './onefile-acceptance.mjs';

const [configPath, outputPath] = process.argv.slice(2);
assert(configPath && outputPath, 'Usage: CONFIG.json OUTPUT');
assert.equal(process.platform, 'win32', 'This acceptance must run on Windows');
const config = JSON.parse(await readFile(configPath, 'utf8'));
const mode = config.mode ?? 'player';
assert(['player', 'engineering-fixture'].includes(mode));
assert(!config.profile_template, 'A legacy template cannot establish the prepared dynamic-source path; use an explicitly bound live_epoch_fixture in engineering mode');
if (mode === 'player') assert(!config.live_epoch_fixture, 'Player release acceptance forbids injecting private research resources');
else assert(config.live_epoch_fixture, 'Engineering mode requires the complete currently bound live epoch fixture');
const output = resolve(outputPath);
await mkdir(output, {recursive: true});
const outer = resolve(config.outer), python = config.python || 'python';
const staticTool = resolve('tools/verify_loadout_release.py');
const inspection = JSON.parse(execFileSync(python, [staticTool, 'check', outer], {encoding: 'utf8'}));
assert.equal(inspection.passed, true);
assert.equal(inspection.scenes.length, 1, 'This runner currently covers the one-scene 0.8.13 beta; extend selectors before shipping other scenes');
const scene = inspection.scenes[0];
assert(Object.keys(scene.files).every(name => !name.includes('/')), 'This beta runner only accepts flat scene input bundles');
assert.equal(scene.scene_id, config.scene_id);
const source = await readFile(config.save);
const sourceSha = digest(source);
const game = JSON.parse(await readFile(config.game_identity, 'utf8'));
assert.equal(digest(await readFile(game.path)), game.sha256.toLowerCase());
const referenceRaw = await readFile(config.reference);
const reference = JSON.parse(referenceRaw);
assert.equal(reference.complete, true);
assert.equal(reference.save_source_sha256?.toLowerCase(), sourceSha, 'Reference was produced from a different owned save');
assert(reference.ranked?.length > 0 && reference.progress?.planned > 0);
assert(Number.isInteger(config.set_key) && Number.isInteger(config.set_pieces));
assert.deepEqual(reference.required_sets, [{key: config.set_key, minimum_tier: config.set_pieces}], 'Reference has different set constraints');
assert.equal(reference.top_case?.components?.state, 'available', 'Reference must include independently checked winner components');
assert.equal(reference.top_case.components.contacts.length, 1, 'This beta gate requires a one-contact reference; extend accounting explicitly for multi-contact objectives');
const artifact = await inspectOnefile(outer);
const report = {mode, release_ready: false, private_resources_copied: mode === 'engineering-fixture', schema: 'nioh3-packaged-loadout-e2e/v1', passed: false, outer_sha256: artifact.sha256,
  source_commit: inspection.source_commit, environment: {platform: process.platform, arch: arch(), os: release()},
  runtime_overrides: [], source_save_sha256: sourceSha, scenes: [{scene_id: scene.scene_id, files: scene.files, runs: []}],
  no_private_profile: {ran: false}, fresh_install: {passed: false}, private_context_bytes_in_receipt: false};
const reportPath = join(output, 'packaged-loadout-e2e.json');
const pathInside = (child, parent) => resolve(child).toLowerCase().startsWith(resolve(parent).toLowerCase() + sep);
const hashFile = async file => digest(await readFile(file));
const floatBits = value => {const bytes = Buffer.alloc(4); bytes.writeFloatLE(value); return bytes.readUInt32LE();};
const componentBits = detail => {
  assert.equal(detail?.state, 'available', 'Published damage components unavailable');
  const channels = row => ['total', 'physical', 'elemental'].map(key => {assert(Number.isFinite(row[key])); return floatBits(row[key]);});
  assert(Array.isArray(detail.contacts) && detail.contacts.length > 0);
  return {channels: channels(detail), contacts: detail.contacts.map(row => [row.contact, row.template, ...channels(row)])};
};
async function checkLoadedScene(sceneDir) {
  const actual = await readdir(sceneDir, {withFileTypes: true});
  const runtimeFiles = actual;
  assert(runtimeFiles.every(entry => entry.isFile()), 'Unexpected scene directory or symbolic link');
  assert.deepEqual(runtimeFiles.map(entry => entry.name).sort(), [...Object.keys(scene.files), 'runtime-contract.json'].sort(), 'Loaded scene file set differs from final payload');
  assert.equal(await hashFile(join(sceneDir, 'runtime-contract.json')), scene.contract_sha256);
  for (const [name, expectedFile] of Object.entries(scene.files)) assert.equal(await hashFile(join(sceneDir, name)), expectedFile.sha256, 'Loaded scene differs from embedded file ' + name);
}
const normalized = rows => rows.map(row => ({damage: row.damage,
  items: (row.items ?? row.assignments).map(item => [item.slot, item.slot_index ?? item.save_slot]),
  swap: row.move_previous_weapon_to_slot ?? null}));

function observeModules(root, stopFile, logFile) {
  // Pass paths as env vars. No data is interpolated into PowerShell source.
  const script = `$ErrorActionPreference='Stop'; $seen=@{}; while(!(Test-Path -LiteralPath $env:NIOH3_OBSERVER_STOP)) {
    foreach($p in @(Get-Process -Name 'nioh3-protected-worker' -ErrorAction SilentlyContinue)) {
      try { if(!$p.Path -or !$p.Path.StartsWith($env:NIOH3_OBSERVER_ROOT,[StringComparison]::OrdinalIgnoreCase)){continue}
        foreach($m in $p.Modules) { if($m.ModuleName -notin @('main_cpu.dll','main_device.dll')){continue}
          $key=[string]$p.Id+':'+$m.FileName; if($seen.ContainsKey($key)){continue}; $seen[$key]=$true
          @{pid=$p.Id;worker=$p.Path;library=$m.FileName;module=$m.ModuleName;observed_utc=[DateTime]::UtcNow.ToString('o')} |
            ConvertTo-Json -Compress | Add-Content -LiteralPath $env:NIOH3_OBSERVER_LOG -Encoding UTF8
        }
      } catch { @{error=$_.Exception.Message} | ConvertTo-Json -Compress | Add-Content -LiteralPath $env:NIOH3_OBSERVER_LOG -Encoding UTF8 }
    }; Start-Sleep -Milliseconds 100
  }`;
  return spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-EncodedCommand', Buffer.from(script, 'utf16le').toString('base64')],
    {windowsHide: true, stdio: 'ignore', env: {...process.env, NIOH3_OBSERVER_ROOT: root, NIOH3_OBSERVER_STOP: stopFile, NIOH3_OBSERVER_LOG: logFile}});
}

async function run(backend, withProfile) {
  const leg = withProfile ? backend : 'no-private-profile';
  const root = join(output, leg);
  await mkdir(root); // Refuse stale evidence directories.
  const {env, profile, port} = await isolatedEnvironment(root);
  for (const key of Object.keys(env)) if (/^NIOH3_(RUST|E2E|LOADOUT|TAURI_PACKAGE)/i.test(key)) delete env[key];
  // This selects the public CPU/card policy only. All code/data paths still
  // come from the outer EXE's embedded package and the real production broker.
  env.NIOH3_LOADOUT_GPU = backend === 'cpu' ? '0' : '1';
  await mkdir(profile, {recursive: true});
  let privateContextHashes = null;
  if (withProfile && mode === 'engineering-fixture') {
    const epochSource = resolve(config.live_epoch_fixture);
    const active = JSON.parse(await readFile(join(epochSource, 'active.json'), 'utf8'));
    assert(/^[0-9a-f]{64}$/.test(active.capture_sha256));
    const generation = join(epochSource, active.capture_sha256);
    const binding = JSON.parse(await readFile(join(generation, 'LIVE_BINDING.json'), 'utf8'));
    const epoch = JSON.parse(await readFile(join(generation, 'EPOCH_INPUT.json'), 'utf8'));
    assert.equal(binding.capture_sha256, active.capture_sha256);
    assert.equal(binding.context_sha256, reference.context_sha256);
    assert.equal(epoch.context_sha256, reference.context_sha256);
    const target = join(env.NIOH3_STATE_ROOT, 'loadout-live-epochs', config.scene_id);
    await mkdir(target, {recursive: true});
    const rejectLinks = async path => {
      assert.equal((await lstat(path)).isSymbolicLink(), false, 'Fixture contains a symbolic link');
      return true;
    };
    await copyFile(join(epochSource, 'active.json'), join(target, 'active.json'));
    const privateGeneration = join(target, active.capture_sha256);
    await mkdir(join(privateGeneration, 'bundle'), {recursive: true});
    for (const file of ['EPOCH_INPUT.json', 'LIVE_BINDING.json']) {
      await rejectLinks(join(generation, file));
      await copyFile(join(generation, file), join(privateGeneration, file));
    }
    await cp(join(generation, 'bundle', 'context'), join(privateGeneration, 'bundle', 'context'),
      {recursive: true, errorOnExist: true, force: false, filter: rejectLinks});
    const contextDir = join(privateGeneration, 'bundle', 'context');
    privateContextHashes = {};
    for (const name of await readdir(contextDir)) {
      const file = join(contextDir, name);
      assert((await lstat(file)).isFile(), 'Expected a private context file');
      privateContextHashes[name] = await hashFile(file);
    }
  }
  await writeFile(join(profile, 'game-install.json'), JSON.stringify({schema: 'nioh3-game-install/v1', executable: game.path}));
  const saveTarget = join(env.LOCALAPPDATA, 'KoeiTecmo', 'NIOH3', 'Savedata', basename(dirname(dirname(config.save))), basename(dirname(config.save)), 'SAVEDATA.BIN');
  await mkdir(dirname(saveTarget), {recursive: true});
  await writeFile(saveTarget, source);
  const stopFile = join(root, 'observer.stop'), moduleLog = join(root, 'modules.jsonl');
  const observer = observeModules(env.LOCALAPPDATA, stopFile, moduleLog);
  const child = spawn(outer, ['--user-data-dir', profile], {env, cwd: root, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe']});
  let browser, page, diagnostic, error, spawnError, observerError;
  child.once('error', caught => {spawnError = caught;});
  observer.once('error', caught => {observerError = caught;});
  child.stdout.resume();
  let stderr = '';
  child.stderr.on('data', bytes => {stderr = (stderr + bytes.toString()).slice(-24000);});
  const evidence = {leg, passed: false, requested_backend: backend};
  try {
    const endpoint = 'http://127.0.0.1:' + port;
    const deadline = Date.now() + 60000;
    while (Date.now() < deadline) {
      if (spawnError) throw spawnError;
      assert.equal(child.exitCode, null);
      try {if ((await fetch(endpoint + '/json/version')).ok) break;} catch { /* WebView start */ }
      await pause(200);
    }
    browser = await chromium.connectOverCDP(endpoint, {timeout: 10000});
    page = browser.contexts()[0].pages()[0];
    await page.locator('.shell').waitFor({timeout: 30000});
    diagnostic = await page.evaluate(() => window.support.diagnostics());
    assert.equal(diagnostic.packageVerification.ok, true);
    assert.equal(diagnostic.packageVerification.sourceCommit, inspection.source_commit);
    await page.evaluate(() => {
      window.__packagedLoadout = {jobs: {}, calls: []};
      const ops = window.operations, log = window.__packagedLoadout;
      const execute = ops.execute.bind(ops), snapshot = ops.snapshot.bind(ops);
      ops.execute = async command => {
        log.calls.push({method: command.method, params: command.params});
        const result = await execute(command);
        if (result?.job_id) log.jobs[result.job_id] = result;
        return result;
      };
      ops.snapshot = async (role, id) => {const job = await snapshot(role, id); log.jobs[id] = job; return job;};
    });
    await page.getByRole('button', {name: '配装优化', exact: true}).click();
    await page.waitForFunction(() => Object.values(window.__packagedLoadout.jobs).some(job => job.kind === 'save.loadout_context' && ['completed', 'failed'].includes(job.state)), null, {timeout: 60000});
    const contextJobs = () => page.evaluate(() => Object.values(window.__packagedLoadout.jobs).filter(job => job.kind === 'save.loadout_context'));
    let contexts = await contextJobs();
    const first = contexts.at(-1);
    evidence.first_read = first;
    if (!withProfile) {
      assert(first.state === 'failed' || first.result?.state === 'unavailable', 'Fresh profile unexpectedly acquired a context; review provisioning separately');
      assert.equal(await page.getByTestId('loadout-result-coverage').count(), 0);
      const start = page.getByTestId('loadout-search-start');
      assert((await start.count()) === 0 || await start.isDisabled());
      report.no_private_profile = {ran: true, false_complete: false, job_state: first.state, reason: first.result?.reason ?? first.error?.message};
      evidence.passed = true;
      return;
    }
    assert.equal(first.state, 'completed', JSON.stringify(first.error));
    assert.equal(first.result?.state, 'ready');
    assert.equal(first.result?.source, 'live');
    assert.equal(first.result?.source_sha256?.toLowerCase(), sourceSha);
    assert.equal(first.result?.bundle, config.scene_id);
    await page.getByRole('button', {name: '刷新角色状态', exact: true}).click();
    await page.waitForFunction(id => Object.values(window.__packagedLoadout.jobs).some(job => job.kind === 'save.loadout_context' && job.job_id !== id && ['completed', 'failed'].includes(job.state)), first.job_id, {timeout: 60000});
    contexts = await contextJobs();
    const refresh = contexts.at(-1);
    assert.equal(refresh.state, 'completed', JSON.stringify(refresh.error));
    assert.equal(refresh.result?.state, 'ready');
    assert.equal(refresh.result?.source, 'live');
    evidence.refresh = refresh;
    await page.getByRole('combobox', {name: '添加套装条件', exact: true}).selectOption(String(config.set_key));
    await page.getByRole('combobox', {name: '生效件数', exact: true}).selectOption(String(config.set_pieces));
    await page.getByTestId('loadout-search-start').click();
    const resultNode = page.getByTestId('loadout-result-coverage');
    await resultNode.waitFor({timeout: config.search_timeout_ms ?? 180000});
    assert.equal(await resultNode.getAttribute('data-coverage'), 'complete');
    const jobs = await page.evaluate(() => window.__packagedLoadout.jobs);
    const job = Object.values(jobs).filter(row => row.kind === 'save.loadout_search').at(-1);
    assert.equal(job?.state, 'completed', JSON.stringify(job?.error));
    const result = job.result;
    assert.equal(result.backend, backend, 'Requested CUDA silently fell back to CPU');
    assert.equal(result.complete, true);
    assert.equal(result.answered, reference.progress.planned);
    assert.equal(result.searched, result.answered);
    assert.equal(result.full_size, reference.progress.requested);
    assert.equal(result.excluded_by_set_bound, reference.progress.excluded_by_set_bound);
    assert.equal(result.refused + result.no_path + result.source_uncomputed, 0);
    const executed = result.execution_receipt;
    assert.equal(executed?.schema, 'nioh3-loadout-execution/v1');
    assert.equal(executed.scope, 'prepared_exact_sweep');
    assert.equal(executed.backend, backend);
    assert.equal(executed.planned, result.answered);
    assert.equal(executed.completed, result.answered);
    if (backend === 'card') {
      assert(executed.native_calls > 0, 'Loaded CUDA module executed no native call');
      assert.equal(executed.submitted_contacts, result.answered);
      assert.equal(executed.completed_contacts, result.answered);
      assert.equal(executed.failed_batches, 0);
      assert.equal(executed.reuse_enabled, false, 'Default-policy acceptance must execute every submitted contact');
    } else {
      assert(executed.cpu_executed_candidates > 0);
      assert.equal(executed.cpu_executed_candidates + executed.cpu_certified_candidates, result.answered);
    }
    const expected = normalized(reference.ranked.slice(0, result.best.length));
    assert.equal(result.best.length, Math.min(20, reference.ranked.length), 'Published rank list was truncated');
    for (const row of result.best) assert.equal(componentBits(row.damage_details).channels[0], floatBits(row.damage));
    assert.deepEqual(componentBits(result.best[0].damage_details), componentBits(reference.top_case.components), 'Winner total/physical/elemental/contact bits differ from independent reference');
    assert.deepEqual(normalized(result.best), expected, 'Published ranks/assignments/ties differ from independent reference');
    const ranks = result.best.map(row => ({...normalized([row])[0], damage_details: row.damage_details}));
    await pause(300); // Let the independent module observer flush its last record.
    if (observerError) throw observerError;
    const lines = (await readFile(moduleLog, 'utf8')).split(/\r?\n/).map(line => line.replace(/^\uFEFF/, '').trim()).filter(Boolean).map(line => JSON.parse(line));
    const libraryName = backend === 'cpu' ? 'main_cpu.dll' : 'main_device.dll';
    const matching = lines.filter(line => line.module === libraryName);
    assert(matching.length > 0, 'No actual loaded scorer module observed; do not substitute a filename guess');
    const loaded = matching.at(-1), sceneDir = dirname(loaded.library);
    assert(pathInside(loaded.worker, env.LOCALAPPDATA));
    const packagedScene = join(dirname(loaded.worker), 'runtime', 'data', 'loadout', scene.scene_id);
    assert.equal(resolve(sceneDir).toLowerCase(), resolve(packagedScene).toLowerCase(),
      'Loaded scorer must come from the installed scene next to the packaged worker');
    assert(loaded.worker.toLowerCase().includes(artifact.payloadSha256));
    assert.equal(await hashFile(loaded.library), scene.files[libraryName].sha256);
    await checkLoadedScene(sceneDir);
    const abi = JSON.parse(execFileSync(python, [resolve('tools/probe_loadout_abi.py'), loaded.library, backend], {encoding: 'utf8'}));
    assert.equal(abi.passed, true);
    const runReceipt = {private_context_file_hashes: privateContextHashes, execution_receipt: executed, requested_backend: backend, actual_backend: result.backend, complete: result.complete,
      planned: reference.progress.planned, answered: result.answered, searched: result.searched,
      refused: result.refused, no_path: result.no_path, source_uncomputed: result.source_uncomputed,
      winner_components_matched: true, winner_component_bits: componentBits(result.best[0].damage_details),
      rank_rows_matched: result.best.length, rank_sha256: digest(Buffer.from(JSON.stringify(ranks))), reference_sha256: digest(referenceRaw),
      observed_abis: abi.observed_abis, loaded_library: {path: loaded.library, worker: loaded.worker,
        sha256: abi.sha256, scene_files_match_payload: true, worker_from_payload_cache: true},
      first_read_job: first.job_id, refresh_job: refresh.job_id, search_job: job.job_id};
    report.scenes[0].runs.push(runReceipt);
    evidence.search = job;
    evidence.loaded_modules = lines;
    evidence.passed = true;
    await page.screenshot({path: join(root, 'result.png')});
  } catch (caught) {
    error = caught;
    evidence.error = String(caught);
    if (page) evidence.visible_text = await page.locator('body').innerText().catch(() => '');
  } finally {
    evidence.stderr = stderr;
    evidence.package_diagnostics = diagnostic;
    let cleanupError;
    try {
      assert.equal(await hashFile(saveTarget), sourceSha, 'Acceptance modified isolated save');
      assert.equal(await hashFile(config.save), sourceSha, 'Acceptance modified original save');
    } catch (caught) {cleanupError = caught; evidence.cleanup_error = String(caught); evidence.passed = false;}
    try {
      await writeFile(stopFile, 'stop');
      await Promise.race([new Promise(done => observer.once('exit', done)), pause(3000)]);
    } catch (caught) {cleanupError ??= caught; evidence.cleanup_error = String(caught); evidence.passed = false;}
    finally {if (observer.exitCode === null) observer.kill();}
    try {
      if (browser && page) await closeSession({browser, page}, child); else if (child.exitCode === null) child.kill();
    } catch (caught) {cleanupError ??= caught; evidence.cleanup_error = String(caught); evidence.passed = false;}
    await writeFile(join(root, 'run.json'), JSON.stringify(evidence, null, 2));
    await writeFile(reportPath, JSON.stringify(report, null, 2));
    if (cleanupError) throw cleanupError;
  }
  if (error) throw error;
}

try {
  if (mode === 'engineering-fixture') await run('cpu', false);
  await run('cpu', true);
  await run('card', true);
  report.passed = true;
  if (mode === 'player') {
    report.release_ready = true;
    report.no_private_profile = {ran: true, false_complete: false, ready: true};
    report.fresh_install = {passed: true, source: 'live', searches_complete: true, private_resources_copied: false, provisioned_by: 'product'};
  }
  await writeFile(reportPath, JSON.stringify(report, null, 2));
  // The same parser used by the release verifier consumes the generated receipt.
  if (mode === 'player') execFileSync(python, [staticTool, 'check', outer, '--require-host-e2e', '--host-receipt', reportPath,
    '--output', join(output, 'release-gate.json')], {stdio: 'inherit'});
} catch (error) {
  report.passed = false;
  report.release_ready = false;
  report.error = String(error);
  await writeFile(reportPath, JSON.stringify(report, null, 2));
  throw error;
}
console.log(JSON.stringify({passed: true, release_ready: report.release_ready, mode, receipt: reportPath}));
