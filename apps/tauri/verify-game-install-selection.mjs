/** Preimplementation native/session E2E. VERSIONINFO fixtures are never run.
 * Failure cases: V083_INSTALL_AND_STALE_PLAN_ACCEPTANCE_20260930.md.
 */
import assert from 'node:assert/strict';
import {execFileSync,spawn} from 'node:child_process';
import {mkdir,mkdtemp,copyFile,writeFile,readFile} from 'node:fs/promises';
import {basename,join,resolve} from 'node:path';
import {parseArgs} from 'node:util';
import {chromium} from 'playwright';
import {isolatedEnvironment,closeSession,inspectOnefile,pause} from './onefile-acceptance.mjs';
const {values}=parseArgs({options:{exe:{type:'string'},runtime:{type:'string'},out:{type:'string'},onefile:{type:'boolean',default:false}}});
assert(values.exe&&values.out);
const out=resolve(values.out);await mkdir(out,{recursive:true});
const root=await mkdtemp('D:/Nioh3_v080_deliverables/tmp/game-install-');
execFileSync('pwsh',['-NoProfile','-File','tools/prepare_ci_game_identity.ps1','-Root',join(root,'identity'),' -GameFileVersion'.trim(),'2.0.2.0'],{windowsHide:true,stdio:'pipe'});
const external=join(root,'non-steam-install/Nioh3.exe');await mkdir(join(root,'non-steam-install'),{recursive:true});await copyFile(join(root,'identity/ProgramFiles/Steam/steamapps/common/Nioh3/Nioh3.exe'),external);
const report={pass:false,packaged:values.onefile,root,boundary:'actual native host/workers; never-executed VERSIONINFO fixture; no cracked binary or game/save writes',checks:[],screenshots:[]};
const check=(name,yes)=>{report.checks.push({name,pass:!!yes});assert(yes,name)};
const nativePath=path=>resolve(path.replace(/^\\\\\?\\/,'')).toLowerCase();
let child,session;
async function open(profileRoot){
 const {env,profile,port}=await isolatedEnvironment(profileRoot);
 if(!values.onefile){assert(values.runtime);env.NIOH3_TAURI_PACKAGE_ROOT=resolve(values.runtime)}
 child=spawn(resolve(values.exe),['--user-data-dir',profile],{env,windowsHide:true,stdio:'ignore'});
 for(let i=0;i<150;i++){assert(child.exitCode===null,'shell exited');try{if((await fetch('http://127.0.0.1:'+port+'/json/version')).ok)break}catch{}await pause(300)}
 const browser=await chromium.connectOverCDP('http://127.0.0.1:'+port);const page=browser.contexts()[0].pages()[0];session={browser,page};await page.locator('.shell').waitFor();return {page,profile};
}
try{
 if(values.onefile){const id=await inspectOnefile(values.exe);report.executableSha256=id.sha256;report.payloadSha256=id.payloadSha256;}
 const good=join(root,'good');await mkdir(join(good,'profile'),{recursive:true});await writeFile(join(good,'profile/game-install.json'),JSON.stringify({schema:'nioh3-game-install/v1',executable:external,file_version:'99.99.99.99'}));
 let {page}=await open(good);
 const handshake=await page.evaluate(()=>window.nioh.handshake());report.handshake=handshake;
 check('Selected non-Steam VERSIONINFO establishes production context',handshake.context.game_file_version==='2.0.2.0');
 for(const role of ['save','runtime']){
  const current=await page.evaluate(role=>window.operations.current(role),role);check(role+' worker starts independently with its role prerequisites',current&&typeof current.busy==='boolean');
 }
 await page.evaluate(()=>window.operations.execute({method:'runtime.status',params:{}}));
 const diagnostics=await page.evaluate(()=>window.support.diagnostics());report.diagnostics=diagnostics;
 check('Search and save share the selected context; runtime remains deferred',diagnostics.workers.length===3&&diagnostics.workers.every(w=>w.role==='runtime'?w.contextDigest===null:w.contextDigest===handshake.context.context_digest));
 check('Feedback identifies actual source commit',/^[0-9a-f]{40}$/.test(diagnostics.packageVerification.sourceCommit));
 await page.locator('.app-build').waitFor();
 check('Visible build matches verified manifest',(await page.locator('.app-build').innerText())===diagnostics.packageVerification.sourceCommit.slice(0,7));
 report.buildGeometry=await page.locator('.app-build').evaluate(element=>{
  const label=element.getBoundingClientRect(),brand=element.closest('.brand').getBoundingClientRect();
  return {label:label.toJSON(),brand:brand.toJSON(),viewport:{width:innerWidth,height:innerHeight,dpr:devicePixelRatio},unclipped:label.top>=brand.top&&label.bottom<=brand.bottom&&label.left>=brand.left&&label.right<=brand.right};
 });
 check('Build identity is fully visible inside the fixed brand',report.buildGeometry.unclipped);
 check('Feedback identifies inner executable',typeof diagnostics.executablePath==='string'&&basename(diagnostics.executablePath).toLowerCase()===(values.onefile?'nioh3studio.exe':'nioh3-studio.exe'));
 if(values.onefile)check('Feedback identifies outer executable',nativePath(diagnostics.outerExecutable)===nativePath(values.exe));
 await page.locator('.settings').click();await page.locator('.side-popup .game-install').waitFor();
 check('Settings show selected executable',(await page.locator('.side-popup .game-install').innerText()).includes('Nioh3.exe'));
 const cdp=await page.context().newCDPSession(page);
 report.scaleBoundary='CDP constrained viewport inside native WebView2; system DPI unchanged';
 report.settingsGeometry=[];
 for(const [locale,label]of[['zh-CN','简体中文'],['en-US','English'],['ja-JP','日本語']]){
  await page.locator('.popup-dismiss').click();
  await page.locator('.side-popup .game-install').waitFor({state:'detached'});
  await page.locator('.language-button').click();await page.locator('.side-popup button').filter({hasText:label}).click();await page.locator('.settings').click();await page.locator('.side-popup .game-install').waitFor();
  await page.locator('.side-popup .game-install > p > code').waitFor();
  await page.waitForFunction(()=>{const button=document.querySelector('.side-popup [data-action=select-game-executable]');return button&&!button.disabled});
  check(locale+' game picker is reachable',await page.locator('.side-popup [data-action=select-game-executable]').isEnabled());
  await page.screenshot({path:join(out,'game-settings-'+locale+'.png')});report.screenshots.push('game-settings-'+locale+'.png');
  await cdp.send('Emulation.setDeviceMetricsOverride',{width:1020,height:640,deviceScaleFactor:1.5,mobile:false});
  const geometry=await page.locator('.side-popup').evaluate(e=>{const r=e.getBoundingClientRect();return {top:r.top,bottom:r.bottom,clientHeight:e.clientHeight,scrollHeight:e.scrollHeight,viewport:{width:innerWidth,height:innerHeight,dpr:devicePixelRatio}}});
  report.settingsGeometry.push({locale,...geometry});
  check(locale+' settings stay below the title bar at constrained height',geometry.top>=49&&geometry.bottom<=geometry.viewport.height);
  await page.locator('.side-popup [data-action=select-game-executable]').scrollIntoViewIfNeeded();
  check(locale+' game picker is reachable at constrained height',await page.locator('.side-popup [data-action=select-game-executable]').evaluate(e=>{const r=e.getBoundingClientRect();return r.top>=50&&r.bottom<=innerHeight}));
  await page.screenshot({path:join(out,'game-settings-constrained-'+locale+'.png')});report.screenshots.push('game-settings-constrained-'+locale+'.png');
  await cdp.send('Emulation.clearDeviceMetricsOverride');
 }
 const search=await page.evaluate(()=>window.nioh.handshake());check('All reads retain the same session context',search.context.context_digest===handshake.context.context_digest);
 await page.locator('.side-popup [data-action=reset-game-executable]').click();
 await page.locator('.side-popup .game-install .notice').waitFor();
 const afterReset=await page.evaluate(()=>window.nioh.handshake());check('Reset does not change running worker identity',afterReset.context.context_digest===handshake.context.context_digest);
 const config=JSON.parse(await readFile(join(good,'profile/game-install.json'),'utf8'));check('Reset persists automatic discovery for next launch',config.executable===null);
 await closeSession(session,child);session=null;child=null;
 const bad=join(root,'bad');await mkdir(join(bad,'profile'),{recursive:true});await writeFile(join(bad,'profile/game-install.json'),JSON.stringify({schema:'nioh3-game-install/v1',executable:join(root,'missing/Nioh3.exe')}));
 ({page}=await open(bad));const result=await page.evaluate(async()=>{try{await window.nioh.handshake();return null}catch(e){return e.message||String(e)}});
 check('Missing explicit selection refuses instead of silently using Steam',/GAME_EXECUTABLE_UNREADABLE/.test(result));
 await page.locator('.settings').click();await page.locator('.side-popup .game-install > p > code').waitFor();check('Picker remains available without worker startup',await page.locator('.side-popup [data-action=select-game-executable]').isEnabled());
 await page.screenshot({path:join(out,'missing-selection.png')});report.screenshots.push('missing-selection.png');
 await closeSession(session,child);session=null;child=null;
 const malformed=join(root,'malformed');await mkdir(join(malformed,'profile'),{recursive:true});await writeFile(join(malformed,'profile/game-install.json'),JSON.stringify({schema:'nioh3-game-install/v1'}));
 ({page}=await open(malformed));const invalid=await page.evaluate(async()=>{try{await window.nioh.handshake();return null}catch(e){return e.message||String(e)}});
 check('Malformed configured selection refuses instead of silently using Steam',/GAME_INSTALL_CONFIG_INVALID/.test(invalid));
 await closeSession(session,child);session=null;child=null;
 execFileSync('pwsh',['-NoProfile','-File','tools/prepare_ci_game_identity.ps1','-Root',join(root,'unsupported-identity'),'-GameFileVersion','9.9.9.9'],{windowsHide:true,stdio:'pipe'});
 const unsupported=join(root,'unsupported');await mkdir(join(unsupported,'profile'),{recursive:true});
 await writeFile(join(unsupported,'profile/game-install.json'),JSON.stringify({schema:'nioh3-game-install/v1',executable:join(root,'unsupported-identity/ProgramFiles/Steam/steamapps/common/Nioh3/Nioh3.exe')}));
 ({page}=await open(unsupported));const rejected=await page.evaluate(async()=>{try{await window.nioh.handshake();return null}catch(e){return e.message||String(e)}});
 report.unsupportedVersionError=rejected;
 check('Unsupported selected VERSIONINFO cannot establish a worker context',/GAME_VERSION_UNSUPPORTED/.test(rejected)&&rejected.includes('9.9.9.9'));
 const unsupportedDiagnostics=await page.evaluate(()=>window.support.diagnostics());report.unsupportedDiagnostics=unsupportedDiagnostics;
 check('Unsupported selected version blocks only search and save',unsupportedDiagnostics.workers.every(w=>w.role==='runtime'&&w.contextDigest===null));
 const control=await page.evaluate(()=>window.operations.execute({method:'runtime.status',params:{}}));
 check('Runtime controls remain available despite unsupported installation',control.safe_to_shutdown===true);
 await page.locator('.settings').click();await page.locator('.side-popup .game-install > p > code').waitFor();
 check('Settings remain available for unsupported selected versions',await page.locator('.side-popup [data-action=select-game-executable]').isEnabled());
 report.pass=true;
}catch(error){report.pass=false;report.error=error.stack||String(error);if(session){report.visibleText=await session.page.locator('body').innerText().catch(()=>null);await session.page.screenshot({path:join(out,'failure.png')}).catch(()=>{})}}
finally{await closeSession(session,child).catch(e=>{report.pass=false;report.closeError=String(e)});await writeFile(join(out,'game-install-e2e.json'),JSON.stringify(report,null,2));}
console.log(JSON.stringify({pass:report.pass,checks:report.checks.length,error:report.error,closeError:report.closeError}));if(!report.pass)process.exitCode=1;
