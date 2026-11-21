import {parseArgs} from 'node:util';
import {resolve} from 'node:path';
import {existsSync} from 'node:fs';
import {appendGithubOutput, balance, ensure, git, printFailure, readJson, seal, sourceIdentity, writeJson} from './core.mjs';
import {inventory, laneCounts} from './inventory.mjs';

export function createPlan(root, options = {}) {
  const eventName = options.eventName ?? process.env.GITHUB_EVENT_NAME ?? 'workflow_dispatch';
  const event = options.event ?? (process.env.GITHUB_EVENT_PATH ? readJson(process.env.GITHUB_EVENT_PATH) : {});
  const source = sourceIdentity(root, eventName, event, {base: options.base, runId: options.runId});
  const discovered = inventory(root);
  const timingPath = resolve(root, 'tools/ci/timings.json');
  const timing = existsSync(timingPath) ? readJson(timingPath) : {tasks: {}};
  const groups = [];
  for (const [lane, count] of Object.entries(laneCounts)) {
    const tasks = discovered.tasks.filter(task => task.lane === lane);
    ensure(tasks.length > 0, 'CI_LANE_EMPTY', lane);
    for (const shard of balance(tasks, Math.min(count, tasks.length), timing.tasks)) {
      if(lane==='build')shard.tasks.sort((a,b)=>a==='static:generated'?-1:b==='static:generated'?1:a.localeCompare(b));
      groups.push({id: `${lane}-${shard.id}`, lane, shard: shard.id, tasks: shard.tasks, estimatedMs: shard.estimatedMs});
    }
  }
  const comparison = eventName === 'pull_request' || eventName === 'workflow_dispatch' ? 'Shadow' : 'FullOnly';
  // Selection never controls this phase's reference execution. Main/queue/manual
  // and PRs all execute the same complete declared normal inventory.
  return seal({schema: 'nioh3-ci-plan/v1', ...source, profile: 'normal-full', execution: {mode: 'Full', reason: 'Affected selection is not permitted to prune this rollout.', selected: discovered.tasks.map(task => task.id)}, comparison, inventory: discovered, groups, timings: timing.provenance ?? {basis: 'conservative fallback'}});
}

async function main() {
  const {values} = parseArgs({options: {root: {type: 'string'}, out: {type: 'string'}, event: {type: 'string'}, base: {type: 'string'}, 'run-id': {type: 'string'}}});
  const root = resolve(values.root ?? process.cwd());
  const plan = createPlan(root, {eventName: values.event, base: values.base, runId: values['run-id']});
  const out = resolve(values.out ?? 'deliverables/ci/plan');
  writeJson(resolve(out, 'plan.json'), plan);
  for (const lane of Object.keys(laneCounts)) appendGithubOutput(lane.replaceAll('-', '_'), {include: plan.groups.filter(group => group.lane === lane).map(group => ({id: group.id}))});
  appendGithubOutput('comparison', plan.comparison);
  appendGithubOutput('plan_digest', plan.digest);
  console.log(JSON.stringify({status: 'Planned', source: plan.identity, taskCount: plan.inventory.tasks.length, groups: plan.groups.length, execution: plan.execution.mode, comparison: plan.comparison, plan: resolve(out, 'plan.json')}));
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(import.meta.filename)) {
  try { await main(); } catch (error) {
    const outIndex=process.argv.indexOf('--out');
    if(outIndex>=0&&process.argv[outIndex+1])writeJson(resolve(process.argv[outIndex+1],'blocked.json'),{state:'Blocked',code:error.code??'CI_UNEXPECTED',message:error.message});
    printFailure(error);
  }
}
