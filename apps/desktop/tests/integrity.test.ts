import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { join, dirname } from 'node:path';
import { tmpdir } from 'node:os';
import { verifyPortable, hashFile } from '../../../packages/packaging/integrity.mjs';

test('portable verification rejects corrupt files, traversal and missing worker coverage', async () => {
  const root = await mkdtemp(join(tmpdir(), 'nioh3-package-check-'));
  try {
    const paths = ['Nioh3ScrollEditorV2.exe', 'resources/app/main.cjs', 'resources/app/preload.cjs',
      'resources/app/review.js', 'resources/app/review.css', 'resources/app/review.html', 'resources/app/extract-update.ps1', 'resources/app/apply-update.ps1', 'resources/assets/nioh3-scroll-generator-icon.png', 'resources/app/renderer.js', 'resources/app/index.html', 'resources/app/package.json',
      'resources/worker/nioh3-search-worker.exe', 'resources/worker/nioh3-protected-worker.exe',
      ...['request', 'response', 'protected-request', 'protected-response'].map(name => `resources/packages/contracts/${name}.schema.json`)];
    const files = [];
    for (const path of paths) {
      await mkdir(dirname(join(root, path)), { recursive: true });
      await writeFile(join(root, path), '0');
      files.push({ path, size: 1, sha256: await hashFile(join(root, path)) });
    }
    const manifest = { schema: 'nioh3-portable-manifest/v2', version: 'test', signed: false, files };
    const write = () => writeFile(join(root, 'build-manifest.json'), JSON.stringify(manifest));
    await write(); assert.equal((await verifyPortable(root)).fileCount, files.length);
    await writeFile(join(root, paths[0]), '1');
    await assert.rejects(verifyPortable(root), /PACKAGE_FILE_MISMATCH/);
    await writeFile(join(root, paths[0]), '0');
    const saved = files[0].path; files[0].path = '../outside'; await write();
    await assert.rejects(verifyPortable(root), /PACKAGE_MANIFEST_INVALID_ENTRY/);
    files[0].path = saved;
    manifest.files = files.filter(entry => !entry.path.endsWith('nioh3-protected-worker.exe')); await write();
    await assert.rejects(verifyPortable(root), /PACKAGE_FILE_UNLISTED/);
  } finally { await rm(root, { recursive: true, force: true }); }
});
