import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

// Guard the shipped Tauri release path: the signed manifest must cover the
// archive this workflow builds, and the withdrawn Electron packager must not
// return to the workflow or its signer.
test('The Tauri release workflow signs the archive it builds and never publishes the withdrawn Electron package', () => {
  const workflow = readFileSync(new URL('../../../.github/workflows/release.yml', import.meta.url), 'utf8');
  const signer = readFileSync(new URL('../../../tools/build_tauri_update_manifest.mjs', import.meta.url), 'utf8');
  const archiveLine = workflow.split('\n').find(line => line.includes('run: python tools/archive_frontend_v2.py '));
  assert.ok(archiveLine);
  const name = archiveLine.match(/deliverables\/release\/([^/]+\.zip)\s*$/)?.[1];
  const signedName = workflow.match(/\$zip='deliverables\/release\/([^']+)'/)?.[1];
  assert.ok(name);
  assert.equal(signedName, name);
  assert.ok(workflow.includes('node tools/build_tauri_update_manifest.mjs $zip'));
  assert.ok(workflow.includes('60MB'));
  assert.ok(signer.includes("schema:'nioh3-tauri-update/v1'"));
  assert.ok(signer.includes('releases/download/v${version}/${name}'));
  assert.ok(signer.includes('verify(null,payload,publicKey,signature)'));
  assert.ok(!workflow.includes('softprops/action-gh-release'));
  assert.ok(!workflow.includes('build_frontend_v2.ps1'));
});
