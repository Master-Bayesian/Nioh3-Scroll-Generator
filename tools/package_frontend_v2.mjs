/** Assemble a reviewable portable directory; never invoke the legacy EXE updater. */
import { cp, copyFile, mkdir, readFile, writeFile, readdir, stat, rename } from 'node:fs/promises';
import { resolve, join, relative, dirname } from 'node:path';
import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
import { hashFile, verifyPortable } from '../packages/packaging/integrity.mjs';

const output = resolve(process.argv[2] || `deliverables/frontend-v2/portable-${Date.now()}`);
const workers = resolve(process.argv[3] || '.codex_tmp/v2-worker-clean-dist');
const workspace = JSON.parse(await readFile('package.json', 'utf8'));
const pythonEnvironment = JSON.parse(await readFile(join(workers, 'python-build-environment.json'), 'utf8'));
try { await stat(output); throw new Error('Output already exists; use a new directory'); }
catch (error) { if (error.code !== 'ENOENT') throw error; }
// Electron 44 downloads its binary lazily; packaging must not depend on a prior launch.
const electronDirectory = dirname(createRequire(import.meta.url).resolve('electron/package.json'));
execFileSync(process.execPath, [join(electronDirectory, 'install.js')], { stdio: 'inherit', windowsHide: true });
await mkdir(output, { recursive: true });
await cp(join(electronDirectory, 'dist'), output, { recursive: true });
await rename(join(output, 'electron.exe'), join(output, 'Nioh3ScrollEditorV2.exe'));
execFileSync(process.env.NIOH3_BUILD_PYTHON || 'python', ['tools/stamp_v2_executable.py', join(output, 'Nioh3ScrollEditorV2.exe'), workspace.version], { windowsHide: true });
const resources = join(output, 'resources');
await mkdir(join(resources, 'assets'), { recursive: true });
await copyFile(resolve('assets/nioh3-scroll-generator-icon.png'),join(resources,'assets/nioh3-scroll-generator-icon.png'));
await mkdir(join(resources, 'app'), { recursive: true });
await cp(resolve('apps/desktop/dist'), join(resources, 'app'), { recursive: true });
await writeFile(join(resources, 'app/package.json'), JSON.stringify({ name: 'nioh3-scroll-editor-v2', version: workspace.version, main: 'main.cjs' }, null, 2));
await mkdir(join(resources, 'worker'), { recursive: true });
await mkdir(join(resources, 'live-add'), { recursive: true });
for (const name of ['live_add_ce_server.lua', 'live_add_layout_ce.lua', 'probe_pickup_dispatch_noop_ce.lua',
  'build_dispatch_probe_code.lua', 'owned_breakpoint_lifecycle_ce.lua', 'ce_main_thread_timer.lua']) {
  await copyFile(resolve('research', name), join(resources, 'live-add', name));
}
await copyFile(resolve('tools/configure_live_add.ps1'), join(output, 'configure-live-add.ps1'));
for (const name of ['nioh3-search-worker.exe', 'nioh3-protected-worker.exe']) {
  await copyFile(join(workers, name), join(resources, 'worker', name));
}
await mkdir(join(resources, 'packages/contracts'), { recursive: true });
for (const name of await readdir('packages/contracts')) if (name.endsWith('.schema.json')) {
  await copyFile(resolve('packages/contracts', name), join(resources, 'packages/contracts', name));
}
await cp(join(workers, 'licenses'), join(output, 'licenses'), { recursive: true });
await copyFile(resolve('THIRD_PARTY_NOTICES.md'), join(output, 'THIRD_PARTY_NOTICES.md'));
await copyFile(resolve('third_party/nioh_savefile_decrypt/LICENSE'), join(output, 'licenses/Nioh-Savedata-Decryption-Tool-LICENSE'));
const javascript = [], visited = new Set();
async function dependency(name, from = resolve('.')) {
  const require = createRequire(join(from, 'package.json'));
  let folder = dirname(require.resolve(name)), info;
  for (;;) {
    try { info = JSON.parse(await readFile(join(folder, 'package.json'), 'utf8')); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
    if (info?.name === name) break;
    const parent = dirname(folder);
    if (parent === folder) throw new Error(`Cannot locate dependency metadata: ${name}`);
    folder = parent;
  }
  const key = `${info.name}@${info.version}`;
  if (visited.has(key)) return;
  visited.add(key);
  const notices = [];
  for (const entry of await readdir(folder, { withFileTypes: true })) {
    if (!entry.isFile() || !/^(license|copying|notice)/i.test(entry.name)) continue;
    const destination = join('licenses/javascript', key.replaceAll('/', '_'), entry.name);
    await mkdir(dirname(join(output, destination)), { recursive: true });
    await copyFile(join(folder, entry.name), join(output, destination));
    notices.push(destination.replaceAll('\\', '/'));
  }
  if (!notices.length) throw new Error(`Dependency notice missing: ${key}`);
  javascript.push({ name: info.name, version: info.version, license: info.license, notices });
  for (const child of Object.keys(info.dependencies || {}).sort()) await dependency(child, folder);
}
for (const name of Object.keys(workspace.dependencies).sort()) await dependency(name);
const dependencies = { schema: 'nioh3-portable-dependencies/v1',
  javascript: javascript.sort((a, b) => a.name.localeCompare(b.name)), pythonBuildEnvironment: pythonEnvironment,
  electron: JSON.parse(await readFile('node_modules/electron/package.json', 'utf8')).version,
  nodeBuildRuntime: process.versions.node,
  notices: 'Electron LICENSE and LICENSES.chromium.html are included unchanged. Python build tools are conservatively included in the environment inventory; this is not a complete linked SBOM.' };
await writeFile(join(output, 'dependency-manifest.json'), JSON.stringify(dependencies, null, 2) + '\n');
await copyFile(resolve('packaging/portable-readme.txt'), join(output, 'README.txt'));
const files = [];
async function scan(folder) {
  for (const entry of await readdir(folder, { withFileTypes: true })) {
    const path = join(folder, entry.name);
    if (entry.isDirectory()) await scan(path);
    else files.push({ path: relative(output, path).replaceAll('\\', '/'), size: (await stat(path)).size, sha256: await hashFile(path) });
  }
}
await scan(output);
const git = args => execFileSync('git', args, { encoding: 'utf8', windowsHide: true }).trim();
const changes = git(['status', '--porcelain']);
if (process.env.NIOH3_REQUIRE_CLEAN_SOURCE === '1' && changes) throw new Error('RELEASE_SOURCE_DIRTY:\n' + changes);
await writeFile(join(output, 'build-manifest.json'), JSON.stringify({ schema: 'nioh3-portable-manifest/v2', version: workspace.version, signed: false,
  git: { commit: git(['rev-parse', 'HEAD']), dirty: !!changes },
  dependencyLocks: { npm: await hashFile('package-lock.json'), python: await hashFile('packaging/requirements-v2.lock.txt') },
  files: files.sort((a, b) => a.path.localeCompare(b.path)) }, null, 2) + '\n');
await verifyPortable(output);
console.log(output);
