/** Current-source native UI acceptance. All game/save responses are explicit fixtures; no writes. */
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdir, mkdtemp, readFile, writeFile} from 'node:fs/promises';
import {join, resolve} from 'node:path';
import {chromium} from 'playwright';
import {isolatedEnvironment, closeSession, pause, digest, inspectOnefile} from './onefile-acceptance.mjs';
import {detectTextFit} from '../workshop/text-fit-detect.mjs';

const out=resolve(process.env.NIOH3_UI_EVIDENCE_ROOT??'D:/Nioh3_v080_deliverables/deliverables/live-equipment-effect-search/native');
await mkdir(out,{recursive:true});
const root=await mkdtemp('D:/Nioh3_v080_deliverables/tmp/live-equipment-native-');
const target='D:/Nioh3_v080_deliverables/build-cache/tauri-target';
const exe=process.env.NIOH3_UI_NATIVE_EXE??join(target,'debug/nioh3-studio.exe');
const onefile=process.env.NIOH3_UI_ONEFILE==='1';
const {env,profile,port}=await isolatedEnvironment(root);
if(!onefile)Object.assign(env,{NIOH3_RUST_PROTECTED_WORKER:join(target,'debug/nioh3-protected-worker.exe'),NIOH3_RUST_SEARCH_WORKER:join(target,'debug/nioh3-readonly-worker.exe'),NIOH3_RUST_PROTECTED_GAME_FILE_VERSION:'2.0.2.0',NIOH3_RUST_SEARCH_GAME_FILE_VERSION:'2.0.2.0'});
const report={pass:false,packaged:onefile,boundary:'Current-source Tauri/WebView2 shell; scripted game/save responses, no native builder, insertion, save write or live-game acceptance',executable:exe,executableSha256:digest(await readFile(exe)),checks:[],geometry:[],screenshots:[],gameWrites:0,saveWrites:0};
if(onefile)report.payloadSha256=(await inspectOnefile(exe)).payloadSha256;
const check=(name,condition)=>{report.checks.push({name,pass:!!condition});assert(condition,name);};
function fixtures() {
  window.__seedCalls=[];window.__seedOperation=null;
  const reference={save_id:'seed-fixture',account_id:'123',save_slot:0,path:'D:/fixture/SAVEDATA.BIN',summary:{playthrough:3,character_level:180,play_time_seconds:36000,saved_at_unix:1794672000}};
  window.operations.current=async()=>({job:null,busy:false});
  window.operations.selectSave=async()=>reference;
  window.operations.execute=async command=>{
    window.__seedCalls.push(command);const p=command.params;
    if(command.method==='save.discover')return{saves:[reference]};
    if(command.method==='save.inventory')return{save_id:reference.save_id,account_id:'123',snapshot_id:'b'.repeat(64),source_sha256:'c'.repeat(64),empty_slots:400,entries:[]};
    if(command.method==='save.operations')return{operations:[]};
    if(command.method==='save.character')return{save_id:reference.save_id,source_sha256:'c'.repeat(64),currencies:{amrita:100,gold:100},equipment_slots:2500,equipment:[],generation:{difficulty:3,difficulties:[{difficulty:3,progress:[1,2,3,4]}]}};
    if(command.method==='runtime.character_snapshot')return{source:'runtime',game_version:'2.0.2.0',process_id:42,currencies:{amrita:100,gold:100},equipment_slots:2500,equipment:[],items:[]};
    if(command.method==='runtime.menu_selection')return{process_id:42,menu_open:false};
    if(command.method==='runtime.equipment_rules')return{item_id:p.item_id,known:true,rarity:p.rarity,level:p.level,random_pool:[{effect_id:44634,star:false,min:10,max:20,group:1,masks:[0,0]}]};
    if(command.method==='runtime.equipment_seeds')return{item_id:p.item_id,rarity:p.rarity,level:p.level,difficulty:p.difficulty,seeds:65536,empty:0,matches:2,outcomes:[123,456].map(seed=>({seed,effects:[{effect_id:44634,value:150,star:false,role:'random',roll:100}]}))};
    if(command.method==='runtime.equipment_add_prepare'){
      window.__seedOperation={operation_id:p.operation_id,plan_digest:'a'.repeat(64),state:'prepared',process_id:42,slot_index:3,error:null,preview:{slot_index:3,item_id:p.item_id,appearance_id:p.item_id,quantity:1,level:p.level,level_before_forge:p.level,plus:p.plus,familiarity:0,inventory_key:0,seed:p.seed,rarity:p.rarity,effects:[{effect_id:44634,value:150,star:false}]}};
      return{equipment_add:window.__seedOperation};
    }
    if(command.method==='runtime.equipment_add_cancel'){window.__seedOperation.state='cancelled';return{equipment_add:window.__seedOperation};}
    if(command.method==='runtime.equipment_add_status')return{equipment_add:window.__seedOperation};
    throw Error('UNEXPECTED_FIXTURE_METHOD: '+command.method);
  };
}
let child,session;
try {
  child=spawn(exe,['--user-data-dir',profile],{cwd:process.cwd(),env,windowsHide:true,stdio:'ignore'});
  for(let i=0;i<150;i++){assert(child.exitCode===null,'native shell exited');try{if((await fetch('http://127.0.0.1:'+port+'/json/version')).ok)break;}catch{}await pause(300);}
  const browser=await chromium.connectOverCDP('http://127.0.0.1:'+port);
  const page=browser.contexts()[0].pages()[0];session={browser,page};
  await page.locator('.shell').waitFor({timeout:45000});
  report.diagnostics=await page.evaluate(()=>window.support.diagnostics());
  check('isolated user-data directory',resolve(report.diagnostics.dataDirectory).toLowerCase()===resolve(profile).toLowerCase());
  if(onefile){
    check('outer package manifest verified',report.diagnostics.packageVerification?.ok===true&&report.diagnostics.packageVerification?.sourceDirty===false);
  }
  const cdp=await browser.contexts()[0].newCDPSession(page);
  async function shot(name) {const path=join(out,name+'.png');await page.screenshot({path});report.screenshots.push(path);}
  async function geometry(name) {
    const snapshot=await page.evaluate(()=>({width:innerWidth,height:innerHeight,dpr:devicePixelRatio,screen:{width:screen.width,height:screen.height},overflow:document.documentElement.scrollWidth>innerWidth+2}));
    check(name+' no document overflow',!snapshot.overflow);report.geometry.push({name,...snapshot});
  }
  async function spacing(name) {
    const measured=await page.evaluate(()=>{
      const box=selector=>document.querySelector(selector).getBoundingClientRect();
      const save=document.querySelector('.live-equipment-save'),style=getComputedStyle(save);
      return {
        headingText:box('.live-equipment-heading p').top-box('.live-equipment-heading h2').bottom,
        headingNote:box('.live-equipment-add>.equipment-notes').top-box('.live-equipment-heading').bottom,
        noteColumns:box('.live-equipment-layout').top-box('.live-equipment-add>.equipment-notes').bottom,
        fieldsSave:box('.live-equipment-backup').top-box('.live-equipment-fields').bottom,
        saveSearch:box('.live-equipment-seed-search').top-box('.live-equipment-backup').bottom,
        searchHeading:box('.live-equipment-seed-body').top-box('.live-equipment-seed-search legend').bottom,
        searchPrepare:box('.live-equipment-prepare').top-box('.live-equipment-seed-search').bottom,
        outerSaveBorder:['borderTopWidth','borderRightWidth','borderBottomWidth','borderLeftWidth'].reduce((sum,key)=>sum+parseFloat(style[key]),0),
      };
    });
    check(name+' heading and description separated',measured.headingText>=5);
    check(name+' form groups have breathing room',Object.entries(measured).filter(([key])=>!['outerSaveBorder','headingText'].includes(key)).every(([,gap])=>gap>=8));
    check(name+' save outer fieldset is borderless',measured.outerSaveBorder===0);
    report.geometry.push({name:name+' spacing',...measured});
  }
  for(const locale of ['zh-CN','en-US','ja-JP']) {
    await cdp.send('Emulation.clearDeviceMetricsOverride');
    await page.evaluate(locale=>{localStorage.clear();localStorage.setItem('nioh3-ui-locale',locale);},locale);
    await page.reload();await page.locator('.shell').waitFor();
    await page.waitForFunction(()=>!!window.operations);
    // Settle startup reads before replacing the transport with presentation fixtures.
    await page.evaluate(()=>Promise.all(['save','runtime'].map(role=>window.operations.current(role))));
    await page.evaluate(fixtures);
    await page.locator('nav.nav>button').nth(4).click();
    await page.locator('.character-sections>button').nth(1).click();
    await page.locator('[data-action=pick-equipment]').first().click();
    await page.locator('.live-equipment-save .save-picker>button').last().click();
    const find=page.locator('[data-action=find-equipment-seeds]');
    await page.waitForFunction(()=>!document.querySelector('[data-action=find-equipment-seeds]')?.disabled);
    const picker=page.locator('.live-equipment-seed-search .effect-picker input');
    await picker.focus();await picker.press('Enter');
    check(locale+' keyboard chooses wanted effect',await page.locator('.seed-chip').count()===1);
    await find.click();await page.locator('.seed-outcomes [role=option]').first().waitFor();
    check(locale+' search is read-only',await page.evaluate(()=>!window.__seedCalls.some(c=>c.method==='runtime.equipment_add_prepare'||c.method==='runtime.equipment_add_execute')));
    await spacing(locale+' native default');
    await page.locator('.live-equipment-add').evaluate(el=>{el.scrollTop=0;});await shot(locale+'-form-top');
    await page.locator('.live-equipment-backup').scrollIntoViewIfNeeded();await shot(locale+'-form-save-search');
    for(const state of ['default','maximized','restored']) {
      if(state!=='default'){await page.evaluate(()=>window.review.windowAction('maximize'));await pause(350);}
      await geometry(locale+' native '+state);await shot(locale+'-native-'+state);
    }
    for(const [width,height,dpr] of [[1860,1000,1],[1100,740,1],[1280,720,1.25],[1067,600,1.5]]) {
      await cdp.send('Emulation.setDeviceMetricsOverride',{width,height,deviceScaleFactor:dpr,mobile:false});await pause(100);
      for(const theme of ['light','dark']) {
        await page.evaluate(theme=>{document.documentElement.dataset.theme=theme;},theme);
        const name=locale+'-'+theme+'-'+width+'x'+height+'-dpr'+dpr;
        await geometry(name);
        await spacing(name);
        // Reachability uses real scrolling within the production page and the seed list.
        await picker.scrollIntoViewIfNeeded();await picker.focus();await picker.press('Escape');
        check(name+' picker dismisses with Escape',await page.locator('.effect-picker-list').count()===0);
        const option=page.locator('.seed-outcomes [role=option]').nth(1);await option.scrollIntoViewIfNeeded();await option.click();
        check(name+' seed outcome reachable',await page.locator('[data-field=seed]').inputValue()==='456');
        await find.scrollIntoViewIfNeeded();
        const failures=await page.evaluate(detectTextFit);
        const affected=failures.filter(f=>f.at?.includes('live-equipment')||f.at?.includes('seed-'));
        check(name+' affected text fits',affected.length===0);
        await shot(name);
        if(width===1860&&theme==='dark'){
          await page.locator('.live-equipment-add').evaluate(el=>{el.scrollTop=0;});
          await shot(name+'-top');
        }
      }
    }
    const prepare=page.locator('[data-action=prepare-equipment]');await prepare.scrollIntoViewIfNeeded();await prepare.click();
    await page.locator('[data-state=prepared]').waitFor();
    const cancel=page.locator('[data-action=cancel-equipment]');await cancel.scrollIntoViewIfNeeded();
    check(locale+' native preview can be reviewed before confirmation',await page.locator('[data-action=confirm-equipment]').isEnabled()&&await page.locator('[data-action=execute-equipment]').isDisabled());
    await shot(locale+'-preview');await cancel.click();await page.locator('[data-state=cancelled]').waitFor();
    check(locale+' no actual insertion or save write requested',await page.evaluate(()=>!window.__seedCalls.some(c=>c.method==='runtime.equipment_add_execute'||c.method.startsWith('save.prepare')||c.method==='save.commit')));
    await page.locator('[data-action=new-equipment]').click();
  }
  await cdp.send('Emulation.clearDeviceMetricsOverride');
  report.pass=true;
} catch(error) {report.error=error.stack??String(error);}
finally {
  if(session){await writeFile(join(out,'last-state.txt'),await session.page.locator('body').innerText().catch(()=>''));await session.page.screenshot({path:join(out,'last-state.png')}).catch(()=>{});}
  await closeSession(session,child).catch(error=>{report.cleanupError=String(error);report.pass=false;});
  await writeFile(join(out,'native-ui.json'),JSON.stringify(report,null,2));
}
console.log(JSON.stringify({pass:report.pass,checks:report.checks.length,out,error:report.error,cleanupError:report.cleanupError}));
if(!report.pass)process.exitCode=1;
