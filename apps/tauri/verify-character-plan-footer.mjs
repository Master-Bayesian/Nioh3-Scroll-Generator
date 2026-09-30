/** Preimplementation full product UI E2E, reusable in Chromium/native/onefile.
 * Enumerated failure cases: V083_PLAN_FOOTER_ACCEPTANCE_20260930.md.
 * Inventory, plans and commits are scripted; no real save/game writes.
 */
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdir,mkdtemp,writeFile} from 'node:fs/promises';
import {createServer} from 'node:http';
import {join,resolve} from 'node:path';
import {parseArgs} from 'node:util';
import {build} from 'esbuild';
import {chromium} from 'playwright';
import {isolatedEnvironment,inspectOnefile,closeSession,pause} from './onefile-acceptance.mjs';
const {values}=parseArgs({options:{exe:{type:'string'},out:{type:'string'},onefile:{type:'boolean',default:false}}});
const out=resolve(values.out||'D:/Nioh3_v080_deliverables/deliverables/codex-v083-plan-footer-20260930/browser');
await mkdir(out,{recursive:true});
const root=await mkdtemp('D:/Nioh3_v080_deliverables/tmp/plan-footer-');
const report={pass:false,packaged:values.onefile,native:!!values.exe,root,gameWrites:0,realSaveWrites:0,checks:[],screenshots:[]};
const check=(name,condition,details)=>{report.checks.push({name,pass:!!condition,details});assert(condition,name)};
let context,page,server,child,session;
const fixture=()=>{
 const reference={save_id:'plan-footer-fixture',account_id:'10001',save_slot:0,path:'D:/synthetic/SAVEDATA00/SAVEDATA.BIN'};
 const inventory={save_id:reference.save_id,account_id:reference.account_id,snapshot_id:'b'.repeat(64),source_sha256:'a'.repeat(64),empty_slots:399,entries:[]};
 const equipment={slot_index:0,item_id:171,appearance_id:171,quantity:1,level:180,level_before_forge:180,plus:0,familiarity:0,inventory_key:1,seed:2222,rarity:4,type_class:39,worn:false,
 effects:Array.from({length:5},(_,index)=>({index,effect_id:[388,171,540,15,111][index],value:index===0?1:30+index,star:false}))};
 const character={save_id:reference.save_id,source_sha256:inventory.source_sha256,currencies:{amrita:0,gold:0},equipment_slots:2500,equipment:[],items:[],generation:{difficulty:3,difficulties:[{difficulty:3,progress:[6510,7710,0,7710]}]}};
 window.__planFooter={calls:[],size:5,mode:'normal',inventory,character,reference,equipment,serial:0,plans:0};
 const api=window.operations;
 const originalSnapshot=api.snapshot?.bind(api);
 api.selectSave=async()=>reference;
 api.current=async()=>({job:null,busy:false});
 const respond=async command=>{
  const f=window.__planFooter,p=command.params;f.calls.push(structuredClone(command));
  if(command.method==='save.discover')return {saves:[reference]};
  if(command.method==='save.inventory')return structuredClone(inventory);
  if(command.method==='save.operations')return {operations:[]};
  if(command.method==='save.character')return structuredClone(character);
  if(command.method==='runtime.character_snapshot')throw Error('no running process matches Nioh3.exe');
  if(command.method==='runtime.equipment_rules')return {item_id:p.item_id,known:true,roles:['innate','random','random','random','grace'],random_pool:[{effect_id:15,star:false,min:1,max:100,group:1,masks:[0,0]}],innate_pool:[{effect_id:388,star:false,min:1,max:1}],grace_pool:[],set_pool:[]};
  if(command.method==='runtime.equipment_seeds')return {item_id:p.item_id,rarity:p.rarity,level:p.level,difficulty:p.difficulty,seeds:65536,empty:0,matches:1,outcomes:[{seed:2222,effects:equipment.effects.map(e=>({...e,roll:90,role:'random'}))}]};
  if(command.method==='save.prepare_character_edit'){
   f.plans++;return {plan_id:'plan-'+f.plans,save_id:reference.save_id,source_sha256:inventory.source_sha256,kind:'edit',expires_in_seconds:600,preview:{added:Array.from({length:f.size},(_,index)=>({slot_index:index,seeded:true,after:{...equipment,slot_index:index},audit:{natural:true,findings:[],unverified:[],verdict:'natural'}}))}};
  }
  if(command.method==='save.discard')return {discarded:true};
  if(command.method==='save.commit'){
   if(f.mode==='refused'){inventory.source_sha256='d'.repeat(64);character.source_sha256=inventory.source_sha256;const id='refused-'+f.serial++;f.jobs??={};f.jobs[id]={job_id:id,kind:command.method,state:'failed',sequence:2,cancellable:false,progress:null,result:null,error:{code:'OPERATION_FAILED',message:'Save changed after preparation; no write attempted'}};return {...f.jobs[id],state:'running',sequence:1,error:null};}
   if(f.mode==='unknown'){const id='lost-'+f.serial++;f.jobs??={};f.jobs[id]={job_id:id,kind:command.method,state:'failed',sequence:2,cancellable:false,progress:null,result:null,error:{code:'TRANSPORT_LOST',message:'Connection closed without a receipt; diagnostic path contains Save changed after preparation; no write attempted'}};return {...f.jobs[id],state:'running',sequence:1,error:null};}
   return {operation_id:p.plan_id,save_id:reference.save_id,commit_status:'committed',restore_status:'not_needed'};
  }
  if(command.method==='save.operation')return {operation_id:p.plan_id,save_id:reference.save_id,commit_status:'unknown',restore_status:'not_needed'};
  if(command.method==='runtime.effect_values')return {values:[]};
 throw Error('FIXTURE_METHOD_NOT_ALLOWED: '+command.method);
 };
 api.execute=async command=>{
  const reply=await respond(command);
  if(!command.method.startsWith('save.')||'job_id' in reply)return reply;
  const f=window.__planFooter,id='save-job-'+f.serial++;f.jobs??={};
  f.jobs[id]={job_id:id,kind:command.method,state:'completed',sequence:2,cancellable:false,progress:null,result:reply,error:null};
  return {...f.jobs[id],state:'running',sequence:1,result:null};
 };
 api.snapshot=async(role,id)=>window.__planFooter.jobs?.[id]??originalSnapshot?.(role,id);
};
async function prepare(count=5){
 await page.evaluate(count=>window.__planFooter.size=count,count);
 await page.locator('.character-queue button.primary').click();
 await page.locator('.character-save-plan').waitFor();
}
async function discard(){await page.locator('[data-action=discard-character-plan]').click();await page.waitForFunction(()=>!document.querySelector('.character-save-plan'))}
async function inspect(label){
 const plan=page.locator('.character-save-plan'),body=plan.locator('.character-plan-details'),footer=plan.locator('.character-plan-footer');
 await plan.evaluate(e=>{for(let n=e;n;n=n.parentElement)n.scrollTop=0});
 await plan.scrollIntoViewIfNeeded();
 const geometry=await footer.evaluate(e=>{const r=e.getBoundingClientRect(),p=e.closest('.character-save-plan').getBoundingClientRect();return {top:r.top,bottom:r.bottom,left:r.left,right:r.right,height:r.height,viewport:{width:innerWidth,height:innerHeight,dpr:devicePixelRatio},plan:{top:p.top,bottom:p.bottom}}});
 check(label+' footer starts inside viewport',geometry.top>=0&&geometry.bottom<=geometry.viewport.height&&geometry.left>=0&&geometry.right<=geometry.viewport.width+1,geometry);
 check(label+' commit starts unchecked',await page.locator('[data-action=commit-character-plan]').isDisabled());
 await body.evaluate(e=>e.scrollTop=e.scrollHeight);
 const after=await footer.boundingBox();check(label+' details scroll without moving footer',Math.abs(after.y-geometry.top)<1);
 check(label+' all plan entries remain scrollable',await body.evaluate(e=>e.scrollHeight>e.clientHeight&&e.scrollTop>0));
 await page.screenshot({path:join(out,label+'.png')});report.screenshots.push(label+'.png');
 await page.locator('[data-action=confirm-character-plan]').check();
 check(label+' confirmation enables commit',await page.locator('[data-action=commit-character-plan]').isEnabled());
}
try{
 if(values.exe){
  const {env,profile,port}=await isolatedEnvironment(root);
  if(!values.onefile)Object.assign(env,{NIOH3_RUST_SEARCH_WORKER:'D:/Nioh3_v080_deliverables/build-cache/tauri-target/debug/nioh3-readonly-worker.exe',NIOH3_RUST_PROTECTED_WORKER:'D:/Nioh3_v080_deliverables/build-cache/tauri-target/debug/nioh3-protected-worker.exe',NIOH3_RUST_SEARCH_GAME_FILE_VERSION:'2.0.2.0',NIOH3_RUST_PROTECTED_GAME_FILE_VERSION:'2.0.2.0'});
  if(values.onefile){const identity=await inspectOnefile(values.exe);report.executableSha256=identity.sha256;report.payloadSha256=identity.payloadSha256;}
  child=spawn(resolve(values.exe),['--user-data-dir',profile],{env,windowsHide:true,stdio:'ignore'});
  for(let i=0;i<150;i++){try{if((await fetch('http://127.0.0.1:'+port+'/json/version')).ok)break}catch{}await pause(300)}
  context=await chromium.connectOverCDP('http://127.0.0.1:'+port);page=context.contexts()[0].pages()[0];session={browser:context,page};
  await page.locator('.shell').waitFor();
  await page.waitForFunction(()=>{const button=document.querySelector('.editor-host .save-picker>button');return button&&!button.disabled},{},{timeout:30000});
 }else{
  const rootSource=resolve(import.meta.dirname,'../..');
  const bundle=await build({entryPoints:[join(rootSource,'apps/workshop/main.tsx')],bundle:true,write:false,outdir:'out',format:'iife',platform:'browser',jsx:'transform',jsxFactory:'localizedElement',tsconfigRaw:{compilerOptions:{jsx:'react',jsxFactory:'localizedElement'}},inject:[join(rootSource,'apps/workshop/presentation-jsx.ts')],define:{'process.env.NODE_ENV':'"production"'}});
  const html=`<!doctype html><html><head><meta charset="utf-8"><style>${bundle.outputFiles.find(f=>f.path.endsWith('.css')).text}</style></head><body><div id="root"></div><script>const canned=async()=>({});window.operations={execute:canned,current:async()=>({job:null,busy:false})};window.nioh={handshake:canned};window.preferences={getLocale:async()=> 'zh-CN',setLocale:canned};window.support={diagnostics:canned};window.review={update:async()=>({phase:'current',canApply:true}),windowAction:canned,favorites:async()=>[],log:canned};</script><script>${bundle.outputFiles.find(f=>f.path.endsWith('.js')).text.replaceAll('</script','<\\/script')}</script></body></html>`;
  server=createServer((_,res)=>res.end(html));await new Promise(r=>server.listen(0,'127.0.0.1',r));
  context=await chromium.launchPersistentContext(join(root,'profile'),{headless:true,viewport:{width:1440,height:900}});page=await context.newPage();await page.goto('http://127.0.0.1:'+server.address().port);
 }
 await page.evaluate(fixture);await page.locator('.nav nav>button').nth(4).click();
 await page.locator('.character-modes button').nth(1).click();await page.locator('.character-page .save-picker>button').nth(1).click();
 await page.locator('.character-sections button').nth(1).click();
 await page.locator('[data-row="add-171"]').click();
 await page.locator('.seed-panel button.primary').click();await page.locator('.seed-outcomes button').first().click();
 await page.locator('.seed-panel .character-actions>button').nth(1).click();await page.locator('.character-queue').waitFor();
 for(const [locale,title] of [['zh-CN','简体中文'],['en-US','English'],['ja-JP','日本語']]){
  await page.locator('.language-button').click();await page.locator('.side-popup button').filter({hasText:title}).click();
  for(const count of [5,16]){
   await prepare(count);await inspect(locale+'-'+count);await discard();
  }
 }
 if(!values.exe){for(const viewport of [{width:1100,height:740},{width:935,height:709}]){await page.setViewportSize(viewport);await prepare(16);await inspect('small-'+viewport.width);await discard();}}
 else{
  const before=await page.evaluate(()=>({width:innerWidth,height:innerHeight,dpr:devicePixelRatio}));await page.evaluate(()=>window.review.windowAction('maximize'));await pause(250);await prepare(16);await inspect('native-maximized');await discard();await page.evaluate(()=>window.review.windowAction('maximize'));await pause(250);report.nativeGeometry={before,restored:await page.evaluate(()=>({width:innerWidth,height:innerHeight,dpr:devicePixelRatio}))};
  const cdp=await page.context().newCDPSession(page);report.scaleBoundary='CDP viewport/scale emulation inside WebView2; system DPI unchanged';
  for(const scale of [1.25,1.5]){
   await cdp.send('Emulation.setDeviceMetricsOverride',{width:935,height:709,deviceScaleFactor:scale,mobile:false});
   for(const [locale,title] of [['zh-CN','简体中文'],['en-US','English'],['ja-JP','日本語']]){
    await page.locator('.language-button').click();await page.locator('.side-popup button').filter({hasText:title}).click();await prepare(16);await inspect('emulated-'+scale+'-'+locale);await discard();
   }
  }
  await cdp.send('Emulation.clearDeviceMetricsOverride');await cdp.detach();
 }
 await page.evaluate(()=>window.__planFooter.mode='refused');await prepare();await page.locator('[data-action=confirm-character-plan]').check();await page.locator('[data-action=commit-character-plan]').click();
 await page.waitForFunction(()=>!document.querySelector('.character-save-plan'));
 check('proven pre-write refusal does not fence the save',await page.locator('.character-page .uncertain-operation').count()===0);
 check('pre-write refusal queries no nonexistent receipt',await page.evaluate(()=>!window.__planFooter.calls.some(c=>c.method==='save.operation')));
 await page.evaluate(()=>window.__planFooter.mode='unknown');await prepare();await page.locator('[data-action=confirm-character-plan]').check();await page.locator('[data-action=commit-character-plan]').click();
 await page.waitForFunction(()=>!document.querySelector('.character-save-plan'));
 check('unknown result still retains receipt recovery',await page.evaluate(()=>window.__planFooter.calls.some(c=>c.method==='save.operation')));
 check('unknown result still visibly fences the save',await page.locator('.character-page .uncertain-operation').count()===1);
 check('discard never commits',await page.evaluate(()=>window.__planFooter.calls.filter(c=>c.method==='save.commit').length===2));
 report.calls=await page.evaluate(()=>window.__planFooter.calls);report.pass=true;
}catch(error){report.error=error.stack||String(error);if(page){report.visibleText=await page.locator('body').innerText().catch(()=>null);await page.screenshot({path:join(out,'failure.png')}).catch(()=>{})}}
finally{if(values.exe)await closeSession(session,child).catch(e=>{report.pass=false;report.closeError=String(e)});else await context?.close();server?.close();await writeFile(join(out,'plan-footer-e2e.json'),JSON.stringify(report,null,2));}
console.log(JSON.stringify({pass:report.pass,checks:report.checks.length,error:report.error,closeError:report.closeError}));if(!report.pass)process.exitCode=1;
