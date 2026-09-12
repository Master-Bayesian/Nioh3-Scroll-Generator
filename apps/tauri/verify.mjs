/** Real WebView2 + Rust broker acceptance; all writable state is isolated. */
import { chromium } from 'playwright';
import { spawn, execFileSync } from 'node:child_process';
import { mkdtemp, mkdir, readFile, readdir, writeFile, cp, realpath } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { createServer } from 'node:net';
import { createInterface } from 'node:readline';
import assert from 'node:assert/strict';
const clipboardPrelude = 'Add-Type -AssemblyName System.Windows.Forms; $ErrorActionPreference="Stop"; ';
function readClipboardText() {
  return Buffer.from(execFileSync('powershell.exe',['-NoProfile','-STA','-Command',clipboardPrelude+'[Console]::Write([Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes([Windows.Forms.Clipboard]::GetText())))'],{windowsHide:true,encoding:'utf8',timeout:10000}),'base64').toString('utf8');
}
async function preserveClipboard() {
  // Keep every available format in one STA process, including images and files.
  const script = clipboardPrelude + `
    $source=[Windows.Forms.Clipboard]::GetDataObject()
    $snapshot=New-Object Windows.Forms.DataObject
    $count=0
    if($null -ne $source){foreach($format in $source.GetFormats($false)){
      $value=$source.GetData($format,$false)
      if($null -ne $value){
        if($value -is [IO.MemoryStream]){$value=New-Object IO.MemoryStream(,$value.ToArray())}
        elseif($value -is [Drawing.Image]){$value=$value.Clone()}
        $snapshot.SetData($format,$false,$value)
        $count++
      }
    }}
    [Console]::WriteLine('ready')
    $null=[Console]::ReadLine()
    if($count){[Windows.Forms.Clipboard]::SetDataObject($snapshot,$true)}else{[Windows.Forms.Clipboard]::Clear()}
    [Console]::WriteLine('restored')
  `;
  const helper=spawn('powershell.exe',['-NoProfile','-STA','-Command',script],{windowsHide:true,stdio:['pipe','pipe','pipe']});
  let errors='';helper.stderr.on('data',data=>errors+=data);
  const lines=createInterface({input:helper.stdout})[Symbol.asyncIterator]();
  const next=async expected=>{
    let timer;
    try {
      const result=await Promise.race([lines.next(),new Promise((_,reject)=>{timer=setTimeout(()=>reject(Error('Clipboard preservation timed out')),10000);})]);
      assert.equal(result.value,expected,`Clipboard preservation failed: ${errors}`);
    } finally {clearTimeout(timer);}
  };
  try {await next('ready');}catch(error){helper.kill();throw error;}
  return async()=>{try{helper.stdin.end('restore\n');await next('restored');}finally{helper.kill();}};
}
const root = await realpath(await mkdtemp(join(tmpdir(), 'nioh3-tauri-ui-')));
const output = resolve('deliverables/frontend-v2/tauri-acceptance'); await mkdir(output, {recursive:true});
const python = process.env.NIOH3_PYTHON || 'python';
const fixture = JSON.parse(execFileSync(python, ['apps/desktop/tests/fixtures/create-restore-save.py', root], {windowsHide:true,encoding:'utf8',timeout:45000}));
const beforeRestore = await readFile(fixture.path), expectedRestore = await readFile(fixture.backup_path);
assert.notDeepEqual(beforeRestore,expectedRestore,'Restore acceptance requires different source and backup bytes');
const expectedVersion = JSON.parse(await readFile('package.json','utf8')).version;
const server = createServer(); await new Promise(r=>server.listen(0,'127.0.0.1',r));
const port=server.address().port; await new Promise(r=>server.close(r));
// Clipboard content remains only in memory, outside acceptance artifacts.
const restoreClipboard = await preserveClipboard();
const child = spawn(resolve(process.env.NIOH3_TAURI_EXE || 'apps/tauri/src-tauri/target/debug/nioh3-studio.exe'), ['--user-data-dir',join(root,'profile')], {
  windowsHide:true, stdio:['ignore','pipe','pipe'], env:{...process.env,NIOH3_PYTHON:python,NIOH3_TAURI_TEST_ROOT:join(root,'profile'),NIOH3_TAURI_TEST_DEBUG_PORT:String(port),NIOH3_STATE_ROOT:join(root,'state'),LOCALAPPDATA:join(root,'local')}
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
  let p;
  for(let i=0;i<150;i++) {
    p=browser.contexts()[0]?.pages()[0];
    if(p) break;
    await new Promise(r=>setTimeout(r,200));
  }
  if(!p) throw Error('WebView2 debugging endpoint opened without a page target');
  await p.getByText('后端已连接，请选择筛选条件。',{exact:true}).waitFor({timeout:45000});
  await p.locator('.app-version').waitFor();
  assert.equal(await p.locator('.app-version').innerText(),`v${expectedVersion}`);
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
  const cancel=p.getByRole('button',{name:'取消',exact:true});
  if(await cancel.isEnabled()) await cancel.click({timeout:2000}).catch(()=>{});
  await p.waitForFunction(async()=>['cancelled','completed'].includes((await window.nioh.currentSearch()).job?.state),null,{timeout:15000});
  await p.locator('.rule-chip .rule-value').selectOption('any-rule-value');
  await p.locator('.primary-button').click();
  await p.waitForFunction(async()=>((await window.nioh.currentSearch()).submitted?.query.auxiliary.required_special_rule_key_groups[0]?.length===69),null,{timeout:15000});
  assert.doesNotMatch(await p.locator('.status').innerText(),/INVALID_REQUEST/);
  if(await cancel.isEnabled()) await cancel.click({timeout:2000}).catch(()=>{});
  await p.waitForFunction(async()=>['cancelled','completed'].includes((await window.nioh.currentSearch()).job?.state),null,{timeout:15000});
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
  await p.locator('.language-button').click();
  await p.locator('.side-popup').getByRole('button',{name:'简体中文',exact:true}).click();
  await p.getByRole('button',{name:'备份与管理',exact:true}).click();
  const backupRow = p.getByRole('checkbox',{name:`选择备份${fixture.backup_id}`,exact:true});
  await backupRow.waitFor({timeout:30000});
  await p.locator('.backup-page[aria-busy="false"]').waitFor();
  await backupRow.check();
  await p.getByRole('button',{name:'恢复选中备份',exact:true}).click();
  const restoreButton = p.getByRole('button',{name:'确认恢复存档',exact:true});
  await restoreButton.waitFor();
  assert.equal(await restoreButton.isDisabled(),true,'Restore requires an explicit title-screen or closed confirmation');
  assert.deepEqual(await readFile(fixture.path),beforeRestore,'Preparing restoration must leave the save unchanged');
  assert.deepEqual(await readdir(join(root,'profile/backups')),[fixture.backup_id],'Preparation must not create a restore checkpoint');
  const confirmation = p.getByRole('checkbox',{name:'游戏已回到标题界面或已关闭',exact:true});
  await confirmation.check();
  await confirmation.uncheck();
  assert.equal(await restoreButton.isDisabled(),true,'Removing confirmation disables restoration again');
  await confirmation.check();
  await restoreButton.click();
  await p.getByText('备份已恢复。',{exact:true}).waitFor({timeout:30000});
  await p.locator('.backup-page[aria-busy="false"]').waitFor();
  assert.deepEqual(await readFile(fixture.path),expectedRestore,'Restore must install the selected encrypted bytes exactly');
  assert.deepEqual(await readFile(fixture.backup_path),expectedRestore,'Restore must preserve the selected backup');
  const checkpoints = (await readdir(join(root,'profile/backups'))).filter(id=>id!==fixture.backup_id);
  assert.equal(checkpoints.length,1,'A restore must create exactly one automatic checkpoint');
  const checkpoint = join(root,'profile/backups',checkpoints[0]);
  assert.deepEqual(await readFile(join(checkpoint,'SAVEDATA.BIN')),beforeRestore,'Checkpoint must preserve the complete pre-restore generation');
  const checkpointManifest = JSON.parse(await readFile(join(checkpoint,'backup-manifest.json'),'utf8'));
  assert.equal(checkpointManifest.action,'pre-restore-checkpoint');
  const mainBackup = checkpointManifest.backup_files.find(file=>file.source_role==='main_save');
  assert.equal(mainBackup.size,beforeRestore.length);
  assert.equal(mainBackup.sha256.toLowerCase(),createHash('sha256').update(beforeRestore).digest('hex'));
  const restoreJournal = JSON.parse(await readFile(join(checkpoint,'restore-journal.json'),'utf8'));
  assert.equal(restoreJournal.state,'committed');
  assert.equal(restoreJournal.source_backup_directory,fixture.backup_id);
  await p.getByRole('button',{name:'绘卷编辑',exact:true}).click();
  await p.locator('.inventory-list button').first().waitFor();
  assert.equal(await p.locator('.inventory-list button').count(),1,'Restored inventory remains readable through the broker');
  await p.getByText('基础信息 · 长期保存',{exact:true}).click();
  await p.getByRole('textbox',{name:'编辑推荐等级（敌人等级）',exact:true}).fill('350');
  await p.getByRole('button',{name:'核对修改',exact:true}).click();
  const editButton=p.getByRole('button',{name:'确认写入存档',exact:true});
  await editButton.waitFor();
  assert.equal(await editButton.isDisabled(),true,'Permanent edits require explicit confirmation');
  assert.equal(await p.getByRole('checkbox',{name:'游戏已完全关闭',exact:true}).count(),0,'Editing must not require closing the game');
  assert.deepEqual(await readFile(fixture.path),expectedRestore,'Preparing an edit must not write the save');
  await p.getByRole('checkbox',{name:'游戏已回到标题界面或已关闭',exact:true}).check();
  await editButton.click();
  await p.getByText('修改已写入存档。',{exact:true}).waitFor({timeout:30000});
  const editedSave=await readFile(fixture.path);
  assert.notDeepEqual(editedSave,expectedRestore,'The permanent edit must change the synthetic save');
  const editBackups=(await readdir(join(root,'profile/backups'))).filter(id=>id!==fixture.backup_id&&!checkpoints.includes(id));
  assert.equal(editBackups.length,1,'The edit must create exactly one automatic backup');
  assert.deepEqual(await readFile(join(root,'profile/backups',editBackups[0],'SAVEDATA.BIN')),expectedRestore,'Edit backup must retain the complete pre-edit generation');
  await p.getByRole('button',{name:'删除选中绘卷',exact:true}).click();
  await editButton.waitFor();
  assert.equal(await editButton.isDisabled(),true,'Deletion requires a new explicit confirmation');
  assert.equal(await p.getByRole('checkbox',{name:'游戏已完全关闭',exact:true}).count(),0,'Deletion must not require closing the game');
  assert.deepEqual(await readFile(fixture.path),editedSave,'Preparing deletion must not write the save');
  await p.getByRole('checkbox',{name:'游戏已回到标题界面或已关闭',exact:true}).check();
  await editButton.click();
  await p.getByText('当前存档没有绘卷，可以先从购物车添加。',{exact:true}).waitFor({timeout:30000});
  const deletedSave=await readFile(fixture.path);
  assert.notDeepEqual(deletedSave,editedSave,'Deletion must remove the synthetic scroll');
  const deleteBackups=(await readdir(join(root,'profile/backups'))).filter(id=>id!==fixture.backup_id&&!checkpoints.includes(id)&&!editBackups.includes(id));
  assert.equal(deleteBackups.length,1,'Deletion must create exactly one automatic backup');
  assert.deepEqual(await readFile(join(root,'profile/backups',deleteBackups[0],'SAVEDATA.BIN')),editedSave,'Deletion backup must retain the complete pre-delete generation');
  const clipboardSentinel = `nioh3-automatic-log-acceptance-${Date.now()}`;
  await p.evaluate(text=>window.review.copyText(text),clipboardSentinel);
  assert.equal(readClipboardText(),clipboardSentinel,'Clipboard sentinel must reach the real Windows clipboard');
  const rejectedRequest = await p.evaluate(async()=>{
    try { await window.operations.execute({method:'save.inventory',params:{save_id:'clipboard-test-invalid-reference'}});return ''; }
    catch(error) { return String(error); }
  });
  assert.match(rejectedRequest,/INVALID_REQUEST: save\.inventory/);
  let supportClipboard='';
  const clipboardDeadline=Date.now()+15000;
  do {
    supportClipboard=readClipboardText();
    if(supportClipboard.includes('INVALID_REQUEST: save.inventory'))break;
    await new Promise(resolve=>setTimeout(resolve,200));
  }while(Date.now()<clipboardDeadline);
  assert.notEqual(supportClipboard,clipboardSentinel,'The failure must automatically replace the existing clipboard');
  assert.match(supportClipboard,/\[automatic-failure\] operations:execute/);
  assert.match(supportClipboard,/\[worker-error\].*role=save method=save\.inventory/);
  assert.match(supportClipboard,/INVALID_REQUEST: save\.inventory/);
  assert.ok(supportClipboard.includes('"version": "'+expectedVersion+'"'));
  assert.match(supportClipboard,/"workers":/);
  assert.deepEqual(await readFile(fixture.path),deletedSave,'The rejected read request must not change the save');
  await writeFile(join(output,'verification.json'),JSON.stringify({webview2:true,pythonHandshake:true,realSeed:76634363,favorites:true,isolatedInventory:true,privateMethodRefused:true,backupRestore:{confirmationGate:true,prepareReadOnly:true,exactBytes:true,automaticCheckpoint:true,committedJournal:true,inventoryReadback:true,scope:'Synthetic save only; no running-game restoration acceptance'},permanentEdit:{titleScreenConfirmation:true,prepareReadOnly:true,automaticBackup:true,scope:'Synthetic save only; no running-game editing acceptance'},scrollDelete:{titleScreenConfirmation:true,prepareReadOnly:true,automaticBackup:true,emptyInventoryReadback:true,scope:'Synthetic save only; no running-game deletion acceptance'},automaticLogClipboard:{overwritesExisting:true,workerFailureContext:true,diagnosticVersion:true,readOnlyFailure:true},gameWrites:0},null,2));
  await p.evaluate(()=>window.review.windowAction('close'));
  console.log('TAURI_WEBVIEW2_SEARCH_FAVORITES_INVENTORY_RESTORE_OK');
} catch(error) {
  await writeFile(join(output,'startup-failure.json'),JSON.stringify({root,stderr,exitCode:child.exitCode,error:String(error)},null,2));
  await cp(join(root,'profile/logs'),join(output,'failed-logs'),{recursive:true}).catch(()=>{});
  if(browser) { const p=browser.contexts()[0]?.pages()[0];if(p)await writeFile(join(output,'failure.txt'),await p.locator('body').innerText().catch(()=>stderr)); }
  throw error;
} finally {
  try {
    if(browser) {const p=browser.contexts()[0]?.pages()[0];if(p)await p.evaluate(()=>window.review?.windowAction('close')).catch(()=>{});await browser.close();}
  } finally { await restoreClipboard(); }
}
