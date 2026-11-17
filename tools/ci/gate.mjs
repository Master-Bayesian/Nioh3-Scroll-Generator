import {existsSync, readFileSync, readdirSync} from 'node:fs';
import {dirname, resolve} from 'node:path';
import {parseArgs} from 'node:util';
import {CIError, digest, ensure, inside, loadPlan, printFailure, readJson, verifySealed, verifySource, writeJson} from './core.mjs';
import {inventory} from './inventory.mjs';

function sameSet(actual, expected, code) {
  ensure(actual.length === expected.length && new Set(actual).size === actual.length && [...actual].sort().every((value,index)=>value===[...expected].sort()[index]), code, 'Expected exact unique case/group coverage.');
}

function receiptPaths(root) {
  if (!existsSync(root)) return [];
  if(existsSync(resolve(root,'receipt.json')))return [resolve(root,'receipt.json')];
  return readdirSync(root,{withFileTypes:true}).flatMap(entry=>entry.isDirectory()?receiptPaths(resolve(root,entry.name)):entry.name==='receipt.json'?[resolve(root,entry.name)]:[]);
}

export function aggregate(root, plan, reportsRoot, shadow = null, lanes = null, jobs = {}) {
  verifySource(root, plan);
  const current = inventory(root);
  sameSet(plan.inventory.tasks.map(task=>task.id),current.tasks.map(task=>task.id),'CI_INVENTORY_DRIFT');
  ensure(current.inputHash===plan.inventory.inputHash,'CI_INVENTORY_DRIFT','Planned input bytes changed.');
  ensure(plan.execution.mode==='Full','CI_REFERENCE_PRUNED','This rollout requires an unpruned reference execution.');
  const requiredComparison=['pull_request','workflow_dispatch'].includes(plan.identity.event)?'Shadow':'FullOnly';
  ensure(plan.comparison===requiredComparison,'CI_COMPARISON_POLICY','The event comparison policy was changed.');
  sameSet(plan.execution.selected,plan.inventory.tasks.map(task=>task.id),'CI_REFERENCE_PRUNED');
  sameSet(plan.groups.flatMap(group=>group.tasks),plan.execution.selected,'CI_ASSIGNMENT_INVALID');
  for (const [name,job] of Object.entries(jobs)) {
    const allowedSkip=plan.comparison==='FullOnly'&&name==='shadow';
    ensure(job.result==='success'||allowedSkip&&job.result==='skipped','CI_PREREQUISITE_FAILED',`Prerequisite ${name}: ${job.result}`);
  }
  const referenceGroups=plan.groups.filter(group=>!lanes||lanes.includes(group.lane));
  const compare=!lanes||lanes.includes('frontend');
  const shadowRequired=compare&&plan.comparison==='Shadow';
  if(shadowRequired){
    ensure(shadow,'CI_SHADOW_MISSING','Shadow comparison is required for this event.');
    verifySealed(shadow);
    ensure(shadow.planDigest===plan.digest&&shadow.sourceId===plan.sourceId,'CI_SHADOW_SOURCE','Shadow metadata belongs to another reference.');
    ensure(shadow.pruningEnabled===false,'CI_SHADOW_PRUNING','Shadow mode cannot authorize reference pruning.');
    ensure(['Full','Affected'].includes(shadow.decision.mode),'CI_SHADOW_BLOCKED','Shadow inventory is blocked.');
    const frontend=plan.inventory.tasks.filter(task=>task.lane==='frontend').map(task=>task.id);
    ensure(shadow.decision.selected.every(id=>frontend.includes(id)),'CI_SHADOW_UNKNOWN','Shadow contains an unknown case.');
    if(shadow.decision.mode==='Full')sameSet(shadow.decision.selected,frontend,'CI_SHADOW_FULL_INCOMPLETE');
    sameSet(shadow.groups.flatMap(group=>group.tasks),shadow.decision.selected,'CI_SHADOW_ASSIGNMENT');
  }
  const expected=[...referenceGroups,...(shadowRequired?shadow.groups:[])];
  const expectedIds=expected.map(group=>group.id);
  const loaded=receiptPaths(reportsRoot).map(path=>({path,receipt:readJson(path)}));
  // A lane gate sees the same downloaded run, but verifies only its own groups.
  const relevant=lanes?loaded.filter(item=>expectedIds.includes(item.receipt.group)):loaded;
  sameSet(relevant.map(item=>item.receipt.group),expectedIds,'CI_REPORT_COVERAGE');
  const completed=[];
  const taskTimings={};
  for(const group of expected){
    const {path,receipt}=relevant.find(item=>item.receipt.group===group.id);
    ensure(receipt.schema==='nioh3-ci-shard/v1'&&receipt.sourceId===plan.sourceId&&receipt.testedSha===plan.identity.testedSha&&receipt.planDigest===plan.digest,'CI_RECEIPT_SOURCE',`Stale or malformed receipt: ${group.id}`);
    const profile=group.lane==='shadow'?'shadow':'reference';
    ensure(receipt.profile===profile,'CI_RECEIPT_PROFILE',group.id);
    sameSet(receipt.expected,group.tasks,'CI_RECEIPT_ASSIGNMENT');
    sameSet(receipt.tasks.map(task=>task.id),group.tasks,'CI_TASK_COVERAGE');
    ensure(receipt.status===(group.tasks.length?'Passed':'NotApplicable'),'CI_RECEIPT_FAILED',group.id);
    for(const task of receipt.tasks){
      ensure(task.state==='Passed'&&Number.isFinite(task.durationMs)&&task.durationMs>=0,'CI_TASK_FAILED',task.id);
      ensure(task.commands.length>0&&task.commands.every(command=>command.exitCode===0&&!command.signal),'CI_COMMAND_FAILED',task.id);
      for(const command of task.commands){
        const log=inside(dirname(path),command.artifactPath);
        ensure(existsSync(log)&&digest(readFileSync(log))===command.sha256,'CI_LOG_CHANGED',task.id);
      }
      if(task.evidence?.artifactPath){
        const proof=inside(dirname(path),task.evidence.artifactPath);
        ensure(existsSync(proof)&&digest(readFileSync(proof))===task.evidence.sha256,'CI_PROOF_CHANGED',task.id);
      }
      completed.push({id:task.id,profile,durationMs:task.durationMs,group:group.id});
      if(profile==='reference')taskTimings[task.id]={durationMs:Math.max(1,task.durationMs),basis:'observed complete passing task; setup/shard wall time recorded separately',environment:receipt.environment};
    }
  }
  if(shadowRequired){
    const actual=completed.filter(task=>task.profile==='shadow').map(task=>task.id);
    sameSet(actual,shadow.decision.selected,'CI_SHADOW_EXECUTION');
    ensure(actual.every(id=>completed.some(task=>task.profile==='reference'&&task.id===id)),'CI_SHADOW_REFERENCE','A shadow case has no successful full reference.');
  }
  return {schema:'nioh3-ci-gate/v1',state:'Passed',sourceId:plan.sourceId,testedSha:plan.identity.testedSha,planDigest:plan.digest,profile:plan.profile,lanes:lanes??'all',executionMode:'Full',referenceTasks:completed.filter(task=>task.profile==='reference').length,shadow:shadowRequired?{state:'Compared',mode:shadow.decision.mode,cases:shadow.decision.selected.length,pruningEnabled:false}:{state:'NotRequired',reason:'Complete integration profile or a non-frontend lane.'},groups:expected.map(group=>({id:group.id,estimatedMs:group.estimatedMs,observedMs:relevant.find(item=>item.receipt.group===group.id).receipt.durationMs})),completed,timings:{schema:'nioh3-ci-timings/v1',provenance:{sourceCommit:plan.identity.testedSha,runId:plan.identity.runId,profile:plan.profile,method:'actual successful child commands; not queue/setup time'},tasks:taskTimings}};
}

async function main(){
  const {values}=parseArgs({options:{root:{type:'string'},plan:{type:'string'},reports:{type:'string'},shadow:{type:'string'},out:{type:'string'},lanes:{type:'string'}}});
  ensure(values.plan&&values.reports&&values.out,'CI_ARGUMENTS','--plan, --reports and --out are required.');
  const output=resolve(values.out);
  try{
    const plan=loadPlan(resolve(values.plan));
    const shadow=values.shadow&&existsSync(values.shadow)?readJson(resolve(values.shadow)):null;
    const jobs=process.env.NIOH3_CI_JOB_RESULTS?JSON.parse(process.env.NIOH3_CI_JOB_RESULTS):{};
    const result=aggregate(resolve(values.root??process.cwd()),plan,resolve(values.reports),shadow,values.lanes?.split(','),jobs);
    writeJson(resolve(output,'gate.json'),result);writeJson(resolve(output,'timings.json'),result.timings);
    console.log(JSON.stringify({state:result.state,referenceTasks:result.referenceTasks,shadow:result.shadow,source:result.testedSha}));
  }catch(error){
    writeJson(resolve(output,'gate.json'),{schema:'nioh3-ci-gate/v1',state:error instanceof CIError&&['CI_REPORT_COVERAGE','CI_TASK_COVERAGE','CI_SHADOW_MISSING'].includes(error.code)?'Incomplete':'Failed',code:error.code??'CI_UNEXPECTED',message:error.message});
    throw error;
  }
}

if(process.argv[1]&&resolve(process.argv[1])===resolve(import.meta.filename)){
  try{await main();}catch(error){printFailure(error);}
}
