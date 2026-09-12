/** Direct download-to-launch acceptance. Only isolated caches and profiles are writable. */
import {spawn} from 'node:child_process';
import {access, cp, mkdir, mkdtemp, readdir, readFile, stat, utimes, writeFile} from 'node:fs/promises';
import {join, resolve, basename} from 'node:path';
import {tmpdir} from 'node:os';
import assert from 'node:assert/strict';
import {closeSession, connect, executable, inspectOnefile, isolatedEnvironment, registrySnapshot} from './onefile-acceptance.mjs';

const source = executable(), original = await inspectOnefile(source);
const root = await mkdtemp(join(tmpdir(), 'nioh3-onefile-launch-'));
const download = join(root, 'download'), target = join(download, basename(source));
await mkdir(download); await cp(source, target);
const {env, profile, port} = await isolatedEnvironment(root);
const cache = join(env.LOCALAPPDATA, 'Nioh3Studio', 'onefile');
const runtime = join(cache, original.payloadSha256);
const registryBefore = registrySnapshot();
let session, child;
const launch = async () => {
  child = spawn(target, ['--user-data-dir', profile], {env, windowsHide:true, stdio:'ignore'});
  session = await connect(port, child);
  assert.equal(await session.page.locator('.selected-body .selected-row').count(), 0);
  const status = await session.page.evaluate(() => window.review.update({action:'status', channel:'stable'}));
  assert.equal(status.canApply, true, JSON.stringify(status));
  await closeSession(session, child); session = undefined; child = undefined;
};
try {
  await launch();
  const manifestPath = join(runtime, 'build-manifest.json');
  const manifestBefore = await stat(manifestPath);
  // These copies have valid manifests and explicit launcher ownership markers.
  // They exercise bounded stale-cache deletion without touching an existing cache.
  const oldCaches = [];
  const marker = JSON.parse(await readFile(join(runtime, '.onefile-cache.json'), 'utf8'));
  for (let index = 1; index <= 3; index++) {
    const id = String(index).repeat(64), path = join(cache, id);
    assert.notEqual(id, original.payloadSha256);
    await cp(runtime, path, {recursive:true});
    await writeFile(join(path, '.onefile-cache.json'), JSON.stringify({...marker, payload_sha256:id}));
    const timestamp = new Date(Date.now() - index * 86400000);
    await utimes(join(path, '.lease'), timestamp, timestamp);
    oldCaches.push(path);
  }
  const foreign = join(cache, 'unowned-not-a-runtime');
  await mkdir(foreign); await writeFile(join(foreign, 'keep.txt'), 'unowned fixture');
  await launch();
  assert.equal((await stat(manifestPath)).mtimeMs, manifestBefore.mtimeMs, 'Identical payload was unnecessarily extracted again');
  const remaining = (await readdir(cache)).filter(name => /^[a-f0-9]{64}$/.test(name));
  assert.equal(remaining.length, 2, 'Inactive runtime caches must be bounded to two');
  assert(remaining.includes(original.payloadSha256));
  await assert.rejects(access(oldCaches[1])); await assert.rejects(access(oldCaches[2]));
  assert.equal(await readFile(join(foreign, 'keep.txt'), 'utf8'), 'unowned fixture');
  assert.deepEqual(await readdir(download), [basename(source)], 'The download folder must contain one executable only');
  assert.equal(registrySnapshot(), registryBefore, 'Installation registry changed');
  assert.equal((await inspectOnefile(source)).sha256, original.sha256);
  const output = resolve('deliverables/frontend-v2/tauri-acceptance'); await mkdir(output, {recursive:true});
  await writeFile(join(output, 'onefile-direct-launch.json'), JSON.stringify({sourceSha256:original.sha256,
    payloadSha256:original.payloadSha256, root, directLaunch:true, workerHandshake:true, initialFiltersEmpty:true,
    updateAvailable:true, launches:2, cacheReused:true, staleCachesPruned:2, retainedCaches:2,
    unrelatedFilesPreserved:true, noInstallationRegistryChanges:true, downloadContainsOneExe:true, gameWrites:0}, null, 2));
  console.log('TAURI_ONEFILE_DIRECT_LAUNCH_CACHE_OK');
} finally {if (session || child) await closeSession(session, child);}
