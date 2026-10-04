/** Production prediction component E2E; read-only recorded host responses. */
import assert from 'node:assert/strict';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {createServer} from 'node:http';
import {resolve,join} from 'node:path';
import {build} from 'esbuild';
import {chromium} from 'playwright';

const root=resolve(import.meta.dirname,'../..');
const output='D:/Nioh3_v080_deliverables/deliverables/codex-v083-missing-features-20260930/ui';
// Recorded host replies live in the repository so CI can replay them.
const fixture=JSON.parse(await readFile(join(root,'test_fixtures/scroll-completion-exchanges.json'),'utf8'));
const responses=fixture.exchanges.filter(x=>x.request.method==='runtime.scroll_completion_predict'&&x.response.ok);
const pre=responses[0].request.params.record_hex;
const fresh=responses.find(x=>x.response.result.completion_prediction.seed===47878870).request.params.record_hex;
await mkdir(output,{recursive:true});
const built=await build({stdin:{contents:`
import React,{useState} from 'react';
import {createRoot} from 'react-dom/client';
import {ScrollCompletion} from './ScrollCompletion';
import './style.css';
function Harness(){const [record,setRecord]=useState(window.__pre),[dirty,setDirty]=useState(false);
return <><button id="switch" onClick={()=>setRecord(window.__fresh)}>Switch</button>
<button id="dirty" onClick={()=>setDirty(!dirty)}>Dirty</button>
<ScrollCompletion recordHex={record} identity={record} disabled={dirty}/></>}
createRoot(document.getElementById('root')).render(<Harness/>);
`,resolveDir:join(root,'apps/workshop'),loader:'tsx'},bundle:true,write:false,outdir:'out',format:'iife',platform:'browser',jsx:'transform',jsxFactory:'localizedElement',tsconfigRaw:{compilerOptions:{jsx:'react',jsxFactory:'localizedElement'}},inject:[join(root,'apps/workshop/presentation-jsx.ts')],define:{'process.env.NODE_ENV':'"production"'}});
const js=built.outputFiles.find(x=>x.path.endsWith('.js')).text;
const css=built.outputFiles.find(x=>x.path.endsWith('.css')).text;
const html=`<html><head><meta charset="utf-8"><style>${css}</style></head><body><div id="root"></div><script>
window.__pre=${JSON.stringify(pre)};window.__fresh=${JSON.stringify(fresh)};
window.__responses=${JSON.stringify(responses)};window.__calls=[];window.__defer=false;
window.nioh={handshake:async()=>({context:{context_digest:${JSON.stringify(responses[0].request.params.context_digest)}}})};
window.operations={execute:async command=>{window.__calls.push(command);if(command.method!=='runtime.scroll_completion_predict')throw Error('WRITE_FORBIDDEN');
const found=window.__responses.find(x=>x.request.params.record_hex===command.params.record_hex);if(!found)throw Error('NO_RECORDED_RESULT');
if(window.__defer){window.__defer=false;await new Promise(resolve=>window.__release=resolve)}return structuredClone(found.response.result)}};
</script><script>${js.replaceAll('</script','<\\/script')}</script></body></html>`;
const server=createServer((_,res)=>res.end(html));
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const profile='D:/Nioh3_v080_deliverables/tmp/completion-ui-'+process.pid;
const browser=await chromium.launchPersistentContext(profile,{headless:true,viewport:{width:1280,height:900}});
const checks=[],screenshots=[];
try {
 for(const locale of ['zh-CN','en-US','ja-JP']) {
  const page=await browser.newPage();
  page.on('pageerror',error=>console.error('Browser:',error.message));
  await page.addInitScript(locale=>localStorage.setItem('nioh3-ui-locale',locale),locale);
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  assert.equal(await page.locator('.scroll-completion>summary').innerText(),{'zh-CN':'洗词条与添画预测','en-US':'Reroll & extra-painting prediction','ja-JP':'特殊効果の入れ替え・添画予測'}[locale]);
  await page.locator('.scroll-completion summary').click();
  await page.locator('[data-action=predict]').click();
  await page.locator('.completion-results').waitFor();
  assert.equal(await page.locator('.completion-choices button').count(),4);
  assert.ok((await page.locator('.completion-results').innerText()).includes('150'));
  await page.locator('.completion-choices button').nth(1).click();
  await page.locator('[data-action=next]').click();
  await page.locator('.completion-round[data-round="2"]').waitFor();
  assert.ok((await page.locator('.completion-painting').innerText()).includes('66'));
  await page.locator('.completion-painting summary').click();
  assert.ok((await page.locator('.completion-painting').innerText()).includes('3388'));
  await page.screenshot({path:join(output,locale+'.png'),fullPage:true});screenshots.push(locale+'.png');
  await page.locator('#dirty').click();
  await page.waitForFunction(()=>!document.querySelector('.completion-results'));
  assert.equal(await page.locator('[data-action=predict]').isDisabled(),true);
  await page.locator('#dirty').click();
  await page.evaluate(()=>window.__defer=true);
  await page.locator('[data-action=predict]').click();
  await page.waitForFunction(()=>!!window.__release);
  await page.locator('#switch').click();
  await page.evaluate(()=>window.__release());
  await page.waitForTimeout(50);
  assert.equal(await page.locator('.completion-results').count(),0);
  await page.locator('[data-action=predict]').click();
  await page.locator('.completion-results').waitFor();
  assert.ok((await page.locator('.completion-results').innerText()).includes('57'));
  await page.locator('.completion-painting summary').click();
  assert.ok((await page.locator('.completion-results').innerText()).includes('416'));
  assert.equal(await page.evaluate(()=>window.__calls.every(c=>c.method==='runtime.scroll_completion_predict')),true);
  checks.push({locale,choices:4,branchRound:true,painting:true,dirtyDisabled:true,staleIgnored:true,readOnly:true});
  await page.close();
 }
 await writeFile(join(output,'completion-ui-e2e.json'),JSON.stringify({pass:true,boundary:'production component; Chromium; recorded Rust host replies; no game/save writes',viewport:{width:1280,height:900},checks,screenshots},null,2));
 console.log('Completion UI E2E passed in three locales');
} finally {await browser.close();server.close();}
