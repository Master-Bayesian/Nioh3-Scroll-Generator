/**
 * Truthful worker-identity acceptance (M4.4).
 *
 * The historical one-file contract *records* `workerHandshake: true` without
 * proving which worker answered. This helper replaces that claim with a real
 * one: it hashes the packaged binary, spawns it, performs a genuine framed-JSON
 * handshake, sends one safe read-only representative request, and reports what
 * happened. Nothing here trusts the manifest: the role-to-binary mapping, the
 * CLI grammar, the path confinement, the required resources/schemas and the
 * digest are all re-derived independently, so a manifest cannot self-assert a
 * successful launch.
 *
 * Two entry points:
 *   verifyWorkerIdentity(...)  strict, for a staged Rust runtime (the shipped
 *                              graph);
 *   verifyShippedWorker(...)   best-effort, never throws, for a package with no
 *                              staged manifest (the development/parity graph).
 *
 * Usage:
 *   node apps/tauri/verify-worker-identity.mjs \
 *     --runtime <runtime root> --role offline_search|save|runtime [--out file]
 */
import {createHash} from 'node:crypto';
import {existsSync, mkdirSync, realpathSync} from 'node:fs';
import {mkdir, readFile, stat, writeFile} from 'node:fs/promises';
import {join, relative, resolve, sep} from 'node:path';
import {tmpdir} from 'node:os';
import {spawn} from 'node:child_process';
import {fileURLToPath} from 'node:url';

// Independently derived expectations. The manifest is data to validate, never
// an oracle.
const ROLE_SPEC = {
  offline_search: {
    binary: 'nioh3-search-worker.exe',
    requiredFlags: ['--packaged-worker'],
    schemas: ['request.schema.json', 'response.schema.json'],
    representative: {method: 'candidate.preview', params: {seed: 1, rarity: 3, level: 180}},
    modeFlag: '--packaged-worker',
  },
  save: {
    binary: 'nioh3-protected-worker.exe',
    requiredFlags: [],
    schemas: ['protected-request.schema.json', 'protected-response.schema.json'],
    representative: {method: 'save.discover', params: {}},
    roleArgument: 'save',
  },
  runtime: {
    binary: 'nioh3-protected-worker.exe',
    requiredFlags: [],
    schemas: ['protected-request.schema.json', 'protected-response.schema.json'],
    representative: {method: 'runtime.status', params: {}},
    roleArgument: 'runtime',
  },
};

const VALUE_FLAGS = new Set(['--data-root', '--contract-dir', '--state-root', '--role', '--accelerator']);

/** Scratch root for an isolated state root: prefer the D: workspace when present. */
function scratchRoot() {
  const preferred = 'D:/Nioh3_v080_deliverables/tmp';
  try {
    mkdirSync(preferred, {recursive: true});
    return preferred;
  } catch {
    return tmpdir();
  }
}

function inside(root, candidate, label) {
  const resolved = resolve(candidate);
  const rel = relative(root, resolved);
  if (rel === '' || (!rel.startsWith('..' + sep) && rel !== '..' && !/^[a-zA-Z]:/.test(rel))) {
    return resolved;
  }
  throw new Error(`${label} escapes the package root: ${candidate}`);
}

function parseArgs(argv) {
  const options = {
    role: 'offline_search',
    out: null,
    stateRoot: null,
    shipped: false,
    localAppData: null,
    request: null,
    params: null,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index];
    if (key === '--runtime') options.runtime = argv[++index];
    else if (key === '--role') options.role = argv[++index];
    else if (key === '--out') options.out = argv[++index];
    else if (key === '--state-root') options.stateRoot = argv[++index];
    else if (key === '--shipped') options.shipped = true;
    else if (key === '--local-app-data') options.localAppData = argv[++index];
    else if (key === '--request') options.request = argv[++index];
    else if (key === '--params') options.params = argv[++index];
    else throw new Error(`unknown argument: ${key}`);
  }
  if (!options.runtime) throw new Error('--runtime is required');
  return options;
}

function writeFrame(stream, value) {
  const body = Buffer.from(JSON.stringify(value), 'utf8');
  const header = Buffer.alloc(4);
  header.writeUInt32LE(body.length, 0);
  stream.write(Buffer.concat([header, body]));
}

function readFrame(stream) {
  return new Promise((accept, reject) => {
    let buffer = Buffer.alloc(0);
    const onData = (chunk) => {
      buffer = Buffer.concat([buffer, chunk]);
      if (buffer.length < 4) return;
      const size = buffer.readUInt32LE(0);
      if (buffer.length < 4 + size) return;
      stream.off('data', onData);
      accept(JSON.parse(buffer.subarray(4, 4 + size).toString('utf8')));
    };
    stream.on('data', onData);
    stream.on('error', reject);
  });
}

async function sha256(path) {
  return createHash('sha256').update(await readFile(path)).digest('hex');
}

/** One worker session: spawn, handshake, one representative request. */
async function runWorkerSession({
  binary,
  args,
  environment,
  representative,
  extraRequest,
  timeoutMs,
}) {
  const child = spawn(binary, args, {
    stdio: ['pipe', 'pipe', 'pipe'],
    windowsHide: true,
    env: environment,
  });
  let stderr = '';
  child.stderr.on('data', (chunk) => {
    stderr += chunk.toString('utf8');
  });
  const exited = new Promise((_, reject) => {
    child.on('error', (error) => reject(new Error(`spawn failed: ${error.message}`)));
    child.on('close', (code) => {
      reject(
        new Error(
          `worker exited (code ${code}) before answering` +
            (stderr ? `: ${stderr.trim().split('\n').slice(-3).join(' | ')}` : ''),
        ),
      );
    });
  });
  exited.catch(() => {});
  const timer = setTimeout(() => child.kill(), timeoutMs);
  try {
    writeFrame(child.stdin, {protocol: 1, id: 'identity-1', method: 'handshake', params: {}});
    const handshake = await Promise.race([readFrame(child.stdout), exited]);
    if (!handshake.ok) throw new Error(`handshake refused: ${JSON.stringify(handshake.error)}`);
    // Protected operations are job-based: an accepted reply carries a job_id
    // and the work is only done once the snapshot is terminal. Every request
    // here waits for its own terminal state before the next one is sent - the
    // shipped host refuses a second request while the previous owner is still
    // running, and a faster backend would otherwise be refused for a job that
    // finished microseconds later.
    const settleJob = async (reply, method) => {
      const jobId = reply.result?.job_id;
      if (!jobId) return reply;
      const deadline = Date.now() + timeoutMs;
      for (;;) {
        writeFrame(child.stdin, {
          protocol: 1,
          id: 'identity-job',
          method: 'job.snapshot',
          params: {job_id: jobId},
        });
        const snapshot = await Promise.race([readFrame(child.stdout), exited]);
        if (!snapshot.ok) {
          throw new Error(`job.snapshot refused: ${JSON.stringify(snapshot.error)}`);
        }
        const state = snapshot.result?.state;
        if (state === 'completed' || state === 'failed' || state === 'cancelled') {
          return snapshot;
        }
        if (Date.now() > deadline) throw new Error(`${method} did not finish`);
      }
    };
    let representativeReply = null;
    if (representative) {
      writeFrame(child.stdin, {
        protocol: 1,
        id: 'identity-2',
        method: representative.method,
        params: representative.params,
      });
      representativeReply = await Promise.race([readFrame(child.stdout), exited]);
      if (!representativeReply.ok) {
        throw new Error(
          `representative ${representative.method} refused: ${JSON.stringify(representativeReply.error)}`,
        );
      }
      representativeReply = await settleJob(representativeReply, representative.method);
      if (representativeReply.result?.state === 'failed') {
        throw new Error(
          `representative ${representative.method} failed: ${JSON.stringify(representativeReply.result.error)}`,
        );
      }
    }
    let extraReply = null;
    if (extraRequest) {
      writeFrame(child.stdin, {
        protocol: 1,
        id: 'identity-3',
        method: extraRequest.method,
        params: extraRequest.params,
      });
      extraReply = await Promise.race([readFrame(child.stdout), exited]);
      if (!extraReply.ok) {
        throw new Error(
          `${extraRequest.method} refused: ${JSON.stringify(extraReply.error)}`,
        );
      }
      extraReply = await settleJob(extraReply, extraRequest.method);
    }
    writeFrame(child.stdin, {protocol: 1, id: 'identity-3', method: 'shutdown', params: {}});
    return {
      contractDigest: handshake.result?.contract_digest ?? null,
      contextDigest: handshake.result?.context?.context_digest ?? null,
      capabilities: handshake.result?.capabilities ?? null,
      representativeMethod: representative?.method ?? null,
      representativeOk: representativeReply ? representativeReply.ok === true : null,
      extraRequestMethod: extraRequest?.method ?? null,
      extraRequestResult: extraReply?.result ?? null,
      stderrBytes: Buffer.byteLength(stderr, 'utf8'),
    };
  } finally {
    clearTimeout(timer);
    child.stdin.end();
    child.kill();
  }
}

/**
 * Validate a staged Rust manifest against the independent expectations above and
 * return the argv this helper will actually execute.
 */
export function validateStagedManifest(manifest, root, role) {
  const spec = ROLE_SPEC[role];
  if (!spec) throw new Error(`unsupported role ${role}`);
  if (manifest.backend !== 'rust') {
    throw new Error(
      `worker-backend.json reports backend ${manifest.backend}; the identity ` +
        'acceptance only covers the staged Rust graph',
    );
  }
  const contract = manifest.launchContract;
  if (!contract || contract.schema !== 'nioh3-worker-launch-contract/v1') {
    throw new Error('worker-backend.json carries no launch contract');
  }
  const binaries = manifest.binaries || [];
  const entry = binaries.find((item) => item.packagedName === spec.binary);
  if (!entry) throw new Error(`worker-backend.json has no binary for role ${role}`);
  if ((entry.sha256 || '').length !== 64 || !/^[0-9a-f]{64}$/.test(entry.sha256)) {
    throw new Error(`binary digest for ${spec.binary} is not a sha256 hex string`);
  }
  const record = manifest.invocation?.[role];
  if (!record || !Array.isArray(record.argv)) {
    throw new Error(`worker-backend.json has no invocation for role ${role}`);
  }
  if (record.mode !== 'packaged' || record.binary !== spec.binary) {
    throw new Error(`invocation for ${role} does not describe a packaged ${spec.binary}`);
  }
  for (const flag of spec.requiredFlags) {
    if (!record.argv.includes(flag)) throw new Error(`invocation for ${role} lacks ${flag}`);
  }
  const allowed = new Set(contract.allowedFlags || []);
  const values = {};
  for (let index = 0; index < record.argv.length; index += 1) {
    const token = record.argv[index];
    if (!token.startsWith('--')) throw new Error(`invocation for ${role} has a stray value ${token}`);
    if (!allowed.has(token)) throw new Error(`invocation for ${role} uses ${token}, which the contract does not allow`);
    if (VALUE_FLAGS.has(token)) {
      const value = record.argv[index + 1];
      if (value === undefined) throw new Error(`invocation for ${role} has ${token} without a value`);
      values[token] = value;
      index += 1;
    }
  }
  if (spec.roleArgument && values['--role'] !== spec.roleArgument) {
    throw new Error(`invocation for ${role} passes --role ${values['--role']}`);
  }
  // User state is broker-owned and must live outside the package. The manifest
  // may only carry the placeholder, so it can never pick its own write path
  // (and state can never land in the extracted one-file cache).
  const statePolicy = contract.stateRoot || {};
  const placeholder = statePolicy.placeholder || '<state root>';
  const usesStateRoot = Object.prototype.hasOwnProperty.call(values, '--state-root');
  if (usesStateRoot && values['--state-root'] !== placeholder) {
    throw new Error(
      `invocation for ${role} names a state root (${values['--state-root']}); it must be the ` +
        `${placeholder} placeholder the broker injects`,
    );
  }
  if (statePolicy.policy !== 'broker-injected-external' || statePolicy.packageConfined !== false) {
    throw new Error('launch contract does not declare a broker-injected external state root');
  }
  const dataRoot = inside(root, (values['--data-root'] || '').replace('<runtime>', root), '--data-root');
  const contractDir = inside(root, (values['--contract-dir'] || '').replace('<runtime>', root), '--contract-dir');
  const accelerator = values['--accelerator']
    ? inside(root, values['--accelerator'].replace('<runtime>', root), '--accelerator')
    : null;
  for (const resource of contract.requiredResources || []) {
    const path = join(root, resource);
    if (!(path === root || path.startsWith(root + sep))) {
      throw new Error(`required resource ${resource} is outside the package`);
    }
  }
  // The role's own contract schemas must ship in the package; the manifest
  // cannot declare a contract it does not carry.
  const schemaDir = join(root, 'packages', 'contracts');
  for (const schema of spec.schemas) {
    const candidate = join(schemaDir, schema);
    if (!(candidate.startsWith(schemaDir + sep))) {
      throw new Error(`schema ${schema} is outside the contract directory`);
    }
    if (!existsSync(candidate)) {
      throw new Error(`required contract schema is missing: ${schema}`);
    }
  }
  return {entry, dataRoot, contractDir, accelerator, argv: record.argv, spec, usesStateRoot};
}

/** SHA-256 of the role set's two schema files, in the workers' own order. */
async function packageContractDigest(root, role) {
  const spec = ROLE_SPEC[role];
  const digest = createHash('sha256');
  for (const schema of spec.schemas) {
    digest.update(await readFile(join(root, 'packages', 'contracts', schema)));
  }
  return digest.digest('hex');
}

/** Strict identity acceptance for one role of a staged Rust runtime. */
export async function verifyWorkerIdentity({
  runtime,
  role,
  stateRoot,
  localAppData,
  extraRequest,
  timeoutMs = 30000,
}) {
  const root = resolve(runtime);
  const spec = ROLE_SPEC[role];
  if (!spec) throw new Error(`unsupported role ${role}; expected offline_search, save or runtime`);
  const manifestPath = join(root, 'worker', 'worker-backend.json');
  const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
  const validated = validateStagedManifest(manifest, root, role);
  const {entry} = validated;
  const binary = join(root, 'worker', entry.packagedName);
  await stat(binary);
  const digest = await sha256(binary);
  if (digest !== entry.sha256) {
    throw new Error(
      `packaged ${entry.packagedName} does not match the staged manifest ` +
        `(expected ${entry.sha256}, found ${digest})`,
    );
  }
  const invocation = manifest.invocation?.[role];
  const isolatedState = resolve(stateRoot || join(scratchRoot(), `nioh3-identity-${process.pid}`));
  if (validated.usesStateRoot) {
    const rel = relative(root, isolatedState);
    if (!(rel === '..' || rel.startsWith('..' + sep) || /^[a-zA-Z]:/.test(rel))) {
      throw new Error(
        `the injected state root must live outside the package: ${isolatedState}`,
      );
    }
  }
  await mkdir(isolatedState, {recursive: true});
  const args = invocation.argv.map((value) => {
    if (value === '<runtime>') return root;
    if (value === '<state root>') return isolatedState;
    return value.replace('<runtime>', root);
  });
  const environment = {...process.env};
  if (localAppData) environment.LOCALAPPDATA = resolve(localAppData);
  delete environment.NIOH3_PYTHON;
  if (environment.PATH) {
    // No ambient Python: the packaged worker must start from its own resources.
    environment.PATH = environment.PATH.split(';')
      .filter((part) => !/python/i.test(part))
      .join(';');
  }
  const session = await runWorkerSession({
    binary,
    args,
    environment,
    representative: spec.representative,
    extraRequest:
      extraRequest && extraRequest.method
        ? {method: extraRequest.method, params: extraRequest.params || {}}
        : null,
    timeoutMs,
  });
  // Independent contract digest: the manifest's recorded value, the package's
  // own bytes and the worker's handshake must all agree.
  const roleSet = role === 'offline_search' ? 'offline_search' : 'protected';
  const recorded = manifest.launchContract?.contractDigests?.[roleSet] || null;
  const recomputed = await packageContractDigest(root, role);
  if (!recorded || recorded !== recomputed) {
    throw new Error(
      `contract digest mismatch: manifest ${recorded}, package ${recomputed}`,
    );
  }
  if ((session.contractDigest || '') !== recomputed) {
    throw new Error(
      `the worker reports contract digest ${session.contractDigest}, the package carries ${recomputed}`,
    );
  }
  return {
    role,
    binary: entry.packagedName,
    sha256: digest,
    backend: 'rust',
    handshake: true,
    contextDigest: session.contextDigest,
    capabilities: session.capabilities,
    contractDigest: recomputed,
    contractDigestSources: 'manifest+package+handshake',
    representativeMethod: session.representativeMethod,
    representativeOk: session.representativeOk,
    extraRequestMethod: session.extraRequestMethod,
    extraRequestResult: session.extraRequestResult,
    dataRoot: relative(root, validated.dataRoot),
    contractDir: relative(root, validated.contractDir),
    stateRootUsed: validated.usesStateRoot ? isolatedState : null,
    stateRootSource: validated.usesStateRoot ? 'broker-injected-external' : null,
    localAppDataUsed: localAppData ? resolve(localAppData) : null,
    graph: 'rust',
  };
}

/**
 * Best-effort identity report for a package with no staged Rust manifest (the
 * development/parity graph). Never throws: an unverifiable worker is reported
 * as unverified, never as a fabricated success.
 */
export async function verifyShippedWorker({runtime, role, stateRoot, timeoutMs = 60000}) {
  const root = resolve(runtime);
  const spec = ROLE_SPEC[role];
  const report = {role, verified: false, binary: spec?.binary ?? null};
  if (!spec) return {...report, reason: `unsupported role ${role}`};
  const binary = join(root, 'worker', spec.binary);
  try {
    await stat(binary);
  } catch (error) {
    return {...report, reason: `packaged worker missing: ${spec.binary}`};
  }
  const digest = await sha256(binary);
  const isolatedState = resolve(stateRoot || join(scratchRoot(), `nioh3-shipped-${process.pid}-${role}`));
  const environment = {...process.env, NIOH3_STATE_ROOT: isolatedState};
  const args = spec.roleArgument ? ['--role', spec.roleArgument] : [];
  try {
    if (spec.roleArgument) await mkdir(isolatedState, {recursive: true});
    const session = await runWorkerSession({
      binary,
      args,
      environment,
      representative: spec.representative,
      timeoutMs,
    });
    return {
      ...report,
      verified: true,
      sha256: digest,
      handshake: true,
      contextDigest: session.contextDigest,
      representativeMethod: session.representativeMethod,
      representativeOk: session.representativeOk,
    };
  } catch (error) {
    return {...report, sha256: digest, handshake: false, reason: error.message};
  }
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.shipped) {
    // Reporting mode for the default/shipped graph: always reports, never
    // throws, and never claims a handshake it did not observe.
    const report = await verifyShippedWorker({
      runtime: options.runtime,
      role: options.role,
      stateRoot: options.stateRoot,
    });
    if (options.out) {
      await writeFile(resolve(options.out), JSON.stringify(report, null, 2) + '\n', 'utf8');
    }
    console.log(JSON.stringify(report));
    console.log(report.verified ? 'TAURI_WORKER_IDENTITY_VERIFIED' : 'TAURI_WORKER_IDENTITY_UNVERIFIED');
    return;
  }
  const stagedManifest = join(resolve(options.runtime), 'worker', 'worker-backend.json');
  if (process.env.NIOH3_WORKER_IDENTITY_OPT_IN !== '1' && !existsSync(stagedManifest)) {
    throw new Error(
      'refusing to claim a worker identity for a package with no staged Rust ' +
        'worker manifest (set NIOH3_WORKER_IDENTITY_OPT_IN=1 only to force a ' +
        'graph that is known to be staged)',
    );
  }
  const identity = await verifyWorkerIdentity({
    runtime: options.runtime,
    role: options.role,
    stateRoot: options.stateRoot,
    localAppData: options.localAppData,
    extraRequest: options.request
      ? {method: options.request, params: options.params ? JSON.parse(options.params) : {}}
      : null,
  });
  if (options.out) {
    await writeFile(resolve(options.out), JSON.stringify(identity, null, 2) + '\n', 'utf8');
  }
  console.log(JSON.stringify(identity));
  console.log('TAURI_WORKER_IDENTITY_OK');
}

/**
 * Run the command line only when this file is the entry point.
 *
 * `verify-onefile.mjs` imports `verifyWorkerIdentity` to assert the identity
 * from inside the single-file product. An unguarded `main()` ran on that
 * import with the importer's argv, failed with "--runtime is required" and
 * forced a failure exit for a gate that had actually passed.
 */
function invokedDirectly() {
  const entry = process.argv[1];
  if (!entry) return false;
  try {
    return (
      realpathSync(entry).toLowerCase() ===
      realpathSync(fileURLToPath(import.meta.url)).toLowerCase()
    );
  } catch {
    return false;
  }
}

if (invokedDirectly()) {
  main().catch((error) => {
    console.error('TAURI_WORKER_IDENTITY_FAILED: ' + error.message);
    process.exitCode = 1;
  });
}
