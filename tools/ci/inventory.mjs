import {existsSync} from 'node:fs';
import {resolve} from 'node:path';
import {ensure, trackedFiles, trackedEntries, digest, readJson} from './core.mjs';

const rust = [
  ['domain', 'crates/nioh3-domain/Cargo.toml', []],
  ['data', 'crates/nioh3-data/Cargo.toml', []],
  ['worker', 'crates/nioh3-worker/Cargo.toml', []],
  ['save', 'crates/nioh3-save/Cargo.toml', []],
  ['runtime', 'crates/nioh3-runtime/Cargo.toml', ['--features', 'test-helper']],
  ['protected', 'crates/nioh3-protected/Cargo.toml', []],
  ['tauri', 'apps/tauri/src-tauri/Cargo.toml', []],
  ['launcher', 'apps/launcher/Cargo.toml', []],
];

const browser = [
  ['live-equipment', 'apps/workshop/verify-live-equipment-add.mjs', 'equipment-ui-e2e.json'],
  ['runtime-compatibility', 'apps/workshop/verify-runtime-compatibility.mjs', 'compatibility-ui.json'],
  ['character-draft', 'apps/workshop/verify-character-draft.mjs', 'verify-character-draft-final.json'],
  ['scroll-completion', 'apps/workshop/verify-scroll-completion.mjs', 'completion-ui-e2e.json'],
];

export const locales = ['zh-CN', 'en-US', 'ja-JP'];

const nonNormalScripts = {
  'tools/ui-audit/verify-scroll-editor-input.mjs':'Retained v0.8.13 manual maintenance acceptance for scroll entry and save identity; outside the current main CI profile.',
  'apps/workshop/verify-collection-layout.mjs':'Imported by the normal roster/recovery UI acceptance.',
  'apps/workshop/verify-model.ts':'Retained manual model probe; TypeScript source is typechecked.',
  'apps/workshop/verify-groups.mjs':'Retained manual UI audit outside the baseline normal profile.',
  'apps/workshop/verify-hover.mjs':'Retained manual UI audit outside the baseline normal profile.',
  'apps/workshop/verify-section-help.mjs':'Retained manual UI audit outside the baseline normal profile.',
  'apps/workshop/verify-sidebar-alignment.mjs':'Retained manual UI audit outside the baseline normal profile.',
  ...Object.fromEntries(['worker-identity','v083-missing-features','v081-native','update','search-continuation','rust-backend','running-game-discovery','packaged-frontend','onefile','onefile-update','onefile-standalone','onefile-rollback','host-package','game-install-selection','equipment-browser','enemy-states','enemy-preview','compatibility-choice','character-plan-footer','add-layout'].map(name=>[`apps/tauri/verify-${name}.mjs`,'Packaged release or explicitly scoped native/manual acceptance; not removed from its existing profile.'])),
};

export function inventory(root, files = trackedFiles(root)) {
  const tasks = [];
  const add = task => tasks.push(task);
  const nodeTests = files.filter(file => /^apps\/desktop\/tests\/[^/]+\.test\.ts$/.test(file));
  ensure(nodeTests.length > 0, 'CI_INVENTORY_EMPTY', 'No Node tests were discovered.');
  for (const file of nodeTests) add({id: `node:${file}`, lane: 'frontend', kind: 'node-test', file, roots: [file], estimateMs: 1000});
  for (const [name, script, proof] of browser) add({id: `browser:${name}`, lane: 'frontend', kind: 'browser', script, proof, roots: [script, 'apps/workshop/main.tsx'], estimateMs: 60000});
  for (const locale of locales) {
    add({id: `browser:text-fit:${locale}`, lane: 'frontend', kind: 'browser', script: 'apps/workshop/verify-text-fit.mjs', locale, proof: 'text-fit.json', roots: ['apps/workshop/verify-text-fit.mjs', 'apps/workshop/main.tsx'], estimateMs: 60000});
    add({id: `browser:expanded:${locale}`, lane: 'frontend', kind: 'browser', script: 'tools/ui-audit/verify-arc-expanded.mjs', locale, proof: 'verify-arc-expanded.json', roots: ['tools/ui-audit/verify-arc-expanded.mjs', 'apps/workshop/main.tsx'], estimateMs: 45000});
    add({id: `browser:hell-seeds:${locale}`, lane: 'frontend', kind: 'browser', script: 'tools/ui-audit/verify-hell-equipment-seeds.mjs', locale, proof: 'verify-hell-equipment-seeds.json', roots: ['tools/ui-audit/verify-hell-equipment-seeds.mjs', 'apps/workshop/main.tsx'], estimateMs: 15000});
  }
  for (const name of ['lint', 'types', 'quality', 'generated', 'build']) add({id: `static:${name}`, lane: name === 'generated' || name === 'build' ? 'build' : 'static', kind: 'static', check: name, roots: [], estimateMs: 10000});
  for (const [id, manifest, testFeatures] of rust) {
    ensure(files.includes(manifest), 'CI_RUST_MANIFEST_MISSING', manifest);
    add({id: `rust-test:${id}`, lane: 'rust-tests', kind: 'rust-test', manifest, testFeatures, roots: [manifest], estimateMs: 180000});
    const lintFeatures = id === 'worker' ? ['--all-features'] : id === 'runtime' ? ['--features', 'test-fake,test-helper'] : [];
    add({id: `rust-static:${id}`, lane: 'rust-static', kind: 'rust-static', manifest, lintFeatures, roots: [manifest], estimateMs: 30000});
  }
  const models = files.filter(file => file.startsWith('spec/') && file.endsWith('.tla'));
  ensure(models.length > 0, 'CI_MODELS_EMPTY', 'No formal models were discovered.');
  for (const file of models) {
    const configuration = file.replace(/\.tla$/, '.cfg');
    const controlsFile = file.replace(/\.tla$/, '.negative.json');
    ensure(existsSync(resolve(root, configuration)) && existsSync(resolve(root, controlsFile)), 'CI_MODEL_INPUT_MISSING', file);
    const controls = readJson(resolve(root, controlsFile));
    ensure(Array.isArray(controls) && controls.length > 0, 'CI_MODEL_CONTROLS_EMPTY', file);
    ensure(controls.every(control=>typeof control.name==='string'&&control.name&&typeof control.expected==='string'&&control.expected)&&new Set(controls.map(control=>control.name)).size===controls.length,'CI_MODEL_CONTROLS_INVALID',file);
    const name = file.split('/').at(-1).replace(/\.tla$/, '');
    add({id: `model:${name}`, lane: 'models', kind: 'model', model: name, file, configuration, controlsFile, controls: controls.map(item => ({name: item.name, expected: item.expected})), roots: [file, configuration, controlsFile], estimateMs: (controls.length + 1) * 2000});
  }
  add({id: 'native:packaging', lane: 'packaging', kind: 'packaging', roots: [], estimateMs: 90000});
  add({id: 'native:frontend-roster', lane: 'frontend-native', kind: 'frontend-native', roots: ['apps/workshop/verify-small-bugfixes.mjs', 'apps/workshop/main.tsx'], estimateMs: 60000});
  add({id: 'ci:orchestration-e2e', lane: 'ci', kind: 'ci-e2e', roots: [], estimateMs: 15000});
  const ids = tasks.map(task => task.id);
  ensure(new Set(ids).size === ids.length, 'CI_INVENTORY_DUPLICATE', 'Task IDs are not unique.');
  for (const task of tasks) if (task.script) ensure(files.includes(task.script), 'CI_BROWSER_SCRIPT_MISSING', task.script);
  const normalScripts=new Set(tasks.flatMap(task=>task.script?[task.script]:task.kind==='frontend-native'?['apps/workshop/verify-small-bugfixes.mjs']:task.kind==='ci-e2e'?['tools/ci/verify-ci-e2e.mjs']:[]));
  const discoveredScripts=files.filter(file=>/^(?:apps\/(?:workshop|tauri)|tools\/(?:ui-audit|ci))\/verify-[^/]+\.(?:mjs|ts)$/.test(file));
  ensure(discoveredScripts.every(file=>normalScripts.has(file)||nonNormalScripts[file]),'CI_TEST_UNCLASSIFIED',`Classify new standalone acceptance scripts: ${discoveredScripts.filter(file=>!normalScripts.has(file)&&!nonNormalScripts[file]).join(', ')}`);
  // Callers first verify clean committed source. Git blobs normalize text under
  // .gitattributes while preserving binary/native identities across checkouts.
  const entries = new Map(trackedEntries(root).map(entry => [entry.path, entry]));
  const inputHash = digest(tasks.flatMap(task => task.roots).sort().map(path => {
    const entry = entries.get(path);
    ensure(entry, 'CI_INPUT_IDENTITY_MISSING', path);
    return [path, entry.mode, entry.blob];
  }));
  return {schema: 'nioh3-ci-inventory/v1', tasks, inputHash, otherProfiles:discoveredScripts.filter(file=>!normalScripts.has(file)).map(file=>({file,reason:nonNormalScripts[file]}))};
}

export const laneCounts = {frontend: 4, static: 3, build: 1, 'rust-tests': 4, 'rust-static': 2, models: 3, packaging: 1, 'frontend-native': 1, ci: 1};
