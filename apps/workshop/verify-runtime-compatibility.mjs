/** Production component, real Chromium interactions, scripted native replies. */
import assert from 'node:assert/strict';
import {build} from 'esbuild';
import {chromium} from 'playwright';
import {createServer} from 'node:http';
import {mkdir,mkdtemp,writeFile,rm} from 'node:fs/promises';
import {join,resolve} from 'node:path';
const out=resolve('D:/Nioh3_v080_deliverables/deliverables/codex-v083-compatibility-20261001/browser');
await mkdir(out,{recursive:true});
const root=await mkdtemp('D:/Nioh3_v080_deliverables/tmp/compatibility-ui-');
const bundle=await build({stdin:{contents:'import React from "react";import{createRoot}from"react-dom/client";import{RuntimeCompatibility}from"./apps/workshop/RuntimeCompatibility";import{setUiLocale}from"./apps/workshop/presentation";import"./apps/workshop/style.css";window.setLocale=setUiLocale;createRoot(document.getElementById("root")).render(<RuntimeCompatibility/>);',resolveDir:process.cwd(),loader:'tsx'},bundle:true,write:false,outdir:'out',format:'iife',platform:'browser',jsx:'transform',jsxFactory:'localizedElement',tsconfigRaw:{compilerOptions:{jsx:'react',jsxFactory:'localizedElement'}},inject:[resolve('apps/workshop/presentation-jsx.ts')],define:{'process.env.NODE_ENV':'"production"'}});
const fixture=()=>{
 window.nioh={};window.review={};window.testCase={verified:true,calls:[]};
 window.operations={execute:async command=>{
  const f=window.testCase,p=command.params;f.calls.push(command);
  if(command.method!=='runtime.compatibility')throw Error('WRITE_FORBIDDEN');
  if(p.action==='accept'&&(!p.confirmed||!p.backup_confirmed))throw Error('ACK_REQUIRED');
  return{compatibility:{present:true,warning:true,accepted:p.action==='accept',game_version:'2.0.1.0',executable:'D:/fixture/Nioh3.exe',features:{live_character:true,live_equipment_add:false},backup:p.action==='inspect'?null:{attempted:true,verified:f.verified,paths:f.verified?['D:/fixture-backup/00-SAVEDATA.BIN']:[],error:f.verified?null:'No automatic save found'}}};
 }};
};
const html='<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><style>'+bundle.outputFiles.find(f=>f.path.endsWith('.css')).text+'</style></head><body><div id="root"></div><script>('+fixture.toString()+')();</script><script>'+bundle.outputFiles.find(f=>f.path.endsWith('.js')).text.replaceAll('</script','<\\/script')+'</script></body></html>';
const server=createServer((_,response)=>response.end(html));await new Promise(r=>server.listen(0,'127.0.0.1',r));
let context;
const report={pass:false,boundary:'production component/browser; native replies scripted; no game/save writes',checks:[],screenshots:[]};
const check=(name,condition)=>{report.checks.push({name,pass:!!condition});assert(condition,name)};
try{
 context=await chromium.launchPersistentContext(join(root,'profile'),{headless:true,viewport:{width:1440,height:900}});
 const page=await context.newPage();await page.goto('http://127.0.0.1:'+server.address().port);
 await page.locator('.runtime-compatibility-banner').waitFor();
 for(const locale of ['zh-CN','en-US','ja-JP']){
  await page.evaluate(locale=>window.setLocale(locale),locale);
  for(const verified of [true,false]){
   await page.evaluate(verified=>{window.testCase.verified=verified;window.dispatchEvent(new Event('nioh3:compatibility-required'));},verified);
   const dialog=page.locator('.compatibility-dialog');await dialog.waitFor();
   check(locale+' heading translated '+verified,(await dialog.locator('h2').innerText())===({'zh-CN':'游戏版本兼容提示','en-US':'Game compatibility notice','ja-JP':'ゲーム版の互換性について'})[locale]);
   await page.waitForFunction(()=>window.testCase.calls.at(-1)?.params.action==='prepare');
   check(locale+' backup outcome visible '+verified,(await dialog.innerText()).includes(verified?'00-SAVEDATA.BIN':'No automatic save found'));
   const accept=page.locator('[data-action=compatibility-accept]');
   check(locale+' consent starts disabled '+verified,await accept.isDisabled());
   await page.locator('[data-action=compatibility-risk]').check();
   check(locale+' risk alone cannot continue '+verified,await accept.isDisabled());
   await page.locator('[data-action=compatibility-backup]').check();
   check(locale+' both confirmations enable continuation '+verified,await accept.isEnabled());
   await page.setViewportSize({width:1020,height:640});await accept.scrollIntoViewIfNeeded();
   check(locale+' footer reachable '+verified,await accept.evaluate(e=>{const r=e.getBoundingClientRect();return r.top>=0&&r.bottom<=innerHeight}));
   await page.screenshot({path:join(out,locale+'-'+verified+'.png')});report.screenshots.push(locale+'-'+verified+'.png');
   await accept.click();await dialog.waitFor({state:'detached'});
   check(locale+' explicit confirmations sent '+verified,await page.evaluate(()=>{const p=window.testCase.calls.at(-1).params;return p.action==='accept'&&p.confirmed&&p.backup_confirmed}));
   await page.setViewportSize({width:1440,height:900});
  }
 }
 report.calls=await page.evaluate(()=>window.testCase.calls);report.pass=true;
}catch(error){report.error=error.stack||String(error);}
finally{await context?.close();await new Promise(r=>server.close(r));await writeFile(join(out,'compatibility-ui.json'),JSON.stringify(report,null,2));await rm(root,{recursive:true,force:true});}
console.log(JSON.stringify({pass:report.pass,checks:report.checks.length,error:report.error}));if(!report.pass)process.exitCode=1;
