/** Production component and framed-job observer; all native replies are fixtures. */
import assert from 'node:assert/strict';
import {mkdir,mkdtemp,writeFile} from 'node:fs/promises';
import {createServer} from 'node:http';
import {resolve,join} from 'node:path';
import {build} from 'esbuild';
import {chromium} from 'playwright';
const root=resolve(import.meta.dirname,'../..');
const evidence='D:/Nioh3_v080_deliverables/deliverables/codex-compatibility-policy-20261002';
const output=join(evidence,'equipment-browser-'+new Date().toISOString().replace(/[:.]/g,'-'));
await mkdir(output,{recursive:true});await mkdir(join(evidence,'tmp'),{recursive:true});
const profile=await mkdtemp(join(evidence,'tmp','equipment-ui-'));
const built=await build({stdin:{contents:`
import React,{useState} from 'react';import {createRoot} from 'react-dom/client';
import {LiveEquipmentAdd} from './LiveEquipmentAdd';import './style.css';
function Harness(){const [visible,setVisible]=useState(true);return <><button id="mount" onClick={()=>setVisible(!visible)}>Mount</button>{visible&&<LiveEquipmentAdd onAdded={()=>window.__added++}/>}</>}
createRoot(document.getElementById('root')).render(<Harness/>);
`,resolveDir:join(root,'apps/workshop'),loader:'tsx'},bundle:true,write:false,outdir:'out',format:'iife',platform:'browser',jsx:'transform',jsxFactory:'localizedElement',tsconfigRaw:{compilerOptions:{jsx:'react',jsxFactory:'localizedElement'}},inject:[join(root,'apps/workshop/presentation-jsx.ts')],define:{'process.env.NODE_ENV':'"production"'}});
const fixture=()=>{
 window.__calls=[];window.__states={};window.__jobs={};window.__lost=false;window.__added=0;window.__fault=null;window.__statusError=null;
 window.nioh={};window.review={};
 const empty=(id,state)=>({operation_id:id,plan_digest:null,state,process_id:42,slot_index:null,error:null,preview:null});
 window.operations={current:async()=>({job:null,busy:false}),snapshot:async(role,id)=>window.__jobs[id],execute:async command=>{
  // The shared save picker scans for saves; this fixture has none.
  if(command.method==='save.discover')return{saves:[]};
  window.__calls.push(command);const p=command.params,id=p.operation_id;
  if(command.method==='runtime.equipment_add_prepare'){
   if(!localStorage.getItem('nioh3-live-equipment-add'))throw Error('UUID_NOT_PERSISTED');
   if(window.__fault){const unknown=window.__fault==='unknown';window.__states[id]=empty(id,unknown?'uncertain':'rejected_before_dispatch');window.__fault=null;throw Error(unknown?'RESPONSE_LOST: fixture status unresolved':"EQUIPMENT_BACKUP_SOURCE_REQUIRED: More than one save was found. Select the current character's SAVEDATA.BIN backup path, then prepare again; no native preview has run.");}
   window.__states[id]={operation_id:id,plan_digest:'a'.repeat(64),state:'prepared',process_id:42,slot_index:3,error:null,
   preview:{slot_index:3,item_id:p.item_id,appearance_id:p.item_id,quantity:1,level:p.level,level_before_forge:p.level,plus:p.plus,familiarity:0,inventory_key:0,seed:p.seed,rarity:p.rarity,effects:[{effect_id:44634,value:150,star:false}]}};
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
 browser=await chromium.launchPersistentContext(profile,{headless:true,viewport:report.viewport});
 for(const locale of ['zh-CN','en-US','ja-JP']){
  const page=await browser.newPage();await page.addInitScript(locale=>{localStorage.clear();localStorage.setItem('nioh3-ui-locale',locale)},locale);
  await page.goto('http://127.0.0.1:'+server.address().port);
  const backup=page.locator('[data-field=seed]'),prepare=page.locator('[data-action=prepare-equipment]'),next=page.locator('[data-action=new-equipment]');
  const settled=state=>page.waitForFunction(state=>document.querySelector('[data-state="'+state+'"]')&&!document.querySelector('[data-action=status-equipment]')?.disabled,state);
  const resetCalls=()=>page.evaluate(()=>{window.__calls=[];});
  const calls=()=>page.evaluate(()=>window.__calls.map(c=>({method:c.method,id:c.params.operation_id,path:c.params.save_path})));
  check(locale+' heading localized',(await page.locator('.live-equipment-add>h2').innerText())===({'zh-CN':'实时添加装备','en-US':'Add equipment live','ja-JP':'装備をリアルタイムで追加'})[locale]);
  await page.locator('[data-action=pick-equipment]').first().click();
  await page.locator('[data-field=seed]').fill('65536');check(locale+' numeric validation retained',await prepare.isDisabled());
  await page.locator('[data-field=seed]').fill('123');await prepare.click();await settled('prepared');
  check(locale+' no selected save omits save_path',await page.evaluate(()=>!Object.hasOwn(window.__calls[0].params,'save_path')));
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
  check(locale+' remount only inspects retained UUID',await page.evaluate(id=>window.__calls.at(-1).method.endsWith('_status')&&window.__calls.at(-1).params.operation_id===id,unknownId));
  await page.evaluate(()=>{window.__recoverState='cancelled';});await page.locator('[data-action=recover-equipment]').click();await settled('cancelled');await next.click();
  await page.locator('[data-action=pick-equipment]').first().click();
  await resetCalls();await page.evaluate(()=>{window.__fault='rejected';window.__statusError='STATUS_READ_FAILED: fixture receipt unreadable';});await prepare.click();await settled('uncertain');
  const failedStatus=await calls();
  check(locale+' failed receipt query remains fenced',failedStatus.length===2&&failedStatus[1].method.endsWith('_status')&&await next.count()===0&&await backup.isDisabled());
  check(locale+' original and status error both visible',(await page.locator('.live-equipment-form .notice code').allTextContents()).join(' ').includes('EQUIPMENT_BACKUP_SOURCE_REQUIRED')&&(await page.locator('.live-equipment-form').innerText()).includes('STATUS_READ_FAILED'));
  await page.evaluate(()=>{window.__statusError=null;});await page.locator('[data-action=status-equipment]').click();await settled('rejected_before_dispatch');
  check(locale+' manual status retry safely resolves prewrite failure',await next.isEnabled());
  const geometry=await page.locator('.live-equipment-add').evaluate(el=>({scroll:el.scrollWidth,width:el.clientWidth,dpr:devicePixelRatio}));
  check(locale+' no horizontal overflow',geometry.scroll<=geometry.width+2);
  report.locales.push({locale,geometry,rejected,lost,unknown,failedStatus});await page.close();
 }
 report.pass=true;
}catch(error){report.error=error.stack||String(error);}
finally{await browser?.close();await new Promise(r=>server.close(r));await writeFile(join(output,'equipment-ui-e2e.json'),JSON.stringify(report,null,2));}
console.log(JSON.stringify({pass:report.pass,checks:report.checks.length,output,profile,error:report.error}));if(!report.pass)process.exitCode=1;
