import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, mkdir, readFile, writeFile, copyFile, cp, readdir, unlink, access} from 'node:fs/promises';
import {join, dirname, resolve} from 'node:path';
import {tmpdir} from 'node:os';
import {execFile,spawn} from 'node:child_process';
import {once} from 'node:events';
import {promisify} from 'node:util';
import {createHash,generateKeyPairSync,sign} from 'node:crypto';
import {PortableUpdate,signedUpdatePayload,type UpdateManifest} from '../src/portable-update';
import {finishInstalledUpdate} from '../src/update-cleanup';

const run=promisify(execFile),hash=(bytes:Buffer)=>createHash('sha256').update(bytes).digest('hex');
const required=['Nioh3ScrollEditorV2.exe','resources/app/main.cjs','resources/app/preload.cjs','resources/app/renderer.js','resources/app/index.html','resources/app/package.json','resources/app/review.js','resources/app/review.css','resources/app/review.html','resources/app/apply-update.ps1','resources/app/extract-update.ps1','resources/assets/nioh3-scroll-generator-icon.png','resources/worker/nioh3-search-worker.exe','resources/worker/nioh3-protected-worker.exe',...['request','response','protected-request','protected-response'].map(n=>`resources/packages/contracts/${n}.schema.json`)];
async function ps(script:string,args:string[]=[]){return run('powershell.exe',['-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',script,...args],{windowsHide:true,timeout:30000})}
async function fixture(root:string,version:string,validExe=true){
 const files=[];
 for(const path of required){const destination=join(root,path);await mkdir(dirname(destination),{recursive:true});
  if(path==='Nioh3ScrollEditorV2.exe'&&validExe)await copyFile(join(process.env.SystemRoot!,'System32/whoami.exe'),destination);
  else await writeFile(destination,version+' synthetic '+path);
  const bytes=await readFile(destination);files.push({path,size:bytes.length,sha256:hash(bytes)});
 }
 await writeFile(join(root,'build-manifest.json'),JSON.stringify({schema:'nioh3-portable-manifest/v2',version,files}));
}
test('Signed local beta feed stages a complete archive; real helper replaces and rolls back failed launch', {skip:process.platform!=='win32',timeout:90000},async()=>{
 const root=await mkdtemp(join(tmpdir(),'nioh3-update-workflow-')),stage=join(root,'stage'),target=join(root,'installed');
 await fixture(stage,'0.7.0-rc.1');await fixture(target,'0.6.10');
 const zip=join(root,'package.zip'),zipScript=join(root,'zip.ps1');
 await writeFile(zipScript,"param($Source,$Output)\nAdd-Type -AssemblyName System.IO.Compression.FileSystem\n[IO.Compression.ZipFile]::CreateFromDirectory($Source,$Output)\n");
 await ps(zipScript,['-Source',stage,'-Output',zip]);
 const bytes=await readFile(zip),keys=generateKeyPairSync('ed25519');
 const assetUrl='https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/download/v0.7.0-rc.1/Nioh3ScrollEditorV2-0.7.0-rc.1.zip';
 const manifest:UpdateManifest={schema:'nioh3-v2-update/v1',version:'0.7.0-rc.1',channel:'beta',platform:'win32-x64',notes:'Synthetic update',asset:{name:'Nioh3ScrollEditorV2-0.7.0-rc.1.zip',url:assetUrl,size:bytes.length,sha256:hash(bytes)},signature:''};
 manifest.signature=sign(null,Buffer.from(signedUpdatePayload(manifest)),keys.privateKey).toString('base64');
 const manifestUrl=assetUrl.replace(/[^/]+$/,'v2-update.json');
 const fetcher=(async(url:unknown)=>new Response(String(url)===assetUrl?new Uint8Array(bytes):JSON.stringify(String(url)===manifestUrl?manifest:[{prerelease:true,assets:[{name:'v2-update.json',browser_download_url:manifestUrl}]}]))) as typeof fetch;
 const updater=new PortableUpdate(join(root,'updates'),'0.6.10',resolve('apps/desktop/extract-update.ps1'),{fetch:fetcher,publicKey:keys.publicKey.export({format:'der',type:'spki'}).subarray(-32).toString('base64')});
 await updater.check('stable');assert.equal(updater.state.phase,'current');
 await updater.check('beta');assert.equal(updater.state.phase,'available');await updater.download();assert.equal(updater.state.phase,'ready',updater.state.error);
 await assert.rejects(access(join(dirname(updater.stagedDirectory!),'package.zip')));
 const helper=join(root,'updates','apply-update.ps1');await copyFile('apps/desktop/apply-update.ps1',helper);
 await ps(helper,['-ProcessId','2147483647','-Target',target,'-Staged',updater.stagedDirectory!,'-ManifestHash',updater.manifestHash!]);
 assert.equal(JSON.parse(await readFile(join(target,'build-manifest.json'),'utf8')).version,'0.7.0-rc.1');
 const receiptPath=join(root,'updates','last-update-result.json');
 const receipt=JSON.parse(await readFile(receiptPath,'utf8'));
 assert.equal(receipt.status,'awaiting-startup');
 await access(receipt.previous); // Launch alone must never destroy rollback data.
 const invalid=join(root,'invalid');await fixture(invalid,'0.7.1',false);
 const before=await readFile(join(target,'build-manifest.json'));
 await assert.rejects(ps(helper,['-ProcessId','2147483647','-Target',target,'-Staged',invalid,'-ManifestHash',hash(await readFile(join(invalid,'build-manifest.json')))]));
 assert.deepEqual(await readFile(join(target,'build-manifest.json')),before);
 // Mutation after staging is rejected before the installed tree is moved.
 await writeFile(join(invalid,'resources/app/main.cjs'),'tampered');
 await assert.rejects(ps(helper,['-ProcessId','2147483647','-Target',target,'-Staged',invalid,'-ManifestHash',hash(await readFile(join(invalid,'build-manifest.json')))]));
 assert.deepEqual(await readFile(join(target,'build-manifest.json')),before);
 const lockScript=join(root,'lock.ps1');
 await writeFile(lockScript,"param($Path)\n$stream=[IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::None)\ntry{[Console]::WriteLine('READY');[Console]::ReadLine()|Out-Null}finally{$stream.Dispose()}\n");
 const locker=spawn('powershell.exe',['-NoProfile','-NonInteractive','-File',lockScript,'-Path',join(target,'Nioh3ScrollEditorV2.exe')],{windowsHide:true,stdio:'pipe'});
 try {
  const [ready]=await once(locker.stdout,'data');assert.match(String(ready),/READY/);
  await assert.rejects(ps(helper,['-ProcessId','2147483647','-Target',target,'-Staged',updater.stagedDirectory!,'-ManifestHash',updater.manifestHash!]));
  assert.deepEqual(await readFile(join(target,'build-manifest.json')),before);
 } finally {locker.stdin.end('\n');if(locker.exitCode===null)await once(locker,'exit')}
 const broken=Buffer.from('This is not a zip archive');
 manifest.asset={...manifest.asset,size:broken.length,sha256:hash(broken)};
 manifest.signature=sign(null,Buffer.from(signedUpdatePayload(manifest)),keys.privateKey).toString('base64');
 const badFetch=(async(url:unknown)=>String(url)===assetUrl?new Response(new Uint8Array(broken)):fetcher(url as string)) as typeof fetch;
 const badUpdater=new PortableUpdate(join(root,'bad-update'),'0.6.10',resolve('apps/desktop/extract-update.ps1'),{fetch:badFetch,publicKey:keys.publicKey.export({format:'der',type:'spki'}).subarray(-32).toString('base64')});
 await badUpdater.check('beta');await badUpdater.download();assert.equal(badUpdater.state.phase,'failed');assert.equal(badUpdater.stagedDirectory,null);
 assert.deepEqual(await readdir(join(root,'bad-update')),[],'Failed downloads leave no cache');
 assert.deepEqual(await readFile(join(target,'build-manifest.json')),before);
 assert.ok(!(await readdir(root)).some(name=>name.startsWith('installed.update-')),'Failed replacement copies are removed');
 // The published 0.7.0 helper writes this older receipt shape. Upgrade it too.
 await writeFile(receiptPath,JSON.stringify({status:'launched',previous:receipt.previous,time:receipt.time}));
 await writeFile(join(receipt.previous,'my-save.dat'),'User data');
 await assert.rejects(finishInstalledUpdate(join(root,'updates'),target,'0.7.0-rc.1'),/USER_FILES/);
 assert.equal(await readFile(join(receipt.previous,'my-save.dat'),'utf8'),'User data');
 await unlink(join(receipt.previous,'my-save.dat'));
 await finishInstalledUpdate(join(root,'updates'),target,'0.7.0-rc.1');
 await assert.rejects(access(receipt.previous));
 await assert.rejects(access(updater.stagedDirectory!));
 assert.equal(JSON.parse(await readFile(receiptPath,'utf8')).status,'completed');
 assert.deepEqual(await readFile(join(target,'build-manifest.json')),before);
 await finishInstalledUpdate(join(root,'updates'),target,'0.7.0-rc.1'); // Idempotent.
});
