/** Real Windows process-image discovery with an owned idle C# helper, not the game. */
import assert from 'node:assert/strict';
import {execFileSync,spawn} from 'node:child_process';
import {mkdir,mkdtemp,writeFile,readFile,rm} from 'node:fs/promises';
import {join,resolve} from 'node:path';
import {parseArgs} from 'node:util';
import {chromium} from 'playwright';
import {isolatedEnvironment,closeSession,pause,inspectOnefile} from './onefile-acceptance.mjs';
const {values}=parseArgs({options:{exe:{type:'string'},out:{type:'string'}}});assert(values.exe&&values.out);
const out=resolve(values.out);await mkdir(out,{recursive:true});
const root=await mkdtemp('D:/Nioh3_v080_deliverables/tmp/running-game-compatibility-');
const report={pass:false,boundary:'real outer EXE/Windows discovery; owned idle VERSIONINFO helper named Nioh3.exe; no actual game/save writes',checks:[],screenshots:[]};
const check=(name,condition)=>{report.checks.push({name,pass:!!condition});assert(condition,name)};
let helper,child,session;
try{
 const identity=await inspectOnefile(values.exe);report.executableSha256=identity.sha256;report.payloadSha256=identity.payloadSha256;
 for(const version of ['2.0.0.2','2.0.1.0','2.0.2.0']){
  const dir=join(root,version);await mkdir(dir,{recursive:true});
  const source=join(dir,'IdleIdentity.cs'),exe=join(dir,'Nioh3.exe');
  await writeFile(source,'using System.Reflection;[assembly:AssemblyFileVersion("'+version+'")][assembly:AssemblyVersion("'+version+'")]public static class IdleIdentity{public static void Main(){System.Threading.Thread.Sleep(600000);}}');
  execFileSync(join(process.env.WINDIR,'Microsoft.NET/Framework64/v4.0.30319/csc.exe'),['/nologo','/target:exe','/platform:x64','/out:'+exe,source],{windowsHide:true,stdio:'pipe'});
  helper=spawn(exe,[],{windowsHide:true,stdio:'ignore'});await pause(500);
  const {env,profile,port}=await isolatedEnvironment(join(dir,'app'));
  child=spawn(resolve(values.exe),['--user-data-dir',profile],{env,windowsHide:true,stdio:'ignore'});
  for(let i=0;i<150;i++){assert(child.exitCode===null,'launcher exited');try{if((await fetch('http://127.0.0.1:'+port+'/json/version')).ok)break}catch{}await pause(300)}
  const browser=await chromium.connectOverCDP('http://127.0.0.1:'+port),page=browser.contexts()[0].pages()[0];session={browser,page};
  await page.locator('.shell').waitFor();
  const handshake=await page.evaluate(()=>window.nioh.handshake());
  check(version+' running image supplies actual VERSIONINFO',handshake.context.game_file_version===version);
  const inspected=await page.evaluate(()=>window.operations.execute({method:'runtime.compatibility',params:{action:'inspect'}}));
  report['compatibility_'+version]=inspected.compatibility;
  check(version+' helper is marked as unverified',inspected.compatibility.warning&&!inspected.compatibility.accepted);
  const prepared=await page.evaluate(()=>window.operations.execute({method:'runtime.compatibility',params:{action:'prepare'}}));
  check(version+' missing automatic save is explicit',prepared.compatibility.backup.attempted&&!prepared.compatibility.backup.verified);
  await page.evaluate(()=>window.dispatchEvent(new Event('nioh3:compatibility-required')));
  await page.locator('.compatibility-dialog').waitFor();
  await page.locator('[data-action=compatibility-risk]').check();
  check(version+' missing backup confirmation prevents continue',await page.locator('[data-action=compatibility-accept]').isDisabled());
  await page.locator('[data-action=compatibility-backup]').check();
  await page.screenshot({path:join(out,version+'.png')});report.screenshots.push(version+'.png');
  await page.locator('[data-action=compatibility-accept]').click();await page.locator('.compatibility-dialog').waitFor({state:'detached'});
  const accepted=await page.evaluate(()=>window.operations.execute({method:'runtime.compatibility',params:{action:'inspect'}}));
  check(version+' explicit consent is retained by the real host',accepted.compatibility.accepted);
  const diagnostics=await page.evaluate(()=>window.support.diagnostics());
  check(version+' feedback includes accelerator capabilities',diagnostics.workers.some(w=>w.role==='offline_search'&&w.capabilities));
  report['diagnostics_'+version]=diagnostics;
  await closeSession(session,child);session=null;child=null;
  helper.kill();await new Promise(r=>helper.once('exit',r));helper=null;
 }
 report.pass=true;
}catch(error){report.error=error.stack||String(error);if(session)await session.page.screenshot({path:join(out,'failure.png')}).catch(()=>{});}
finally{
 await closeSession(session,child).catch(e=>{report.pass=false;report.closeError=String(e)});
 if(helper){helper.kill();await new Promise(r=>helper.once('exit',r));}
 await writeFile(join(out,'running-image-e2e.json'),JSON.stringify(report,null,2));await rm(root,{recursive:true,force:true});
}
console.log(JSON.stringify({pass:report.pass,checks:report.checks.length,error:report.error,closeError:report.closeError}));if(!report.pass)process.exitCode=1;
