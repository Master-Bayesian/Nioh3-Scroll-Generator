import {spawn} from 'node:child_process';
import {closeSync, copyFileSync, mkdirSync, openSync} from 'node:fs';
import {cpus, totalmem} from 'node:os';
import {resolve} from 'node:path';
import {parseArgs} from 'node:util';
import {ensure, loadPlan, printFailure, readJson, verifySource, writeJson} from './core.mjs';
import {prepareWorkshopBundle} from './workshop-bundle.mjs';

async function main(){
  const {values}=parseArgs({options:{plan:{type:'string'},out:{type:'string'},mode:{type:'string'}}});
  ensure(values.plan&&values.out&&['sequential','parallel'].includes(values.mode),'CI_ARGUMENTS','Use --plan, --out and --mode sequential|parallel.');
  const mode=/** @type {'sequential'|'parallel'} */ (values.mode);
  const root=process.cwd();const plan=loadPlan(resolve(values.plan));verifySource(root,plan);
  const output=resolve(values.out);mkdirSync(output,{recursive:true});
  const bundleRoot=resolve(output,'bundle');mkdirSync(bundleRoot,{recursive:true});
  const bundle=prepareWorkshopBundle(root,resolve(bundleRoot,'workshop.json'));
  copyFileSync(bundle.path,resolve(bundleRoot,'workshop.html'));
  const groups=plan.groups.filter(group=>group.lane==='frontend');
  const observed=[];
  const run=group=>new Promise(done=>{
    const directory=resolve(output,group.id);mkdirSync(directory,{recursive:true});
    const log=resolve(directory,'runner.log');const fd=openSync(log,'w');const start=performance.now();
    const child=spawn(process.execPath,['tools/ci/run.mjs','--plan',resolve(values.plan),'--group',group.id,'--out',directory],{cwd:root,windowsHide:true,env:{...process.env,NIOH3_CI_WORKSHOP_BUNDLE:resolve(bundleRoot,'workshop.json')},stdio:['ignore',fd,fd]});
    let spawnError=null;
    child.once('error',error=>{spawnError=String(error);});
    child.once('close',code=>{
      closeSync(fd);observed.push({group:group.id,exitCode:code,spawnError,durationMs:Math.round(performance.now()-start),log});
      // Finish observing every shard even if one fails; retain the actual partial
      // coverage rather than abandoning running children or claiming a speedup.
      done();
    });
  });
  const started=performance.now();
  switch(mode){
    case 'sequential': for(const group of groups)await run(group); break;
    case 'parallel': await Promise.all(groups.map(run)); break;
  }
  const durationMs=Math.round(performance.now()-started);
  const receipts=groups.map(group=>readJson(resolve(output,group.id,'receipt.json')));
  const expected=groups.flatMap(group=>group.tasks);
  const executed=receipts.flatMap(receipt=>receipt.tasks.filter(task=>task.state==='Passed').map(task=>task.id));
  const passed=observed.every(item=>item.exitCode===0)&&receipts.every(item=>item.status==='Passed')&&executed.length===expected.length&&new Set(executed).size===expected.length&&executed.every(id=>expected.includes(id));
  const result={schema:'nioh3-frontend-scheduling-benchmark/v1',pass:passed,mode:values.mode,testedSha:plan.identity.testedSha,sourceId:plan.sourceId,planDigest:plan.digest,durationMs,groups:observed,expectedCases:expected,executedCases:executed,bundleSha256:bundle.sha256,environment:{platform:process.platform,arch:process.arch,node:process.version,cpuCount:cpus().length,memoryBytes:totalmem()},scope:'Same source and declared frontend case inventory, measured locally after one shared bundle build; excludes npm install, CI queue and native/backend lanes.'};
  writeJson(resolve(output,'benchmark.json'),result);
  ensure(passed,'CI_BENCHMARK_FAILED','A benchmark case failed or coverage was incomplete; inspect retained logs.');
  console.log(JSON.stringify({pass:passed,mode:values.mode,durationMs,cases:executed.length,artifact:resolve(output,'benchmark.json')}));
}
try{await main();}catch(error){printFailure(error);}
