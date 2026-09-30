/** Preimplementation UI E2E: confirmation, durable ownership, lost response,
 * restart/status/recovery, cancellation, input validation and locale geometry.
 * Native insertion is mocked here; the separate helper E2E covers OS dispatch.
 */
import assert from 'node:assert/strict';
import {mkdir,writeFile} from 'node:fs/promises';
import {createServer} from 'node:http';
import {resolve,join} from 'node:path';
import {build} from 'esbuild';
import {chromium} from 'playwright';
const root=resolve(import.meta.dirname,'../..');
const output='D:/Nioh3_v080_deliverables/deliverables/codex-v083-missing-features-20260930/ui';
await mkdir(output,{recursive:true});
const built=await build({stdin:{contents:`
import React,{useState} from 'react';import {createRoot} from 'react-dom/client';
import {LiveEquipmentAdd} from './LiveEquipmentAdd';import './style.css';
function Harness(){const [visible,setVisible]=useState(true);return <><button id="mount" onClick={()=>setVisible(!visible)}>Mount</button>{visible&&<LiveEquipmentAdd/>}</>}
createRoot(document.getElementById('root')).render(<Harness/>);
`,resolveDir:join(root,'apps/workshop'),loader:'tsx'},bundle:true,write:false,outdir:'out',format:'iife',platform:'browser',jsx:'transform',jsxFactory:'localizedElement',tsconfigRaw:{compilerOptions:{jsx:'react',jsxFactory:'localizedElement'}},inject:[join(root,'apps/workshop/presentation-jsx.ts')],define:{'process.env.NODE_ENV':'"production"'}});
const js=built.outputFiles.find(f=>f.path.endsWith('.js')).text,css=built.outputFiles.find(f=>f.path.endsWith('.css')).text;
const html=`<html><head><meta charset="utf-8"><style>${css}</style></head><body><div id="root"></div><script>
window.__calls=[];window.__states={};window.__jobs={};window.__lost=false;
window.nioh={};window.review={};
window.operations={current:async()=>({job:null,busy:false}),snapshot:async(role,id)=>window.__jobs[id],execute:async command=>{
window.__calls.push(command);const p=command.params,id=p.operation_id;
if(command.method==='runtime.equipment_add_prepare'){
if(!localStorage.getItem('nioh3-live-equipment-add'))throw Error('UUID_NOT_PERSISTED');
window.__states[id]={operation_id:id,plan_digest:'a'.repeat(64),state:'prepared',process_id:42,slot_index:3,error:null,
preview:{slot_index:3,item_id:p.item_id,appearance_id:p.item_id,quantity:1,level:p.level,level_before_forge:p.level,plus:p.plus,familiarity:0,inventory_key:0,seed:p.seed,rarity:p.rarity,effects:[{effect_id:44634,value:150,star:false}]}};
}else if(command.method==='runtime.equipment_add_execute'){
if(p.confirmed!==true||p.plan_digest!=='a'.repeat(64))throw Error('NOT_CONFIRMED');
window.__states[id].state='verified';if(window.__lost){window.__lost=false;throw Error('RESPONSE_LOST')}
}else if(command.method==='runtime.equipment_add_cancel'){window.__states[id].state='cancelled'}
else if(!['runtime.equipment_add_status','runtime.equipment_add_recover'].includes(command.method))throw Error('UNEXPECTED_METHOD');
const job={job_id:id,kind:command.method,state:'completed',cancellable:false,sequence:2,result:{equipment_add:structuredClone(window.__states[id])},error:null};window.__jobs[id]=job;return {...job,state:'running',sequence:1,result:null};}};
</script><script>${js.replaceAll('</script','<\\/script')}</script></body></html>`;
const server=createServer((_,res)=>res.end(html));await new Promise(r=>server.listen(0,'127.0.0.1',r));
const browser=await chromium.launchPersistentContext('D:/Nioh3_v080_deliverables/tmp/equipment-ui-'+process.pid,{headless:true,viewport:{width:1280,height:900}});
const checks=[];
try{
for(const locale of ['zh-CN','en-US','ja-JP']){
 const page=await browser.newPage();await page.addInitScript(locale=>{localStorage.clear();localStorage.setItem('nioh3-ui-locale',locale)},locale);
 await page.goto('http://127.0.0.1:'+server.address().port);
 assert.equal(await page.locator('.live-equipment-add>h2').innerText(),{'zh-CN':'实时添加装备','en-US':'Add equipment live','ja-JP':'装備をリアルタイムで追加'}[locale]);
 await page.locator('[data-action=pick-equipment]').first().click();
 await page.locator('[data-field=seed]').fill('65536');assert.equal(await page.locator('[data-action=prepare-equipment]').isDisabled(),true);
 await page.locator('[data-field=seed]').fill('123');await page.locator('[data-action=prepare-equipment]').click();
 await page.locator('.live-equipment-preview').waitFor();assert.equal(await page.evaluate(()=>window.__calls.some(c=>c.method.endsWith('_execute'))),false);
 assert.equal(await page.locator('[data-action=execute-equipment]').isDisabled(),true);
 await page.screenshot({path:join(output,'equipment-'+locale+'.png'),fullPage:true});
 await page.locator('[data-action=cancel-equipment]').click();await page.waitForFunction(()=>document.querySelector('[data-state=cancelled]'));
 await page.locator('[data-action=new-equipment]').click();await page.locator('[data-action=prepare-equipment]').click();await page.locator('.live-equipment-preview').waitFor();
 await page.locator('[data-action=confirm-equipment]').check();await page.evaluate(()=>window.__lost=true);
 await page.locator('[data-action=execute-equipment]').click();await page.waitForFunction(()=>document.querySelector('[data-state=uncertain]'));
 assert.equal(await page.locator('[data-action=execute-equipment]').count(),0);
 await page.locator('#mount').click();await page.locator('#mount').click();await page.waitForFunction(()=>document.querySelector('[data-state=verified]'));
 await page.locator('[data-action=recover-equipment]').click();
 assert.equal(await page.evaluate(()=>window.__calls.filter(c=>c.method.endsWith('_execute')).length),1);
 assert.equal(await page.evaluate(()=>window.__calls.filter(c=>c.method.endsWith('_prepare')).length),2);
 const geometry=await page.locator('.live-equipment-add').evaluate(el=>({scroll:el.scrollWidth,width:el.clientWidth,dpr:devicePixelRatio}));assert.ok(geometry.scroll<=geometry.width+2);
 checks.push({locale,confirmationRequired:true,persistedBeforeRequest:true,cancel:true,lostResponseNoRetry:true,restartStatus:true,recoveryReadOnly:true,geometry});await page.close();
}
await writeFile(join(output,'equipment-ui-e2e.json'),JSON.stringify({pass:true,boundary:'production component, mocked bridge; no game or save',checks},null,2));console.log('Live equipment UI E2E passed in three locales');
}finally{await browser.close();server.close();}
