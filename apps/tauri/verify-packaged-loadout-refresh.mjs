/** Read a selected save through the packaged broker and observe the real refresh reply. */
import {selectLoadoutScene} from './loadout-scene-control.mjs';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdir, copyFile, readFile, writeFile, mkdtemp} from 'node:fs/promises';
import {basename, dirname, join, resolve} from 'node:path';
import {chromium} from 'playwright';
import {closeSession, digest, inspectOnefile, isolatedEnvironment, pause} from './onefile-acceptance.mjs';

const [exe, saveFile, gameIdentity, template, directory, mode] = process.argv.slice(2);
assert(exe && saveFile && gameIdentity && template && directory && ['offline','live'].includes(mode));
const outer = resolve(exe);
const input = resolve(saveFile);
const identity = JSON.parse(await readFile(gameIdentity, 'utf8'));
assert.equal(digest(await readFile(identity.path)), identity.sha256.toLowerCase());
await mkdir(directory, {recursive:true});
const root = await mkdtemp(join(resolve(directory), mode + '-'));
const {env, profile, port} = await isolatedEnvironment(root);
for (const key of Object.keys(env)) if (/^NIOH3_(RUST|E2E|LOADOUT|TAURI_PACKAGE)/i.test(key)) delete env[key];
const contextDir = join(profile, 'loadout-contexts', 'kusarigama-strong');
await mkdir(contextDir, {recursive:true});
for (const name of ['context.json','inventory.bin','frame.bin','image.bin','image_known.bin','scene_entry.bin','profile.json']) {
  await copyFile(join(template,name), join(contextDir,name));
}
await writeFile(join(profile,'game-install.json'), JSON.stringify({schema:'nioh3-game-install/v1', executable:identity.path}));
const target = join(env.LOCALAPPDATA,'KoeiTecmo','NIOH3','Savedata',basename(dirname(dirname(input))),basename(dirname(input)),'SAVEDATA.BIN');
await mkdir(dirname(target),{recursive:true});
const raw = await readFile(input);
await writeFile(target,raw);
const {sha256,payloadSha256} = await inspectOnefile(outer);
const report = {passed:false, mode, sha256, payloadSha256, sourceSha:digest(raw)};
const child = spawn(outer,['--user-data-dir',profile],{env,cwd:root,windowsHide:true,stdio:'ignore'});
let browser,page;
try {
  const endpoint = 'http://127.0.0.1:' + port;
  const end = Date.now() + 45000;
  while (Date.now() < end) {
    assert.equal(child.exitCode,null);
    try { if ((await fetch(endpoint+'/json/version')).ok) break; } catch { /* startup */ }
    await pause(200);
  }
  browser = await chromium.connectOverCDP(endpoint,{timeout:10000});
  page = browser.contexts()[0].pages()[0];
  await page.locator('.shell').waitFor();
  await page.evaluate(() => {
    const original = window.operations.snapshot.bind(window.operations);
    window.operations.snapshot = async (role,id) => {
      const job = await original(role,id);
      if(job.kind === 'save.loadout_context' && ['completed','failed'].includes(job.state)) window.__refreshJob = job;
      return job;
    };
  });
  await page.getByRole('button',{name:'配装优化',exact:true}).click();
  await selectLoadoutScene(page, JSON.parse(await readFile(new URL('../../data/loadout/kusarigama-strong/bundle.json', import.meta.url), 'utf8')));
  await page.waitForFunction(() => !!window.__refreshJob,null,{timeout:60000});
  const first = await page.evaluate(() => window.__refreshJob);
  report.firstRead = {id:first.job_id, state:first.state, reason:first.result?.reason, source:first.result?.source};
  assert.equal(first.state,'completed',JSON.stringify(first.error));
  await page.getByRole('button',{name:'刷新角色状态',exact:true}).click();
  await page.waitForFunction(previous => window.__refreshJob?.job_id !== previous, first.job_id, {timeout:60000});
  const job = await page.evaluate(() => window.__refreshJob);
  assert.notEqual(job.job_id,first.job_id);
  report.refreshJobId = job.job_id;
  report.job = {state:job.state,error:job.error,source:job.result?.source,reason:job.result?.reason,
    contextState:job.result?.state,bundle:job.result?.bundle,level:job.result?.level};
  assert.equal(job.state,'completed',JSON.stringify(job.error));
  if(mode === 'offline') {
    assert.equal(job.result.reason,'LOADOUT_CONTEXT_CHARACTER_NOT_LOADED');
    await page.locator('[data-testid="loadout-context"][data-state="Failed"]').waitFor();
  } else {
    assert.equal(job.result.state,'ready');
    assert.equal(job.result.source,'live');
    assert.equal(job.result.source_sha256.toLowerCase(),report.sourceSha);
    assert.equal(job.result.bundle,'kusarigama-strong');
    await page.locator('[data-testid="loadout-context"][data-state="Ready"]').waitFor();
  }
  const diagnostics = await page.evaluate(()=>window.support.diagnostics());
  assert.equal(diagnostics.packageVerification.ok,true);
  report.commit = diagnostics.packageVerification.sourceCommit;
  await page.screenshot({path:join(root,'refresh.png')});
  assert.equal(digest(await readFile(target)),report.sourceSha);
  report.passed = true;
} catch(error) {
  report.error=String(error);
  report.screen=await page?.locator('body').innerText().catch(()=>null);
  throw error;
} finally {
  await writeFile(join(root,'result.json'),JSON.stringify(report,null,2));
  if(browser && page) await closeSession({browser,page},child); else if(child.exitCode===null) child.kill();
}
console.log(JSON.stringify({passed:true, mode, output:root, job:report.job}));
