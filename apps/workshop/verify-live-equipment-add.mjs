/** Production component and framed-job observer; all native replies are fixtures. */
import assert from 'node:assert/strict';
import {mkdir,mkdtemp,writeFile} from 'node:fs/promises';
import {createServer} from 'node:http';
import {resolve,join} from 'node:path';
import {build} from 'esbuild';
import {chromium} from 'playwright';
const root=resolve(import.meta.dirname,'../..');
const evidence=process.env.NIOH3_UI_EVIDENCE_ROOT??'D:/Nioh3_v080_deliverables/deliverables/live-equipment-effect-search';
const output=join(evidence,'equipment-browser-'+new Date().toISOString().replace(/[:.]/g,'-'));
await mkdir(output,{recursive:true});await mkdir(join(evidence,'tmp'),{recursive:true});
const profile=await mkdtemp(join(evidence,'tmp','equipment-ui-'));
const built=await build({stdin:{contents:`
import React,{useState} from 'react';import {createRoot} from 'react-dom/client';
import {LiveEquipmentAdd} from './LiveEquipmentAdd';import {saveSession} from './save-workspace';import './style.css';import './arc.css';
const reference=id=>({save_id:id,account_id:'123',save_slot:0,path:'D:/fixture/'+id+'/SAVEDATA.BIN'});
function Harness(){const [visible,setVisible]=useState(true);return <><button id="mount" onClick={()=>setVisible(!visible)}>Mount</button><button id="select-save" onClick={()=>saveSession.select(reference('fixture'))}>Select fixture</button><button id="switch-save" onClick={()=>saveSession.select(reference('other'))}>Switch fixture</button>{visible&&<LiveEquipmentAdd onAdded={()=>window.__added++}/>}</>}
createRoot(document.getElementById('root')).render(<Harness/>);
`,resolveDir:join(root,'apps/workshop'),loader:'tsx'},bundle:true,write:false,outdir:'out',format:'iife',platform:'browser',jsx:'transform',jsxFactory:'localizedElement',tsconfigRaw:{compilerOptions:{jsx:'react',jsxFactory:'localizedElement'}},inject:[join(root,'apps/workshop/presentation-jsx.ts')],loader:{'.woff2':'dataurl'},define:{'process.env.NODE_ENV':'"production"'}});
const fixture=()=>{
 window.__calls=[];window.__states=JSON.parse(sessionStorage.getItem('fixture-receipts')??'{}');window.__jobs={};window.__lost=false;window.__added=0;window.__fault=null;window.__statusError=null;
 window.nioh={};window.review={};
 const empty=(id,state)=>({operation_id:id,plan_digest:null,state,process_id:42,slot_index:null,error:null,preview:null});
 window.operations={current:async()=>({job:null,busy:false}),snapshot:async(role,id)=>window.__jobs[id],execute:async command=>{
  // The shared save picker scans for saves; this fixture has none.
  if(command.method==='save.discover')return{saves:[]};
  window.__calls.push(command);const p=command.params,id=p.operation_id;
  if(command.method==='save.inventory')return{save_id:p.save_id,account_id:'123',snapshot_id:'b'.repeat(64),source_sha256:'c'.repeat(64),empty_slots:400,entries:[]};
  if(command.method==='save.operations')return{operations:[]};
  if(command.method==='runtime.equipment_rules')return{item_id:p.item_id,known:true,rarity:p.rarity,level:p.level,random_pool:[{effect_id:44634,star:false,min:10,max:20,group:1,masks:[0,0]},{effect_id:44635,star:true,min:20,max:30,group:2,masks:[0,0]}]};
  if(command.method==='save.character'){
   if(window.__readError)throw Error('SAVE_READ_FAILED');
   return{save_id:p.save_id,source_sha256:'c'.repeat(64),currencies:{},equipment_slots:2500,equipment:[],generation:window.__noGeneration?null:{difficulty:3,difficulties:[{difficulty:3,progress:[1,2,3,4]}]}};
  }
  if(command.method==='runtime.equipment_seeds'){
   if(window.__holdSearch)await new Promise(resolve=>{window.__releaseSearch=resolve;});
   if(window.__searchError)throw Error('SEED_SEARCH_FAILED');
   return{item_id:p.item_id,rarity:p.rarity,level:p.level,difficulty:p.difficulty,seeds:65536,empty:0,matches:window.__noMatches?0:2,outcomes:window.__noMatches?[]:[123,456].map(seed=>({seed,effects:[{effect_id:44634,value:150,star:false,role:'random',roll:100}]}))};
  }
  if(command.method==='runtime.equipment_add_prepare'){
   if(!localStorage.getItem('nioh3-live-equipment-add'))throw Error('UUID_NOT_PERSISTED');
   if(window.__fault){const unknown=window.__fault==='unknown';window.__states[id]=empty(id,unknown?'uncertain':'rejected_before_dispatch');window.__fault=null;throw Error(unknown?'RESPONSE_LOST: fixture status unresolved':"EQUIPMENT_BACKUP_SOURCE_REQUIRED: More than one save was found. Select the current character's SAVEDATA.BIN backup path, then prepare again; no native preview has run.");}
   window.__states[id]={operation_id:id,plan_digest:'a'.repeat(64),state:'prepared',process_id:42,slot_index:3,error:null,
   preview:{slot_index:3,item_id:p.item_id,appearance_id:p.item_id,quantity:1,level:p.level,level_before_forge:p.level,plus:p.plus,familiarity:0,inventory_key:0,seed:p.seed,rarity:p.rarity,effects:(window.__previewEffects??[44634]).map(effect_id=>({effect_id,value:150,star:false}))}};
  }else if(command.method==='runtime.equipment_add_execute'){
   if(p.confirmed!==true||p.plan_digest!=='a'.repeat(64))throw Error('NOT_CONFIRMED');
   window.__states[id].state='verified';if(window.__lost){window.__lost=false;throw Error('RESPONSE_LOST');}
  }else if(command.method==='runtime.equipment_add_cancel')window.__states[id].state='cancelled';
  else if(command.method==='runtime.equipment_add_status'){
   if(window.__statusError)throw Error(window.__statusError);
   window.__states[id]??=empty(id,'rejected_before_dispatch');
  }else if(command.method==='runtime.equipment_add_recover'){
   if(window.__recoverState)window.__states[id].state=window.__recoverState;
  }else throw Error('UNEXPECTED_METHOD');
  sessionStorage.setItem('fixture-receipts',JSON.stringify(window.__states));
  const job={job_id:id,kind:command.method,state:'completed',cancellable:false,sequence:2,result:{equipment_add:structuredClone(window.__states[id])},error:null};window.__jobs[id]=job;return {...job,state:'running',sequence:1,result:null};
 }};
};
const js=built.outputFiles.find(f=>f.path.endsWith('.js')).text,css=built.outputFiles.find(f=>f.path.endsWith('.css')).text;
const html='<html><head><meta charset="utf-8"><style>'+css+'</style></head><body><div id="root"></div><script>('+fixture.toString()+')();</script><script>'+js.replaceAll('</script','<\\/script')+'</script></body></html>';
const server=createServer((_,res)=>res.end(html));await new Promise(r=>server.listen(0,'127.0.0.1',r));
const report={pass:false,boundary:'Production component and job observer; mocked bridge only; no native window, game or save access',viewport:{width:1020,height:640,deviceScaleFactor:1},checks:[],screenshots:[],locales:[],profile};
const check=(name,condition)=>{report.checks.push({name,pass:!!condition});assert(condition,name);};
let browser;
try{
 browser=await chromium.launchPersistentContext(profile,{headless:true,viewport:report.viewport,channel:process.env.NIOH3_UI_BROWSER_CHANNEL??'msedge'});
 for(const locale of ['zh-CN','en-US','ja-JP']){
  const page=await browser.newPage();await page.addInitScript(locale=>{if(!sessionStorage.getItem('fixture-initialized')){localStorage.clear();sessionStorage.setItem('fixture-initialized','1');}localStorage.setItem('nioh3-ui-locale',locale)},locale);
  await page.goto('http://127.0.0.1:'+server.address().port);
  const backup=page.locator('[data-field=seed]'),prepare=page.locator('[data-action=prepare-equipment]'),next=page.locator('[data-action=new-equipment]');
  const settled=state=>page.waitForFunction(state=>document.querySelector('[data-state="'+state+'"]')&&!document.querySelector('[data-action=status-equipment]')?.disabled,state);
  const resetCalls=()=>page.evaluate(()=>{window.__calls=[];});
  const calls=()=>page.evaluate(()=>window.__calls.filter(c=>c.method.startsWith('runtime.equipment_add_')).map(c=>({method:c.method,id:c.params.operation_id,path:c.params.save_path})));
  check(locale+' heading localized',(await page.locator('.live-equipment-heading>h2').innerText())===({'zh-CN':'实时添加装备','en-US':'Add equipment live','ja-JP':'装備をリアルタイムで追加'})[locale]);
  await page.locator('[data-action=pick-equipment]').first().click();
  await page.locator('[data-field=seed]').fill('65536');check(locale+' numeric validation retained',await prepare.isDisabled());
  await page.locator('[data-field=seed]').fill('123');await prepare.click();await settled('prepared');
  check(locale+' no selected save omits save_path',await page.evaluate(()=>!Object.hasOwn(window.__calls.find(c=>c.method==='runtime.equipment_add_prepare').params,'save_path')));
  check(locale+' backup input locked with prepared operation',await backup.isDisabled());
  check(locale+' preview does not add without confirmation',await page.locator('[data-action=execute-equipment]').isDisabled()&&!await page.evaluate(()=>window.__calls.some(c=>c.method.endsWith('_execute'))));
  await page.locator('[data-action=cancel-equipment]').click();await settled('cancelled');await next.click();
  await resetCalls();await page.evaluate(()=>{window.__fault='rejected';});await prepare.click();await settled('rejected_before_dispatch');
  const rejected=await calls();
  check(locale+' rejected prepare automatically reads same UUID once',rejected.length===2&&rejected[0].method.endsWith('_prepare')&&rejected[1].method.endsWith('_status')&&rejected[0].id===rejected[1].id);
  check(locale+' prewrite refusal allows next preparation',await next.isEnabled()&&await page.locator('[data-action=execute-equipment]').count()===0);
  check(locale+' source selection refusal gives localized recovery',(await page.locator('.live-equipment-form .notice p').first().innerText()).includes(({'zh-CN':'目标存档','en-US':'Target save','ja-JP':'対象セーブ'})[locale]));
  check(locale+' exact backend error retained in technical details',(await page.locator('.live-equipment-form .notice code').allTextContents()).join(' ').includes('EQUIPMENT_BACKUP_SOURCE_REQUIRED'));
  check(locale+' requested seed retained',await page.locator('[data-field=seed]').inputValue()==='123');
  await prepare.scrollIntoViewIfNeeded();const shot=join(output,'equipment-'+locale+'-backup-recovery.png');await page.screenshot({path:shot,fullPage:true});report.screenshots.push(shot);
  await next.click();check(locale+' next preparation retains editable input',await backup.isEnabled()&&await backup.inputValue()==='123');
  await resetCalls();await prepare.click();await settled('prepared');await page.locator('[data-action=confirm-equipment]').check();await page.evaluate(()=>{window.__lost=true;});await page.locator('[data-action=execute-equipment]').click();await settled('verified');
  const lost=await calls();
  check(locale+' lost execute reply reads receipt without replay',lost.map(c=>c.method).join('|')==='runtime.equipment_add_prepare|runtime.equipment_add_execute|runtime.equipment_add_status'&&lost.every(c=>c.id===lost[0].id));
  check(locale+' verified receipt refreshes once',await page.evaluate(()=>window.__added===1));
  await next.click();await resetCalls();await page.evaluate(()=>{window.__fault='unknown';});await prepare.click();await settled('uncertain');
  const unknown=await calls(),unknownId=unknown[0].id;
  check(locale+' uncertain result keeps operation fenced',await next.count()===0&&await backup.isDisabled()&&await page.evaluate(id=>JSON.parse(localStorage.getItem('nioh3-live-equipment-add')).operation_id===id,unknownId));
  check(locale+' uncertain result never retries mutation',unknown.length===2&&unknown[1].method.endsWith('_status'));
  await page.locator('#mount').click();await page.locator('#mount').click();await settled('uncertain');
  check(locale+' remount only inspects retained UUID',await page.evaluate(id=>window.__calls.filter(c=>c.params.operation_id).at(-1).method.endsWith('_status')&&window.__calls.filter(c=>c.params.operation_id).at(-1).params.operation_id===id,unknownId));
  await page.evaluate(()=>{window.__recoverState='cancelled';});await page.locator('[data-action=recover-equipment]').click();await settled('cancelled');await next.click();
  await page.locator('[data-action=pick-equipment]').first().click();
  await resetCalls();await page.evaluate(()=>{window.__fault='rejected';window.__statusError='STATUS_READ_FAILED: fixture receipt unreadable';});await prepare.click();await settled('uncertain');
  const failedStatus=await calls();
  check(locale+' failed receipt query remains fenced',failedStatus.length===2&&failedStatus[1].method.endsWith('_status')&&await next.count()===0&&await backup.isDisabled());
  check(locale+' original and status error both visible',(await page.locator('.live-equipment-form .notice code').allTextContents()).join(' ').includes('EQUIPMENT_BACKUP_SOURCE_REQUIRED')&&(await page.locator('.live-equipment-form').innerText()).includes('STATUS_READ_FAILED'));
  await page.evaluate(()=>{window.__statusError=null;});await page.locator('[data-action=status-equipment]').click();await settled('rejected_before_dispatch');
  check(locale+' manual status retry safely resolves prewrite failure',await next.isEnabled());
  await next.click();await page.locator('[data-action=pick-equipment]').first().click();
  const find=page.locator('[data-action=find-equipment-seeds]');
  check(locale+' effect search requires target save',await find.isDisabled());
  await page.locator('#select-save').click();await page.waitForFunction(()=>!document.querySelector('[data-action=find-equipment-seeds]').disabled);
  const picker=page.locator('.live-equipment-seed-search .effect-picker input');
  await picker.focus();await page.locator('.effect-picker-list button').nth(1).click();
  check(locale+' wanted effect chip selected',await page.locator('.seed-chip').count()===1);
  await resetCalls();await find.click();await page.locator('.seed-outcomes [role=option]').first().waitFor();
  check(locale+' selected save progress drives search',await page.evaluate(()=>{
   const read=window.__calls.find(c=>c.method==='save.character'),search=window.__calls.find(c=>c.method==='runtime.equipment_seeds');
   return read.params.save_id==='fixture'&&search.params.difficulty===3&&search.params.progress.join(',')==='1,2,3,4'&&search.params.want[0][0]===44634;
  }));
  check(locale+' seed search performs no native preview or insertion',(await calls()).length===0);
  check(locale+' first result selects its seed',await backup.inputValue()==='123');
  await page.locator('.seed-outcomes [role=option]').nth(1).click();
  check(locale+' second result updates seed',await backup.inputValue()==='456');
  await backup.fill('789');check(locale+' manual seed clears selected result',await page.locator('.seed-outcomes [aria-selected=true]').count()===0);
  await page.locator('.seed-outcomes [role=option]').nth(1).click();
  await prepare.click();await settled('prepared');
  check(locale+' chosen seed and backup path enter native preview',await page.evaluate(()=>{const c=window.__calls.find(c=>c.method==='runtime.equipment_add_prepare');return c.params.seed===456&&c.params.save_path==='D:/fixture/fixture/SAVEDATA.BIN';}));
  check(locale+' wanted effects persisted with UUID',await page.evaluate(()=>JSON.parse(localStorage.getItem('nioh3-live-equipment-add')).wanted[0][0]===44634));
  check(locale+' prepared plan locks effect controls',await picker.isDisabled()&&await find.isDisabled());
  check(locale+' matching native preview allows confirmation',await page.locator('[data-action=confirm-equipment]').isEnabled());
  await page.locator('[data-action=confirm-equipment]').check();await page.locator('[data-action=execute-equipment]').click();await settled('verified');await next.click();
  // A live/save-context difference must never silently become an accepted addition.
  await page.evaluate(()=>{window.__previewEffects=[44635];});await prepare.click();await settled('prepared');
  check(locale+' mismatching preview blocks confirmation and insertion',await page.locator('[data-action=confirm-equipment]').isDisabled()&&await page.locator('[data-action=execute-equipment]').isDisabled());
  const mismatchShot=join(output,'equipment-'+locale+'-mismatch.png');await page.screenshot({path:mismatchShot,fullPage:true});report.screenshots.push(mismatchShot);
  await resetCalls();await page.locator('#mount').click();await page.locator('#mount').click();await settled('prepared');
  check(locale+' remount preserves mismatch guard without replay',await page.locator('[data-action=confirm-equipment]').isDisabled()&&(await calls()).every(c=>c.method==='runtime.equipment_add_status'));
  await page.reload();await settled('prepared');
  check(locale+' restart restores wanted constraints from storage',await page.locator('[data-action=confirm-equipment]').isDisabled());
  await page.locator('[data-action=cancel-equipment]').click();await settled('cancelled');await next.click();
  await page.locator('[data-action=pick-equipment]').first().click();await page.locator('#select-save').click();await page.waitForFunction(()=>!document.querySelector('[data-action=find-equipment-seeds]').disabled);
  await find.click();await page.locator('.seed-outcomes [role=option]').first().waitFor();
  await page.locator('[data-field=plus]').fill('21');
  check(locale+' parameter edit invalidates prior search',await page.locator('.seed-outcomes').count()===0);
  await page.evaluate(()=>{window.__noMatches=true;});await find.click();await page.locator('.seed-summary').waitFor();
  check(locale+' zero-match result has no selectable seeds',await page.locator('.seed-outcomes').count()===0&&(await page.locator('.seed-summary').innerText()).includes('0 / 65536'));
  await page.evaluate(()=>{window.__noMatches=false;window.__readError=true;});await resetCalls();await find.click();
  await page.waitForFunction(()=>!document.querySelector('[data-action=find-equipment-seeds]').disabled);
  check(locale+' save read failure does not start seed search',await page.evaluate(()=>!window.__calls.some(c=>c.method==='runtime.equipment_seeds'))&&await page.locator('.live-equipment-seed-search .notice').count()>0);
  await page.evaluate(()=>{window.__readError=false;window.__noGeneration=true;});await resetCalls();await find.click();
  await page.waitForFunction(()=>!document.querySelector('[data-action=find-equipment-seeds]').disabled);
  check(locale+' missing progress refuses search without guessing',await page.evaluate(()=>!window.__calls.some(c=>c.method==='runtime.equipment_seeds')));
  await page.evaluate(()=>{window.__noGeneration=false;window.__searchError=true;});await find.click();
  await page.waitForFunction(()=>!document.querySelector('[data-action=find-equipment-seeds]').disabled);
  check(locale+' search error releases inputs for retry',await backup.isEnabled()&&await page.locator('.live-equipment-seed-search .notice').count()>0);
  await page.evaluate(()=>{window.__searchError=false;window.__holdSearch=true;});await find.click();
  await page.waitForFunction(()=>!!window.__releaseSearch);
  check(locale+' in-flight search locks preview and inputs',await prepare.isDisabled()&&await backup.isDisabled());
  await page.locator('#switch-save').click();await page.waitForFunction(()=>document.querySelector('.save-select select')?.value==='other');
  await page.evaluate(()=>{window.__releaseSearch();window.__holdSearch=false;});await page.waitForFunction(()=>!document.querySelector('[data-action=find-equipment-seeds]').disabled);
  check(locale+' changed save discards late search result',await page.locator('.seed-outcomes').count()===0);
  await find.click();await page.locator('.seed-outcomes [role=option]').first().waitFor();
  const searchShot=join(output,'equipment-'+locale+'-search.png');await page.screenshot({path:searchShot,fullPage:true});report.screenshots.push(searchShot);
  const geometry=await page.locator('.live-equipment-add').evaluate(el=>({scroll:el.scrollWidth,width:el.clientWidth,dpr:devicePixelRatio}));
  check(locale+' no horizontal overflow',geometry.scroll<=geometry.width+2);
  report.locales.push({locale,geometry,rejected,lost,unknown,failedStatus});await page.close();
 }
 report.pass=true;
}catch(error){report.error=error.stack||String(error);}
finally{await browser?.close();await new Promise(r=>server.close(r));await writeFile(join(output,'equipment-ui-e2e.json'),JSON.stringify(report,null,2));}
console.log(JSON.stringify({pass:report.pass,checks:report.checks.length,output,profile,error:report.error}));if(!report.pass)process.exitCode=1;
