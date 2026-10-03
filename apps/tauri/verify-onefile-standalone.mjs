/** Accept the outer EXE alone, with cold owned caches and an unrelated CWD. */
import assert from 'node:assert/strict';
import {spawn, execFileSync} from 'node:child_process';
import {mkdir, mkdtemp, copyFile, readFile, readdir, writeFile, unlink, stat} from 'node:fs/promises';
import {join, resolve, basename, dirname, relative, isAbsolute} from 'node:path';
import {tmpdir} from 'node:os';
import {chromium} from 'playwright';
import {closeSession, digest, executable, inspectOnefile, isolatedEnvironment, pause} from './onefile-acceptance.mjs';

const options = new Map();
for (let index = 2; index < process.argv.length; index += 2) {
  assert(['--scenario', '--out', '--game-fixture'].includes(process.argv[index]), `Unknown argument ${process.argv[index]}`);
  assert(process.argv[index + 1], `${process.argv[index]} requires a value`);
  options.set(process.argv[index], process.argv[index + 1]);
}
const scenario = options.get('--scenario') || 'all';
assert(['all', 'clean', 'poisoned'].includes(scenario));
assert.equal(process.platform, 'win32', 'This acceptance observes actual Windows process and DLL paths');
const ps = script => execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', script],
  {windowsHide: true, encoding: 'utf8', timeout: 15000}).trim();
assert.equal(ps("@(Get-Process -Name Nioh3 -ErrorAction SilentlyContinue).Count"), '0',
  'Close the game yourself before running this offline acceptance; the harness never stops it');
const source = executable(), original = await inspectOnefile(source);
const root = await mkdtemp(join(tmpdir(), 'nioh3-standalone-'));
const output = resolve(options.get('--out') || 'deliverables/frontend-v2/tauri-acceptance/onefile-standalone.json');
const report = {schema: 'nioh3-onefile-standalone/v1', ok: false, source, sourceSha256: original.sha256,
  payloadSha256: original.payloadSha256, root, scenario, gameWrites: 0, launches: [],
  scope: 'Actual Windows outer EXE, isolated state/cache and read-only worker calls; installed system WebView2 and GPU drivers remain OS prerequisites'};
const normalized = path => resolve(path.replace(/^\\\\\?\\/, '')).toLowerCase();
const assertInside = (parent, path) => {
  const rel = relative(normalized(parent), normalized(path));
  assert(rel && !rel.startsWith('..') && !isAbsolute(rel), `${path} is outside ${parent}`);
};
function ownedProcesses(pid) {
  // Query only this test launch's descendants; never inspect unrelated command lines.
  return JSON.parse(ps(`$ErrorActionPreference='Stop'; $rows=@(); $pending=@(${pid});
    while($pending.Count){$next=@(); foreach($owner in $pending){
      foreach($p in @(Get-CimInstance Win32_Process -Filter "ParentProcessId=$owner")){
        $modules=@((Get-Process -Id $p.ProcessId -ErrorAction Stop).Modules | ForEach-Object {$_.FileName});
        $rows+=@{pid=$p.ProcessId;parentPid=$p.ParentProcessId;path=$p.ExecutablePath;commandLine=$p.CommandLine;modules=$modules};
        $next+=$p.ProcessId
      }}; $pending=$next}; ConvertTo-Json -InputObject @($rows) -Depth 5 -Compress`));
}
async function verifyPayload(runtime, diagnostics) {
  const raw = await readFile(join(runtime, 'build-manifest.json'));
  const manifest = JSON.parse(raw);
  assert.equal(diagnostics.packageVerification.ok, true);
  assert.equal(diagnostics.packageVerification.sourceCommit, manifest.git.commit);
  assert.equal(diagnostics.packageVerification.manifestSha256, digest(raw));
  assert.equal(manifest.git.dirty, false);
  for (const member of manifest.files) {
    const path = join(runtime, member.path); assertInside(runtime, path);
    const bytes = await readFile(path);
    assert.equal(bytes.length, member.size, member.path);
    assert.equal(digest(bytes), member.sha256, member.path);
  }
  return manifest;
}
async function fixture(label, poisoned) {
  const area = join(root, label); await mkdir(area);
  const download = join(area, 'download'), cwd = join(area, 'unrelated-cwd');
  await mkdir(download); await mkdir(cwd);
  const target = join(download, basename(source)); await copyFile(source, target);
  const {env, profile, port} = await isolatedEnvironment(area);
  for (const name of Object.keys(env)) {
    if (/^NIOH3_/i.test(name) || /^WEBVIEW2_/i.test(name) || name.toLowerCase() === 'appdata') delete env[name];
  }
  Object.assign(env, {LOCALAPPDATA: join(area, 'local'), APPDATA: join(area, 'roaming'),
    NIOH3_TAURI_TEST_ROOT: profile, NIOH3_TAURI_TEST_DEBUG_PORT: String(port)});
  // Product libraries must come from the payload, not a checkout/tool on PATH.
  for (const name of Object.keys(env)) if (name.toLowerCase() === 'path') delete env[name];
  env.PATH = [join(env.SystemRoot, 'System32'), env.SystemRoot].join(';');
  await mkdir(profile); await mkdir(env.LOCALAPPDATA); await mkdir(env.APPDATA);
  const gameFixture = options.get('--game-fixture');
  if (gameFixture) {
    const identity = JSON.parse(await readFile(resolve(gameFixture), 'utf8'));
    assert.match(identity.scope, /never executed/i);
    assert.equal(digest(await readFile(identity.path)), identity.sha256);
    await writeFile(join(profile, 'game-install.json'), JSON.stringify({schema:'nioh3-game-install/v1', executable:identity.path}));
    report.gameFixture = identity;
  }
  const poison = join(cwd, 'old-version');
  if (poisoned) {
    await mkdir(poison);
    for (const name of ['Nioh3Studio.exe', 'nioh3-search-worker.exe', 'nioh3-protected-worker.exe',
      'nioh3_seed_accelerator.dll', 'nioh3_effect_preimage_accelerator.dll']) {
      await writeFile(join(poison, name), 'invalid older-folder fixture; must never be loaded');
    }
    Object.assign(env, {NIOH3_RUST_SEARCH_WORKER: join(poison, 'nioh3-search-worker.exe'),
      NIOH3_RUST_PROTECTED_WORKER: join(poison, 'nioh3-protected-worker.exe'),
      NIOH3_SEED_ACCELERATOR: join(poison, 'nioh3_seed_accelerator.dll'),
      NIOH3_EFFECT_PREIMAGE_ACCELERATOR: join(poison, 'nioh3_effect_preimage_accelerator.dll'),
      NIOH3_TAURI_PACKAGE_ROOT: poison});
  }
  return {area, download, cwd, target, env, profile, port,
    runtime: join(env.LOCALAPPDATA, 'Nioh3Studio', 'onefile', original.payloadSha256)};
}
async function launch(f, label) {
  assert.deepEqual(await readdir(f.download), [basename(source)]);
  const child = spawn(f.target, ['--user-data-dir', f.profile], {cwd:f.cwd, env:f.env, windowsHide:true, stdio:'ignore'});
  const record = {label, ownedLauncherPid:child.pid, outerExecutable:f.target, cwd:f.cwd, runtime:f.runtime, ok:false};
  report.launches.push(record);
  let session;
  try {
    let available = false;
    for (let attempt = 0; attempt < 160; attempt++) {
      assert.equal(child.exitCode, null, `Launcher exited ${child.exitCode} before UI`);
      try {available = (await fetch(`http://127.0.0.1:${f.port}/json/version`)).ok;} catch {}
      if (available) break;
      await pause(250);
    }
    assert(available, 'Owned app did not expose its acceptance WebView');
    const browser = await chromium.connectOverCDP(`http://127.0.0.1:${f.port}`);
    const page = browser.contexts()[0].pages()[0]; session = {browser, page};
    await page.locator('.shell').waitFor({timeout:15000});
    record.uiUrl = page.url(); assert.match(record.uiUrl, /^(tauri:\/\/localhost|https?:\/\/tauri\.localhost)\/?/);
    let diagnostics;
    for (let attempt = 0; attempt < 100; attempt++) {
      diagnostics = await page.evaluate(() => window.support.diagnostics());
      if (diagnostics.workers.length === 3 && diagnostics.workers.every(w => w.connection === 'ready')) break;
      if (diagnostics.firstSessionFailure) break;
      await pause(100);
    }
    record.diagnostics = diagnostics;
    assert.equal(normalized(diagnostics.executablePath), normalized(join(f.runtime, 'Nioh3Studio.exe')));
    assert.equal(normalized(diagnostics.outerExecutable), normalized(f.target));
    assert.equal(diagnostics.workers.length, 3);
    assert(diagnostics.workers.every(w => w.connection === 'ready'), JSON.stringify(diagnostics.workers));
    assert.equal(diagnostics.firstSessionFailure, null);
    const manifest = await verifyPayload(f.runtime, diagnostics);
    record.sourceCommit = manifest.git.commit; record.verifiedMembers = manifest.files.length;
    record.nativeDependencies = JSON.parse(await readFile(join(f.runtime, 'native-dependencies.json'), 'utf8'));
    assert.equal(record.nativeDependencies.ok, true);
    assert.equal(record.nativeDependencies.staticVcRuntime, true);
    assert(record.nativeDependencies.files.some(f => f.member === 'launcher/Nioh3Launcher.exe'));
    record.processes = ownedProcesses(child.pid);
    const host = record.processes.find(p => normalized(p.path) === normalized(diagnostics.executablePath));
    assert(host && host.parentPid === child.pid, 'Inner host was not launched by the tested outer EXE');
    const workers = record.processes.filter(p => p.parentPid === host.pid && /nioh3-(search|protected)-worker\.exe$/i.test(p.path));
    assert.equal(workers.length, 3, 'Host must own its three packaged workers');
    for (const w of workers) {
      assertInside(f.runtime, w.path);
      if (/search-worker\.exe$/i.test(w.path)) assert.match(w.commandLine, /--packaged-worker/);
      else {assert.match(w.commandLine, /--role (save|runtime)/); assert.doesNotMatch(w.commandLine, /--dev-protected-only/);}
      for (const flag of ['--data-root', '--contract-dir', '--accelerator']) {
        const value = w.commandLine.match(new RegExp(`${flag} (?:"([^"]+)"|(\\S+))`));
        assert(value, `Missing ${flag}`); assertInside(f.runtime, value[1] || value[2]);
      }
      for (const dll of w.modules.filter(path => /nioh3_.*\.dll$/i.test(path))) assertInside(f.runtime, dll);
    }
    const search = workers.find(p => /search-worker\.exe$/i.test(p.path));
    const capabilities = diagnostics.workers.find(w => w.role === 'offline_search').capabilities;
    const cleanReference = report.launches.find(r => r.label === 'cold-clean')?.diagnostics?.workers.find(w => w.role === 'offline_search').capabilities;
    if (cleanReference && label !== 'cold-clean') {
      for (const name of ['cuda_pivot_and_auxiliary','directcompute_effect_filter'])
        assert.equal(capabilities[name], cleanReference[name], `${name} changed under stale environment selectors`);
    }
    for (const [capability, file] of [['cuda_pivot_and_auxiliary','nioh3_seed_accelerator.dll'],
      ['directcompute_effect_filter','nioh3_effect_preimage_accelerator.dll']]) {
      if (capabilities[capability]) assert(search.modules.some(path => basename(path).toLowerCase() === file), `${file} advertised without observed loaded module`);
    }
    record.requests = await page.evaluate(async () => {
      const invoke = async (method, params) => {
        let value = await window.operations.execute({method, params});
        for (let attempt = 0; ['running','cancel_requested'].includes(value.state) && attempt < 100; attempt++) {
          await new Promise(resolve => setTimeout(resolve,100));
          value = await window.operations.snapshot(method.split('.')[0], value.job_id);
        }
        if (value.state === 'failed') throw new Error(JSON.stringify(value.error));
        if (value.job_id && value.state !== 'completed') throw new Error(`Read-only request did not complete: ${method}`);
        return value.job_id ? value.result : value;
      };
      return {preview: await window.review.preview({seed:1, rarity:3, level:180, retain:false}),
        saves: await invoke('save.discover', {}), runtime: await invoke('runtime.status', {})};
    });
    assert.equal(record.requests.preview.candidate.seed, 1); assert.deepEqual(record.requests.saves.saves, []);
    assert.equal(record.requests.runtime.safe_to_shutdown, true);
    record.workerLog = (await readFile(join(f.profile,'logs','desktop.log'),'utf8')).split('\n').filter(line => line.includes('[worker-backend]'));
    assert.equal(record.workerLog.length, 3 * report.launches.filter(r => r.runtime === f.runtime).length);
    assert.deepEqual(await readdir(f.download), [basename(source)]);
    await page.screenshot({path: join(f.area, `${label}.png`)});
    record.ok = true;
  } catch (error) {record.error = String(error.stack || error); throw error;}
  finally {
    if (session) {await closeSession(session, child, 30000); record.normalClose = true; record.exitCode = child.exitCode;}
    else record.closeBlocked = child.exitCode === null;
  }
}
try {
  if (scenario !== 'poisoned') {
    const clean = await fixture('clean', false);
    await assert.rejects(stat(clean.runtime));
    await launch(clean, 'cold-clean');
  }
  if (scenario !== 'clean') {
    const poisoned = await fixture('poisoned', true);
    await assert.rejects(stat(poisoned.runtime));
    await launch(poisoned, 'cold-poisoned');
    // Delete only three named members of this harness's cache after normal close.
    const missing = ['worker/nioh3-search-worker.exe', 'worker/runtime/bin/nioh3_effect_preimage_accelerator.dll',
      'worker/runtime/nioh3_scroll_editor/data/texts.json'];
    const manifest = JSON.parse(await readFile(join(poisoned.runtime,'build-manifest.json'),'utf8'));
    const resource = manifest.files.find(m => m.path.startsWith('worker/runtime/nioh3_scroll_editor/data/') && m.path.endsWith('.json'));
    assert(resource); missing[2] = resource.path;
    for (const member of missing) {const path = join(poisoned.runtime, member); assertInside(poisoned.runtime,path); await unlink(path);}
    report.deletedOwnedCacheMembers = missing;
    poisoned.cwd = join(poisoned.area,'second-unrelated-cwd'); await mkdir(poisoned.cwd);
    await launch(poisoned, 'repair-missing-members');
    report.missingMembersRepaired = true;
  }
  assert.equal((await inspectOnefile(source)).sha256, original.sha256);
  report.ok = true;
  console.log(JSON.stringify({ok:true, sourceSha256:original.sha256, launches:report.launches.length,
    sourceCommit:report.launches[0].sourceCommit, missingMembersRepaired:report.missingMembersRepaired || false, output}));
} catch (error) {report.error = String(error.stack || error); throw error;}
finally {await mkdir(dirname(output), {recursive:true}); await writeFile(output, JSON.stringify(report,null,2)+'\n');}
