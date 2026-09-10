/** Accidental-corruption check. This unsigned manifest is not a trust root. */
import { createHash } from 'node:crypto';
import * as nodeFilesystem from 'node:fs';
import { resolve, relative, isAbsolute } from 'node:path';

export async function hashFile(path, filesystem = nodeFilesystem) {
  const hash = createHash('sha256');
  for await (const chunk of filesystem.createReadStream(path)) hash.update(chunk);
  return hash.digest('hex');
}

export async function verifyPortable(directory, filesystem = nodeFilesystem) {
  // Electron virtualizes .asar as a directory. Its caller supplies original-fs
  // so the manifest always verifies physical archive bytes and file types.
  const { lstat, readFile, realpath } = filesystem.promises;
  const root = await realpath(directory);
  const manifestPath = resolve(root, 'build-manifest.json');
  if ((await lstat(manifestPath)).size > 4 * 1024 * 1024) throw new Error('PACKAGE_MANIFEST_TOO_LARGE');
  const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
  if (manifest.schema !== 'nioh3-portable-manifest/v2' || typeof manifest.version !== 'string' ||
      !Array.isArray(manifest.files) || !manifest.files.length || manifest.files.length > 10000) {
    throw new Error('PACKAGE_MANIFEST_INVALID');
  }
  const paths = new Set();
  for (const entry of manifest.files) {
    if (!entry || typeof entry.path !== 'string' || entry.path.length > 1024 ||
        !/^[a-zA-Z0-9_. /@+()-]+$/.test(entry.path) || entry.path.split('/').some(p => !p || p === '.' || p === '..') ||
        isAbsolute(entry.path) || !/^[0-9a-f]{64}$/.test(entry.sha256) ||
        !Number.isSafeInteger(entry.size) || entry.size < 0 || paths.has(entry.path.toLowerCase())) {
      throw new Error('PACKAGE_MANIFEST_INVALID_ENTRY');
    }
    paths.add(entry.path.toLowerCase());
    const path = resolve(root, entry.path), resolved = await realpath(path);
    const fromRoot = relative(root, resolved);
    if (fromRoot.startsWith('..') || isAbsolute(fromRoot) || !(await lstat(path)).isFile()) throw new Error(`PACKAGE_PATH_INVALID: ${entry.path}`);
    if ((await lstat(path)).size !== entry.size || await hashFile(path, filesystem) !== entry.sha256) {
      throw new Error(`PACKAGE_FILE_MISMATCH: ${entry.path}`);
    }
  }
  for (const required of ['Nioh3ScrollEditorV2.exe', 'resources/app/main.cjs', 'resources/app/preload.cjs',
    'resources/app/renderer.js', 'resources/app/index.html', 'resources/app/package.json',
    'resources/app/review.js', 'resources/app/review.css', 'resources/app/review.html',
    'resources/app/apply-update.ps1', 'resources/app/extract-update.ps1', 'resources/assets/nioh3-scroll-generator-icon.png',
    'resources/worker/nioh3-search-worker.exe', 'resources/worker/nioh3-protected-worker.exe',
    ...['request', 'response', 'protected-request', 'protected-response'].map(name => `resources/packages/contracts/${name}.schema.json`)]) {
    if (!paths.has(required.toLowerCase())) throw new Error(`PACKAGE_FILE_UNLISTED: ${required}`);
  }
  return { version: manifest.version, fileCount: manifest.files.length, signed: manifest.signed === true,
    manifestSha256: await hashFile(manifestPath, filesystem) };
}
