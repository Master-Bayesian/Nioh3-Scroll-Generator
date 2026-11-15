/** Inventory actual packaged E2E reports before the release is signed. */
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFile, writeFile} from 'node:fs/promises';
import {join, resolve} from 'node:path';

const [evidenceArg, releaseArg, sourceCommit, profile] = process.argv.slice(2);
assert(evidenceArg && releaseArg && /^[0-9a-f]{40}$/.test(sourceCommit || ''),
  'Usage: EVIDENCE_DIRECTORY RELEASE_DIRECTORY SOURCE_COMMIT release|extended');
assert(['release', 'extended'].includes(profile), 'Unknown packaged acceptance profile');
const evidence = resolve(evidenceArg), release = resolve(releaseArg);
const manifest = JSON.parse(await readFile(join(release, 'portable/build-manifest.json'), 'utf8'));
assert.equal(manifest.git.commit, sourceCommit, 'The staged package must match this workflow source');
assert.equal(manifest.git.dirty, false, 'The staged package must come from clean source');
const outerName = `Nioh3Studio-${manifest.version}-win-x64.exe`;
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const outerBytes = await readFile(join(release, outerName));
const outerSha256 = hash(outerBytes);
const reports = [];
async function requireReport(path, verify) {
  const bytes = await readFile(join(evidence, path));
  const report = JSON.parse(bytes.toString('utf8'));
  assert(verify(report), `Packaged acceptance report is incomplete or failed: ${path}`);
  reports.push({path, bytes: bytes.length, sha256: hash(bytes)});
}
await requireReport('verification.json', r => r.webview2 === true && r.workerHandshake === true && r.gameWrites === 0);
await requireReport('add-layout/verification.json', r => r.checks?.length > 0 && r.errors?.length === 0 && r.gameWrites === 0);
await requireReport('real-update-restart.json', r => r.replacement === true && r.workerHandshake === true && r.gameWrites === 0);
await requireReport('rust-packaged/host-package.json', r => r.backend === 'rust-packaged' && ['offline_search', 'save', 'runtime'].every(role => r.roles?.[role]));
for (const role of ['offline_search', 'save', 'runtime']) {
  await requireReport(`rust-packaged/identity-${role}.json`, r => r.role === role && r.verified === true && r.handshake === true && r.representativeOk === true && r.graph === 'rust');
}
await requireReport('rust-packaged/packaged-frontend.json', r => r.graph === 'rust-packaged' && r.releaseCandidate === true && r.profile === profile && r.userSaveTouched === false && r.gameWrites === 0 && r.fixtureWrites?.isolatedCopy === true && r.fixtureWrites?.realSaveTouched === false);
await requireReport('onefile-standalone.json', r => r.ok === true && r.sourceSha256 === outerSha256 && r.gameWrites === 0);
await requireReport('onefile-direct-launch.json', r => r.directLaunch === true && r.workerHandshake === true && r.sourceSha256 === outerSha256 && r.gameWrites === 0);
await requireReport('enemy-preview/report.json', r => r.pass === true && r.sourceExeSha256 === outerSha256 && r.sourceCommit === sourceCommit && r.gameWrites === 0);
await requireReport('onefile-real-update-restart.json', r => r.receiptStatus === 'completed' && r.originalSourceUnchanged === true && r.originalSha256 === outerSha256 && r.gameWrites === 0);
await requireReport('onefile-rollback.json', r => r.targetRestored === true && r.receiptStatus === 'failed' && r.sourceExeSha256 === outerSha256);
const inventory = {
  schema: 'nioh3-release-test-inventory/v1',
  version: manifest.version, sourceCommit, profile,
  workflowRunId: process.env.GITHUB_RUN_ID || null,
  outer: {name: outerName, bytes: outerBytes.length, sha256: outerSha256},
  boundary: {
    acceptance: 'Actual Windows host, WebView2 and packaged Rust workers; isolated synthetic encrypted saves and update targets',
    gameWrites: 0, userSaveTouched: false,
    gpu: 'Hosted runner acceptance uses explicit CPU opt-in; physical GPU parity and native UI acceptance are separate evidence',
    quality: 'Source lint, TLA+ and test results are retained by the separate PR workflows',
  },
  reports,
};
const output = join(release, 'test-inventory.json');
await writeFile(output, JSON.stringify(inventory, null, 2) + '\n');
console.log(JSON.stringify({output, sourceCommit, profile, reports: reports.length, outerSha256}));
