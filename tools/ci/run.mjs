import {spawnSync, execFileSync} from 'node:child_process';
import {closeSync, existsSync, mkdirSync, openSync, readFileSync} from 'node:fs';
import {createRequire} from 'node:module';
import {dirname, relative, resolve} from 'node:path';
import {parseArgs, stripVTControlCharacters} from 'node:util';
import {CIError, digest, ensure, git, loadPlan, printFailure, readJson, verifySealed, verifySource, writeJson} from './core.mjs';

/** @typedef {{state:'Planned'} | {state:'Running', startedAt:string} | {state:'Passed', startedAt:string, endedAt:string} | {state:'Failed', startedAt:string, endedAt:string}} TaskState */
/** @typedef {'Start'|'Pass'|'Fail'} TaskEvent */

/** @param {TaskState} current @param {TaskEvent} event @returns {TaskState} */
export function transition(current, event) {
  const now = new Date().toISOString();
  switch (current.state) {
    case 'Planned':
      if (event === 'Start') return {state: 'Running', startedAt: now};
      break;
    case 'Running':
      if (event === 'Pass' || event === 'Fail') return {state: event === 'Pass' ? 'Passed' : 'Failed', startedAt: current.startedAt, endedAt: now};
      break;
    case 'Passed':
    case 'Failed': break;
  }
  throw new CIError('CI_TASK_TRANSITION', `Invalid task transition: ${current.state}/${event}`);
}

function npmCli() {
  const candidates = [process.env.npm_execpath, resolve(dirname(process.execPath), 'node_modules/npm/bin/npm-cli.js')].filter(Boolean);
  const path = candidates.find(candidate => existsSync(candidate) && candidate.endsWith('npm-cli.js'));
  ensure(path, 'CI_NPM_CLI', 'Cannot locate the selected Node installation\'s npm CLI.');
  return path;
}

function findProof(directory, name) {
  // Proof paths are task-owned and known; the few legacy timestamp subfolders
  // are discovered without accepting a different task's report.
  const result = execFileSync(process.execPath, ['-e', 'const f=require("node:fs"),p=require("node:path");function walk(d){for(const x of f.readdirSync(d,{withFileTypes:true})){const q=p.join(d,x.name);if(x.isDirectory())walk(q);else if(x.name===process.argv[2])console.log(q)}}walk(process.argv[1])', directory, name], {encoding: 'utf8', windowsHide: true}).trim().split(/\r?\n/).filter(Boolean);
  ensure(result.length === 1, 'CI_PROOF_MISSING', `Expected exactly one ${name} under the task output.`);
  return result[0];
}

export function executeTask(root, task, out, context) {
  mkdirSync(out, {recursive: true});
  const commands = [];
  const environment = {...process.env, NIOH3_CI_TASK_OUTPUT: out, NIOH3_UI_OUTPUT: out, NIOH3_UI_PROFILE_ROOT: resolve(out, 'profile'), NIOH3_UI_RUN_LABEL:'final', NIOH3_UI_LOCALES:task.locale??'zh-CN,en-US,ja-JP', NIOH3_QUALITY_EVIDENCE: resolve(out, 'evidence'), NIOH3_COMPATIBILITY_EVIDENCE: out, NIOH3_BUGFIX_EVIDENCE: out};
  if (task.locale) environment.NIOH3_UI_LOCALES = task.locale;
  const command = (program, args) => {
    const log = resolve(out, `command-${commands.length + 1}.log`);
    const fd = openSync(log, 'w');
    const start = performance.now();
    let result;
    try { result = spawnSync(program, args, {cwd: root, env: environment, windowsHide: true, stdio: ['ignore', fd, fd]}); }
    finally { closeSync(fd); }
    const observed = {program, args, exitCode: result.status, signal: result.signal, durationMs: Math.round(performance.now() - start), log, artifactPath: relative(context.artifactRoot, log).replaceAll('\\','/'), sha256: digest(readFileSync(log))};
    commands.push(observed);
    ensure(!result.error && result.status === 0, 'CI_CHILD_FAILED', `${task.id}: ${program} exited ${result.status}; ${log}`);
    return stripVTControlCharacters(readFileSync(log, 'utf8'));
  };
  const node = args => command(process.execPath, args);
  const npm = script => node([npmCli(), 'run', script]);
  const cargo = args => command('cargo', args);
  const proof = (path, predicate) => {
    const value = readJson(path);
    ensure(predicate(value), 'CI_PROOF_FAILED', `${task.id}: invalid or unsuccessful proof ${path}`);
    return {path, artifactPath: relative(context.artifactRoot,path).replaceAll('\\','/'), sha256: digest(readFileSync(path)), value};
  };
  let evidence = null;
  switch (task.kind) {
    case 'node-test': {
      const require = createRequire(resolve(root, 'package.json'));
      const text = node([require.resolve('tsx/cli'), '--test', '--test-reporter=tap', task.file]);
      const total = Number(text.match(/# tests (\d+)/)?.[1] ?? 0);
      ensure(total > 0 && /# fail 0(?:\r?\n|$)/.test(text) && /# cancelled 0(?:\r?\n|$)/.test(text) && /# skipped 0(?:\r?\n|$)/.test(text), 'CI_NODE_COVERAGE', 'Node runner did not execute a complete nonempty passing test file.');
      evidence = {tests: total};
      break;
    }
    case 'browser': {
      if (task.id.startsWith('browser:expanded:')) { environment.NIOH3_UI_THEMES = 'light'; environment.NIOH3_UI_MATRIX = 'minimum'; }
      node([task.script]);
      const path = findProof(out, task.proof);
      evidence = proof(path, value => value.pass === true || value.status === 'passed');
      break;
    }
    case 'static':
      switch (task.check) {
        case 'lint': npm('lint'); break;
        case 'types': npm('typecheck'); break;
        case 'quality':
          npm('check:quality');
          evidence = proof(resolve(out, 'evidence/lint-controls.json'), value => value.pass === true && value.cases.length === 4 && value.cases.every(item => item.validAccepted && item.invalidRejected && item.disabledRuleAdmitsDefect));
          break;
        case 'generated':
          npm('contracts'); node(['tools/export_v2_ui_locales.mjs']); node(['tools/audit_v2_ui_locales.mjs']);
          command('git', ['diff', '--exit-code', '--', 'packages/contracts', 'apps/workshop/ui-locales.json']);
          break;
        case 'build': {
          node(['apps/tauri/build.mjs']);
          // This one immutable bundle is reused by all full/shadow UI shards.
          node(['tools/ci/prepare-bundle.mjs', '--out', resolve(out, 'bundle')]);
          evidence = {bundle: readJson(resolve(out, 'bundle/workshop.json'))};
          break;
        }
        default: throw new CIError('CI_STATIC_UNKNOWN', task.check);
      }
      break;
    case 'rust-test':
      if (task.manifest.startsWith('apps/tauri/')) mkdirSync(resolve(root, 'apps/tauri/dist'), {recursive: true});
      cargo(['fetch', '--locked', '--manifest-path', task.manifest]);
      cargo(['test', '--locked', '--offline', '--manifest-path', task.manifest, ...task.testFeatures]);
      break;
    case 'rust-static':
      if (task.manifest.startsWith('apps/tauri/')) mkdirSync(resolve(root, 'apps/tauri/dist'), {recursive: true});
      cargo(['fetch', '--locked', '--manifest-path', task.manifest]);
      cargo(['clippy', '--locked', '--offline', '--all-targets', '--manifest-path', task.manifest, ...task.lintFeatures, '--', '-D', 'warnings']);
      cargo(['fmt', '--check', '--manifest-path', task.manifest]);
      if (task.manifest.includes('nioh3-worker/')) cargo(['run', '--locked', '--offline', '--features', 'workshop-export', '--manifest-path', task.manifest, '--example', 'export_workshop', '--', '--check']);
      break;
    case 'model': {
      command('pwsh', ['-NoProfile', '-File', 'tools/check_models.ps1', '-Model', task.model, '-Evidence', resolve(out, 'models'), '-Java', process.env.NIOH3_CI_JAVA ?? 'java']);
      evidence = proof(resolve(out, 'models/model-controls.json'), value => Array.isArray(value) && value.length === task.controls.length + 1 && value.every(item => item.pass === true && item.model === task.model) && value.filter(item => item.case === 'correct' && item.exitCode === 0).length === 1 && task.controls.every(control => value.some(item => item.case === control.name && item.exitCode === 12 && item.expected === control.expected)));
      break;
    }
    case 'packaging':
      ensure(environment.NIOH3_PYTHON, 'CI_PYTHON_MISSING', 'Select the prepared Python interpreter explicitly.');
      command('pwsh', ['-NoProfile', '-File', 'tools/ci/run-native-packaging.ps1', '-Evidence', out]);
      evidence = proof(resolve(out, 'packaging.json'), value => value.pass === true && value.broker_cases === 4);
      break;
    case 'frontend-native':
      cargo(['fetch', '--locked', '--manifest-path', 'crates/nioh3-worker/Cargo.toml']);
      cargo(['build', '--release', '--locked', '--offline', '--manifest-path', 'crates/nioh3-worker/Cargo.toml', '--bin', 'nioh3-readonly-worker']);
      node(['apps/workshop/verify-small-bugfixes.mjs']);
      evidence = proof(resolve(out, 'report.json'), value => value.pass === true);
      break;
    case 'ci-e2e':
      command('pwsh', ['-NoProfile', '-File', 'tools/ci/check-workflows.ps1']);
      node(['tools/ci/verify-ci-e2e.mjs', '--out', out]);
      evidence = proof(resolve(out, 'ci-e2e.json'), value => value.pass === true && value.checks.length > 0);
      break;
    default: throw new CIError('CI_TASK_KIND', `Unknown inventory kind: ${task.kind}`);
  }
  return {commands, evidence, context};
}

export function runGroup(root, plan, group, output, profile = 'reference') {
  verifySource(root, plan);
  const started = performance.now();
  const receipt = {schema: 'nioh3-ci-shard/v1', sourceId: plan.sourceId, testedSha: plan.identity.testedSha, planDigest: plan.digest, group: group.id, profile, expected: group.tasks, status: 'Running', tasks: [], environment: {platform: process.platform, arch: process.arch, node: process.version, image: process.env.ImageVersion ?? 'local'}};
  const path = resolve(output, 'receipt.json');
  writeJson(path, receipt);
  try {
    for (const id of group.tasks) {
      const task = plan.inventory.tasks.find(item => item.id === id);
      ensure(task, 'CI_UNKNOWN_TASK', id);
      const state = transition({state: 'Planned'}, 'Start');
      const record = {id, ...state, durationMs: null, commands: [], evidence: null};
      receipt.tasks.push(record); writeJson(path, receipt);
      console.log(`CI_TASK_START ${id}`);
      const start = performance.now();
      try {
        const result = executeTask(root, task, resolve(output, 'tasks', digest(id).slice(0, 16)), {sourceId: plan.sourceId, planDigest: plan.digest, artifactRoot: output});
        Object.assign(record, result, transition(state, 'Pass'), {durationMs: Math.round(performance.now() - start)});
      } catch (error) {
        Object.assign(record, transition(state, 'Fail'), {durationMs: Math.round(performance.now() - start), error: {code: error.code ?? 'CI_UNEXPECTED', message: error.message}});
        throw error;
      } finally { writeJson(path, receipt); }
      console.log(`CI_TASK_END ${id} ${record.state} ${record.durationMs}ms`);
    }
    verifySource(root, plan);
    receipt.status = group.tasks.length ? 'Passed' : 'NotApplicable';
  } catch (error) { receipt.status = 'Failed'; receipt.error = {code: error.code ?? 'CI_UNEXPECTED', message: error.message}; throw error; }
  finally { receipt.durationMs = Math.round(performance.now() - started); writeJson(path, receipt); }
  return receipt;
}

async function main() {
  const {values} = parseArgs({options: {root: {type: 'string'}, plan: {type: 'string'}, shadow: {type: 'string'}, group: {type: 'string'}, task: {type: 'string'}, out: {type: 'string'}}});
  ensure(values.plan && values.out && Boolean(values.group) !== Boolean(values.task), 'CI_ARGUMENTS', 'Use --plan, --out and exactly one of --group or --task.');
  ensure(!values.task || !values.shadow, 'CI_ARGUMENTS', 'A diagnostic case cannot masquerade as a shadow shard.');
  const root = resolve(values.root ?? process.cwd());
  const plan = loadPlan(resolve(values.plan));
  const shadow = values.shadow ? verifySealed(readJson(resolve(values.shadow))) : null;
  if (shadow) ensure(shadow.planDigest === plan.digest && shadow.sourceId === plan.sourceId, 'CI_SHADOW_SOURCE', 'Shadow plan belongs to a different reference.');
  const task = values.task ? plan.inventory.tasks.find(item => item.id === values.task) : null;
  const group = values.task && task ? {id:`diagnostic-${digest(task.id).slice(0,12)}`,tasks:[task.id]} : (shadow?.groups ?? plan.groups).find(item => item.id === values.group);
  ensure(group, 'CI_GROUP_UNKNOWN', values.group);
  const result = runGroup(root, plan, group, resolve(values.out), values.task ? 'diagnostic' : shadow ? 'shadow' : 'reference');
  console.log(JSON.stringify({group: group.id, status: result.status, durationMs: result.durationMs, cases: result.tasks.length}));
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(import.meta.filename)) {
  try { await main(); } catch (error) { printFailure(error); }
}
