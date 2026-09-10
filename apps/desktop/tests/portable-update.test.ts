import {test} from 'node:test';import assert from 'node:assert/strict';import {generateKeyPairSync,sign} from 'node:crypto';import {validateUpdate,signedUpdatePayload,compareVersion,type UpdateManifest} from '../src/portable-update';
import {readFileSync} from 'node:fs';
const keys=generateKeyPairSync('ed25519');const publicKey=keys.publicKey.export({format:'der',type:'spki'}).subarray(-32).toString('base64');
function manifest(){const m:UpdateManifest={schema:'nioh3-v2-update/v1',version:'0.7.1',channel:'stable',platform:'win32-x64',notes:'Test',asset:{name:'Nioh3ScrollEditorV2-0.7.1.zip',url:'https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/download/v0.7.1/Nioh3ScrollEditorV2-0.7.1.zip',size:1234,sha256:'a'.repeat(64)},signature:''};m.signature=sign(null,Buffer.from(signedUpdatePayload(m)),keys.privateKey).toString('base64');return m}
test('Signed V2 manifests are bound to the complete artifact and release metadata',()=>{const m=manifest();assert.equal(validateUpdate(m,publicKey),m);for(const field of ['version','notes'] as const){const changed={...m,[field]:field==='version'?'0.7.2':'Changed',asset:{...m.asset}};if(field==='version')changed.asset.url=changed.asset.url.replace('/v0.7.1/','/v0.7.2/');assert.throws(()=>validateUpdate(changed,publicKey),/SIGNATURE/)}assert.throws(()=>validateUpdate({...m,asset:{...m.asset,sha256:'b'.repeat(64)}},publicKey),/SIGNATURE/)});
test('Legacy EXEs and other origins cannot enter the V2 updater',()=>{const m=manifest();assert.throws(()=>validateUpdate({...m,asset:{...m.asset,name:'legacy.exe'}},publicKey),/ASSET/);assert.throws(()=>validateUpdate({...m,asset:{...m.asset,url:'https://example.com/package.zip'}},publicKey),/ORIGIN/)});
function signedAsset(name:string,url?:string){
 const m=manifest();m.asset.name=name;m.asset.url=url||`https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/download/v${m.version}/${encodeURIComponent(name)}`;
 m.signature=sign(null,Buffer.from(signedUpdatePayload(m)),keys.privateKey).toString('base64');return m;
}
test('Signed ZIP packages do not depend on a development label or product name',()=>{
 for(const name of ['Nioh3ScrollEditor-0.7.1-win-x64.zip','Nioh3ScrollEditorV2-0.7.1.zip','IpponStudio-日本語-0.7.1.zip']){
  const m=signedAsset(name);assert.equal(validateUpdate(m,publicKey),m);
 }
});
test('Even signed package names cannot escape the update directory or use Windows devices',()=>{
 for(const name of ['../escape.zip','..\\escape.zip','C:\\escape.zip','/escape.zip','evil:stream.zip','NUL.zip','con.backup.zip','con .zip','COM1.zip','com¹.zip','archive.zip ','archive.exe','archive.zip.exe','a\u0000.zip','archive?.zip','a'.repeat(181)+'.zip','.zip']){
  assert.throws(()=>validateUpdate(signedAsset(name),publicKey),/UPDATE_ASSET_INVALID/,name);
 }
});
test('The signed download URL must identify the declared version and filename',()=>{
 const name='Nioh3ScrollEditor-0.7.1-win-x64.zip';
 for(const suffix of [`v0.6.10/${name}`,'v0.7.1/different.zip']){
  const m=signedAsset(name,`https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/download/${suffix}`);
  assert.throws(()=>validateUpdate(m,publicKey),/UPDATE_ASSET_ORIGIN_INVALID/);
 }
});
test('The Tauri workflow signs the archive it actually builds without publishing Electron',()=>{
 const workflow=readFileSync(new URL('../../../.github/workflows/release.yml',import.meta.url),'utf8');
 const signer=readFileSync(new URL('../../../tools/build_tauri_update_manifest.mjs',import.meta.url),'utf8');
 const archiveLine=workflow.split('\n').find(line=>line.includes('run: python tools/archive_frontend_v2.py '));
 assert.ok(archiveLine);
 const name=archiveLine.match(/deliverables\/release\/([^/]+\.zip)\s*$/)?.[1];
 const signedName=workflow.match(/\$zip='deliverables\/release\/([^']+)'/)?.[1];
 assert.ok(name);assert.equal(signedName,name);
 assert.ok(workflow.includes('node tools/build_tauri_update_manifest.mjs $zip'));
 assert.ok(workflow.includes('60MB'));
 assert.ok(signer.includes("schema:'nioh3-tauri-update/v1'"));
 assert.ok(signer.includes('releases/download/v${version}/${name}'));
 assert.ok(signer.includes('verify(null,payload,publicKey,signature)'));
 assert.ok(!workflow.includes('softprops/action-gh-release'));
 assert.ok(!workflow.includes('build_frontend_v2.ps1'));
});
test('Update version ordering refuses downgrades and separates prerelease stability',()=>{assert.ok(compareVersion('0.7.0','0.7.0-beta.5')>0);assert.ok(compareVersion('0.7.0-rc.1','0.7.0-beta.5')>0);assert.ok(compareVersion('0.6.10','0.7.0-dev.0')<0);assert.equal(compareVersion('0.7.0','0.7.0'),0)});
