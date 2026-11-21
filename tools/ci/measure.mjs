import {execFileSync} from 'node:child_process';
import {resolve} from 'node:path';
import {parseArgs} from 'node:util';
import {ensure, readJson, writeJson} from './core.mjs';

function api(path){return JSON.parse(execFileSync('gh',['api',path],{encoding:'utf8',windowsHide:true,maxBuffer:32*1024*1024}));}
function jobs(repository,run){
  const all=[];
  for(let page=1;page<=100;page++){
    const response=api(`repos/${repository}/actions/runs/${run}/jobs?per_page=100&page=${page}`);
    all.push(...response.jobs);
    if(response.jobs.length<100)return all;
  }
  throw new Error('Unexpected CI job pagination size');
}
const elapsed=(start,end)=>new Date(end).getTime()-new Date(start).getTime();

export function summarize(run,records){
  ensure(run.status==='completed','CI_MEASUREMENT_RUNNING','Measure a completed workflow, not an incomplete duration.');
  const complete=records.filter(job=>job.started_at&&job.completed_at);
  ensure(complete.length>0,'CI_MEASUREMENT_EMPTY','No completed job timings.');
  const first=Math.min(...complete.map(job=>new Date(job.started_at).getTime()));
  const end=Math.max(...complete.map(job=>new Date(job.completed_at).getTime()));
  const frontend=complete.filter(job=>/^Frontend static|^Full frontend|^frontend-build$|^frontend-native$/.test(job.name));
  const shadow=complete.filter(job=>/^Shadow frontend|^shadow-plan$/.test(job.name));
  return {runId:run.id,url:run.html_url,sourceCommit:run.head_sha,event:run.event,conclusion:run.conclusion,createdAt:run.created_at,
    workflowWallMs:end-first,queueBeforeFirstJobMs:Math.max(0,first-new Date(run.created_at).getTime()),
    runnerMs:complete.reduce((sum,job)=>sum+elapsed(job.started_at,job.completed_at),0),
    primaryFrontendWallMs:frontend.length?Math.max(...frontend.map(job=>new Date(job.completed_at).getTime()))-first:null,
    shadowRunnerMs:shadow.reduce((sum,job)=>sum+elapsed(job.started_at,job.completed_at),0),
    jobs:complete.map(job=>({name:job.name,conclusion:job.conclusion,startedAt:job.started_at,completedAt:job.completed_at,durationMs:elapsed(job.started_at,job.completed_at),runnerName:job.runner_name,labels:job.labels})),
    scope:'One observed workflow sample including setup and DAG waits; not a cold/warm distribution or a guaranteed two-minute bound.'};
}

const {values}=parseArgs({options:{run:{type:'string'},out:{type:'string'},baseline:{type:'string'},repository:{type:'string',default:'Master-Bayesian/Nioh3-Scroll-Generator'}}});
ensure(values.run&&/^\d+$/.test(values.run)&&values.out,'CI_ARGUMENTS','--run and --out are required.');
const metadata=api(`repos/${values.repository}/actions/runs/${values.run}`);
const records=jobs(values.repository,values.run);
const after=summarize(metadata,records);
const before=values.baseline?readJson(resolve(values.baseline)):null;
writeJson(resolve(values.out,'run.json'),metadata);writeJson(resolve(values.out,'jobs.json'),records);
writeJson(resolve(values.out,'comparison.json'),{schema:'nioh3-ci-comparison/v1',before,after,
  frontendRatio:before&&after.primaryFrontendWallMs?after.primaryFrontendWallMs/before.frontendWallMs:null,
  beforeWholeCriticalPathMs:before?.wholeCriticalPathMs??null,
  note:'The optimized profile adds orchestration faults, launcher static/tests, explicit broker E2Es and temporary shadow execution. Compare source, inventory and setup/cost as well as wall time.'});
console.log(JSON.stringify(after,null,2));
