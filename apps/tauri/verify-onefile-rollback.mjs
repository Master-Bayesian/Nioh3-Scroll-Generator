/**
 * Focused one-file rollback acceptance.
 *
 * The shipped contract is "a failed startup retains a rollback copy": the
 * applier moves the running outer EXE aside, installs the verified replacement,
 * starts it, and restores the previous file when the replacement cannot run.
 * `verify-onefile-update.mjs` covers the successful replacement,
 * acknowledgement and cleanup legs; this gate drives the failing leg.
 *
 * It is deliberately independent: no running app, no WebView2, no network and
 * no other gate's helper. Everything happens in a task-local scratch root, and
 * the outer EXE is copied rather than modified, so the candidate bytes are
 * never touched.
 *
 * Usage:
 *   NIOH3_ONEFILE_EXE=<outer exe> node apps/tauri/verify-onefile-rollback.mjs \
 *     [--out <json>] [--timeout <seconds>]
 */
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {cp, mkdir, mkdtemp, readFile, readdir, rm, writeFile} from 'node:fs/promises';
import {existsSync} from 'node:fs';
import {join, resolve} from 'node:path';
import {tmpdir} from 'node:os';

function parseArgs(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index];
    if (key === '--out') options.out = argv[++index];
    else if (key === '--timeout') options.timeout = Number(argv[++index]);
    else throw new Error(`unknown argument: ${key}`);
  }
  return options;
}

const source = process.env.NIOH3_ONEFILE_EXE;
if (!source) throw new Error('set NIOH3_ONEFILE_EXE to the outer one-file executable');
if (!existsSync(source)) throw new Error(`the outer executable is missing: ${source}`);

const options = parseArgs(process.argv.slice(2));
const digest = async (path) =>
  createHash('sha256').update(await readFile(path)).digest('hex');

// The applier waits for these two process ids to exit. They are far outside any
// real process id, so both waits return immediately and the gate never depends
// on a running application.
const absentPid = 2147483000;
const absentLauncherPid = 2147483001;

const root = await mkdtemp(join(tmpdir(), 'nioh3-onefile-rollback-'));
let record;
try {
  const target = join(root, 'Nioh3Studio.exe');
  await cp(source, target);
  const previousHash = await digest(target);

  // A verified regular file that cannot start: every digest check passes, the
  // replacement is installed, and process creation then fails.
  const staged = join(root, 'staged.exe');
  await writeFile(staged, 'not a runnable image\n');
  const replacementHash = await digest(staged);

  const helperDirectory = join(root, 'helper');
  await mkdir(helperDirectory);
  const helper = join(helperDirectory, 'apply-onefile-update.ps1');
  await cp(resolve('apps/tauri/src-tauri/apply-onefile-update.ps1'), helper);

  // Windows PowerShell's provider rejects canonical paths; the applier itself
  // takes one, so the target is passed in canonical form.
  const result = spawnSync(
    'powershell.exe',
    [
      '-NoProfile',
      '-NonInteractive',
      '-ExecutionPolicy',
      'Bypass',
      '-File',
      helper,
      '-ProcessId',
      String(absentPid),
      '-LauncherProcessId',
      String(absentLauncherPid),
      '-Target',
      `\\\\?\\${target}`,
      '-Staged',
      staged,
      '-FileHash',
      replacementHash,
      '-PreviousHash',
      previousHash,
      '-ManifestHash',
      '0'.repeat(64),
      '-Profile',
      root,
    ],
    {windowsHide: true, encoding: 'utf8', timeout: (options.timeout || 120) * 1000},
  );

  const receiptPath = join(helperDirectory, 'last-update-result.json');
  const receipt = existsSync(receiptPath)
    ? JSON.parse(await readFile(receiptPath, 'utf8'))
    : null;
  const restoredHash = await digest(target);
  const leftovers = (await readdir(root)).filter((name) => /\.(previous|update)-/.test(name));

  record = {
    schema: 'nioh3-onefile-rollback/v1',
    sourceExe: resolve(source),
    sourceExeSha256: previousHash,
    replacementSha256: replacementHash,
    applierExitCode: result.status,
    receiptStatus: receipt?.status ?? null,
    receiptError: receipt?.error ?? null,
    targetRestored: restoredHash === previousHash,
    restoredSha256: restoredHash,
    siblingLeftovers: leftovers.length,
    filesystemStimulus: 'isolated scratch root; the candidate bytes are read only',
  };

  assert.notEqual(result.status, 0, 'the applier reported success for a replacement that cannot start');
  assert.equal(result.status, 1, `unexpected applier exit code ${result.status}: ${result.stderr}`);
  assert.equal(receipt?.status, 'failed', JSON.stringify(record));
  assert.equal(record.targetRestored, true, JSON.stringify(record));
  assert.equal(leftovers.length, 0, `the failing update left sibling files behind: ${leftovers}`);

  if (options.out) await writeFile(resolve(options.out), `${JSON.stringify(record, null, 2)}\n`, 'utf8');
  console.log(JSON.stringify(record));
  console.log('TAURI_ONEFILE_ROLLBACK_RESTORES_ORIGINAL_OK');
} finally {
  await rm(root, {recursive: true, force: true, maxRetries: 3});
}
