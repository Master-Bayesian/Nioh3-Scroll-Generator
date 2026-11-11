/** Production component, three locales, scripted bridge only; no game/save access. */
import {build} from 'esbuild';
import {chromium} from 'playwright';
import {createServer} from 'node:http';
import {mkdir,writeFile} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import assert from 'node:assert/strict';
import {withDarkTheme,arcTone} from './dark-theme.mjs';
const output=resolve(process.env.NIOH3_UI_OUTPUT||'D:/Nioh3_v080_deliverables/deliverables/player-editor-20261015/resources/ui');
assert.match(output,/^[Dd]:[\\/]/);
await mkdir(output,{recursive:true});
const root=process.cwd();
const built=await build({entryPoints:['apps/workshop/verify-prestige-fixture.tsx'],bundle:true,
 write:false,outdir:output,platform:'browser',format:'iife',jsx:'transform',jsxFactory:'localizedElement',
 tsconfigRaw:{compilerOptions:{jsx:'react',jsxFactory:'localizedElement'}},
 inject:[join(root,'apps/workshop/presentation-jsx.ts')],external:['game-reference.png'],
 loader:{'.woff2':'dataurl'},define:{'process.env.NODE_ENV':'"production"'}});
const script=built.outputFiles.find(f=>f.path.endsWith('.js')).text;
const css=withDarkTheme(built.outputFiles.find(f=>f.path.endsWith('.css')).text,arcTone);
await writeFile(join(output,'stylesheet.css'),css);
const bridge=`
const canned=async()=>({});
window.nioh={handshake:canned,searchCatalog:canned,currentSearch:canned,resolveRecommendedLevel:canned,
 checkFeasibility:canned,snapshot:canned,cancelSearch:canned,restartWorker:canned};
window.preferences={getLocale:async()=>localStorage.getItem('nioh3-ui-locale')||'zh-CN',setLocale:canned};
window.support={diagnostics:async()=>({version:'0.8.9-test'}),exportDiagnostics:canned};
window.review={windowAction:canned,exportFeedback:canned,update:async()=>({phase:'current'}),
 favorites:async()=>[],log:canned,copyLog:canned,openLink:canned,copyText:canned,
 release:canned,retain:canned,preview:canned,auxiliary:canned,gameInstallation:async()=>null};
window.fixture={calls:[],failure:false,delay:false,receipt:null,reads:0};
window.fixture.playerReceipt=window.savedResourceReceipt||null;
const resources={process_id:42,process_birth:'77',snapshot_id:'b'.repeat(64),experimental:true,
 ingot_owned:true,ingot:'1234567890123',ingot_max:'9999999999999',fragments:'12345',fragments_max:'999999999',
 glory:'12345',glory_max:'2000000000',glory_editable:true,current_clan_id:7,clans:[
 {clan_id:3,rank:5,stored:'15915',tenure:'0'},{clan_id:7,rank:5,stored:'15915',tenure:'0'},
 {clan_id:50,rank:0,stored:'0',tenure:'0'}]};
const emptyPlayer=(id=null)=>({player_edit:{operation_id:id,state:'rejected',field:null,clan_id:0,before:null,after:null,error:null}});
const keys=[50152,19338,41583,29207,60307,1486,18917,50326,41813];
const rows=keys.map((category_id,i)=>({category_id,rank:10,fame:305,earned:500,spent:160,
 available:340,editable:i!==8}));
const empty=(id=null)=>({prestige_edit:{operation_id:id,state:'rejected',category_id:null,
 before:null,after:null,process_id:null,process_birth:null,error:null}});
window.operations={
 current:async()=>({job:null,busy:false}),
 execute:async({method,params})=>{
  window.fixture.calls.push({method,params:structuredClone(params)});
  if(method==='runtime.compatibility')return {compatibility:{present:true,reference_match:true,
   warning:false,accepted:false,plan:null,backup:null,differences:[],hard_blocks:[],
   game_version:'2.0.2.0',executable:'D:/synthetic/Nioh3.exe'}};
  if(method==='runtime.character_snapshot')return {source:'runtime',game_version:'2.0.2.0',process_id:42,
   currencies:{amrita:500,gold:500},equipment_slots:2500,equipment:[],items:[],items_error:null};
  if(method==='runtime.menu_selection')return {menu_open:false,slot_index:null};
  if(method==='runtime.player_status')return window.fixture.playerReceipt||emptyPlayer(params.operation_id||null);
  if(method==='runtime.player_resources'){
   if(window.fixture.missing)throw Error('No loaded character');
   return {player_resources:{...structuredClone(resources),ingot_owned:!window.fixture.noIngot,glory_editable:!window.fixture.unsyncedGlory}};
  }
  if(method==='runtime.player_recover'){
   if(window.fixture.playerReceipt)window.fixture.playerReceipt.player_edit.state='verified';
   return window.fixture.playerReceipt||emptyPlayer(params.operation_id);
  }
  if(method==='runtime.player_edit'){
   const row=resources.clans.find(c=>c.clan_id===params.clan_id);
   const before=params.field==='clan_rank'?String(row.rank):resources[params.field];
   window.fixture.playerReceipt={player_edit:{operation_id:params.operation_id,state:'verified',
    field:params.field,clan_id:params.clan_id,before,after:params.value,error:null}};
   if(params.field==='clan_rank'){row.rank=Number(params.value);row.stored='60000';}
   else resources[params.field]=params.value;
   if(window.fixture.delay)await new Promise(r=>setTimeout(r,500));
   if(window.fixture.playerFailure){window.fixture.playerReceipt.player_edit.state='uncertain';throw Error('injected resource response loss');}
   return window.fixture.playerReceipt;
  }
  if(method==='runtime.prestige_snapshot'){
   if(window.fixture.missing)throw Error('No loaded game character');
   window.fixture.reads++;
   return {prestige:{process_id:42,process_birth:'77',snapshot_id:'a'.repeat(64),max_budget:2000,
    experimental:true,categories:structuredClone(rows)}};
  }
  if(method==='runtime.prestige_status')return window.fixture.receipt||empty(params.operation_id||null);
  if(method==='runtime.prestige_recover'){
   if(window.fixture.receipt)window.fixture.receipt.prestige_edit.state='verified';
   return window.fixture.receipt||empty(params.operation_id);
  }
  if(method==='runtime.prestige_edit'){
   window.fixture.receipt={prestige_edit:{operation_id:params.operation_id,state:'verified',
    category_id:params.category_id,before:340,after:params.available,process_id:42,process_birth:'77',error:null}};
   const row=rows.find(r=>r.category_id===params.category_id);
   row.available=params.available;row.earned=row.spent+params.available;
   if(window.fixture.delay)await new Promise(r=>setTimeout(r,500));
   if(window.fixture.failure){window.fixture.receipt.prestige_edit.state='uncertain';throw Error('injected response loss');}
   return window.fixture.receipt;
  }
  throw Error('unexpected fixture method '+method);
 }
};`;
const html='<!doctype html><html><meta charset="utf-8"><style>'+css+'</style><div id="root"></div><script>'+
 bridge+'</script><script>'+script.replaceAll('</script','<\\/script')+'</script></html>';
const server=createServer((_,res)=>{res.writeHead(200,{'content-type':'text/html;charset=utf-8'});res.end(html);});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
const browser=await chromium.launch({headless:true,channel:'msedge'});
const checks=[],screenshots=[],errors=[];
const check=(name,value)=>{assert.ok(value,name);checks.push(name);};
try{
 for(const locale of ['zh-CN','en-US','ja-JP']){
  const context=await browser.newContext({viewport:{width:1366,height:980},deviceScaleFactor:1});
  const page=await context.newPage();page.on('pageerror',error=>errors.push(String(error)));
  await page.goto('http://127.0.0.1:'+server.address().port+'/?locale='+locale);
  const playerNav=page.locator('.nav [data-page="player"]');
  const expectedTitle={'zh-CN':'人物编辑','en-US':'Player Editor','ja-JP':'人物編集'}[locale];
  check(locale+' top-level player entry',await playerNav.innerText()===expectedTitle);
  await playerNav.click();
  await page.locator('[data-prestige-category]').last().waitFor();
  check(locale+' routed page title',await page.locator('h1').innerText()===expectedTitle);
  check(locale+' all four sections available',await page.locator('.player-editor-sections [role="tab"]').count()===4
    &&await page.locator('.player-editor-sections button:disabled').count()===0);
  check(locale+' nine categories',await page.locator('[data-prestige-category]').count()===9);
  const first=page.locator('[data-prestige-category="50152"]'),input=first.locator('input'),review=first.locator('button');
  check(locale+' missing record disabled',await page.locator('[data-prestige-category="41813"] input').isDisabled());
  for(const value of ['-1','1.2','2000']){
   await input.fill(value);await review.click();
   check(locale+' refuses '+value,await page.locator('.prestige-review').count()===0);
  }
  await input.fill('123');await review.click();
  const panel=page.locator('.prestige-review');
  await panel.waitFor();check(locale+' review shows before and after',(await panel.innerText()).includes('340 → 123'));
  check(locale+' review alone never writes',await page.evaluate(()=>!window.fixture.calls.some(c=>c.method==='runtime.prestige_edit')));
  await page.keyboard.press('Escape');check(locale+' escape cancels review',await panel.count()===0);
  await review.click();await page.evaluate(()=>window.fixture.delay=true);
  await panel.locator('button').first().click();
  check(locale+' busy blocks duplicate review',await review.isDisabled());
  check(locale+' busy retains top-level page ownership',await page.locator('.nav button').evaluateAll(buttons=>buttons.every(b=>b.disabled)));
  await page.waitForFunction(()=>window.fixture.calls.filter(c=>c.method==='runtime.prestige_snapshot').length===2);
  await page.waitForFunction(()=>document.querySelector('[data-prestige-category="50152"] dd:last-child')!==null);
  check(locale+' one confirmed write',await page.evaluate(()=>window.fixture.calls.filter(c=>c.method==='runtime.prestige_edit').length===1));
  check(locale+' exact reviewed identity and target',await page.evaluate(()=>{
   const p=window.fixture.calls.find(c=>c.method==='runtime.prestige_edit').params;
   return p.category_id===50152&&p.available===123&&p.process_id===42&&p.process_birth==='77'&&p.confirmed&&p.snapshot_id==='a'.repeat(64);
  }));
  check(locale+' settled reference cleared',await page.evaluate(()=>localStorage.getItem('nioh3-prestige-operation-v1')===null));
  check(locale+' compatibility fixture stays quiet',await page.locator('.runtime-compatibility-banner').count()===0);
  const shot=join(output,'prestige-'+locale+'.png');await page.screenshot({path:shot,fullPage:true,animations:'disabled'});screenshots.push(shot);
  await page.evaluate(()=>document.documentElement.dataset.theme='dark');
  const darkShot=join(output,'prestige-dark-'+locale+'.png');await page.screenshot({path:darkShot,fullPage:true,animations:'disabled'});screenshots.push(darkShot);
  await writeFile(join(output,'palette-'+locale+'.json'),JSON.stringify(await page.evaluate(()=>{
    const color=selector=>{const s=getComputedStyle(document.querySelector(selector));return {color:s.color,background:s.backgroundColor};};
    return {body:color('body'),nav:color('.nav [data-page="player"]'),label:color('.prestige-category dt'),
      active:color('.player-editor-sections .active'),other:color('.player-editor-sections button:not(.active)')};
  }),null,2));
  check(locale+' dark palette applied',await first.evaluate(card=>getComputedStyle(card).backgroundColor!=='rgb(255, 255, 255)'));
  check(locale+' dark labels and hints stay legible',await page.evaluate(()=>{
    const luminance=color=>{
      const values=color.match(/[\d.]+/g).slice(0,3).map(Number).map(v=>{v/=255;return v<=0.04045?v/12.92:((v+0.055)/1.055)**2.4;});
      return values[0]*0.2126+values[1]*0.7152+values[2]*0.0722;
    };
    return ['.prestige-category dt','.character-hint','.notice-warning'].every(selector=>{
      const node=document.querySelector(selector);
      const background=selector==='.prestige-category dt'?node.closest('.prestige-category'):node.closest('main');
      const fg=luminance(getComputedStyle(node).color),bg=luminance(getComputedStyle(background).backgroundColor);
      return (Math.max(fg,bg)+0.05)/(Math.min(fg,bg)+0.05)>=4.5;
    });
  }));
  await page.setViewportSize({width:640,height:900});
  check(locale+' constrained cards stay within viewport',await page.locator('.prestige-category').evaluateAll(cards=>cards.every(c=>c.getBoundingClientRect().right<=innerWidth)));
  await page.locator('[data-prestige-category="41813"]').scrollIntoViewIfNeeded();
  check(locale+' last category remains reachable',await page.locator('[data-prestige-category="41813"]').evaluate(e=>{
    const r=e.getBoundingClientRect();return r.top>=0&&r.bottom<=innerHeight;
  }));
  await page.setViewportSize({width:1100,height:740});
  check(locale+' minimum native-width navigation stays inside window',await page.locator('.nav [data-page="player"]').evaluate(e=>{
    const r=e.getBoundingClientRect();return r.left>=0&&r.right<=innerWidth;
  }));
  await page.locator('.nav button').nth(4).click();
  await page.locator('.character-sections').waitFor();
  check(locale+' equipment keeps its two sections',await page.locator('.character-sections [role="tab"]').count()===2);
  check(locale+' equipment no longer owns prestige',await page.locator('.prestige-editor').count()===0);
  await playerNav.click();await page.locator('.prestige-editor').waitFor();
  check(locale+' navigation returns without replay',await page.evaluate(()=>window.fixture.calls.filter(c=>c.method==='runtime.prestige_edit').length===1));
  await page.setViewportSize({width:1366,height:980});
  for(const [section,value] of [['ingot','9999999999999'],['fragments','999999999'],['glory','2000000000']]){
   await page.locator('#player-'+section+'-tab').click();
   const card=page.locator('[data-resource-field="'+section+'"]');await card.waitFor();
   const input=card.locator('input'),button=card.locator('button');
   await input.fill(String(BigInt(value)+1n));await button.click();
   check(locale+' '+section+' cap refuses review',await page.locator('.player-review').count()===0);
   await input.fill(value);await button.click();
   check(locale+' '+section+' exact decimal review',(await page.locator('.player-review').innerText()).includes(value));
   await page.locator('.player-review button').first().click();
   await page.waitForFunction(()=>!document.querySelector('.nav [data-page="player"]').disabled);
   check(locale+' '+section+' readback',await card.locator('dd').first().innerText()===value);
   check(locale+' '+section+' clears operation reference',await page.evaluate(()=>localStorage.getItem('nioh3-player-resource-operation-v1')===null));
   const screen=join(output,'resources-'+section+'-'+locale+'.png');
   await page.screenshot({path:screen,animations:'disabled'});screenshots.push(screen);
   await page.evaluate(()=>document.documentElement.dataset.theme='light');
   const light=join(output,'resources-light-'+section+'-'+locale+'.png');
   await page.screenshot({path:light,animations:'disabled'});screenshots.push(light);
   await page.evaluate(()=>document.documentElement.dataset.theme='dark');
  }
  const clanCard=page.locator('[data-resource-field="clan_rank"]');
  await clanCard.locator('select').first().selectOption('3');
  await clanCard.locator('select').last().selectOption('9');await clanCard.locator('button').click();
  check(locale+' selected clan contribution reviewed',(await page.locator('.player-review').innerText()).includes('60000'));
  await page.locator('.player-review button').first().click();
  await page.waitForFunction(()=>!document.querySelector('.nav [data-page="player"]').disabled);
  check(locale+' clan identity and rank transported',await page.evaluate(()=>{
   const p=window.fixture.calls.find(c=>c.method==='runtime.player_edit'&&c.params.field==='clan_rank').params;
   return p.clan_id===3&&p.value==='9'&&p.confirmed&&p.snapshot_id==='b'.repeat(64);
  }));
  check(locale+' edited clan stays selected after readback',await clanCard.locator('select').first().inputValue()==='3');
  check(locale+' resource values always travel as strings',await page.evaluate(()=>window.fixture.calls.filter(c=>c.method==='runtime.player_edit').every(c=>typeof c.params.value==='string')));
  await context.close();
 }
 const context=await browser.newContext({viewport:{width:1100,height:900}}),page=await context.newPage();
 page.on('pageerror',error=>errors.push(String(error)));
 await page.goto('http://127.0.0.1:'+server.address().port);
 await page.locator('.nav [data-page="player"]').click();
 const first=page.locator('[data-prestige-category="50152"]');await first.waitFor();
 await first.locator('input').fill('99');await first.locator('button').click();
 await page.evaluate(()=>window.fixture.failure=true);
 await page.locator('.prestige-review button').first().click();await page.locator('.prestige-recovery').waitFor();
 check('lost reply retains operation UUID',await page.evaluate(()=>!!localStorage.getItem('nioh3-prestige-operation-v1')));
 check('lost reply disables further write controls',await first.locator('button').isDisabled());
 await page.locator('.prestige-recovery button').first().click();
 check('status never replays write',await page.evaluate(()=>window.fixture.calls.filter(c=>c.method==='runtime.prestige_edit').length===1));
 await page.locator('.nav button').nth(4).click();await page.locator('.character-sections').waitFor();
 await page.locator('.nav [data-page="player"]').click();await page.locator('.prestige-recovery').waitFor();
 check('pending operation survives top-level navigation',await page.evaluate(()=>window.fixture.calls.filter(c=>c.method==='runtime.prestige_edit').length===1)
   &&await page.locator('[data-prestige-category] button:enabled').count()===0);
 await page.evaluate(()=>localStorage.removeItem('nioh3-prestige-operation-v1'));
 // Remount the production component without resetting the simulated durable backend.
 await page.evaluate(()=>location.hash='recovery');
 // Reloading a real worker keeps its receipt; preserve it in this scripted bridge.
 const receipt=await page.evaluate(()=>window.fixture.receipt);
 await page.addInitScript(value=>{window.savedPrestigeReceipt=value;},receipt);
 await page.route('**/*',async route=>route.fulfill({contentType:'text/html',body:html.replace("window.fixture={calls:[],failure:false,delay:false,receipt:null,reads:0}","window.fixture={calls:[],failure:false,delay:false,receipt:window.savedPrestigeReceipt||null,reads:0}")}));
 await page.reload();await page.locator('.nav [data-page="player"]').click();await page.locator('.prestige-recovery').waitFor();
 check('receipt rediscovered after local reference loss',await page.evaluate(()=>localStorage.getItem('nioh3-prestige-operation-v1')===window.fixture.receipt.prestige_edit.operation_id));
 await page.locator('.prestige-recovery button').last().click();
 await page.waitForFunction(()=>localStorage.getItem('nioh3-prestige-operation-v1')===null);
 check('recovery only observes',await page.evaluate(()=>!window.fixture.calls.some(c=>c.method==='runtime.prestige_edit')));
 await context.close();
 const blocked=await browser.newContext({viewport:{width:1100,height:900}}),blockedPage=await blocked.newPage();
 await blockedPage.goto('http://127.0.0.1:'+server.address().port);
 await blockedPage.locator('.nav [data-page="player"]').click();
 const blockedRow=blockedPage.locator('[data-prestige-category="50152"]');await blockedRow.waitFor();
 await blockedRow.locator('input').fill('77');await blockedRow.locator('button').click();
 await blockedPage.evaluate(()=>{
   const original=Storage.prototype.setItem;
   Storage.prototype.setItem=function(key,value){if(key==='nioh3-prestige-operation-v1')throw Error('injected storage failure');return original.call(this,key,value);};
 });
 await blockedPage.locator('.prestige-review button').first().click();
 await blockedPage.waitForFunction(()=>!document.querySelector('.prestige-review button').disabled);
 check('failed recovery-reference storage refuses before dispatch',await blockedPage.evaluate(()=>!window.fixture.calls.some(c=>c.method==='runtime.prestige_edit')));
 await blocked.close();
 const absent=await browser.newContext({viewport:{width:1100,height:740}}),absentPage=await absent.newPage();
 absentPage.on('pageerror',error=>errors.push(String(error)));
 await absentPage.goto('http://127.0.0.1:'+server.address().port);
 await absentPage.evaluate(()=>window.fixture.missing=true);
 await absentPage.locator('.nav [data-page="player"]').click();
 await absentPage.waitForFunction(()=>!document.querySelector('.nav [data-page="player"]').disabled);
 check('missing game keeps editor available without write controls',await absentPage.locator('.player-editor').count()===1
   &&await absentPage.locator('[data-prestige-category]').count()===0);
 check('missing game has readable error',(await absentPage.locator('.prestige-editor').innerText()).includes('未能读取或修改恩惠点数'));
 await absent.close();
 const loss=await browser.newContext({viewport:{width:1100,height:740}}),lossPage=await loss.newPage();
 lossPage.on('pageerror',error=>errors.push(String(error)));
 await lossPage.goto('http://127.0.0.1:'+server.address().port);
 await lossPage.locator('.nav [data-page="player"]').click();
 await lossPage.locator('#player-glory-tab').click();
 const glory=lossPage.locator('[data-resource-field="glory"]');await glory.waitFor();
 await glory.locator('input').fill('100');await glory.locator('button').click();
 await lossPage.keyboard.press('Escape');
 check('resource review escape never writes',await lossPage.locator('.player-review').count()===0
   &&await lossPage.evaluate(()=>!window.fixture.calls.some(c=>c.method==='runtime.player_edit')));
 await glory.locator('button').click();await lossPage.evaluate(()=>window.fixture.playerFailure=true);
 await lossPage.locator('.player-review button').first().click();
 await lossPage.locator('.player-recovery button').first().click();
 check('resource response loss preserves UUID',await lossPage.evaluate(()=>!!localStorage.getItem('nioh3-player-resource-operation-v1')));
 await lossPage.locator('#player-fragments-tab').click();
 check('resource pending ownership spans resource tabs',await lossPage.locator('.player-recovery').count()===1
   &&await lossPage.locator('[data-resource-field] button:enabled').count()===0);
 await lossPage.locator('#player-prestige-tab').click();await lossPage.locator('[data-prestige-category]').first().waitFor();
 check('uncertain resource blocks prestige controls',await lossPage.locator('[data-prestige-category] button:enabled').count()===0);
 await lossPage.locator('#player-glory-tab').click();await lossPage.locator('.player-recovery').waitFor();
 await lossPage.evaluate(()=>localStorage.removeItem('nioh3-player-resource-operation-v1'));
 const resourceReceipt=await lossPage.evaluate(()=>window.fixture.playerReceipt);
 await lossPage.addInitScript(value=>{window.savedResourceReceipt=value;},resourceReceipt);
 await lossPage.reload();await lossPage.locator('.nav [data-page="player"]').click();
 await lossPage.locator('#player-glory-tab').click();await lossPage.locator('.player-recovery').waitFor();
 check('resource durable discovery survives browser reference loss',await lossPage.evaluate(()=>localStorage.getItem('nioh3-player-resource-operation-v1')===window.fixture.playerReceipt.player_edit.operation_id));
 await lossPage.locator('.player-recovery button').last().click();
 await lossPage.waitForFunction(()=>localStorage.getItem('nioh3-player-resource-operation-v1')===null);
 check('resource recovery never replays',await lossPage.evaluate(()=>!window.fixture.calls.some(c=>c.method==='runtime.player_edit')));
 await loss.close();
 const ownership=await browser.newContext({viewport:{width:1100,height:740}}),ownershipPage=await ownership.newPage();
 await ownershipPage.goto('http://127.0.0.1:'+server.address().port);
 await ownershipPage.evaluate(()=>window.fixture.noIngot=true);
 await ownershipPage.locator('.nav [data-page="player"]').click();await ownershipPage.locator('#player-ingot-tab').click();
 await ownershipPage.locator('[data-resource-field="ingot"]').waitFor();
 check('unowned ingot cannot be edited',await ownershipPage.locator('[data-resource-field="ingot"] input').isDisabled());
 await ownershipPage.evaluate(()=>window.fixture.unsyncedGlory=true);
 await ownershipPage.locator('#player-glory-tab').click();
 await ownershipPage.locator('.character-toolbar button').click();
 await ownershipPage.waitForFunction(()=>document.querySelector('[data-resource-field="glory"] input')?.disabled);
 check('different Glory markers disable only the balance editor',await ownershipPage.locator('[data-resource-field="glory"] input').isDisabled());
 check('different Glory markers preserve clan rank controls',await ownershipPage.locator('[data-resource-field="clan_rank"] select').first().isEnabled());
 await ownershipPage.locator('#player-fragments-tab').click();
 const fragment=ownershipPage.locator('[data-resource-field="fragments"]');
 check('different Glory markers preserve fragment editing',await fragment.locator('input').isEnabled());
 await fragment.locator('input').fill('77');await fragment.locator('button').click();
 await ownershipPage.evaluate(()=>{const old=Storage.prototype.setItem;Storage.prototype.setItem=function(k,v){if(k==='nioh3-player-resource-operation-v1')throw Error('storage failure');return old.call(this,k,v);};});
 await ownershipPage.locator('.player-review button').first().click();
 await ownershipPage.waitForFunction(()=>!document.querySelector('.player-review button').disabled);
 check('resource reference storage failure refuses dispatch',await ownershipPage.evaluate(()=>!window.fixture.calls.some(c=>c.method==='runtime.player_edit')));
 await ownership.close();check('no browser runtime errors',errors.length===0);
}finally{
 await writeFile(join(output,'result.json'),JSON.stringify({checks,screenshots,errors,boundary:'Production component, scripted bridge; no game, save, native window or packaged acceptance'},null,2));
 await browser.close();await new Promise(r=>server.close(r));
}
console.log(JSON.stringify({checks:checks.length,screenshots,errors}));
