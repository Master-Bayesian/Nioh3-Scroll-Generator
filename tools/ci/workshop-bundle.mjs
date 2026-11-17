import {existsSync, readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {execFileSync} from 'node:child_process';
import {digest, ensure, git, readJson, writeJson} from './core.mjs';

function inputs(root) {
  ensure(!git(root,['status','--porcelain']),'CI_BUNDLE_DIRTY','A reusable bundle requires clean committed source.');
  // Generated TypeScript may be rewritten from CRLF to LF without a Git diff.
  // Bind the normalized committed input blobs, then verify actual HTML bytes.
  // Native/binary inputs keep their repository-defined byte identities.
  const entries=execFileSync('git',['ls-files','--stage','-z'],{cwd:root,windowsHide:true}).toString('utf8').split('\0').filter(Boolean).map(line=>{
    const match=line.match(/^(\d+) ([a-f0-9]+) (\d)\t([\s\S]+)$/);
    ensure(match&&match[3]==='0','CI_BUNDLE_INDEX','The source index is unresolved.');
    return {path:match[4],mode:match[1],blob:match[2]};
  }).filter(entry=>/^(?:apps|packages|assets)\//.test(entry.path)||['package.json','package-lock.json'].includes(entry.path));
  return digest(entries);
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
