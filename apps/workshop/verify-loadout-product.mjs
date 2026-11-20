/** Production React/SaveSession/OperationController checks with synthetic in-memory replies.
 * No native worker, game process or real save is opened. A simulated commit changes only
 * fixture objects so the renderer's undo ownership can be tested end to end.
 */
import assert from 'node:assert/strict';
import { createHash, randomUUID } from 'node:crypto';
import { createServer } from 'node:http';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { build } from 'esbuild';
import { chromium } from 'playwright';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const output = resolve(process.argv[2] ?? join(repo, 'build/loadout-product-e2e'));
await mkdir(output, { recursive: true });
const evidence = { schema: 'nioh3-loadout-product-e2e/v1', pass: false,
  environment: { os: process.platform, node: process.version, nativeWorker: false, game: false, realSaveWrites: 0 },
  checks: [], failures: [], source: {} };
for (const file of ['apps/workshop/LoadoutOptimizer.tsx', 'apps/workshop/loadout-context-state.ts', 'apps/desktop/src/save-session.ts', 'apps/desktop/src/operation-controller.ts'])
  evidence.source[file] = createHash('sha256').update(await readFile(join(repo, file))).digest('hex');
const bridge = `
window.__fixture = { calls: [], generation: 0, contexts: 0, plans: 0, searchMode: 'complete', job: null,
  cancelCalls: [], failDiscard: false, pendingSearch: null, deferPlan: false, pendingPlan: null, foreignPreview: false };
const f = window.__fixture;
f.save = { save_id: 'synthetic-save', path: 'synthetic/SAVEDATA00.BIN', account_id: '123', save_slot: 0 };
f.sha = () => String.fromCharCode(97 + f.generation).repeat(64);
const piece = (slot_index, inventory_key, worn) => ({ slot_index, item_id: 141, appearance_id: 141,
  quantity: 1, level: 180, level_before_forge: 180, plus: 0, familiarity: 0, inventory_key,
  seed: 7, rarity: 5, type_class: 24, effects: [], worn });
f.items = [piece(1, 101, true), piece(2, 102, false)];
f.bundle = { id: 'synthetic-attack', name: 'Synthetic attack', weapon: 'Sword', damage: 'First hit', school: 'samurai', weapon_class: 0,
  diagnostic: false, scene: { attack_id: 'first', stance: 'mid', kind: 'quick', buff: 'current' } };
f.result = params => ({ bundle: params.bundle, context_id: params.context_id, client_request_id: params.client_request_id,
  start_attrs: [10,10,10,10,10,10,10], best: [{ damage: 100, attrs: [10,10,10,10,10,10,10],
    items: [{ slot: 4, slot_index: 2, item_id: 141 }], facts: { load: 0.3, defense: 100, health: 1000 } }],
  space: 2, full_size: 2, searched: 2, answered: 2, no_path: 0, refused: 0, outside_limits: 0, missed: 0,
  complete: true, source_uncomputed: 0, source_model_scope: 'runtime_bound_rebuild', seconds: 0.1,
  polish_rounds: 0, backend: 'cpu', enhanced: false,
  current: { status: 'answered', damage: 80, items: [{slot:4, slot_index:1, item_id:141}], kept_slots:[], facts:{load:0.3,defense:90,health:1000} } });
f.execute = async command => {
  const { method, params } = command;
  f.calls.push(structuredClone(command));
  switch (method) {
    case 'save.discover': return { saves: [f.save] };
    case 'save.inventory': return { ...f.save, snapshot_id: 'snapshot-' + f.generation, source_sha256: f.sha(), empty_slots: 20, entries: [] };
    case 'save.operations': return { operations: [] };
    case 'save.character': return { save_id: f.save.save_id, source_sha256: f.sha(), currencies: {amrita:0,gold:0}, equipment_slots:2, equipment:structuredClone(f.items) };
    case 'save.loadout_context':
      if(f.sourcesOnly)return {state:'sources_read',save_id:params.save_id,source_sha256:params.source_sha256,
        bundle:params.bundle,source:'live',school:'samurai',level:71,attrs:[10,10,10,10,10,10,10],
        submissions:3,source_digest:'a'.repeat(64),source_state:'current_maps_matched',missing:'damage_context'};
      return { state:'ready', context_id:'context-' + ++f.contexts, save_id:params.save_id,
      source_sha256:params.source_sha256, bundle:params.bundle, captured_at:'2026-11-19T00:00:00Z', attrs:[10,10,10,10,10,10,10], level:71,
      school:'samurai', source:'live', skills:[], buffs:[], notes:[] };
    case 'save.loadout_sets': return { bundle:params.bundle, context_id:params.context_id, sets:[{key:123,item_ids:[141],count:2}], reduction_items:0 };
    case 'save.loadout_search': {
      const result = f.result(params);
      if (f.searchMode === 'partial') { result.no_path=1; result.answered=1; result.complete=false; }
      if (f.searchMode === 'source-unknown') delete result.source_uncomputed;
      if (f.searchMode === 'score-mismatch') result.best[0].damage_details = {state:'unavailable',reason:'score_mismatch'};
      if (f.searchMode === 'pending') {
        f.pendingSearch=result;
        f.job={job_id:'synthetic-job-' + f.calls.length,kind:method,state:'running',cancellable:true,sequence:1,result:null,error:null,
          progress:{schema:'nioh3-loadout-progress/v1',...params,stage:'evaluation',backend:'cpu',completed:0,total:2,batch_completed:0,batch_total:null,batch_index:0,attempt:0}};
        return structuredClone(f.job);
      }
      return result;
    }
    case 'save.prepare_character_edit': {
      const plan={ plan_id:'synthetic-plan-' + ++f.plans,save_id:params.save_id,source_sha256:params.source_sha256,
        expires_in_seconds:60,preview:{worn:[{slot_index:1,item_id:141,before:4,after:null},{slot_index:2,item_id:141,before:null,after:4}]} };
      if(f.foreignPreview){f.foreignPreview=false;plan.preview.worn[0].slot_index=99;}
      if(f.deferPlan){f.deferPlan=false;return new Promise(resolve=>{f.pendingPlan=()=>resolve(plan);});}
      return plan;
    }
    case 'save.discard': if(f.failDiscard) throw Error('SYNTHETIC_DISCARD_FAILURE'); else return {discarded:true};
    case 'save.commit': {
      f.generation++; f.items[0].worn=false; f.items[1].worn=true;
      return {operation_id:params.plan_id,save_id:f.save.save_id,commit_status:'committed'};
    }
    default: throw Error('UNEXPECTED_SYNTHETIC_COMMAND: ' + method);
  }
};
window.nioh={};
window.review={log:async()=>{}};
window.operations={execute:f.execute,current:async()=>({busy:false,job:f.job}),snapshot:async()=>structuredClone(f.job),
  cancel:async(role,id)=>{
    f.cancelCalls.push(id);
    // Deliberately late success: the renderer must retire it after a cancel intent.
    f.job={...f.job,state:'completed',sequence:2,result:f.pendingSearch};
    return structuredClone(f.job);
  }};
`;
const built = await build({ stdin: { contents: `import React from 'react'; import {createRoot} from 'react-dom/client';
import {LoadoutOptimizer} from './apps/workshop/LoadoutOptimizer';
import {saveSession} from './apps/workshop/save-workspace';
window.__session=saveSession; window.__root=createRoot(document.getElementById('root'));
window.__root.render(React.createElement(LoadoutOptimizer,{bundles:[window.__fixture.bundle,
  {...window.__fixture.bundle,id:'synthetic-other',scene:{...window.__fixture.bundle.scene,buff:'untriggered'}}]}));`,
  resolveDir: repo, sourcefile: 'loadout-product-e2e.tsx', loader: 'tsx' },
  bundle:true,write:false,format:'iife',platform:'browser',jsx:'automatic',define:{'process.env.NODE_ENV':'"production"'} });
const html = '<!doctype html><html lang="zh-CN"><meta charset="utf-8"><body><div id="root"></div><script>' + bridge + '</script><script>' + built.outputFiles[0].text.replaceAll('</script','<\\/script') + '</script></body></html>';
const server = createServer((_req,res)=>{res.writeHead(200,{'content-type':'text/html; charset=utf-8'});res.end(html);});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
let browser;
/** A DOM-only runner is useful on hosts that cannot create Chromium's IPC sockets.
 * It runs the same production ReactDOM bundle and real DOM events, without claiming
 * browser layout, WebView2, native IPC or game acceptance. jsdom is an optional tool
 * dependency, supplied explicitly as NIOH3_DOM_MODULE=/.../jsdom/lib/api.js.
 */
async function domSurface(modulePath) {
  const {JSDOM,VirtualConsole}=await import(pathToFileURL(resolve(modulePath)).href);
  const errors=[];
  const virtualConsole=new VirtualConsole();
  virtualConsole.on('jsdomError',error=>errors.forEach(listener=>listener(error)));
  let dom;
  const wait=async check=>{const deadline=Date.now()+5000;while(Date.now()<deadline){if(await check())return;await new Promise(done=>setTimeout(done,5));}throw Error('DOM wait timed out');};
  const nodes=root=>typeof root==='function'?root():[root];
  const select=(root,predicate)=>()=>nodes(root).flatMap(node=>Array.from(node.querySelectorAll('*')).filter(predicate));
  const locator=query=>({
    count:async()=>query().length,
    getAttribute:async name=>query()[0]?.getAttribute(name),
    isDisabled:async()=>query()[0]?.disabled===true,
    isEnabled:async()=>query()[0]?.disabled===false,
    innerText:async()=>query()[0]?.textContent,
    waitFor:async()=>wait(()=>query().length>0),
    click:async()=>{await wait(()=>query().length>0&&!query()[0].disabled);query()[0].click();await new Promise(done=>setTimeout(done,0));},
    check:async()=>{if(!query()[0]?.checked)query()[0].click();await new Promise(done=>setTimeout(done,0));},
    selectOption:async value=>{const element=query()[0];Object.getOwnPropertyDescriptor(dom.window.HTMLSelectElement.prototype,'value').set.call(element,value);element.dispatchEvent(new dom.window.Event('change',{bubbles:true}));await new Promise(done=>setTimeout(done,0));},
    fill:async value=>{const element=query()[0];Object.getOwnPropertyDescriptor(dom.window.HTMLInputElement.prototype,'value').set.call(element,value);element.dispatchEvent(new dom.window.Event('input',{bubbles:true}));await new Promise(done=>setTimeout(done,0));},
    getByRole:(role,options)=>byRole(query,role,options),
  });
  const byRole=(root,role,options={})=>locator(select(root,node=>{
    const matches=role==='button'?node.tagName==='BUTTON':role==='combobox'?node.tagName==='SELECT':role==='checkbox'?node.tagName==='INPUT'&&node.type==='checkbox':node.getAttribute('role')===role;
    const name=node.getAttribute('aria-label')??node.textContent.trim();
    return matches&&(!options.name||(options.exact?name===options.name:name.includes(options.name)));
  }));
  const page={
    on:(type,listener)=>{if(type==='pageerror')errors.push(listener);},
    goto:async url=>{dom=new JSDOM(html,{url,runScripts:'dangerously',pretendToBeVisual:true,virtualConsole,beforeParse(window){window.structuredClone=structuredClone;window.crypto.randomUUID=randomUUID;}});},
    waitForFunction:async fn=>wait(()=>dom.window.eval('('+fn.toString()+')()')),
    evaluate:async(fn,arg)=>structuredClone(await dom.window.eval('('+fn.toString()+')('+(arg===undefined?'undefined':JSON.stringify(arg))+')')),
    getByRole:(role,options)=>byRole(()=>[dom.window.document],role,options),
    getByTestId:id=>locator(()=>Array.from(dom.window.document.querySelectorAll('[data-testid="'+id+'"]'))),
    locator:selector=>locator(()=>Array.from(dom.window.document.querySelectorAll(selector))),
  };
  return {version:()=>`jsdom DOM-only`,newPage:async()=>page,close:async()=>dom?.window.close()};
}
try {
  browser=process.env.NIOH3_DOM_MODULE?await domSurface(process.env.NIOH3_DOM_MODULE):
    await chromium.launch({headless:true,executablePath:process.env.NIOH3_CHROMIUM || undefined,args:['--no-sandbox']});
  evidence.environment.surface=process.env.NIOH3_DOM_MODULE?'ReactDOM in jsdom (no browser)':'Chromium';
  evidence.environment.browser=browser.version();
  const page=await browser.newPage();
  const errors=[];
  page.on('pageerror',error=>errors.push(String(error)));
  const ready=()=>page.waitForFunction(()=>document.querySelector('[data-testid="loadout-context"]')?.dataset.state==='Ready');
  const checks=async(name,task)=>{try{await task();evidence.checks.push({name,pass:true});}catch(error){evidence.checks.push({name,pass:false,error:String(error)});evidence.failures.push(name);}};
  await page.goto('http://127.0.0.1:'+server.address().port);
  await ready();
  await checks('first-and-manual-refresh-use-distinct-contexts',async()=>{
    const old=await page.getByTestId('loadout-context').getAttribute('data-context-id');
    await page.getByRole('button',{name:'刷新角色状态',exact:true}).click();await ready();
    assert.notEqual(await page.getByTestId('loadout-context').getAttribute('data-context-id'),old);
  });
  await checks('source-only-refresh-retires-damage-context',async()=>{
    await page.evaluate(()=>{window.__fixture.sourcesOnly=true;});
    await page.getByRole('button',{name:'刷新角色状态',exact:true}).click();
    await page.waitForFunction(()=>document.querySelector('[data-testid="loadout-context"]')?.dataset.state==='SourcesRead');
    const panel=page.getByTestId('loadout-context');
    assert.match(await panel.innerText(),/71/);
    assert.equal(await panel.getAttribute('data-context-id'),null);
    const start=page.getByTestId('loadout-search-start');
    assert((await start.count())===0 || await start.isDisabled());
    assert.equal(await page.getByTestId('loadout-result-coverage').count(),0);
    await page.evaluate(()=>{window.__fixture.sourcesOnly=false;});
    await page.getByRole('button',{name:'刷新角色状态',exact:true}).click();await ready();
  });
  await checks('required-set-controls-search-admission',async()=>{
    assert(await page.getByTestId('loadout-search-start').isDisabled());
    await page.getByRole('combobox',{name:'添加套装条件',exact:true}).selectOption('123');
    assert(await page.getByTestId('loadout-search-start').isEnabled());
  });
  const search=async(mode)=>{await page.evaluate(m=>{window.__fixture.searchMode=m;},mode);await page.getByTestId('loadout-search-start').click();};
  for(const mode of ['partial','source-unknown']) await checks(mode+'-cannot-show-complete',async()=>{
    await search(mode);await page.getByTestId('loadout-result-coverage').waitFor();
    assert.equal(await page.getByTestId('loadout-result-coverage').getAttribute('data-coverage'),'partial');
  });
  await checks('contradictory-score-cannot-publish-complete-or-wear',async()=>{
    await search('score-mismatch');await page.getByTestId('loadout-result-coverage').waitFor();
    assert.equal(await page.getByTestId('loadout-result-coverage').getAttribute('data-coverage'),'unanswered');
    assert.equal(await page.getByRole('button',{name:'穿上这套（写入存档）',exact:true}).count(),0);
  });
  await checks('cancel-discards-late-success-and-only-own-job',async()=>{
    await search('pending');await page.getByTestId('loadout-search-cancel').click();
    await page.getByTestId('loadout-search-cancelled').waitFor();
    assert.equal(await page.getByTestId('loadout-result-coverage').count(),0);
    assert.deepEqual(await page.evaluate(()=>window.__fixture.cancelCalls),[await page.evaluate(()=>window.__fixture.job.job_id)]);
  });
  await checks('double-click-submits-exactly-one-search',async()=>{
    const before=await page.evaluate(()=>window.__fixture.calls.filter(c=>c.method==='save.loadout_search').length);
    await page.evaluate(()=>{window.__fixture.searchMode='pending';const button=document.querySelector('[data-testid="loadout-search-start"]');button.click();button.click();});
    await page.getByTestId('loadout-search-cancel').waitFor();
    assert.equal(await page.evaluate(()=>window.__fixture.calls.filter(c=>c.method==='save.loadout_search').length),before+1);
    await page.getByTestId('loadout-search-cancel').click();await page.getByTestId('loadout-search-cancelled').waitFor();
  });
  await checks('scene-change-retires-result-and-rebinds-context',async()=>{
    await search('complete');await page.getByTestId('loadout-result-coverage').waitFor();
    const before=await page.getByTestId('loadout-context').getAttribute('data-context-id');
    await page.getByRole('combobox',{name:'增益条件',exact:true}).selectOption('synthetic-other');await ready();
    assert.notEqual(await page.getByTestId('loadout-context').getAttribute('data-context-id'),before);
    assert.equal(await page.getByTestId('loadout-result-coverage').count(),0);
    const request=await page.evaluate(()=>window.__fixture.calls.filter(c=>c.method==='save.loadout_context').at(-1));
    assert.equal(request.params.bundle,'synthetic-other');
  });
  await checks('late-plan-after-input-change-is-retired',async()=>{
    await search('complete');await page.getByTestId('loadout-result-coverage').waitFor();
    await page.evaluate(()=>{window.__fixture.deferPlan=true;});
    await page.getByRole('button',{name:'穿上这套（写入存档）',exact:true}).click();
    await page.waitForFunction(()=>!!window.__fixture.pendingPlan);
    await page.locator('.loadout-field input[inputmode="decimal"]').fill('50');
    await page.evaluate(()=>window.__fixture.pendingPlan());
    await page.waitForFunction(()=>window.__session.getSnapshot().plan===null&&!window.__session.getSnapshot().busy);
    assert.equal(await page.locator('.character-save-plan').count(),0);
    assert(await page.getByTestId('loadout-search-start').isEnabled());
  });
  await checks('failed-discard-retains-owner-and-retry-clears-it',async()=>{
    await search('complete');await page.getByTestId('loadout-result-coverage').waitFor();
    await page.getByRole('button',{name:'穿上这套（写入存档）',exact:true}).click();await page.locator('.character-save-plan').waitFor();
    const reviewed=await page.evaluate(()=>window.__session.getSnapshot().plan.plan_id);
    await page.evaluate(()=>{window.__fixture.failDiscard=true;});
    await page.getByRole('button',{name:'放弃',exact:true}).click();await page.getByTestId('loadout-plan-retirement-failed').waitFor();
    assert(await page.getByTestId('loadout-search-start').isDisabled());
    assert.equal(await page.evaluate(()=>window.__session.getSnapshot().plan.plan_id),reviewed);
    await page.evaluate(()=>{window.__fixture.failDiscard=false;});
    await page.getByRole('button',{name:'重试撤销换装计划',exact:true}).click();
    await page.waitForFunction(()=>window.__session.getSnapshot().plan===null&&!window.__session.getSnapshot().busy);
    assert.equal(await page.locator('.character-save-plan').count(),0);
    assert(await page.getByTestId('loadout-search-start').isEnabled());
  });
  await checks('invalid-preview-after-claim-retires-exact-plan',async()=>{
    await search('complete');await page.getByTestId('loadout-result-coverage').waitFor();
    await page.evaluate(()=>{window.__fixture.foreignPreview=true;});
    await page.getByRole('button',{name:'穿上这套（写入存档）',exact:true}).click();
    await page.waitForFunction(()=>window.__session.getSnapshot().plan===null&&!window.__session.getSnapshot().busy);
    assert.equal(await page.locator('.character-save-plan').count(),0);
    assert(await page.getByTestId('loadout-search-start').isEnabled());
  });
  await checks('preview-and-synthetic-commit-produce-undo',async()=>{
    await search('complete');await page.getByTestId('loadout-result-coverage').waitFor();
    await page.getByRole('button',{name:'穿上这套（写入存档）',exact:true}).click();
    const plan=page.locator('.character-save-plan');await plan.waitFor();
    assert(await plan.getByRole('button',{name:'写入存档',exact:true}).isDisabled());
    await plan.getByRole('checkbox').check();await plan.getByRole('button',{name:'写入存档',exact:true}).click();
    await ready();await page.getByRole('button',{name:'换回原来的装备',exact:true}).waitFor();
    assert.equal(await page.evaluate(()=>window.__fixture.calls.filter(c=>c.method==='save.commit').length),1);
  });
  await checks('same-item-id-replacement-cannot-restore-as-original',async()=>{
    const before=await page.evaluate(()=>window.__fixture.plans);
    await page.evaluate(()=>{window.__fixture.items[0].inventory_key+=1000;window.__fixture.generation++;});
    await page.getByRole('button',{name:'刷新角色状态',exact:true}).click();await ready();
    await page.getByRole('button',{name:'换回原来的装备',exact:true}).click();
    await page.waitForFunction(()=>document.querySelector('.character-save-plan') || document.querySelector('[role="alert"]'));
    assert.equal(await page.evaluate(()=>window.__fixture.plans),before,'Replacement item with same item_id must not be passed to save.prepare_character_edit');
  });
  await checks('unmount-cancels-owned-search-and-never-publishes-late-success',async()=>{
    await search('pending');await page.getByTestId('loadout-search-cancel').waitFor();
    const id=await page.evaluate(()=>window.__fixture.job.job_id);
    await page.evaluate(()=>window.__root.unmount());
    await page.waitForFunction(()=>window.__fixture.job.state==='completed');
    assert((await page.evaluate(()=>window.__fixture.cancelCalls)).includes(id));
    assert.equal(await page.getByTestId('loadout-result-coverage').count(),0);
  });
  evidence.calls=await page.evaluate(()=>window.__fixture.calls);
  evidence.domText=await page.locator('#root').innerText();
  evidence.pageErrors=errors;
  assert.equal(errors.length,0);
  evidence.pass=evidence.failures.length===0;
} catch(error){evidence.failures.push(String(error));}
finally{if(browser)await browser.close();await new Promise(done=>server.close(done));await writeFile(join(output,'product-ui.json'),JSON.stringify(evidence,null,2)+'\n');}
console.log(JSON.stringify({pass:evidence.pass,checks:evidence.checks.length,failures:evidence.failures,evidence:join(output,'product-ui.json')}));
if(!evidence.pass)process.exitCode=1;
