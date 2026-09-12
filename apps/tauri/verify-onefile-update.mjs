/** Replace an isolated outer EXE, start it, and verify real startup cleanup. */
import {cp, mkdir, mkdtemp, readFile, readdir, writeFile, access} from 'node:fs/promises';
import {execFileSync, spawn} from 'node:child_process';
import {randomUUID} from 'node:crypto';
import {join, resolve} from 'node:path';
import {tmpdir} from 'node:os';
import assert from 'node:assert/strict';
import {closeSession, connect, digest, executable, inspectOnefile, isolatedEnvironment, pause} from './onefile-acceptance.mjs';

const source = executable(), original = await inspectOnefile(source);
const root = await mkdtemp(join(tmpdir(), 'nioh3-onefile-update-'));
const {env, profile, port} = await isolatedEnvironment(root);
const cache = join(profile, 'updates'), download = join(cache, randomUUID());
const target = join(root, 'Nioh3Studio.exe'), staged = join(download, 'replacement.exe');
await cp(source, target);
const originalLauncher = spawn(target, ['--user-data-dir', profile], {env, windowsHide:true, stdio:'ignore'});
let session, helperProcess, helperErrors = '';
try {
session = await connect(port, originalLauncher);
const processId = Number(execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command',
  `$ErrorActionPreference='Stop'; $rows=@(Get-CimInstance Win32_Process -Filter 'ParentProcessId = ${originalLauncher.pid}' | Where-Object Name -eq 'Nioh3Studio.exe'); if($rows.Count -ne 1){throw 'Expected one launcher-owned runtime'}; [Console]::Write($rows[0].ProcessId)`],
  {windowsHide:true, encoding:'utf8', timeout:15000}).trim());
assert(Number.isInteger(processId) && processId > 0);
// Prepare the update after the original startup cleanup has finished.
await mkdir(download, {recursive:true});
const archive = join(download, 'package.zip'); await writeFile(archive, original.payload);
const python = process.env.NIOH3_PYTHON || 'python';
// Change only the ZIP comment, preserving every signed runtime file. This proves
// replacement of different outer bytes at the same app version without rebuilding.
execFileSync(python, ['-c', "import sys,zipfile; z=zipfile.ZipFile(sys.argv[1],'a'); z.comment=b'isolated-onefile-update-acceptance'; z.close()", archive], {windowsHide:true, timeout:15000});
execFileSync(python, ['tools/build_tauri_onefile.py', archive, staged], {windowsHide:true, timeout:45000});
const next = await inspectOnefile(staged); assert.notEqual(next.sha256, original.sha256);
const manifestHash = execFileSync(python, ['-c', "import sys,zipfile,hashlib; print(hashlib.sha256(zipfile.ZipFile(sys.argv[1]).read('build-manifest.json')).hexdigest())", archive],
  {windowsHide:true, encoding:'utf8', timeout:15000}).trim();
const helper = join(cache, 'apply-onefile-update.ps1'); await cp('apps/tauri/src-tauri/apply-onefile-update.ps1', helper);
const canonicalTarget = '\\\\?\\' + target;
helperProcess = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', helper,
  '-ProcessId', String(processId), '-LauncherProcessId', String(originalLauncher.pid), '-Target', canonicalTarget, '-Staged', staged,
  '-FileHash', next.sha256, '-PreviousHash', original.sha256, '-ManifestHash', manifestHash, '-Profile', profile],
  {windowsHide:true, stdio:['ignore', 'ignore', 'pipe'], env});
helperProcess.stderr.on('data', chunk => helperErrors += chunk);
let prepared = false;
for (let index = 0; index < 150; index++) {
  if (helperProcess.exitCode !== null) throw Error(`Update helper exited early: ${helperErrors}`);
  prepared = (await readdir(root)).some(name => /^Nioh3Studio\.exe\.update-[a-f0-9]{32}$/.test(name));
  if (prepared) break;
  await pause(100);
}
assert(prepared, 'Update helper did not prepare replacement while the original app was running');
assert.equal(digest(await readFile(target)), original.sha256, 'The helper replaced an active executable');
assert.equal(originalLauncher.exitCode, null);
await closeSession(session, originalLauncher); session = undefined;
for (let index = 0; index < 300 && helperProcess.exitCode === null; index++) await pause(100);
assert.equal(helperProcess.exitCode, 0, helperErrors);
await pause(300);
  session = await connect(port);
  let receipt;
  for (let index = 0; index < 150; index++) {
    receipt = JSON.parse(await readFile(join(cache, 'last-update-result.json'), 'utf8'));
    if (receipt.status === 'completed') break;
    await pause(200);
  }
  assert.equal(receipt.status, 'completed', JSON.stringify(receipt));
  assert.equal(receipt.mode, 'onefile');
  assert.equal(receipt.fileHash, next.sha256); assert.equal(receipt.previousHash, original.sha256);
  assert.equal(receipt.manifestHash, manifestHash);
  assert.equal(digest(await readFile(target)), next.sha256);
  await assert.rejects(access(receipt.previous)); await assert.rejects(access(download));
  assert.equal((await inspectOnefile(source)).sha256, original.sha256);
  assert.equal((await readdir(root)).filter(name => /\.previous-|\.update-/.test(name)).length, 0);
  const output = resolve('deliverables/frontend-v2/tauri-acceptance'); await mkdir(output, {recursive:true});
  await writeFile(join(output, 'onefile-real-update-restart.json'), JSON.stringify({root, originalSha256:original.sha256,
    replacementSha256:next.sha256, manifestHash, distinctOuterBytes:true, sameVersionRuntimeFixture:true,
    waitedForActualRuntimeAndLauncher:true, canonicalWindowsTarget:true,
    realLauncherStarted:true, realApplicationStarted:true, workerHandshake:true, mode:receipt.mode,
    receiptStatus:receipt.status, previousRemoved:true, downloadRemoved:true, originalSourceUnchanged:true,
    signedFeedValidation:'covered separately by updater signature tests', gameWrites:0}, null, 2));
  console.log('TAURI_ONEFILE_REAL_UPDATE_RESTART_CLEANUP_OK');
} finally {
  await closeSession(session, originalLauncher.exitCode === null ? originalLauncher : undefined);
  if (helperProcess?.exitCode === null) helperProcess.kill();
}
