/** Actual Electron layouts and count-edit boundaries using an isolated save. */
import {_electron as electron} from 'playwright';
import {resolve,join} from 'node:path';
import {mkdtemp,mkdir,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {execFileSync} from 'node:child_process';
import assert from 'node:assert/strict';

const python=process.env.NIOH3_PYTHON||'python';
const root=await mkdtemp(join(tmpdir(),'nioh3-release-surfaces-'));
const output=resolve(process.env.NIOH3_SURFACE_OUTPUT||'deliverables/frontend-v2/release-surfaces');
await mkdir(output,{recursive:true});
execFileSync(python,[resolve('apps/desktop/tests/fixtures/create-save.py'),join(root,'local','KoeiTecmo','NIOH3','Savedata')],{windowsHide:true});
const app=await electron.launch({...(process.env.NIOH3_PORTABLE_EXE?{executablePath:resolve(process.env.NIOH3_PORTABLE_EXE),args:[]}:{args:[resolve('apps/desktop/dist/main.cjs')]}),env:{...process.env,NIOH3_PYTHON:python,NIOH3_REVIEW_UI:'1',NIOH3_ELECTRON_TEST:'1',LOCALAPPDATA:join(root,'local'),NIOH3_STATE_ROOT:join(root,'state')}});
const checks=[],errors=[],measurements=[];
const check=(name,passed)=>{assert.ok(passed,name);checks.push(name);console.log(name)};
try{
  const p=await app.firstWindow();p.on('pageerror',e=>errors.push(e.message));
  await app.evaluate(({BrowserWindow})=>BrowserWindow.getAllWindows()[0].webContents.setBackgroundThrottling(false));
  await p.getByText('后端已连接，请选择筛选条件。',{exact:true}).waitFor({timeout:30000});
  await p.locator('.nav nav button').nth(1).click();
  await p.locator('.inventory-list button').first().waitFor();
  await p.getByText('当前可挑战次数',{exact:true}).click();
  const count=p.getByRole('spinbutton',{name:'当前可挑战次数',exact:true});
  await count.fill('');check('Empty count cannot prepare a write',await p.getByRole('button',{name:'核对次数修改',exact:true}).isDisabled());
  await count.fill('8');check('Out-of-range count cannot prepare a write',await p.getByRole('button',{name:'核对次数修改',exact:true}).isDisabled());
  await count.fill('2');check('Valid count can be reviewed',await p.getByRole('button',{name:'核对次数修改',exact:true}).isEnabled());
  for(const method of ['save.count_edit_source','runtime.count_prepare']){
    check('Renderer rejects private '+method,await p.evaluate(async method=>{try{await window.operations.execute({method,params:{}});return false}catch{return true}},method));
  }
  check('Broker rejects invalid count before selecting a save',await p.evaluate(async()=>{try{await window.operations.prepareCount({save_id:'',snapshot_id:'',slot_index:0,new_count:8});return false}catch(e){return String(e).includes('INVALID_COUNT')}}));
  for(const [locale,label] of [['zh-CN','简体中文'],['en-US','English'],['ja-JP','日本語']]){
    if(await p.evaluate(()=>document.documentElement.lang)!==locale){
      await p.locator('.language-button').click();await p.locator('.side-popup').getByRole('button',{name:label,exact:true}).click();
    }
    for(const [width,height,zoom] of [[1280,800,1],[1920,1080,1.25],[2560,1440,1.5]]){
      await app.evaluate(({BrowserWindow},s)=>{const w=BrowserWindow.getAllWindows()[0];w.setContentSize(s.width,s.height);w.webContents.setZoomFactor(s.zoom)},{width,height,zoom});
      for(const [index,name,selector] of [[0,'search','.search-page'],[1,'editor','.editor-page'],[2,'backups','.backup-page']]){
        await p.locator('.nav nav button').nth(index).click();await p.locator(selector).waitFor();
        const state=await p.evaluate(selector=>{const e=document.querySelector(selector);return {lang:document.documentElement.lang,width:innerWidth,height:innerHeight,pageOverflow:document.documentElement.scrollWidth>innerWidth,panelOverflow:e.scrollWidth>e.clientWidth+2,text:e.innerText}},selector);
        assert.equal(state.lang,locale);assert.equal(state.pageOverflow,false,`${locale} ${width} ${name}: page overflow`);
        assert.equal(state.panelOverflow,false,`${locale} ${width} ${name}: panel overflow`);
        assert.ok(await p.locator('.nav nav button').evaluateAll(nodes=>nodes.every(e=>e.getBoundingClientRect().right<=e.closest('.nav').getBoundingClientRect().right+1)),`${locale}: navigation labels fit`);
        const targets=name==='search'?['.search-page main','.result-pane']:name==='editor'?['.inventory-pane','.editor-work','.editor-review']:['.backup-page'];
        for(const target of targets)assert.ok(await p.locator(target).evaluate(e=>{const r=e.getBoundingClientRect();return r.left>=-1&&r.right<=innerWidth+2}),`${locale}: ${target} stays inside the window`);
        if(locale==='en-US')assert.ok(!/[\u3400-\u9fff]/.test(state.text),`${name}: untranslated Chinese`);
        measurements.push({locale,width,height,zoom,name,...state});
        if(index===1){
          for(const section of await p.locator('.editor-work>details.editor-section>summary').all()){
            await section.click();
            assert.ok(await p.locator('.editor-work').evaluate(e=>e.scrollWidth<=e.clientWidth+2),'Expanded editor section fits');
          }
        }
        if(width===1280&&process.env.NIOH3_CAPTURE_UI==='1'){
          await app.evaluate(({BrowserWindow})=>BrowserWindow.getAllWindows()[0].showInactive());
          await p.screenshot({path:join(output,`${locale}-${name}-100pct.png`),fullPage:true,timeout:15000});
        }
      }
    }
    checks.push(`${locale}: search/editor/backups at three viewport and zoom settings`);
  }
  await app.evaluate(({BrowserWindow})=>{const w=BrowserWindow.getAllWindows()[0];w.setContentSize(1600,1000);w.webContents.setZoomFactor(1)});
  await p.locator('.language-button').click();
  await p.locator('.side-popup').getByRole('button',{name:'English',exact:true}).click();
  await p.locator('.nav nav button').nth(1).click();
  const countSection=p.locator('.editor-work>details').filter({has:p.getByText('Remaining attempts',{exact:true})});
  if(!await countSection.evaluate(e=>e.open))await countSection.locator('summary').click();
  // Deliberately simulated receipts exercise renderer states, not game mutation.
  await app.evaluate(({ipcMain})=>{
    const plan={operation_id:'00000000-0000-4000-8000-000000000001',plan_digest:'a'.repeat(64),state:'prepared',seed:123,rarity:3,old_count:6,new_count:2,error:null};
    globalThis.countUiWrites=0;
    ipcMain.removeHandler('operations:prepare-count');ipcMain.removeHandler('operations:execute');
    ipcMain.handle('operations:prepare-count',()=>({count_edit:{...plan,state:'prepared'}}));
    ipcMain.handle('operations:execute',(_,command)=>{
      if(command.method==='runtime.count_execute'){globalThis.countUiWrites++;if(globalThis.countUiLoseReply)throw Error('Simulated lost count reply');return {count_edit:{...plan,state:'uncertain'}}}
      if(command.method==='runtime.count_recover')return {count_edit:{...plan,state:'verified'}};
      throw Error('Only simulated count operations are allowed in this phase');
    });
  });
  const input=p.getByRole('spinbutton',{name:'Remaining attempts',exact:true});await input.fill('2');
  await p.getByRole('button',{name:'Review count change',exact:true}).click();
  const confirm=p.getByRole('button',{name:'Confirm count change',exact:true});await confirm.waitFor();
  check('Count review waits for explicit confirmation',await app.evaluate(()=>globalThis.countUiWrites)===0);
  await input.fill('3');check('Changed count invalidates prepared confirmation',await confirm.isDisabled());await input.fill('2');
  await confirm.click();
  await p.getByText('The result is unconfirmed. Check the previous operation before making another change.',{exact:true}).waitFor();
  check('Uncertain count result blocks a new write',await p.getByRole('button',{name:'Review count change',exact:true}).isDisabled());
  await p.getByRole('button',{name:'Check previous count change',exact:true}).click();
  await p.getByText('Remaining count changed. Save normally in the game.',{exact:true}).waitFor();
  check('Count recovery never repeats execution',await app.evaluate(()=>globalThis.countUiWrites)===1);
  check('Verified count result clears the pending review',await p.evaluate(()=>localStorage.getItem('nioh3-count-edit-review'))===null);
  await app.evaluate(()=>{globalThis.countUiLoseReply=true});
  await p.getByRole('button',{name:'Review count change',exact:true}).click();await confirm.waitFor();await confirm.click();
  await confirm.waitFor({state:'hidden'});
  check('Lost count reply retains the operation reference',await p.evaluate(()=>!!localStorage.getItem('nioh3-count-edit-review')));
  await p.getByRole('button',{name:'Check previous count change',exact:true}).click();
  await p.getByText('Remaining count changed. Save normally in the game.',{exact:true}).waitFor();
  check('Lost-reply recovery resumes the observer without another execute',await app.evaluate(()=>globalThis.countUiWrites)===2);
  assert.deepEqual(errors,[]);
  await writeFile(join(output,'verification.json'),JSON.stringify({checks,errors,measurements,scope:'Synthetic encrypted save; final count receipts are simulated. Viewport and Chromium zoom coverage, not native Windows DPI or live-write acceptance'},null,2));
}catch(error){
  const p=await app.firstWindow();
  await writeFile(join(output,'failure.txt'),String(error)+'\n'+await p.locator('body').innerText());
  throw error;
}finally{await app.close()}
