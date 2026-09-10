import { lstat, readFile, readdir, realpath, rm, writeFile } from 'node:fs/promises';
import { basename, dirname, join, resolve } from 'node:path';
import { verifyPortable } from '../../../packages/packaging/integrity.mjs';

const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const samePath = (a: string, b: string) => process.platform === 'win32' ? a.toLowerCase() === b.toLowerCase() : a === b;
const missing = (e: unknown) => (e as NodeJS.ErrnoException).code === 'ENOENT';

async function physicalFiles(root: string): Promise<string[]> {
  const files: string[] = [];
  async function walk(folder: string) {
    for (const entry of await readdir(folder, { withFileTypes: true })) {
      const path = join(folder, entry.name);
      if ((await lstat(path)).isSymbolicLink()) throw Error('UPDATE_CLEANUP_LINK_REFUSED');
      if (entry.isDirectory()) await walk(path);
      else if (entry.isFile()) files.push(path.slice(root.length + 1).replaceAll('\\', '/'));
      else throw Error('UPDATE_CLEANUP_SPECIAL_FILE_REFUSED');
    }
  }
  await walk(root);
  return files;
}

/** Only delete a physical, immediate child of the explicitly owned parent. */
async function removeChild(parent: string, child: string) {
  const canonicalParent = await realpath(parent);
  const expected = join(canonicalParent, basename(child));
  if (!samePath(resolve(child), join(resolve(parent), basename(child))) ||
      (await lstat(child)).isSymbolicLink() || !samePath(await realpath(child), expected)) {
    throw Error('UPDATE_CLEANUP_PATH_REFUSED');
  }
  await physicalFiles(child);
  await rm(child, { recursive: true, force: false });
}

export async function removeUpdateCache(root: string, folder: string) {
  if (!uuid.test(basename(folder))) throw Error('UPDATE_CACHE_NAME_REFUSED');
  try { await removeChild(root, folder); } catch (error) { if (!missing(error)) throw error; }
}

/** Called only after the installed package, renderer and worker have started. */
export async function finishInstalledUpdate(root: string, target: string, version: string, filesystem?: typeof import('node:fs')) {
  const reportPath = join(root, 'last-update-result.json');
  let receipt: Record<string, any> | null = null;
  try { receipt = JSON.parse(await readFile(reportPath, 'utf8')); }
  catch (error) { if (!missing(error)) throw error; }
  if (receipt && ['awaiting-startup', 'launched'].includes(receipt.status)) {
    const previous = receipt.previous;
    const parent = dirname(resolve(target));
    const prefix = basename(target) + '.previous-';
    if (typeof previous !== 'string' || !samePath(dirname(resolve(previous)), parent) ||
        !basename(previous).startsWith(prefix) || !/^[0-9a-f]{32}$/i.test(basename(previous).slice(prefix.length)) ||
        receipt.target && !samePath(resolve(receipt.target), resolve(target))) throw Error('UPDATE_RECEIPT_TARGET_REFUSED');
    const installed = await verifyPortable(target, filesystem);
    if (installed.version !== version || receipt.manifestHash && installed.manifestSha256 !== receipt.manifestHash.toLowerCase()) {
      throw Error('UPDATE_RECEIPT_PACKAGE_MISMATCH');
    }
    try {
      const old = await verifyPortable(previous, filesystem);
      if (old.version === version) throw Error('UPDATE_PREVIOUS_VERSION_REFUSED');
      const manifest = JSON.parse(await readFile(join(previous, 'build-manifest.json'), 'utf8'));
      const allowed = new Set(['build-manifest.json', ...manifest.files.map((f: { path: string }) => f.path)]);
      const files = await physicalFiles(previous);
      // A user may have stored saves or notes beside the EXE. Never remove them.
      if (files.some(file => !allowed.has(file))) throw Error('UPDATE_PREVIOUS_HAS_USER_FILES');
      await removeChild(parent, previous);
    } catch (error) { if (!missing(error)) throw error; }
    await writeFile(reportPath, JSON.stringify({ ...receipt, status: 'completed', completedAt: new Date().toISOString() }) + '\n');
  } else if (receipt && !['completed', 'failed'].includes(receipt.status)) {
    // An installer still preparing or waiting for shutdown retains ownership.
    return;
  }
  let entries;
  try { entries = await readdir(root, { withFileTypes: true }); }
  catch (error) { if (missing(error)) return; throw error; }
  for (const entry of entries) if (entry.isDirectory() && uuid.test(entry.name)) {
    await removeUpdateCache(root, join(root, entry.name));
  }
}
