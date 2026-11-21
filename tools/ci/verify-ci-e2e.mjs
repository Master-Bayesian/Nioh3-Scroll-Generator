import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {cpSync, existsSync, mkdirSync, readFileSync, symlinkSync, unlinkSync, writeFileSync} from 'node:fs';
import {dirname, resolve} from 'node:path';
import {randomUUID} from 'node:crypto';
import {parseArgs} from 'node:util';
import {digest, readJson, seal, writeJson} from './core.mjs';

const {values}=parseArgs({options:{out:{type:'string'}}});
const source=resolve(import.meta.dirname,'../..');
const output=resolve(values.out??'deliverables/ci-orchestration-e2e');
const root=resolve(output,`owned-repository-${randomUUID()}`);
mkdirSync(root,{recursive:true});
const checks=[];
const calls=[];
const put=(name,text)=>{const path=resolve(root,name);mkdirSync(dirname(path),{recursive:true});writeFileSync(path,text,'utf8');};
const check=(name,condition,details={})=>{assert.ok(condition,`${name}: ${JSON.stringify(details)}`);checks.push({name,passed:true,...details});};
function run(program,args,cwd=root,extraEnv={}){
  const result=spawnSync(program,args,{cwd,encoding:'utf8',windowsHide:true,env:{...process.env,...extraEnv},maxBuffer:32*1024*1024});
  const entry={program,args,status:result.status,stdout:result.stdout??'',stderr:result.stderr??''};
  calls.push(entry);return entry;
}
function git(args){const result=run('git',args);assert.equal(result.status,0,result.stderr);return result.stdout.trim();}
function commit(message){git(['add','.']);git(['-c','user.name=CI Fixture','-c','user.email=ci-fixture@example.invalid','-c','commit.gpgsign=false','-c',`core.hooksPath=${resolve(output,'empty-hooks')}`,'commit','-m',message]);return git(['rev-parse','HEAD']);}
const ownedEvent=resolve(output,'owned-event.json');
writeJson(ownedEvent,{});
function cli(file,args,extraEnv={}){return run(process.execPath,[resolve(source,'tools/ci',file),'--root',root,...args],root,{GITHUB_EVENT_PATH:ownedEvent,GITHUB_RUN_ID:'owned-e2e',GITHUB_RUN_ATTEMPT:'1',...extraEnv});}
function planAt(name,base,event='workflow_dispatch'){
  const directory=resolve(output,name);const result=cli('plan.mjs',['--event',event,'--base',base,'--run-id','owned-e2e','--out',directory]);
  assert.equal(result.status,0,result.stderr);return {path:resolve(directory,'plan.json'),value:readJson(resolve(directory,'plan.json'))};
}
function shadowAt(plan,name){const directory=resolve(output,name);const result=cli('shadow.mjs',['--plan',plan.path,'--out',directory]);assert.equal(result.status,0,result.stderr);return {path:resolve(directory,'shadow.json'),value:readJson(resolve(directory,'shadow.json'))};}

try{
  git(['init','-b','main']);git(['config','core.autocrlf','false']);
  put('.gitignore','node_modules/\n');
  put('.gitattributes','*.ts text\n*.mjs -text\n');
  put('package.json',JSON.stringify({name:'nioh3-owned-ci-fixture',private:true,type:'module'}));
  put('tsconfig.json',JSON.stringify({compilerOptions:{target:'ES2022',module:'ESNext',moduleResolution:'Bundler',strict:true,noEmit:true},include:['apps/**/*.ts']}));
  put('apps/desktop/src/leaf.ts','export const value = 1;\n');
  put('apps/desktop/src/other.ts','export const other = 7;\n');
  put('apps/desktop/tests/leaf.test.ts','import {test} from "node:test";import assert from "node:assert/strict";import {value} from "../src/leaf.ts";test("real affected assertion",()=>assert.equal(value,1));\n');
  put('apps/desktop/tests/other.test.ts','import {test} from "node:test";import assert from "node:assert/strict";import {other} from "../src/other.ts";test("unrelated assertion",()=>assert.equal(other,7));\n');
  for(const manifest of ['crates/nioh3-domain','crates/nioh3-data','crates/nioh3-worker','crates/nioh3-save','crates/nioh3-runtime','crates/nioh3-protected','apps/tauri/src-tauri','apps/launcher'])put(manifest+'/Cargo.toml','[package]\nname="owned-fixture"\nversion="0.0.0"\n');
  for(const file of ['apps/workshop/main.tsx','apps/workshop/verify-live-equipment-add.mjs','apps/workshop/verify-runtime-compatibility.mjs','apps/workshop/verify-character-draft.mjs','apps/workshop/verify-scroll-completion.mjs','apps/workshop/verify-text-fit.mjs','apps/workshop/verify-small-bugfixes.mjs','tools/ui-audit/verify-arc-expanded.mjs','tools/ui-audit/verify-hell-equipment-seeds.mjs'])put(file,'export {};\n');
  for(const name of ['CIValidation.tla','CIValidation.cfg','CIValidation.negative.json'])put('spec/ci_validation/'+name,readFileSync(resolve(source,'spec/ci_validation',name),'utf8'));
  put('tools/ci/timings.json',readFileSync(resolve(source,'tools/ci/timings.json'),'utf8'));
  mkdirSync(resolve(output,'empty-hooks'),{recursive:true});
  symlinkSync(resolve(source,'node_modules'),resolve(root,'node_modules'),process.platform==='win32'?'junction':'dir');
  const base=commit('Create owned protocol fixture');
  put('apps/desktop/src/leaf.ts','export const value = 1; // semantic-preserving leaf edit\n');commit('Edit the imported leaf');
  const plan=planAt('valid-plan',base);
  const shadow=shadowAt(plan,'valid-shadow');
  const leaf='node:apps/desktop/tests/leaf.test.ts';const other='node:apps/desktop/tests/other.test.ts';
  check('reference remains Full even when shadow is Affected',plan.value.execution.mode==='Full'&&plan.value.execution.selected.length===plan.value.inventory.tasks.length&&shadow.value.decision.mode==='Affected');
  check('transitive import selects the affected assertion',shadow.value.decision.selected.includes(leaf));
  check('unrelated pure Node assertion can remain out of the shadow',!shadow.value.decision.selected.includes(other));
  const assigned=plan.value.groups.flatMap(group=>group.tasks);
  check('duration shards retain every required case exactly once',new Set(assigned).size===assigned.length&&assigned.length===plan.value.inventory.tasks.length);
  for(const id of [leaf,other]){
    const result=cli('run.mjs',['--plan',plan.path,'--task',id,'--out',resolve(output,'actual-node',digest(id).slice(0,8))]);
    check('actual existing Node runner passes '+id,result.status===0,{exitCode:result.status});
  }
  const leafPath=resolve(root,'apps/desktop/tests/leaf.test.ts');
  git(['config','core.autocrlf','true']);
  unlinkSync(leafPath);
  git(['checkout-index','--force','--index','--','apps/desktop/tests/leaf.test.ts']);
  const crlf=planAt('crlf-plan',base);
  check('normalized input identities survive LF and CRLF checkouts',readFileSync(leafPath,'utf8').includes('\r\n')&&crlf.value.inventory.inputHash===plan.value.inventory.inputHash&&crlf.value.sourceId===plan.value.sourceId);
  // Positive/fault aggregation fixtures are controlled protocol records, not
  // claims that Cargo/browser/model product suites executed in this fixture.
  const reportRoot=resolve(output,'controlled-receipts');
  function receipts(directory,p=plan.value,s=shadow.value){
    for(const group of [...p.groups,...s.groups]){
      const dir=resolve(directory,group.id);mkdirSync(dir,{recursive:true});writeFileSync(resolve(dir,'fixture.log'),'Controlled aggregation fixture; not product-suite execution.\n');
      const hash=digest(readFileSync(resolve(dir,'fixture.log')));
      writeJson(resolve(dir,'receipt.json'),{schema:'nioh3-ci-shard/v1',sourceId:p.sourceId,testedSha:p.identity.testedSha,planDigest:p.digest,group:group.id,profile:group.lane==='shadow'?'shadow':'reference',expected:group.tasks,status:group.tasks.length?'Passed':'NotApplicable',durationMs:1,environment:{scope:'controlled aggregate fixture'},tasks:group.tasks.map(id=>({id,state:'Passed',durationMs:1,commands:[{exitCode:0,signal:null,artifactPath:'fixture.log',sha256:hash}],evidence:null}))});
    }
  }
  receipts(reportRoot);
  function gate(p,s,reports,name,env={}){return cli('gate.mjs',['--plan',p,'--shadow',s,'--reports',reports,'--out',resolve(output,'gate-results',name)],env);}
  const valid=gate(plan.path,shadow.path,reportRoot,'positive');
  check('actual aggregate CLI accepts complete matching controlled receipts',valid.status===0,{exitCode:valid.status});
  const sensitivePath=resolve(root,'apps/workshop/verify-text-fit.mjs');
  const sensitiveBytes=readFileSync(sensitivePath);
  writeFileSync(sensitivePath,Buffer.concat([sensitiveBytes,Buffer.from('\r\n')]));
  const sensitive=cli('shadow.mjs',['--plan',plan.path,'--out',resolve(output,'byte-sensitive-edit')]);
  check('byte-sensitive source edits remain rejected',sensitive.status!==0&&sensitive.stderr.includes('CI_DIRTY_SOURCE'));
  writeFileSync(sensitivePath,sensitiveBytes);
  function fault(name,expected,mutate){
    const folder=resolve(output,'faults',name);mkdirSync(folder,{recursive:true});cpSync(reportRoot,resolve(folder,'reports'),{recursive:true});
    const state={plan:structuredClone(plan.value),shadow:structuredClone(shadow.value),reports:resolve(folder,'reports'),env:{}};
    mutate(state);
    writeJson(resolve(folder,'plan.json'),seal(state.plan));writeJson(resolve(folder,'shadow.json'),seal(state.shadow));
    const result=gate(resolve(folder,'plan.json'),resolve(folder,'shadow.json'),state.reports,name,state.env);
    const outcome=readJson(resolve(output,'gate-results',name,'gate.json'));
    check(name,result.status!==0&&outcome.code===expected,{expected,actual:outcome.code,exitCode:result.status});
  }
  const first=plan.value.groups.find(group=>group.tasks.length);
  const receiptFile=state=>resolve(state.reports,first.id,'receipt.json');
  fault('missing-shard','CI_REPORT_COVERAGE',state=>{const path=receiptFile(state);const value=readJson(path);value.group='unplanned';writeJson(path,value);});
  fault('duplicate-shard','CI_REPORT_COVERAGE',state=>{const folder=resolve(state.reports,'duplicate');mkdirSync(folder);cpSync(resolve(state.reports,first.id),folder,{recursive:true});});
  fault('stale-source','CI_RECEIPT_SOURCE',state=>{const path=receiptFile(state);const value=readJson(path);value.testedSha='0'.repeat(40);writeJson(path,value);});
  fault('missing-case','CI_TASK_COVERAGE',state=>{const path=receiptFile(state);const value=readJson(path);value.tasks.pop();writeJson(path,value);});
  fault('failed-shard','CI_RECEIPT_FAILED',state=>{const path=receiptFile(state);const value=readJson(path);value.status='Failed';writeJson(path,value);});
  fault('failed-command','CI_COMMAND_FAILED',state=>{const path=receiptFile(state);const value=readJson(path);value.tasks[0].commands[0].exitCode=1;writeJson(path,value);});
  fault('cancelled-job','CI_PREREQUISITE_FAILED',state=>{state.env.NIOH3_CI_JOB_RESULTS=JSON.stringify({required:{result:'cancelled'}});});
  fault('reference-pruning','CI_REFERENCE_PRUNED',state=>{state.plan.execution.mode='Affected';});
  fault('duplicate-case-assignment','CI_ASSIGNMENT_INVALID',state=>{state.plan.groups[1].tasks.push(state.plan.groups[0].tasks[0]);});
  fault('shadow-assignment-mismatch','CI_SHADOW_ASSIGNMENT',state=>{state.shadow.groups[0].tasks.push(other);});
  fault('changed-log','CI_LOG_CHANGED',state=>{writeFileSync(resolve(state.reports,first.id,'fixture.log'),'Changed evidence bytes.');});
  const absent=gate(plan.path,resolve(output,'absent-shadow.json'),reportRoot,'missing-shadow');
  check('missing shadow cannot pass',absent.status!==0&&readJson(resolve(output,'gate-results/missing-shadow/gate.json')).code==='CI_SHADOW_MISSING');
  const wrongAttempt=cli('shadow.mjs',['--plan',plan.path,'--out',resolve(output,'wrong-attempt')],{GITHUB_RUN_ATTEMPT:'2'});
  check('workflow attempts cannot mix source plans and receipts',wrongAttempt.status!==0&&wrongAttempt.stderr.includes('CI_INVOCATION_MISMATCH'));
  const noHistory=planAt('no-history','b'.repeat(40));const noHistoryShadow=shadowAt(noHistory,'no-history-shadow');
  check('unavailable merge-base history falls back to Full',noHistoryShadow.value.decision.mode==='Full');
  const main=planAt('main-plan',base,'push');
  check('integration events stay FullOnly',main.value.comparison==='FullOnly'&&main.value.execution.mode==='Full');
  const beforeLock=git(['rev-parse','HEAD']);put('package-lock.json','{"lockfileVersion":3}\n');commit('Add an external dependency input');
  const lock=planAt('lock-plan',beforeLock);const lockShadow=shadowAt(lock,'lock-shadow');
  check('non-imported dependency input forces Full',lockShadow.value.decision.mode==='Full');
  const beforeFault=git(['rev-parse','HEAD']);put('apps/desktop/src/leaf.ts','export const value = 2;\n');commit('Introduce an observable assertion fault');
  const bad=planAt('assertion-fault-plan',beforeFault);const badShadow=shadowAt(bad,'assertion-fault-shadow');
  const fullFault=cli('run.mjs',['--plan',bad.path,'--task',leaf,'--out',resolve(output,'actual-fault')]);
  const faultLog=readFileSync(resolve(output,'actual-fault/tasks',digest(leaf).slice(0,16),'command-1.log'),'utf8');
  check('real source assertion fault is selected and detected',badShadow.value.decision.selected.includes(leaf)&&fullFault.status!==0&&faultLog.includes('ERR_ASSERTION'),{exitCode:fullFault.status});
  const oldPlan=cli('shadow.mjs',['--plan',plan.path,'--out',resolve(output,'stale-plan')]);
  check('stale checkout cannot produce a new shadow',oldPlan.status!==0&&oldPlan.stderr.includes('CI_SOURCE_MISMATCH'));
  put('apps/workshop/verify-unregistered.mjs','export {};\n');commit('Introduce an undeclared acceptance script');
  const unclassified=cli('plan.mjs',['--event','workflow_dispatch','--base',base,'--out',resolve(output,'unclassified-script')]);
  check('new unclassified acceptance script blocks planning',unclassified.status!==0&&readJson(resolve(output,'unclassified-script/blocked.json')).code==='CI_TEST_UNCLASSIFIED');
  writeJson(resolve(output,'ci-e2e.json'),{pass:true,boundary:'Actual orchestration CLIs and Node assertions in an owned Git fixture; controlled aggregation inputs are not product/Cargo/browser/model acceptance; no game/save access.',checks,calls,fixture:root,referencePlan:plan.path,shadowPlan:shadow.path});
  console.log(JSON.stringify({pass:true,checks:checks.length,output}));
}catch(error){
  writeJson(resolve(output,'ci-e2e.json'),{pass:false,boundary:'Owned orchestration fixture only',checks,calls,error:error.stack??String(error),fixture:root});
  throw error;
}
