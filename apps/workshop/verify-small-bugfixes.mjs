/** Production UI E2E: real read-only previews and a scripted recovery layout.
 * No game process or user save is opened. All mutation routes are forbidden.
 */
import assert from 'node:assert/strict';
import {build} from 'esbuild';
import {chromium} from 'playwright';
import {spawn} from 'node:child_process';
import {createServer} from 'node:http';
import {mkdir,writeFile,readFile} from 'node:fs/promises';
import {join,resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {withDarkTheme,arcTone} from './dark-theme.mjs';
import {addKnownEnemyVariantsToCart,verifyCollectionLayout} from './verify-collection-layout.mjs';

const root=resolve('.');
const output=resolve(process.env.NIOH3_BUGFIX_EVIDENCE||'D:/Nioh3_v080_deliverables/deliverables/bugfix-v0810/ui');
const target=process.env.CARGO_TARGET_DIR||'D:/Nioh3_v080_deliverables/build-cache/tauri-target';
await mkdir(output,{recursive:true});
const calls=[];
class Core {
  constructor(){
    this.serial=0;this.pending=new Map();this.buffer=Buffer.alloc(0);this.stderr='';
    this.child=spawn(join(target,'release/nioh3-readonly-worker.exe'),['--packaged-worker','--game-file-version','2.0.2.0','--data-root',join(root,'data'),'--contract-dir',join(root,'packages/contracts'),'--accelerator',join(root,'bin/nioh3_seed_accelerator.dll')],{windowsHide:true,stdio:['pipe','pipe','pipe']});
    this.child.stderr.on('data',data=>{this.stderr+=data.toString();});
    this.child.stdout.on('data',data=>{
      this.buffer=Buffer.concat([this.buffer,data]);
      while(this.buffer.length>=4){const length=this.buffer.readUInt32LE(0);if(this.buffer.length<length+4)break;const reply=JSON.parse(this.buffer.subarray(4,length+4));this.buffer=this.buffer.subarray(length+4);const owner=this.pending.get(reply.id);if(!owner)continue;this.pending.delete(reply.id);clearTimeout(owner.timer);if(reply.ok)owner.resolve(reply.result);else owner.reject(Error(JSON.stringify(reply.error)));}
    });
  }
  request(method,params={}){return new Promise((resolve,reject)=>{
    const id='bugfix-'+(++this.serial);const body=Buffer.from(JSON.stringify({protocol:1,id,method,params}));const prefix=Buffer.alloc(4);prefix.writeUInt32LE(body.length);const timer=setTimeout(()=>reject(Error('Worker timeout: '+method)),30000);this.pending.set(id,{resolve,reject,timer});this.child.stdin.write(Buffer.concat([prefix,body]));
  });}
  async close(){await this.request('shutdown').catch(()=>{});this.child.stdin.end();if(this.child.exitCode===null)this.child.kill();}
}
const core=new Core();
// The broker performs this before catalogs or previews; retain that contract.
const workerIdentity=await core.request("handshake");
const bundle=await build({entryPoints:['apps/workshop/main.tsx'],bundle:true,write:false,outdir:'out',format:'iife',platform:'browser',minify:true,jsx:'transform',jsxFactory:'localizedElement',tsconfigRaw:{compilerOptions:{jsx:'react',jsxFactory:'localizedElement'}},inject:['apps/workshop/presentation-jsx.ts'],loader:{'.woff2':'dataurl'},define:{'process.env.NODE_ENV':'"production"'}});
const js=bundle.outputFiles.find(file=>file.path.endsWith('.js')).text.replaceAll('</script','<\\/script');
const css=withDarkTheme(bundle.outputFiles.find(file=>file.path.endsWith('.css')).text,arcTone);
const html='<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><style>'+css+'</style></head><body><div id="root"></div><script>'+js+'</script></body></html>';
const server=createServer(async(request,response)=>{
  if(request.url!=='/rpc'){response.setHeader('content-type','text/html;charset=utf-8');response.end(html);return;}
  try{
    let body='';for await(const chunk of request)body+=chunk;const {method,params}=JSON.parse(body);calls.push({method,params});
    let result;
    if(method==='preview'){
      result=await core.request('candidate.preview',{seed:params.seed,rarity:params.rarity,level:params.level,playthrough:params.playthrough??3});
      if(params.retain)result.reference_id=result.candidate.candidate_id;
    }else result=await core.request(method,params);
    response.setHeader('content-type','application/json');response.end(JSON.stringify({result}));
  }catch(error){response.statusCode=500;response.end(JSON.stringify({error:String(error)}));}
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const origin='http://127.0.0.1:'+server.address().port;

/** Install only a read-only desktop boundary before the production bundle. */
function bridge({origin,recovery=false,theme='light'}){
  localStorage.setItem('nioh3-ui-locale','zh-CN');localStorage.setItem('nioh3-theme',theme);
  const state=window.__bugfix={previews:[],operations:[],writes:[],serial:0,hold:false,held:false};
  const rpc=async(method,params={})=>{const response=await fetch(origin+'/rpc',{method:'POST',body:JSON.stringify({method,params})});const body=await response.json();if(!response.ok)throw Error(body.error);return body.result;};
  const save={save_id:'a'.repeat(64),account_id:'123',path:'D:/owned-ui-fixture/SAVEDATA00/SAVEDATA.BIN',save_slot:0,summary:{playthrough:3,character_level:450,play_time_seconds:3600,saved_at_unix:1794740000}};
  const inventory={save_id:save.save_id,account_id:save.account_id,snapshot_id:'b'.repeat(32),source_sha256:'c'.repeat(64),empty_slots:400,entries:[]};
  const unresolved={operation_id:'d'.repeat(32),save_id:save.save_id,commit_status:'unknown',warning:null,details:{}};
  const completed=(kind,result)=>({job_id:(++state.serial).toString(16).padStart(32,'0'),kind,state:'completed',sequence:1,cancellable:false,progress:null,result,error:null});
  const forbidden=async value=>{state.writes.push(value);throw Error('WRITE_FORBIDDEN_UI_FIXTURE');};
  window.nioh={handshake:()=>rpc('handshake'),searchCatalog:(rarity,locale)=>rpc('search.catalog',{playthrough:3,rarity,locale}),resolveRecommendedLevel:value=>rpc('recommended_level.resolve',{displayed_level:value}),checkFeasibility:query=>rpc('search.feasibility',{query}),startSearch:params=>rpc('search.start',params),currentSearch:()=>rpc('job.current'),snapshot:job_id=>rpc('job.snapshot',{job_id}),cancelSearch:job_id=>rpc('job.cancel',{job_id}),restartWorker:()=>rpc('handshake')};
  window.operations={execute:async command=>{
    state.operations.push(command);
    switch(command.method){
      case 'save.discover':return completed(command.method,{saves:recovery?[save]:[]});
      case 'save.inventory':return completed(command.method,inventory);
      case 'save.operations':return completed(command.method,{operations:recovery?[unresolved]:[]});
      case 'save.backups':return completed(command.method,{backups:[]});
      case 'save.discard':return completed(command.method,{discarded:true});
      case 'runtime.compatibility':
        if(command.params.action!=='inspect')return forbidden(command);
        return {compatibility:{present:true,reference_match:true,warning:false,accepted:false,plan:null,backup:null,differences:[],hard_blocks:[],game_version:'2.0.2.0',executable:'D:/owned-ui-fixture/Nioh3.exe'}};
      default:return forbidden(command);
    }
  },current:async()=>({job:null,busy:false}),snapshot:forbidden,cancel:forbidden,selectSave:async()=>save,prepareInstall:forbidden,prepareLiveAdd:forbidden,generate:forbidden,searchNative:forbidden};
  window.preferences={getLocale:async()=>'zh-CN',setLocale:async locale=>locale};
  window.support={diagnostics:async()=>({version:'0.8.10-ui-e2e'}),exportDiagnostics:async()=>null};
  window.review={favorites:async()=>[],release:async()=>null,retain:forbidden,update:async()=>({phase:'current',canApply:true}),windowAction:async()=>null,log:async()=>null,copyLog:async()=>'',copyText:async()=>null,openLink:async()=>null,dataDirectory:async()=>({path:'D:/owned-ui-fixture'}),gameInstallation:async()=>({source:'selected',executable:'D:/owned-ui-fixture/Nioh3.exe',file_version:'2.0.2.0',restart_required:false,identity_error:null,compatibility:{status:'known',display_version:'PC v2.02',data_version:'PC v2.02',runtime_profile:'pc_v2_02',resource_directory:'r4_finalizer/pc_v2_02/resource_v1',features:{offline_scroll_generation:'supported',native_scroll_add:'supported',character_read_edit:'supported',native_equipment_add:'supported'},reason:'Read-only UI fixture'}}),preview:async params=>{
    state.previews.push(structuredClone(params));const result=await rpc('preview',params);
    if(state.hold){state.hold=false;state.held=true;await new Promise(resolve=>{state.release=resolve;});state.held=false;}
    return result;
  },auxiliary:forbidden,prepareCart:forbidden,exportFeedback:async()=>null,openSaveFolder:async()=>null,openBackupFolder:async()=>null};
}
const report={schema:'nioh3-small-bugfix-ui-e2e/v1',workerIdentity,workerSha256:createHash('sha256').update(await readFile(join(target,'release/nioh3-readonly-worker.exe'))).digest('hex'),bundleSha256:createHash('sha256').update(js+css).digest('hex'),pass:false,scope:'Production React/CSS; real packaged-mode current-resource read-only worker; scripted save recovery; no native window/game/user-save acceptance',checks:[],screenshots:[],source:{},requests:[],failures:[]};
const check=(name,pass,details=null)=>{report.checks.push({name,pass:!!pass,details});assert(pass,name+': '+JSON.stringify(details));};
let browser;
let activePage;
try{
 browser=await chromium.launch({headless:true});
 const context=await browser.newContext({viewport:{width:1440,height:1000},deviceScaleFactor:1});
 await context.addInitScript(bridge,{origin});const page=await context.newPage();activePage=page;await context.tracing.start({screenshots:true,snapshots:true,sources:true});
 const errors=[];page.on('pageerror',error=>errors.push(String(error)));
 await page.goto(origin);await page.getByRole('button',{name:/开始搜索/}).waitFor();
 const input=page.getByRole('textbox',{name:'已知绘卷ID',exact:true});const view=page.getByRole('button',{name:'查看',exact:true});
 const current=()=>page.waitForFunction(()=>document.querySelector('.direct-seed')?.dataset.previewState==='current');
 const stale=()=>page.waitForFunction(()=>document.querySelector('.direct-seed')?.dataset.previewState==='stale');
 const add=page.locator('.install-current');
 await input.fill('10030609');await view.click();await current();
 check('known-ID action exposes effective generation and output values',(await page.locator('.known-id-context').innerText()).includes('180')&&(await page.locator('.known-id-context').innerText()).includes('350'));
 const before=await page.evaluate(()=>window.__bugfix.previews.length);
 await page.getByRole('spinbutton',{name:'候选数量',exact:true}).fill('1');await current();
 check('search-only candidate count does not invalidate or regenerate known ID',await page.evaluate(count=>window.__bugfix.previews.length===count,before));
 for(const [label,value]of [['绘卷等级','170'],['推荐等级','300'],['转手次数','2']]){
   await page.getByRole('spinbutton',{name:label,exact:true}).fill(value);await stale();
   check(label+' invalidates addition until a new preview',await add.isDisabled()&&await page.locator('.cart-toggle').isDisabled());
   await view.click();await current();check(label+' is captured with the next preview',await add.isEnabled());
 }
 await input.fill('10030610');await stale();check('changing only ID invalidates previous preview',await add.isDisabled());await view.click();await current();
 await page.getByRole('combobox',{name:'稀有度',exact:true}).selectOption('3');check('rarity change cannot use old preview',await add.isDisabled());await view.click();await current();
 check('real preview receives changed rarity',await page.evaluate(()=>window.__bugfix.previews.at(-1).rarity===3));
 await page.getByRole('combobox',{name:'周目',exact:true}).selectOption('2');check('playthrough change cannot use old preview',await add.isDisabled());await view.click();await current();
 check('real preview receives changed playthrough',await page.evaluate(()=>window.__bugfix.previews.at(-1).playthrough===2));
 await page.getByRole('spinbutton',{name:'绘卷等级',exact:true}).fill('180');
 await page.evaluate(()=>{window.__bugfix.hold=true;});await view.click();await page.waitForFunction(()=>window.__bugfix.held);
 await page.getByRole('spinbutton',{name:'绘卷等级',exact:true}).fill('160');check('in-flight preview cannot be used',await add.isDisabled());
 await page.evaluate(()=>window.__bugfix.release());await stale();
 check('late response keeps requested values instead of rebinding the form',await page.evaluate(()=>window.__bugfix.previews.at(-1).level===180));
 const callsBefore=await page.evaluate(()=>window.__bugfix.previews.length);
 await add.evaluate(button=>{button.disabled=false;button.click();});
 check('handler independently refuses stale addition',await page.locator('dialog[open]').count()===0&&await page.evaluate(value=>window.__bugfix.previews.length===value,callsBefore));
 await view.click();await current();
 await input.fill('');await view.click();check('empty ID is refused without becoming seed zero',await page.evaluate(value=>window.__bugfix.previews.length===value,callsBefore+1));
 await input.fill('10030610');await current();
 report.knownRequests=await page.evaluate(()=>window.__bugfix.previews);
 const languages=[['English','en-US'],['日本語','ja-JP'],['简体中文','zh-CN']];
 const language=async(page,label,lang)=>{if(await page.evaluate(()=>document.documentElement.lang)===lang)return;await page.locator('.language-button').click();await page.locator('.side-popup').getByRole('button',{name:label,exact:true}).click();await page.waitForFunction(lang=>document.documentElement.lang===lang,lang);};
 for(const theme of ['light','dark'])for(const[label,lang]of languages){
   await language(page,label,lang);await page.evaluate(theme=>{document.documentElement.dataset.theme=theme;},theme);
   check('known-ID '+theme+' '+lang+' has no horizontal overflow',await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
   const file='known-id-'+theme+'-'+lang+'.png';await page.screenshot({path:join(output,file),animations:'disabled'});report.screenshots.push(file);
 }
 report.knownFixture=await page.evaluate(()=>({operations:window.__bugfix.operations,writes:window.__bugfix.writes}));
 check('known-ID flow caused no mutation requests',report.knownFixture.writes.length===0,report.knownFixture.writes);
 check('known-ID fixture has no unrelated compatibility failure',!(await page.locator('body').innerText()).includes('UNEXPECTED_COMPATIBILITY_RESULT'));
 check('equipment entry is available in top navigation',await page.getByRole('button',{name:'装备与道具',exact:true}).evaluate(element=>element.getBoundingClientRect().top<100));
 // Recheck older open layout reports using genuine current-resource cards.
 await page.getByRole('combobox',{name:'周目',exact:true}).selectOption('3');
 await page.getByRole('combobox',{name:'稀有度',exact:true}).selectOption('4');
 await page.getByRole('spinbutton',{name:'绘卷等级',exact:true}).fill('180');
 report.collectionSeeds=await addKnownEnemyVariantsToCart(page);
 await page.locator('.result-cart-actions .compare-button').click();
 const collection=await verifyCollectionLayout(page,{outputDirectory:output,label:'current-cart',verifyPreview:true});
 check('existing cart favorite/remove buttons align on real cards',collection.geometry.length===3);
 check('existing preview text rows remain separated',collection.preview.effects.every(row=>row.height>=row.lineHeight-1));
 await page.locator('dialog[open] > header > button').click();
 await context.tracing.stop({path:join(output,'known-id-trace.zip')});await context.close();
 for(const geometry of [{width:1100,height:900,dpr:1},{width:1440,height:1000,dpr:1.25},{width:1440,height:1000,dpr:1.5}]){
  const ctx=await browser.newContext({viewport:{width:geometry.width,height:geometry.height},deviceScaleFactor:geometry.dpr});await ctx.addInitScript(bridge,{origin,recovery:true});const p=await ctx.newPage();activePage=p;p.on('pageerror',error=>errors.push(String(error)));await p.goto(origin);
  await p.getByRole('textbox',{name:'已知绘卷ID',exact:true}).fill('10030609');await p.getByRole('button',{name:'查看',exact:true}).click();await p.waitForFunction(()=>document.querySelector('.direct-seed')?.dataset.previewState==='current');
  await p.getByRole('radio',{name:'回标题界面后添加到存档',exact:true}).check();await p.locator('.install-mode .uncertain-operation').waitFor();await p.locator('.install-current').click();
  const recovery=p.locator('.current-add-review .uncertain-operation');await recovery.waitFor();
  const measure=async()=>{
    const handle=await p.waitForFunction(()=>{
      const element=document.querySelector('.current-add-review .uncertain-operation');
      const parent=element?.parentElement;
      if(!element?.isConnected||!parent)return null;
      const r=element.getBoundingClientRect(),box=parent.getBoundingClientRect();
      if(!r.width||!box.width)return null;
      return{width:r.width,parentWidth:box.width,ratio:r.width/box.width,left:r.left,parentLeft:box.left,right:r.right,parentRight:box.right,viewport:{width:innerWidth,height:innerHeight,dpr:devicePixelRatio}};
    },null,{timeout:15000});
    const value=await handle.jsonValue();await handle.dispose();return value;
  };
  for(const theme of ['light','dark'])for(const[label,lang]of languages){
   // The modal keeps the language selector behind its backdrop; set the same
   // persisted locale and dispatch the production locale event through a reload
   // is unnecessary here: the first geometry captures the three locales below.
   if(lang!=='zh-CN'&&geometry.dpr!==1)continue;
   await p.evaluate(theme=>{document.documentElement.dataset.theme=theme;},theme);
   if(lang!==await p.evaluate(()=>document.documentElement.lang)){
     await p.locator('dialog[open] > header > button').click();await p.locator('.current-add-review').waitFor({state:'detached'});await language(p,label,lang);await p.locator('.install-current').click();await recovery.waitFor();
   }
   const metrics=await measure();check('recovery full width '+JSON.stringify({theme,lang,...geometry}),metrics.ratio>.98&&metrics.left>=metrics.parentLeft-1&&metrics.right<=metrics.parentRight+1,metrics);
   const file=`recovery-${theme}-${lang}-${geometry.width}-${geometry.dpr}.png`;await p.screenshot({path:join(output,file),animations:'disabled'});report.screenshots.push(file);
  }
  const mutant=await p.addStyleTag({content:'.current-add-review .save-picker>.uncertain-operation{grid-column:auto!important}'});const narrow=await measure();check('geometry control detects original three-column defect',narrow.ratio<.5,narrow);await mutant.evaluate(element=>element.remove());
  check('recovery state survives geometry controls without a write',await p.evaluate(()=>window.__bugfix.writes.length===0));check('recovery fixture has no unrelated compatibility failure',!(await p.locator('body').innerText()).includes('UNEXPECTED_COMPATIBILITY_RESULT'));await ctx.close();
 }
 check('no production React errors',errors.length===0,errors);

 report.pass=true;
}catch(error){report.failures.push(error.stack||String(error));if(activePage&&!activePage.isClosed()){await activePage.screenshot({path:join(output,"failure.png")}).catch(()=>{});report.body=await activePage.locator("body").innerText().catch(()=>"");}throw error;}
finally{
 await browser?.close();await core.close();await new Promise(resolve=>server.close(resolve));
 report.requests=calls;
 for(const file of ['apps/workshop/main.tsx','apps/workshop/known-id-preview.ts','apps/workshop/style.css','apps/workshop/arc.css','apps/workshop/result-source.ts','apps/workshop/ui-locales.json'])report.source[file]=createHash('sha256').update(await readFile(join(root,file))).digest('hex');
 await writeFile(join(output,'worker.stderr.log'),core.stderr);await writeFile(join(output,'report.json'),JSON.stringify(report,null,2)+'\n');
 console.log(JSON.stringify({pass:report.pass,checks:report.checks.length,screenshots:report.screenshots.length,output,failures:report.failures}));
}
