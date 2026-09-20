/**
 * Focused packaged-host resolution acceptance (F1/F2).
 *
 * The development variant (`verify-rust-backend.mjs`) selects the Rust worker
 * through a development environment variable. This one exercises the packaged
 * path instead: it points the real host at a staged Rust package root, and then
 * reads the host's own startup record of which worker graph and which exact argv
 * it resolved for every role.
 *
 * The record is written before the host validates the package manifest, so this
 * gate is usable against a staged runtime that is not a finished single-file
 * release yet. It deliberately stops at "the real host resolved the staged
 * binary and roots"; the packaged WebView2 workflow acceptance stays with
 * `verify-rust-backend.mjs` and the one-file gate.
 *
 * Usage:
 *   node apps/tauri/verify-host-package.mjs --package <portable root> \
 *     [--exe <nioh3-studio.exe>] [--out <json>] [--timeout <seconds>]
 */
import {spawn} from 'node:child_process';
import {existsSync} from 'node:fs';
import {mkdir, readFile, writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {join, resolve} from 'node:path';
import {tmpdir} from 'node:os';
import {mkdtemp} from 'node:fs/promises';

const ROLES = ['offline_search', 'save', 'runtime'];

/**
 * Fold a Windows path to a comparison form: every extended-length prefix is
 * dropped, separators are flattened and case is folded.
 *
 * The packaged host reports its resources through `resource_dir()`, which is
 * canonical (`\\?\...`), while this harness holds the same location in plain
 * form. Both name the same file, and a canonical path is never normalized by
 * the filesystem, so the two spellings only compare equal once the prefix is
 * folded away here. The prefix is applied globally because an argv string
 * carries one before each path value rather than only at its start.
 */
function compareForm(value) {
  return String(value)
    .replace(/\\\\\?\\UNC\\/gi, '\\\\')
    .replace(/\\\\\?\\/g, '')
    .replace(/[\\/]+/g, '/')
    .toLowerCase();
}

/** Compare package paths without caring which separator style each side used. */
function samePath(left, right) {
  return compareForm(resolve(left)) === compareForm(resolve(right));
}

function parseArgs(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index];
    if (key === '--package') options.package = argv[++index];
    else if (key === '--exe') options.exe = argv[++index];
    else if (key === '--out') options.out = argv[++index];
    else if (key === '--timeout') options.timeout = Number(argv[++index]);
    else throw new Error(`unknown argument: ${key}`);
  }
  if (!options.package) throw new Error('--package is required');
  return options;
}

async function readLogUntil(logPath, deadline) {
  // The host writes its resolution before it validates the package manifest, so
  // a short bounded wait is enough and a missing record is a real failure.
  for (;;) {
    if (existsSync(logPath)) {
      const text = await readFile(logPath, 'utf8');
      if (ROLES.every((role) => text.includes(`role=${role} `))) return text;
    }
    if (Date.now() > deadline) return existsSync(logPath) ? readFile(logPath, 'utf8') : '';
    await new Promise((accept) => setTimeout(accept, 150));
  }
}

/** Raw bytes, not a normalized PE digest: this is the executed file identity. */
async function rawHash(path) {
  const body = await readFile(path);
  return {
    path: resolve(path),
    size: body.byteLength,
    sha256: createHash('sha256').update(body).digest('hex'),
  };
}

function parseRoles(log) {
  const records = {};
  const wanted = new Set(ROLES);
  for (const line of log.split(/\r?\n/)) {
    const match = /\[worker-backend\] role=(\S+) executable=(.+?) sha256=(\S+) argv=(.*)$/.exec(
      line,
    );
    if (!match) continue;
    if (!wanted.has(match[1])) continue;
    records[match[1]] = {
      executable: match[2],
      sha256: match[3],
      argv: match[4].trim(),
      line,
    };
  }
  return records;
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const staged = resolve(options.package);
  const executable = resolve(
    options.exe || 'apps/tauri/src-tauri/target/debug/nioh3-studio.exe',
  );
  const timeoutMs = (options.timeout || 60) * 1000;
  if (!existsSync(executable)) throw new Error(`the host executable is missing: ${executable}`);
  const manifestPath = join(staged, 'worker', 'worker-backend.json');
  if (!existsSync(manifestPath)) {
    throw new Error(`the staged package carries no worker-backend.json: ${manifestPath}`);
  }
  const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));

  const scratch = await mkdtemp(join(tmpdir(), 'nioh3-host-package-'));
  const profile = join(scratch, 'profile');
  await mkdir(profile, {recursive: true});
  const logPath = join(profile, 'logs', 'desktop.log');
  const child = spawn(executable, ['--user-data-dir', profile], {
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
    env: {
      ...process.env,
      NIOH3_TAURI_TEST_ROOT: profile,
      NIOH3_TAURI_PACKAGE_ROOT: staged,
      NIOH3_PYTHON: '',
    },
  });
  let stderr = '';
  child.stderr.on('data', (chunk) => {
    stderr = (stderr + chunk).slice(-16000);
  });
  const log = await readLogUntil(logPath, Date.now() + timeoutMs);
  child.kill();
  const records = parseRoles(log);
  const evidence = {
    executable,
    package: staged,
    developmentBuild: true,
    releaseCandidate: false,
    backend: (/graph=(\S+)/.exec(log) || [])[1] || null,
    roles: {},
    stderrBytes: stderr.length,
  };
  if (evidence.backend !== 'rust-packaged') {
    throw new Error(
      `the packaged host resolved ${evidence.backend}, not the staged Rust graph; ` +
        `stderr=${stderr.slice(-2000)}`,
    );
  }
  for (const role of ROLES) {
    const record = records[role];
    if (!record) throw new Error(`the host logged no resolution for ${role}`);
    const declared = manifest.invocation[role];
    const expected = join(staged, 'worker', declared.binary);
    if (!samePath(record.executable, expected)) {
      throw new Error(`${role} resolved ${record.executable}, expected ${expected}`);
    }
    const dataRoot = join(staged, 'worker', 'runtime', 'nioh3_scroll_editor', 'data');
    const contractRoot = join(staged, 'packages', 'contracts');
    const argvPaths = compareForm(record.argv);
    if (!argvPaths.includes(`--data-root ${compareForm(dataRoot)}`)) {
      throw new Error(`${role} argv lacks the staged data root: ${record.argv}`);
    }
    if (
      !argvPaths.includes(`--contract-dir ${compareForm(contractRoot)}`)
    ) {
      throw new Error(`${role} argv lacks the staged contract root: ${record.argv}`);
    }
    if (!record.argv.includes('--accelerator ')) {
      throw new Error(`${role} argv does not pin the accelerator: ${record.argv}`);
    }
    if (role === 'offline_search') {
      if (!record.argv.startsWith('--packaged-worker ')) {
        throw new Error(`offline_search must use the packaged launch mode: ${record.argv}`);
      }
      if (record.argv.includes('--dev-preview-only')) {
        throw new Error(`offline_search used the development acknowledgement: ${record.argv}`);
      }
    } else {
      if (!record.argv.startsWith(`--role ${role} `)) {
        throw new Error(`${role} argv lacks its role: ${record.argv}`);
      }
      if (record.argv.includes('--dev-protected-only')) {
        throw new Error(`${role} used the development acknowledgement: ${record.argv}`);
      }
      if (!argvPaths.includes(`--state-root ${compareForm(profile)}`)) {
        throw new Error(`${role} argv lacks the injected state root: ${record.argv}`);
      }
    }
    const digest = createHash('sha256').update(await readFile(resolve(record.executable))).digest('hex');
    if (digest !== record.sha256) {
      throw new Error(`${role} recorded ${record.sha256} but the file hashes ${digest}`);
    }
    evidence.roles[role] = record;
  }
  const roleBinaries = {};
  for (const role of ROLES) {
    roleBinaries[role] = await rawHash(resolve(records[role].executable));
  }
  evidence.artifact = {
    innerExe: await rawHash(executable),
    buildManifest: existsSync(join(staged, 'build-manifest.json'))
      ? await rawHash(join(staged, 'build-manifest.json'))
      : null,
    workerManifest: await rawHash(manifestPath),
    roleBinaries,
  };
  evidence.gameFileVersions = Object.fromEntries(
    ROLES.map((role) => [
      role,
      (/--game-file-version\s+(\S+)/.exec(records[role].argv) || [])[1] || null,
    ]),
  );
  if (options.out) {
    const out = resolve(options.out);
    await mkdir(resolve(out, '..'), {recursive: true});
    await writeFile(out, `${JSON.stringify(evidence, null, 2)}\n`, 'utf8');
  }
  console.log(JSON.stringify(evidence));
  console.log('TAURI_HOST_PACKAGE_RESOLUTION_OK');
}

main().catch((error) => {
  console.error(`TAURI_HOST_PACKAGE_RESOLUTION_FAILED: ${error.message}`);
  process.exitCode = 1;
});
