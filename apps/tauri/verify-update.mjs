/** Exercise the real portable restart and startup acknowledgement without game writes. */
import {chromium} from 'playwright';
import {cp,mkdtemp,mkdir,readFile,writeFile,access} from 'node:fs/promises';
import {spawnSync} from 'node:child_process';
import {createHash,randomUUID} from 'node:crypto';
import {dirname,join,resolve} from 'node:path';
import {tmpdir} from 'node:os';
import {createServer} from 'node:net';
import assert from 'node:assert/strict';
const source=dirname(resolve(process.env.NIOH3_TAURI_EXE));
const deadline=setTimeout(()=>{console.error('Update acceptance exceeded 120 seconds');process.exit(1);},120000);deadline.unref();
const root=await mkdtemp(join(tmpdir(),'nioh3-tauri-restart-'));
const profile=join(root,'profile'), cache=join(profile,'updates'), stage=join(cache,randomUUID(),'package'), target=join(root,'installed');
await mkdir(cache,{recursive:true});await cp(source,target,{recursive:true});await cp(source,stage,{recursive:true});
const helper=join(cache,'apply-update.ps1');await cp('apps/tauri/src-tauri/apply-update.ps1',helper);
const hash=createHash('sha256').update(await readFile(join(stage,'build-manifest.json'))).digest('hex');
const server=createServer();await new Promise(r=>server.listen(0,'127.0.0.1',r));const port=server.address().port;await new Promise(r=>server.close(r));
const result=spawnSync('powershell.exe',['-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',helper,'-ProcessId','2147483647','-Target',target,'-Staged',stage,'-ManifestHash',hash,'-Profile',profile],{windowsHide:true,encoding:'utf8',timeout:30000,env:{...process.env,NIOH3_STATE_ROOT:join(root,'state'),WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port}`}});
assert.equal(result.status,0,result.stderr);
let browser;
try {
  for(let i=0;i<100;i++){try{if((await fetch(`http://127.0.0.1:${port}/json/version`)).ok)break;}catch{}await new Promise(r=>setTimeout(r,300));}
  browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  const page=browser.contexts()[0].pages()[0];
  await page.locator('.shell').waitFor({timeout:45000});
  let receipt;
  for(let i=0;i<100;i++) {receipt=JSON.parse(await readFile(join(cache,'last-update-result.json'),'utf8'));if(receipt.status==='completed')break;await new Promise(r=>setTimeout(r,300));}
  assert.equal(receipt.status,'completed',JSON.stringify(receipt));
  let status;
  for(let i=0;i<100;i++){status=await page.evaluate(()=>window.review.update({action:'status',channel:'stable'}));if(status.canApply)break;await new Promise(r=>setTimeout(r,100));}
  assert.equal(status.canApply,true);
  await assert.rejects(access(receipt.previous));await assert.rejects(access(stage));
  const output=resolve('deliverables/frontend-v2/tauri-acceptance');await mkdir(output,{recursive:true});
  await writeFile(join(output,'real-update-restart.json'),JSON.stringify({replacement:true,realApplicationStarted:true,workerHandshake:true,previousRemoved:true,cacheRemoved:true,gameWrites:0},null,2));
  await page.evaluate(()=>window.review.windowAction('close'));
  console.log('TAURI_REAL_UPDATE_RESTART_CLEANUP_OK');
} finally {if(browser){const p=browser.contexts()[0]?.pages()[0];if(p)await p.evaluate(()=>window.review.windowAction('close')).catch(()=>{});await Promise.race([browser.close(),new Promise(r=>setTimeout(r,3000))]);}}
