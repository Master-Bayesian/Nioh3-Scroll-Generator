import {createHash, randomUUID} from 'node:crypto';
import {existsSync, mkdirSync, readFileSync, renameSync, writeFileSync} from 'node:fs';
import {dirname, resolve} from 'node:path';
import {execFileSync} from 'node:child_process';

export class CIError extends Error {
  constructor(code, message) { super(message); this.code = code; }
}

export function ensure(condition, code, message) {
  if (!condition) throw new CIError(code, message);
}

export function canonical(value) {
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
  if (value !== null && typeof value === 'object') {
    return `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${canonical(value[key])}`).join(',')}}`;
  }
  return JSON.stringify(value);
}

export const digest = value => createHash('sha256').update(typeof value === 'string' || Buffer.isBuffer(value) ? value : canonical(value)).digest('hex');
export const readJson = path => JSON.parse(readFileSync(path, 'utf8').replace(/^\uFEFF/, ''));

export function writeJson(path, value) {
  mkdirSync(dirname(path), {recursive: true});
  const temporary = `${path}.${process.pid}.tmp`;
  writeFileSync(temporary, JSON.stringify(value, null, 2) + '\n', 'utf8');
  renameSync(temporary, path);
}

export function git(root, args) {
  return execFileSync('git', args, {cwd: root, encoding: 'utf8', windowsHide: true, stdio: ['ignore', 'pipe', 'pipe']}).trim();
}

export function trackedFiles(root) {
  return execFileSync('git', ['ls-files', '-z'], {cwd: root, windowsHide: true}).toString('utf8').split('\0').filter(Boolean).sort();
}

export function trackedEntries(root) {
  return execFileSync('git', ['ls-files', '--stage', '-z'], {cwd: root, windowsHide: true}).toString('utf8').split('\0').filter(Boolean).map(line => {
    const match = line.match(/^(\d+) ([a-f0-9]+) (\d)\t([\s\S]+)$/);
    ensure(match && match[3] === '0', 'CI_INPUT_INDEX', 'The source index is unresolved.');
    return {path: match[4], mode: match[1], blob: match[2]};
  });
}

export function sourceIdentity(root, eventName, event = {}, options = {}) {
  const testedSha = git(root, ['rev-parse', 'HEAD']);
  const status = git(root, ['status', '--porcelain']);
  ensure(!status, 'CI_DIRTY_SOURCE', 'Commit the source before recording a reproducible CI run.');
  const valid = value => typeof value === 'string' && /^[a-f0-9]{40}$/.test(value) && !/^0+$/.test(value);
  let baseSha = options.base ?? null;
  let headSha = testedSha;
  switch (eventName) {
    case 'pull_request': baseSha ??= event.pull_request?.base?.sha; headSha = event.pull_request?.head?.sha; break;
    case 'merge_group': baseSha ??= event.merge_group?.base_sha; headSha = event.merge_group?.head_sha; break;
    case 'push': baseSha ??= event.before; headSha = event.after ?? testedSha; break;
    case 'workflow_dispatch':
      if (!baseSha) { try { baseSha = git(root, ['rev-parse', 'origin/main']); } catch { baseSha = null; } }
      break;
    default: throw new CIError('CI_EVENT_UNSUPPORTED', `Unsupported validation event: ${eventName}`);
  }
  ensure(valid(headSha), 'CI_HEAD_INVALID', 'The declared head SHA is missing or invalid.');
  if (eventName !== 'pull_request') ensure(headSha === testedSha, 'CI_HEAD_MISMATCH', 'The tested checkout differs from the declared integration head.');
  else {
    try { git(root, ['merge-base', '--is-ancestor', headSha, testedSha]); }
    catch { throw new CIError('CI_HEAD_MISMATCH', 'The tested merge does not contain the PR head.'); }
  }
  if (!valid(baseSha)) baseSha = null;
  let mergeBaseSha = null;
  let changed = [];
  if (baseSha) {
    try {
      mergeBaseSha = git(root, ['merge-base', baseSha, headSha]);
      // No shell interpolation: Git paths may contain spaces or shell characters.
      const raw = execFileSync('git', ['diff', '--name-status', '-z', '--find-renames', mergeBaseSha, testedSha], {cwd: root, windowsHide: true}).toString('utf8').split('\0');
      for (let index = 0; index < raw.length && raw[index];) {
        const statusCode = raw[index++];
        const oldPath = raw[index++];
        const newPath = /^[RC]/.test(statusCode) ? raw[index++] : oldPath;
        ensure(Boolean(oldPath && newPath), 'CI_DIFF_INVALID', 'Git returned an incomplete path change.');
        changed.push({status: statusCode, oldPath, newPath});
      }
    } catch (error) {
      if (error instanceof CIError) throw error;
      mergeBaseSha = null;
      changed = [];
    }
  }
  const identity = {
    testedSha, tree: git(root, ['rev-parse', 'HEAD^{tree}']), baseSha, headSha, mergeBaseSha,
    event: eventName, runId: options.runId ?? process.env.GITHUB_RUN_ID ?? `local-${randomUUID()}`,
    attempt: process.env.GITHUB_RUN_ATTEMPT ?? '1',
  };
  return {identity, sourceId: digest(identity), changed};
}

export function seal(value) {
  const {digest: ignored, ...body} = value;
  void ignored;
  return {...body, digest: digest(body)};
}

export function verifySealed(value, code = 'CI_PLAN_CHANGED') {
  ensure(value && value.digest === seal(value).digest, code, 'The recorded plan no longer matches its digest.');
  return value;
}

export function inside(root, path) {
  const absolute = resolve(root, path);
  const prefix = resolve(root) + (process.platform === 'win32' ? '\\' : '/');
  ensure(absolute.toLowerCase().startsWith(prefix.toLowerCase()), 'CI_PATH_ESCAPE', `Path is outside its owned root: ${path}`);
  return absolute;
}

export function balance(tasks, count, timings) {
  ensure(Number.isInteger(count) && count > 0, 'CI_SHARD_COUNT', 'A positive shard count is required.');
  const shards = Array.from({length: count}, (_, index) => ({id: String(index + 1), tasks: [], estimatedMs: 0}));
  const weighted = tasks.map(task => ({task, cost: timings[task.id]?.durationMs ?? task.estimateMs ?? 60000}));
  ensure(weighted.every(item => Number.isFinite(item.cost) && item.cost > 0), 'CI_TIMING_INVALID', 'Unknown timing must have a conservative positive estimate.');
  weighted.sort((a, b) => b.cost - a.cost || a.task.id.localeCompare(b.task.id));
  for (const {task, cost} of weighted) {
    shards.sort((a, b) => a.estimatedMs - b.estimatedMs || Number(a.id) - Number(b.id));
    shards[0].tasks.push(task.id);
    shards[0].estimatedMs += cost;
  }
  return shards.sort((a, b) => Number(a.id) - Number(b.id));
}

export function appendGithubOutput(name, value) {
  if (process.env.GITHUB_OUTPUT) writeFileSync(process.env.GITHUB_OUTPUT, `${name}=${typeof value === 'string' ? value : JSON.stringify(value)}\n`, {flag: 'a'});
}

export function loadPlan(path) {
  ensure(existsSync(path), 'CI_PLAN_MISSING', 'The complete execution plan is unavailable.');
  return verifySealed(readJson(path));
}

export function verifySource(root, plan) {
  ensure(git(root, ['rev-parse', 'HEAD']) === plan.identity.testedSha && git(root, ['rev-parse', 'HEAD^{tree}']) === plan.identity.tree, 'CI_SOURCE_MISMATCH', 'The checkout is not the planned tested source.');
  ensure(!git(root, ['status', '--porcelain']), 'CI_DIRTY_SOURCE', 'Tracked or untracked source changed after planning.');
  ensure(plan.sourceId === digest(plan.identity), 'CI_SOURCE_ID', 'The source identity digest is invalid.');
  if(process.env.GITHUB_RUN_ID)ensure(plan.identity.runId===process.env.GITHUB_RUN_ID,'CI_INVOCATION_MISMATCH','The plan belongs to another workflow run.');
  if(process.env.GITHUB_RUN_ATTEMPT)ensure(plan.identity.attempt===process.env.GITHUB_RUN_ATTEMPT,'CI_INVOCATION_MISMATCH','Rerun the complete workflow rather than mixing receipt attempts.');
}

export function printFailure(error) {
  console.error(JSON.stringify({status: 'Failed', code: error instanceof CIError ? error.code : 'CI_UNEXPECTED', message: String(error?.message ?? error)}));
  process.exitCode = 1;
}
