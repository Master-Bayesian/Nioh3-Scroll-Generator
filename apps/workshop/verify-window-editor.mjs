import {_electron as electron} from 'playwright';
import {execFileSync} from 'node:child_process';
import {resolve,join} from 'node:path';import {mkdtemp,writeFile} from 'node:fs/promises';import {tmpdir} from 'node:os';import assert from 'node:assert/strict';
const root=await mkdtemp(join(tmpdir(),'nioh3-window-'));execFileSync((process.env.NIOH3_PYTHON||'python'),[resolve('apps/desktop/tests/fixtures/create-save.py'),join(root,'local','KoeiTecmo','NIOH3','Savedata')],{windowsHide:true});const app=await electron.launch({args:[resolve('apps/desktop/dist/main.cjs')],env:{...process.env,NIOH3_REVIEW_UI:'1',NIOH3_ELECTRON_TEST:'1',LOCALAPPDATA:join(root,'local'),NIOH3_STATE_ROOT:join(root,'state'),NIOH3_PYTHON:(process.env.NIOH3_PYTHON||'python')}});const checks=[];
try{const p=await app.firstWindow();p.on('pageerror',e=>console.error('PAGE',e.message));p.on('console',e=>{if(e.type()==='error')console.error(e.text())});await p.getByText('后端已连接，请选择筛选条件。',{exact:true}).waitFor();
assert.equal(await p.locator('.title-controls button').count(),3);checks.push('Three custom window controls');
for(const width of [1366,1600,2560]){
 await app.evaluate(({BrowserWindow},w)=>BrowserWindow.getAllWindows()[0].setContentSize(w,1000),width);
 await p.getByRole('button',{name:'绘卷编辑',exact:true}).click();await p.screenshot({path:'deliverables/frontend-v2/search-ui-demo-v2/editor-debug.png'});
 assert.ok(await p.locator('.editor-page').evaluate(e=>e.scrollWidth<=e.clientWidth+1));
 assert.ok(await p.locator('.inventory-pane').evaluate(e=>e.scrollWidth<=e.clientWidth+1));
 const select=p.locator('.inventory-pane select[aria-label="自动检测的存档"]');assert.ok(await select.evaluate(e=>e.getBoundingClientRect().right<=e.closest('.inventory-pane').getBoundingClientRect().right));
 checks.push('Editor and save selector fit '+width);
 await p.screenshot({path:`deliverables/frontend-v2/search-ui-demo-v2/editor-fixed-${width}.png`});
}
await p.getByText('副本内容 · 临时修改',{exact:true}).click();await p.getByRole('button',{name:'按当前种子预览副本',exact:true}).click();await p.getByText('已预览当前种子的副本内容。',{exact:true}).waitFor();const groups=p.getByRole('combobox',{name:'临时敌人组数',exact:true});const originalCount=await groups.inputValue();await groups.selectOption('1');assert.equal(await p.locator('select[aria-label^="临时修改敌人"]').count(),1);await p.getByRole('button',{name:'撤销',exact:true}).click();assert.equal(await groups.inputValue(),originalCount);await p.getByRole('checkbox',{name:'修改敌人',exact:true}).uncheck();assert.ok(await groups.isDisabled());await p.getByRole('checkbox',{name:'修改地形',exact:true}).uncheck();assert.ok(await p.getByRole('combobox',{name:'临时修改地形影响',exact:true}).isDisabled());checks.push('Temporary draft preview, group count undo, and independent override switches');
await p.getByRole('button',{name:'绘卷搜索',exact:true}).click();await p.locator('.topbar h1').hover();await p.waitForTimeout(350);
const before=await p.locator('.selected-row').first().boundingBox();await p.locator('.selection>header h2').hover();await p.locator('.selection-expanded').waitFor();const after=await p.locator('.selected-row').first().boundingBox();assert.ok(Math.abs(before.x-after.x)<1&&Math.abs(before.y-after.y)<1,JSON.stringify({before,after}));checks.push('Hover leaves chip position unchanged');
await p.evaluate(()=>window.review.windowAction('maximize'));assert.ok(await app.evaluate(({BrowserWindow})=>BrowserWindow.getAllWindows()[0].isMaximized()));await p.evaluate(()=>window.review.windowAction('maximize'));assert.ok(!await app.evaluate(({BrowserWindow})=>BrowserWindow.getAllWindows()[0].isMaximized()));checks.push('Custom maximize and restore');
await writeFile('deliverables/frontend-v2/search-ui-demo-v2/window-editor-verification.json',JSON.stringify({checks},null,2));console.log(checks);
}finally{await app.close()}
