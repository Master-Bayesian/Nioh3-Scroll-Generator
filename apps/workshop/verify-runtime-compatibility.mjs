/** Production components in Chromium; scripted native replies, no game/save access. */
import assert from 'node:assert/strict';
import {build} from 'esbuild';
import {chromium} from 'playwright';
import {createServer} from 'node:http';
import {mkdir,mkdtemp,writeFile} from 'node:fs/promises';
import {join,resolve} from 'node:path';
const evidence=resolve(process.env.NIOH3_COMPATIBILITY_EVIDENCE||'D:/Nioh3_v080_deliverables/deliverables/codex-compatibility-policy-20261002');
const out=join(evidence,'browser-'+new Date().toISOString().replace(/[:.]/g,'-'));
await mkdir(out,{recursive:true});
const tmp=join(evidence,'tmp');await mkdir(tmp,{recursive:true});
const root=await mkdtemp(join(tmp,'compatibility-ui-'));
const bundle=await build({stdin:{contents:'import React from "react";import{createRoot}from"react-dom/client";import{RuntimeCompatibility}from"./apps/workshop/RuntimeCompatibility";import{GameInstallation}from"./apps/workshop/GameInstallation";import{Notice}from"./apps/workshop/Notice";import{setUiLocale}from"./apps/workshop/presentation";import"./apps/workshop/style.css";window.setLocale=setUiLocale;const runtime=createRoot(document.getElementById("runtime"));window.mountRuntime=()=>{window.runtimeMounted=true;runtime.render(<RuntimeCompatibility/>);};window.closeRuntime=()=>{window.runtimeMounted=false;runtime.render(null);};window.mountRuntime();const install=createRoot(document.getElementById("installation"));let generation=0;window.mountInstallation=(startup=false)=>{generation++;install.render(startup?<Notice key={generation} text="GAME_VERSION_UNSUPPORTED: fixture version"/>:<GameInstallation key={generation}/>);};window.mountInstallation();',resolveDir:process.cwd(),loader:'tsx'},bundle:true,write:false,outdir:'out',format:'iife',platform:'browser',jsx:'transform',jsxFactory:'localizedElement',tsconfigRaw:{compilerOptions:{jsx:'react',jsxFactory:'localizedElement'}},inject:[resolve('apps/workshop/presentation-jsx.ts')],define:{'process.env.NODE_ENV':'"production"'}});
const fixture=()=>{
 window.nioh={};
 window.testCase={windowCalls:[],closeError:null,closeDelay:false,verified:true,version:'2.0.1.0',outcome:'unique',calls:[],installCalls:[],plan:null,serial:0,active:0,maxActive:0,accepted:false,events:0,delay:false,acceptError:null};
 window.addEventListener('nioh3:compatibility-accepted',()=>window.testCase.events++);
 window.review={windowAction:async action=>{
  const f=window.testCase;f.windowCalls.push(action);
  if(action!=="close")throw Error("UNEXPECTED_WINDOW_ACTION");
  if(f.closeError){const error=f.closeError;f.closeError=null;throw Error(error);}
  if(f.closeDelay){f.closeDelay=false;await new Promise(resolve=>{f.releaseClose=resolve;});}
  window.closeRuntime();
 },gameInstallation:async action=>{
  const f=window.testCase;f.installCalls.push(action);if(f.installFailure){f.installFailure=false;throw Error('GAME_EXECUTABLE_UNREADABLE: fixture temporarily unavailable');}
  const known=['2.0.0.2','2.0.1.0','2.0.2.0'].includes(f.version),current=f.version==='2.0.2.0',missing=f.version===null;
  return{executable:missing?null:'D:/fixture/Nioh3.exe',file_version:f.version,restart_required:action!=='inspect',source:action==='reset'?'automatic':'selected',identity_error:missing?{code:'GAME_EXECUTABLE_NOT_FOUND',message:'GAME_EXECUTABLE_NOT_FOUND: fixture unavailable'}:null,compatibility:{status:missing?'unavailable':known?'known':'unknown',display_version:known?'PC '+f.version:null,data_version:known?current?'PC v2.02':'PC v2.00.02':null,resource_directory:known?current?'pc-v2.02':'pc-v2.00.02':null,runtime_profile:known?f.version:null,features:{offline_scroll_generation:known?'supported':missing?'unavailable':'unsupported',character_read_edit:known?current?'supported':'experimental':missing?'unavailable':'unsupported',native_scroll_add:known?current?'supported':f.version==='2.0.1.0'?'experimental':'unsupported':missing?'unavailable':'unsupported',native_equipment_add:known?current?'supported':'unsupported':missing?'unavailable':'unsupported'},reason:'Fixture-only registry evidence; no real-game acceptance'}};
 }};
 window.operations={execute:async command=>{
  const f=window.testCase,p=command.params;f.calls.push(command);f.active++;f.maxActive=Math.max(f.maxActive,f.active);
  try{
   if(command.method!=='runtime.compatibility')throw Error('WRITE_FORBIDDEN');
   const known=['2.0.0.2','2.0.1.0','2.0.2.0'].includes(f.version);
   if(p.action==='prepare'){
    f.accepted=false;f.plan=null;if(f.gone)return{compatibility:{present:false}};
    if(f.delay){f.delay=false;await new Promise(resolve=>{f.release=resolve;});}
    if(known&&f.verified&&!f.reference)f.plan={plan_id:(++f.serial).toString(16).padStart(64,'0'),bypassed_checks:['executable_sha256','game_version','character_layout_evidence'],allowed_features:['live_character','native_generation','temporary_override',...(f.version!=='2.0.0.2'?['live_count_edit','challenge_capacity_override']:[])],required_checks:['process_identity','code_and_layout','ownership_and_bounds','verified_backup','single_writer','recovery_receipts','readback'],audit_path:null};
   }
   if(p.action==='cancel'){f.accepted=false;f.plan=null;}
   if(p.action==='accept'){
    if(f.acceptDelay){f.acceptDelay=false;await new Promise(resolve=>{f.releaseAccept=resolve;});}
    if(f.acceptError){const error=f.acceptError;f.acceptError=null;f.plan=null;throw Error(error);}
    if(!f.verified||!known||!f.plan||p.plan_id!==f.plan.plan_id||!p.confirmed||!p.backup_confirmed)throw Error('COMPATIBILITY_PLAN_MISMATCH');
    f.accepted=true;f.plan.audit_path='D:/fixture-consent/verified.json';
   }
   if(f.reference)return{compatibility:{present:true,reference_match:true,warning:false,accepted:false,plan:null,backup:null,differences:[],hard_blocks:[],game_version:'2.0.2.0',executable:'D:/fixture/Nioh3.exe'}};
   const hard_blocks=known?f.verified?[]:[{code:'backup_unverified',detail:'No automatic save found'}]:[{code:'unsupported_version',detail:'No matching resource or runtime profile for '+f.version}];
   return{compatibility:{present:true,operation_scoped_features:known?(f.version==='2.0.2.0'?['live_scroll_add','live_equipment_add']:f.version==='2.0.1.0'?['live_scroll_add']:[]):[],warning:true,accepted:f.accepted,game_version:f.version,executable:'D:/fixture/Nioh3.exe',differences:[{code:'executable_sha256',expected:'a'.repeat(64),actual:'b'.repeat(64)},{code:'game_version',expected:'2.0.2.0',actual:f.version},{code:'character_layout_evidence',expected:'verified_reference',actual:'experimental_version_selected'}],hard_blocks,plan:f.plan?structuredClone(f.plan):null,backup:p.action==='inspect'?null:{attempted:true,verified:f.verified,paths:f.verified?['D:/fixture-backup/00-SAVEDATA.BIN']:[],files:f.verified?[{source:'D:/fixture-save/account-123/SAVEDATA00/SAVEDATA.BIN',copy:'D:/fixture-backup/00-SAVEDATA.BIN',bytes:64,sha256:'c'.repeat(64)}]:[],error:f.verified?null:'No automatic save found'},...(!known?{probe:{status:'structure_only',outcome:f.outcome,candidates:[{profile_version:'2.0.1.0',matched:f.outcome==='unique'||f.outcome==='multiple',error:null},{profile_version:'2.0.2.0',matched:f.outcome==='multiple',error:null}],note:'Read-only fixture probe; no write authorization'}}:{})}};
  }finally{f.active--;}
 }};
};
const html='<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><style>'+bundle.outputFiles.find(f=>f.path.endsWith('.css')).text+'</style></head><body><main style="max-width:760px;padding:20px"><input id="caller-input" aria-label="Fixture caller input" value="original"><div id="runtime"></div><div id="installation"></div></main><script>('+fixture.toString()+')();</script><script>'+bundle.outputFiles.find(f=>f.path.endsWith('.js')).text.replaceAll('</script','<\\/script')+'</script></body></html>';
const server=createServer((_,response)=>response.end(html));await new Promise(r=>server.listen(0,'127.0.0.1',r));
let browser;
const report={pass:false,boundary:'Production components/browser; native replies and host installation inspection scripted; no native window, package, real-game or real-save acceptance',viewport:{width:1020,height:640,deviceScaleFactor:1},checks:[],screenshots:[],profile:root};
const check=(name,condition)=>{report.checks.push({name,pass:!!condition});assert(condition,name)};
try{
 browser=await chromium.launchPersistentContext(join(root,'profile'),{headless:true,viewport:report.viewport});
 const page=await browser.newPage();await page.goto('http://127.0.0.1:'+server.address().port);
 await page.locator('.runtime-compatibility-banner').waitFor();await page.locator('#caller-input').fill('Retain selected equipment and value 321');
 const dialog=page.locator('.compatibility-dialog'),accept=page.locator('[data-action=compatibility-accept]'),risk=page.locator('[data-action=compatibility-risk]'),backed=page.locator('[data-action=compatibility-backup]');
 const ready=async()=>{await dialog.waitFor();await page.waitForFunction(()=>document.querySelector('.compatibility-body')?.getAttribute('aria-busy')==='false');};
 const open=async()=>{if(!await page.evaluate(()=>window.runtimeMounted)){const count=await page.evaluate(()=>window.testCase.calls.length);await page.evaluate(()=>window.mountRuntime());await page.waitForFunction(count=>window.testCase.calls.length>count&&window.testCase.active===0,count);}await page.evaluate(()=>window.dispatchEvent(new Event('nioh3:compatibility-required')));await ready();};
 const close=async(action='compatibility-close')=>{const count=await page.evaluate(()=>window.testCase.windowCalls.length);await page.locator('[data-action='+action+']').click();await dialog.waitFor({state:'detached'});check(action+' requests normal window close',await page.evaluate(count=>window.testCase.windowCalls.length===count+1&&window.testCase.windowCalls.at(-1)==='close'&&!window.runtimeMounted,count));};
 const screenshot=async name=>{const path=join(out,name+'.png');await page.screenshot({path});report.screenshots.push(path);};
 for(const locale of ['zh-CN','en-US','ja-JP']){
  await page.evaluate(locale=>{window.setLocale(locale);window.testCase.reference=false;window.testCase.version='2.0.1.0';window.testCase.verified=true;window.mountInstallation();},locale);
  await open();
  check(locale+' translated title',(await dialog.locator('h2').innerText())===({'zh-CN':'游戏版本兼容提示','en-US':'Game compatibility notice','ja-JP':'ゲーム版の互換性について'})[locale]);
  check(locale+' actual/reference fingerprints readable',(await dialog.innerText()).includes('a'.repeat(64))&&(await dialog.innerText()).includes('b'.repeat(64)));
  check(locale+' live additions have separate checks',await dialog.locator('[data-section=operation-scoped-features]').count()===1&&await page.evaluate(()=>!window.testCase.plan.allowed_features.includes('live_scroll_add')));
  await dialog.locator('[data-section=operation-scoped-features]').scrollIntoViewIfNeeded();await screenshot(locale+'-operation-scoped');
  check(locale+' backup source and destination visible',(await dialog.innerText()).includes('account-123/SAVEDATA00/SAVEDATA.BIN')&&(await dialog.innerText()).includes('00-SAVEDATA.BIN'));
  check(locale+' plan exposes only scoped live operations',await page.evaluate(()=>{const allowed=window.testCase.plan.allowed_features;return allowed.includes('live_count_edit')&&allowed.includes('native_generation')&&!allowed.includes('live_equipment_add')&&!allowed.includes('save_edit')&&!allowed.includes('offline_search');}));
  check(locale+' explicit consent defaults unchecked',!await risk.isChecked()&&!await backed.isChecked()&&await accept.isDisabled());
  await risk.check();check(locale+' risk alone insufficient',await accept.isDisabled());await backed.check();
  check(locale+' verified backup and both confirmations enable plan',await accept.isEnabled());
  check(locale+' footer stays inside constrained viewport',await accept.evaluate(e=>{const r=e.getBoundingClientRect();return r.top>=0&&r.bottom<=innerHeight&&r.left>=0&&r.right<=innerWidth}));
  check(locale+' dialog has no horizontal overflow',await dialog.evaluate(e=>e.scrollWidth<=e.clientWidth+1));
  await screenshot(locale+'-known-plan');
  const first=await page.evaluate(()=>window.testCase.plan.plan_id);
  await page.locator('[data-action=compatibility-prepare]').click();await ready();
  check(locale+' prepare resets both confirmations',!await risk.isChecked()&&!await backed.isChecked()&&await accept.isDisabled());
  check(locale+' prepare replaces plan identity',first!==await page.evaluate(()=>window.testCase.plan.plan_id));
  for(const code of ['COMPATIBILITY_PLAN_MISMATCH','COMPATIBILITY_IDENTITY_CHANGED','COMPATIBILITY_BACKUP_REQUIRED','COMPATIBILITY_AUDIT_FAILED']){
   await risk.check();await backed.check();await page.evaluate(code=>{window.testCase.acceptError=code+': fixture refusal';},code);await accept.click();await ready();
  check(locale+' '+code+' loses client consent',await accept.isDisabled()&&!await risk.isChecked()&&!await backed.isChecked());
  check(locale+' '+code+' gives actionable recovery',await dialog.locator('.notice .notice-actions button').count()>0);
  await page.locator('[data-action=compatibility-prepare]').click();await ready();
  }
  await risk.check();await backed.check();
  const current=await page.evaluate(()=>window.testCase.plan.plan_id);await accept.click();await dialog.waitFor({state:'detached'});
  check(locale+' exact current plan and confirmations sent',await page.evaluate(id=>{const p=window.testCase.calls.at(-1).params;return p.action==='accept'&&p.plan_id===id&&p.confirmed&&p.backup_confirmed;},current));
  await open();check(locale+' enter/exit actions are explicit',(await accept.innerText())===({'zh-CN':'进入兼容模式','en-US':'Enter compatibility mode','ja-JP':'互換モードに入る'})[locale]&&(await page.locator('[data-action=compatibility-exit]').innerText())===({'zh-CN':'关闭工具','en-US':'Close tool','ja-JP':'ツールを終了'})[locale]);await close();await open();await close('compatibility-exit');
  await page.evaluate(()=>{window.testCase.verified=false;});await open();
  check(locale+' failed backup blocks confirmations and accept',await risk.isDisabled()&&await backed.isDisabled()&&await accept.isDisabled());
  check(locale+' failed backup has no force plan',await dialog.locator('.compatibility-plan').count()===0&&(await dialog.innerText()).includes('No automatic save found'));
  check(locale+' blocking reason and remedy visible immediately',await dialog.locator('.compatibility-blocks').first().evaluate(e=>{const r=e.getBoundingClientRect();return r.top>=0&&r.bottom<innerHeight;}));
  await screenshot(locale+'-backup-failure');
  await page.evaluate(()=>{window.testCase.verified=true;});await page.locator('[data-action=compatibility-prepare]').click();await ready();
  check(locale+' backup failure can recover without leaving dialog',await risk.isEnabled()&&await accept.isDisabled()&&!await risk.isChecked()&&!await backed.isChecked());await close();
  await page.evaluate(()=>{window.testCase.verified=true;window.testCase.version='2.0.3.0';});
  for(const outcome of ['unique','multiple','none']){
   await page.evaluate(value=>{window.testCase.outcome=value;},outcome);await open();
   check(locale+' unknown '+outcome+' stays blocked',await accept.isDisabled()&&await dialog.locator('.compatibility-plan').count()===0);
   check(locale+' unknown '+outcome+' retains detected version',(await dialog.innerText()).includes('2.0.3.0'));
   if(outcome==='unique')await screenshot(locale+'-unknown-unique');await close();
  }
  for(const version of ['2.0.0.2','2.0.1.0','2.0.2.0','2.0.3.0',null]){
   await page.evaluate(version=>{window.testCase.version=version;window.mountInstallation();},version);
   await page.waitForFunction(version=>{const e=document.querySelector('[data-field=installed-file-version]');return e&&(!version||e.textContent===version);},version);
   const installed=page.locator('#installation .game-install');
   check(locale+' installation '+version+' four capabilities',await installed.locator('[data-feature]').count()===4);
   if(version==='2.0.3.0')check(locale+' unknown installation has no claimed support',await installed.locator('[data-status=unknown]').count()===1&&await installed.locator('[data-support=unsupported]').count()===4);
   if(version==='2.0.1.0')check(locale+' old experimental scope explicit',await installed.locator('[data-support=experimental]').count()===2&&await installed.locator('[data-feature=native_equipment_add] [data-support=unsupported]').count()===1);
   if(version===null)check(locale+' unavailable inspection has one recovery component',await installed.count()===1&&await installed.locator('[data-status=unavailable]').count()===1);
  }
  await page.evaluate(()=>{window.testCase.version='2.0.3.0';window.mountInstallation(true);});
  await page.locator('#installation [data-status=unknown]').waitFor();
  check(locale+' startup error exposes host-only inspection',await page.locator('#installation [data-field=installed-file-version]').innerText()==='2.0.3.0');
  await page.evaluate(()=>{window.testCase.installFailure=true;});await page.locator('#installation [data-action=inspect-game-executable]').click();await page.waitForFunction(()=>!document.querySelector('#installation [data-action=inspect-game-executable]').disabled);
  await page.locator('#installation [data-action=inspect-game-executable]').click();await page.waitForFunction(()=>!document.querySelector('#installation [data-action=inspect-game-executable]').disabled);
  check(locale+' host-only inspection retries after failure',await page.locator('#installation .game-install').count()===1&&await page.locator('#installation [data-action=select-game-executable]').isEnabled());
  await screenshot(locale+'-startup-unknown');
  await page.evaluate(()=>{window.testCase.version='2.0.1.0';});await open();
  const eventsBeforeReference=await page.evaluate(()=>window.testCase.events);
  await page.evaluate(()=>{window.testCase.reference=true;window.testCase.version='2.0.2.0';});
  await page.locator('[data-action=compatibility-prepare]').click();await ready();
  check(locale+' reconnect to reference explains no consent needed',await dialog.locator('.notice-success').count()===1&&await risk.count()===0&&await backed.count()===0&&await accept.count()===0);
  const returnButton=page.locator('[data-action=compatibility-return]');
  check(locale+' reference result offers enabled return action',await returnButton.isEnabled());
  check(locale+' reference match has no misleading backup block',await dialog.locator('.compatibility-blocks').count()===0);
  await screenshot(locale+'-reference-match');
  await returnButton.click();await dialog.waitFor({state:'detached'});await page.waitForFunction(()=>window.testCase.calls.at(-1)?.params.action==='cancel'&&window.testCase.active===0);
  check(locale+' reference return preserves caller inputs without accepting or writing',await page.locator('#caller-input').inputValue()==='Retain selected equipment and value 321'&&await page.evaluate(count=>window.testCase.events===count,eventsBeforeReference));
  await page.evaluate(()=>{window.testCase.reference=false;});
 }
 await page.evaluate(()=>{window.testCase.version='2.0.1.0';window.testCase.verified=true;window.testCase.delay=true;window.dispatchEvent(new Event('nioh3:compatibility-required'));});
 await page.waitForFunction(()=>typeof window.testCase.release==='function');
 await page.waitForFunction(()=>document.querySelector('.compatibility-dialog')?.open===true);
 await page.keyboard.press('Escape');await dialog.waitFor({state:'detached'});
 await page.evaluate(()=>window.testCase.release());await page.waitForFunction(()=>window.testCase.active===0);
 check('Escape during prepare requests close and ignores late renderer reply',await dialog.count()===0&&await page.evaluate(()=>window.testCase.windowCalls.at(-1)==='close'&&!window.runtimeMounted));
 await open();check('new session after late reply starts unchecked',!await risk.isChecked()&&!await backed.isChecked());
 const outsideCount=await page.evaluate(()=>window.testCase.windowCalls.length);await page.mouse.click(3,3);await dialog.waitFor({state:'detached'});
 check('outside click requests tool close',await page.evaluate(count=>window.testCase.windowCalls.length===count+1&&!window.runtimeMounted,outsideCount));
 await open();await page.evaluate(()=>{window.testCase.closeError='WINDOW_CLOSE_FAILED: fixture refusal';});
 await page.locator('[data-action=compatibility-exit]').click();await page.waitForFunction(()=>document.querySelector('.compatibility-dialog .notice')?.textContent.includes('WINDOW_CLOSE_FAILED'));
 check('close failure stays modal with explicit retry',await dialog.isVisible()&&await page.locator('[data-action=compatibility-exit]').isEnabled()&&await accept.isDisabled());
 await page.locator('[data-action=compatibility-prepare]').click();await ready();
 check('close failure requires fresh unchecked consent',!await risk.isChecked()&&!await backed.isChecked());
 await page.evaluate(()=>{window.testCase.closeDelay=true;});const delayedCloseCount=await page.evaluate(()=>window.testCase.windowCalls.length);
 await page.locator('[data-action=compatibility-exit]').click();await page.waitForFunction(()=>typeof window.testCase.releaseClose==='function');
 check('pending shutdown retains blocking dialog and disables actions',await dialog.isVisible()&&await accept.isDisabled()&&await page.locator('[data-action=compatibility-prepare]').isDisabled()&&await page.locator('[data-action=compatibility-exit]').isDisabled());
 const callsBeforeReopen=await page.evaluate(()=>window.testCase.calls.length);await page.keyboard.press('Escape');await page.mouse.click(3,3);await page.evaluate(()=>window.dispatchEvent(new Event('nioh3:compatibility-required')));
 check('repeated close/reopen cannot duplicate shutdown or rebuild consent',await page.evaluate(({count,calls})=>window.testCase.windowCalls.length===count+1&&window.testCase.calls.length===calls,{count:delayedCloseCount,calls:callsBeforeReopen}));
 await screenshot('pending-safe-shutdown');await page.evaluate(()=>window.testCase.releaseClose());await dialog.waitFor({state:'detached'});
 await open();await risk.check();await backed.check();await page.evaluate(()=>{window.testCase.acceptDelay=true;window.testCase.closeDelay=true;});
 const eventsBeforeClosing=await page.evaluate(()=>window.testCase.events);await accept.click();await page.waitForFunction(()=>typeof window.testCase.releaseAccept==='function');
 await page.locator('[data-action=compatibility-exit]').click();await page.waitForFunction(()=>typeof window.testCase.releaseClose==='function');
 await page.evaluate(()=>window.testCase.releaseAccept());await page.waitForFunction(()=>window.testCase.active===0);
 check('late accepted reply cannot resume the caller during shutdown',await dialog.isVisible()&&await page.evaluate(count=>window.testCase.events===count,eventsBeforeClosing));
 await page.evaluate(()=>window.testCase.releaseClose());await dialog.waitFor({state:'detached'});
 await page.evaluate(()=>{window.testCase.gone=true;});await open();
 check('game absent offers reconnect with consent disabled',await accept.isDisabled()&&await page.locator('[data-action=compatibility-prepare]').isEnabled());
 await page.evaluate(()=>{window.testCase.gone=false;});await page.locator('[data-action=compatibility-prepare]').click();await ready();
 check('game restart rebuilds plan with fresh confirmations',await risk.isEnabled()&&!await risk.isChecked()&&!await backed.isChecked());await close();
 check('caller inputs survive cancel, failure and reconnect',await page.locator('#caller-input').inputValue()==='Retain selected equipment and value 321');
 check('only one native action runs at a time',await page.evaluate(()=>window.testCase.maxActive===1));
 check('fixtures never invoke protected writes',await page.evaluate(()=>window.testCase.calls.every(c=>c.method==='runtime.compatibility')));
 report.calls=await page.evaluate(()=>window.testCase.calls);report.windowCalls=await page.evaluate(()=>window.testCase.windowCalls);report.pass=true;
}catch(error){report.error=error.stack||String(error);}
finally{await browser?.close();await new Promise(r=>server.close(r));await writeFile(join(out,'compatibility-ui.json'),JSON.stringify(report,null,2));}
console.log(JSON.stringify({pass:report.pass,checks:report.checks.length,out,profile:root,error:report.error}));if(!report.pass)process.exitCode=1;
