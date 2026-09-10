/** Real WebView2 + Rust broker acceptance; all writable state is isolated. */
import { chromium } from 'playwright';
import { spawn, execFileSync } from 'node:child_process';
import { mkdtemp, mkdir, writeFile, cp, realpath } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { createServer } from 'node:net';
import assert from 'node:assert/strict';
const root = await realpath(await mkdtemp(join(tmpdir(), 'nioh3-tauri-ui-')));
const output = resolve('deliverables/frontend-v2/tauri-acceptance'); await mkdir(output, {recursive:true});
const python = process.env.NIOH3_PYTHON || 'python';
execFileSync(python, ['apps/desktop/tests/fixtures/create-save.py', join(root, 'local/KoeiTecmo/NIOH3/Savedata')], {windowsHide:true});
const server = createServer(); await new Promise(r=>server.listen(0,'127.0.0.1',r));
const port=server.address().port; await new Promise(r=>server.close(r));
const child = spawn(resolve(process.env.NIOH3_TAURI_EXE || 'apps/tauri/src-tauri/target/debug/nioh3-studio.exe'), ['--user-data-dir',join(root,'profile')], {
  windowsHide:true, stdio:['ignore','pipe','pipe'], env:{...process.env,NIOH3_PYTHON:python,NIOH3_TAURI_TEST_ROOT:join(root,'profile'),NIOH3_STATE_ROOT:join(root,'state'),LOCALAPPDATA:join(root,'local'),WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port}`}
});
let stderr=''; child.stderr.on('data',b=>stderr=(stderr+b).slice(-32000));
let browser;
try {
  for(let i=0;i<200;i++) {
    if(child.exitCode!==null) throw Error(`App exited ${child.exitCode}: ${stderr}`);
    try { const r=await fetch(`http://127.0.0.1:${port}/json/version`);if(r.ok)break; }catch{}
    await new Promise(r=>setTimeout(r,300));
  }
  browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  const p=browser.contexts()[0].pages()[0];
  await p.getByText('后端已连接，请选择筛选条件。',{exact:true}).waitFor({timeout:45000});
  await p.locator('.app-version').waitFor();
  assert.equal(await p.locator('.app-version').innerText(),'v0.7.2');
  assert.equal(await p.locator('.selected-body .selected-row').count(),0,'Fresh startup has no preselected effects');
  await p.getByRole('button',{name:'特殊规则',exact:true}).click();
  await p.locator('.rule-category').filter({hasText:'一难横行'}).click();
  const familyValue=p.getByRole('combobox',{name:'一难横行统一数值',exact:true});
  await familyValue.selectOption({label:'80%'});
  const familyPicker=familyValue.locator('..');
  await familyPicker.getByRole('button',{name:'添加整类',exact:true}).click();
  assert.equal(await p.locator('.rule-chip .rule-value').inputValue(),'80%');
  await p.getByRole('button',{name:'特殊规则',exact:true}).click();
  await p.locator('.primary-button').click();
  await p.waitForFunction(async()=>((await window.nioh.currentSearch()).submitted?.query.auxiliary.required_special_rule_key_groups[0]?.length===23),null,{timeout:15000});
  await p.getByRole('button',{name:'取消',exact:true}).click();
  await p.waitForFunction(async()=>['cancelled','completed'].includes((await window.nioh.currentSearch()).job?.state),null,{timeout:15000});
  await p.locator('.rule-chip .rule-value').selectOption('any-rule-value');
  await p.locator('.primary-button').click();
  await p.waitForFunction(async()=>((await window.nioh.currentSearch()).submitted?.query.auxiliary.required_special_rule_key_groups[0]?.length===69),null,{timeout:15000});
  assert.doesNotMatch(await p.locator('.status').innerText(),/INVALID_REQUEST/);
  await p.getByRole('button',{name:'取消',exact:true}).click();
  await p.getByRole('button',{name:'清空全部',exact:true}).click();
  await p.getByRole('textbox',{name:'已知绘卷ID',exact:true}).fill('76634363');
  await p.getByRole('button',{name:'查看',exact:true}).click();await p.locator('.result-detail .scroll').waitFor();
  assert.equal(await p.evaluate(()=>typeof window.require),'undefined');
  await p.getByRole('button',{name:'收藏绘卷',exact:true}).click();await p.getByRole('button',{name:'取消收藏',exact:true}).waitFor();
  assert.equal((await p.evaluate(()=>window.review.favorites({action:'list'}))).length,1);
  await p.locator('.language-button').click();await p.locator('.side-popup').getByRole('button',{name:'日本語',exact:true}).click();
  assert.doesNotMatch(await p.locator('.result-detail .scroll').innerText(), /RUBY|\^(?:20|21|FE|FF)~/);
  await p.screenshot({path:join(output,'japanese-search.png'),timeout:15000});
  await p.reload();
  await p.locator('.nav nav button').nth(1).click();await p.locator('.inventory-list button').first().waitFor({timeout:30000});
  const privateRefused=await p.evaluate(async()=>{try{await window.operations.execute({method:'save.template',params:{}});return false}catch{return true}});
  assert.equal(privateRefused,true);
  await p.screenshot({path:join(output,'editor.png'),timeout:15000});
  await writeFile(join(output,'verification.json'),JSON.stringify({webview2:true,pythonHandshake:true,realSeed:76634363,favorites:true,isolatedInventory:true,privateMethodRefused:true,gameWrites:0},null,2));
  await p.evaluate(()=>window.review.windowAction('close'));
  console.log('TAURI_WEBVIEW2_SEARCH_FAVORITES_INVENTORY_OK');
} catch(error) {
  await writeFile(join(output,'startup-failure.json'),JSON.stringify({root,stderr,exitCode:child.exitCode,error:String(error)},null,2));
  await cp(join(root,'profile/logs'),join(output,'failed-logs'),{recursive:true}).catch(()=>{});
  if(browser) { const p=browser.contexts()[0]?.pages()[0];if(p)await writeFile(join(output,'failure.txt'),await p.locator('body').innerText().catch(()=>stderr)); }
  throw error;
} finally {
  if(browser) {const p=browser.contexts()[0]?.pages()[0];if(p)await p.evaluate(()=>window.review?.windowAction('close')).catch(()=>{});await browser.close();}
}
