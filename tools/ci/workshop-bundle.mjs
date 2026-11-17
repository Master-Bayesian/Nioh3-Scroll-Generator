import {existsSync, readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {execFileSync} from 'node:child_process';
import {digest, ensure, git, readJson, trackedFiles, writeJson} from './core.mjs';

function inputs(root) {
  return digest(trackedFiles(root).filter(file => /^(?:apps|packages|assets)\//.test(file) || ['package.json', 'package-lock.json'].includes(file)).map(file => [file, digest(readFileSync(resolve(root, file)))]));
}

export function prepareWorkshopBundle(root, output) {
  execFileSync(process.execPath, [resolve(root, 'apps/workshop/build.mjs')], {cwd: root, windowsHide: true, stdio: 'inherit'});
  const path = resolve(root, 'deliverables/frontend-v2/search-ui-demo-v2/index.html');
  const manifest = {schema: 'nioh3-ci-workshop-bundle/v1', testedSha: git(root, ['rev-parse', 'HEAD']), inputs: inputs(root), path, sha256: digest(readFileSync(path))};
  writeJson(output, manifest);
  return manifest;
}

export function workshopBundle(root) {
  const manifestPath = process.env.NIOH3_CI_WORKSHOP_BUNDLE;
  if (!manifestPath) {
    execFileSync(process.execPath, [resolve(root, 'apps/workshop/build.mjs')], {cwd: root, windowsHide: true, stdio: 'inherit'});
    return resolve(root, 'deliverables/frontend-v2/search-ui-demo-v2/index.html');
  }
  const manifest = readJson(manifestPath);
  ensure(manifest.schema === 'nioh3-ci-workshop-bundle/v1' && manifest.testedSha === git(root, ['rev-parse', 'HEAD']) && manifest.inputs === inputs(root), 'CI_BUNDLE_SOURCE', 'The prebuilt fixture bundle belongs to different inputs.');
  // A downloaded artifact carries its sibling HTML, not another runner's path.
  const sibling = resolve(manifestPath, '..', 'workshop.html');
  const path = existsSync(sibling) ? sibling : manifest.path;
  ensure(existsSync(path) && digest(readFileSync(path)) === manifest.sha256, 'CI_BUNDLE_BYTES', 'The immutable fixture bundle changed.');
  return path;
}

export function selectedLocales(allowed) {
  const selected = (process.env.NIOH3_UI_LOCALES ?? allowed.join(',')).split(',');
  ensure(selected.length > 0 && new Set(selected).size === selected.length && selected.every(locale => allowed.includes(locale)), 'CI_LOCALE_INVALID', 'Locale selection must be a nonempty supported set.');
  return selected;
}
