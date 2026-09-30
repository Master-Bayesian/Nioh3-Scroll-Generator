/** Native UI acceptance, then reusable outer-EXE acceptance.
 * Failures: stale workers, missing peer entry points, job/result mismatch,
 * hidden painting/choices, clipping in three locales, and accidental writes.
 * Only retained scroll records go to the real read-only prediction backend.
 * Equipment preview and save inventory are explicit presentation fixtures.
 */
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdir,mkdtemp,readFile,writeFile} from 'node:fs/promises';
import {join,resolve} from 'node:path';
import {parseArgs} from 'node:util';
import {chromium} from 'playwright';
import {isolatedEnvironment,closeSession,pause,inspectOnefile} from './onefile-acceptance.mjs';
const {values}=parseArgs({options:{exe:{type:'string'},out:{type:'string'},onefile:{type:'boolean',default:false}}});
assert(values.exe&&values.out);
const out=resolve(values.out),target='D:/Nioh3_v080_deliverables/build-cache/tauri-target';
await mkdir(out,{recursive:true});const root=await mkdtemp('D:/Nioh3_v080_deliverables/tmp/v083-native-');
const {env,profile,port}=await isolatedEnvironment(root);
if(!values.onefile)Object.assign(env,{NIOH3_RUST_PROTECTED_WORKER:join(target,'debug/nioh3-protected-worker.exe'),NIOH3_RUST_SEARCH_WORKER:join(target,'debug/nioh3-readonly-worker.exe'),NIOH3_RUST_PROTECTED_GAME_FILE_VERSION:'2.0.2.0',NIOH3_RUST_SEARCH_GAME_FILE_VERSION:'2.0.2.0'});
const recordHex=(await readFile('crates/nioh3-protected/tests/fixtures/scroll-completion/seed121723131_pre_c1.hex','utf8')).trim();
const raw=Buffer.from(recordHex,'hex');
const reference={save_id:'presentation-fixture',account_id:'76561198000000123',save_slot:0,path:join(root,'presentation-only/SAVEDATA.BIN')};
const inventory={save_id:reference.save_id,account_id:reference.account_id,snapshot_id:'b'.repeat(64),source_sha256:'c'.repeat(64),empty_slots:399,entries:[{
 slot_index:0,record_hex:recordHex,header:{seed:raw.readUInt32LE(0x20),playthrough:3,rarity:4,level:raw.readUInt16LE(6),recommended_level:raw.readUInt16LE(8),transfer_count:0},
 effects:Array.from({length:7},(_,slot_index)=>{const at=0x34+slot_index*0x18;return {slot_index,effect_id:raw.readUInt32LE(at+4),value:raw.readUInt32LE(at+8),prefix:raw.readUInt32LE(at),metadata:raw.readUInt32LE(at+12),tail_0:raw.readUInt32LE(at+16),tail_1:raw.readUInt32LE(at+20)}}),
 derived:{initial_challenge_capacity:3,remaining_challenge_attempts:raw[0x33],recommended_displayed_level:180,recommended_raw_was_clamped:false,recommended_raw_level:raw.readUInt16LE(8)}
}]};
const report={pass:false,packaged:values.onefile,root,scope:'native WebView2; real read-only prediction/receipt guards; fixture save inventory/equipment preview',gameWrites:0,saveWrites:0,checks:[],screenshots:[]};
const check=(name,pass)=>{assert(pass,name);report.checks.push(name)};
let child,session;
try{
 if(values.onefile){const identity=await inspectOnefile(resolve(values.exe));report.executableSha256=identity.sha256;report.payloadSha256=identity.payloadSha256;}
 child=spawn(resolve(values.exe),['--user-data-dir',profile],{cwd:process.cwd(),env,windowsHide:true,stdio:'ignore'});
 for(let i=0;i<150;i++){assert(child.exitCode===null,'native shell exited');try{if((await fetch('http://127.0.0.1:'+port+'/json/version')).ok)break}catch{}await pause(300)}
 const browser=await chromium.connectOverCDP('http://127.0.0.1:'+port);const page=browser.contexts()[0].pages()[0];session={browser,page};
 await page.locator('.shell').waitFor({timeout:45000});await page.waitForFunction(()=>!!window.operations&&!!window.nioh);
 report.diagnostics=await page.evaluate(()=>window.support.diagnostics());
 if(values.onefile)check('Native package manifest verified',report.diagnostics.packageVerification.ok);
 const prediction=await page.evaluate(async record_hex=>{const h=await window.nioh.handshake();return window.operations.execute({method:'runtime.scroll_completion_predict',params:{record_hex,context_digest:h.context.context_digest}})},recordHex);
 check('Real native bridge predicts three replacement choices',prediction.completion_prediction.candidates.length===3);
 check('Real native bridge predicts painting draw 5659',prediction.completion_prediction.painting.draw===5659);
 report.prediction=prediction;
 const guarded=await page.evaluate(async()=>{const started=await window.operations.execute({method:'runtime.equipment_add_status',params:{operation_id:'77777777-7777-4777-8777-777777777777'}});if(!('job_id' in started))return started;for(let i=0;i<100;i++){const job=await window.operations.snapshot('runtime',started.job_id);if(job.state==='completed')return job.result;if(job.state==='failed')throw Error(JSON.stringify(job.error));await new Promise(r=>setTimeout(r,100))}throw Error('STATUS_TIMEOUT')});
 check('Real equipment status job returns its typed final result',guarded.equipment_add.state==='rejected_before_dispatch');
 await page.evaluate(({reference,inventory})=>{
   window.__acceptanceCalls=[];const original=window.operations.execute.bind(window.operations);
   window.operations.selectSave=async()=>reference;
   window.operations.execute=async command=>{
    window.__acceptanceCalls.push(command);
    if(command.method==='save.discover')return {saves:[reference]};
    if(command.method==='save.inventory')return inventory;
    if(command.method==='save.operations')return {operations:[]};
    if(command.method==='runtime.equipment_add_prepare'){
      const p=command.params;return {equipment_add:{operation_id:p.operation_id,plan_digest:'a'.repeat(64),state:'prepared',process_id:42,slot_index:3,error:null,preview:{slot_index:3,item_id:p.item_id,appearance_id:p.item_id,quantity:1,level:p.level,level_before_forge:p.level,plus:p.plus,familiarity:0,inventory_key:0,seed:p.seed,rarity:p.rarity,effects:[{effect_id:44634,value:150,star:false}]}}};
    }
    if(command.method==='runtime.equipment_add_cancel')return {equipment_add:{operation_id:command.params.operation_id,plan_digest:'a'.repeat(64),state:'cancelled',process_id:42,slot_index:3,error:null,preview:null}};
    if(command.method==='runtime.equipment_add_execute'||command.method.startsWith('save.prepare')||command.method==='save.commit')throw Error('WRITE_FORBIDDEN');
    return original(command);
   };
 },{reference,inventory});
 await page.locator('.nav nav>button').nth(1).click();
 await page.locator('.editor-host .save-picker>button').nth(1).click();
 await page.waitForFunction(()=>!!document.querySelector('.scroll-completion [data-action=predict]')&&!document.querySelector('.scroll-completion [data-action=predict]').disabled);
 for(const [locale,label] of [['zh-CN','简体中文'],['en-US','English'],['ja-JP','日本語']]){
   await page.locator('.language-button').click();await page.locator('.side-popup button').filter({hasText:label}).click();
   await page.waitForFunction(locale=>document.documentElement.lang===locale,locale);
   await page.locator('.nav nav>button').nth(1).click();
   const section=page.locator('.scroll-completion');if(!await section.evaluate(e=>e.open))await section.locator(':scope>summary').click();
   await section.locator('[data-action=predict]').click();await section.locator('.completion-results').waitFor();
   check(locale+' prediction has reachable choices',await section.locator('.completion-choices>button').count()===4);
   await section.locator('.completion-choices>button').nth(1).click();await section.locator('[data-action=next]').click();
   await section.locator('.completion-round[data-round="2"]').waitFor();
   check(locale+' painting result is visible',(await section.locator('.completion-painting').innerText()).includes('66'));
   await section.evaluate(e=>{for(let node=e;node;node=node.parentElement)node.scrollTop=0;e.querySelector('.editor-section-body').scrollTop=0});await page.evaluate(()=>window.scrollTo(0,0));
   report['completionGeometry_'+locale]=await section.evaluate(e=>{const rows=[];for(let node=e;node;node=node.parentElement){const r=node.getBoundingClientRect(),s=getComputedStyle(node);rows.push({tag:node.tagName,class:node.className,top:r.top,height:r.height,scrollHeight:node.scrollHeight,scrollTop:node.scrollTop,display:s.display,overflow:s.overflow,flex:s.flex})}return rows});
   check(locale+' editor uses the available native viewport',await page.locator('.editor-host').evaluate(e=>e.clientHeight>=innerHeight-60));
   await section.locator('.completion-painting').scrollIntoViewIfNeeded();
   await page.screenshot({path:join(out,'native-completion-'+locale+'.png'),fullPage:true});report.screenshots.push('native-completion-'+locale+'.png');
   await page.locator('.nav nav>button').nth(4).click();await page.locator('.character-sections button').nth(1).click();
   await page.locator('[data-action=pick-equipment]').first().click();await page.locator('[data-action=prepare-equipment]').click();await page.locator('.live-equipment-preview').waitFor();
   check(locale+' equipment confirmation prevents insertion',await page.locator('[data-action=execute-equipment]').isDisabled());
   await page.locator('.live-equipment-add').evaluate(e=>e.scrollTop=0);await page.evaluate(()=>window.scrollTo(0,0));await page.screenshot({path:join(out,'native-equipment-'+locale+'.png')});report.screenshots.push('native-equipment-'+locale+'.png');
   await page.locator('.live-equipment-confirm').scrollIntoViewIfNeeded();check(locale+' confirmation is inside the native viewport',await page.locator('.live-equipment-confirm').evaluate(e=>{const r=e.getBoundingClientRect();return r.top>=50&&r.bottom<=innerHeight}));check(locale+' native header stays visible',await page.locator('.topbar').evaluate(e=>Math.abs(e.getBoundingClientRect().top)<1));await page.screenshot({path:join(out,'native-equipment-confirm-'+locale+'.png')});report.screenshots.push('native-equipment-confirm-'+locale+'.png');
   await page.locator('[data-action=cancel-equipment]').click();await page.locator('[data-state=cancelled]').waitFor();await page.locator('[data-action=new-equipment]').click();
 }
 // Fractional native DPI may round a restored client area by one CSS pixel.
 const before=await page.evaluate(()=>({width:innerWidth,height:innerHeight,dpr:devicePixelRatio}));await page.evaluate(()=>window.review.windowAction('maximize'));await pause(300);const maximized=await page.evaluate(()=>({width:innerWidth,height:innerHeight,dpr:devicePixelRatio}));await page.evaluate(()=>window.review.windowAction('maximize'));await pause(300);const restored=await page.evaluate(()=>({width:innerWidth,height:innerHeight,dpr:devicePixelRatio}));report.geometry={before,maximized,restored,roundingToleranceCss:1};check('Native maximize and restore preserve dimensions',Math.abs(restored.width-before.width)<=1&&Math.abs(restored.height-before.height)<=1&&restored.dpr===before.dpr);
 check('Acceptance issued no insertion or save write',await page.evaluate(()=>!window.__acceptanceCalls.some(c=>c.method==='runtime.equipment_add_execute'||c.method==='save.commit'||c.method.startsWith('save.prepare'))));
 report.pass=true;
}catch(error){report.error=error.stack||String(error);if(session){await session.page.screenshot({path:join(out,'failure.png')}).catch(()=>{});report.visibleText=await session.page.locator('body').innerText().catch(()=>null)}}finally{
 try{await closeSession(session,child)}catch(error){report.pass=false;report.closeError=error.stack||String(error)}
 await writeFile(join(out,'native-missing-features.json'),JSON.stringify(report,null,2));
}
console.log(JSON.stringify({pass:report.pass,checks:report.checks.length,error:report.error,closeError:report.closeError}));if(!report.pass)process.exitCode=1;
