import {test} from 'node:test';import assert from 'node:assert/strict';import {generateKeyPairSync,sign} from 'node:crypto';import {validateUpdate,signedUpdatePayload,compareVersion,type UpdateManifest} from '../src/portable-update';
import {readFileSync} from 'node:fs';
const keys=generateKeyPairSync('ed25519');const publicKey=keys.publicKey.export({format:'der',type:'spki'}).subarray(-32).toString('base64');
function manifest(){const m:UpdateManifest={schema:'nioh3-v2-update/v1',version:'0.7.1',channel:'stable',platform:'win32-x64',notes:'Test',asset:{name:'Nioh3ScrollEditorV2-0.7.1.zip',url:'https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/download/v0.7.1/Nioh3ScrollEditorV2-0.7.1.zip',size:1234,sha256:'a'.repeat(64)},signature:''};m.signature=sign(null,Buffer.from(signedUpdatePayload(m)),keys.privateKey).toString('base64');return m}
test('Signed V2 manifests are bound to the complete artifact and release metadata',()=>{const m=manifest();assert.equal(validateUpdate(m,publicKey),m);for(const field of ['version','notes'] as const){const changed={...m,[field]:field==='version'?'0.7.2':'Changed'};assert.throws(()=>validateUpdate(changed,publicKey),/SIGNATURE/)}assert.throws(()=>validateUpdate({...m,asset:{...m.asset,sha256:'b'.repeat(64)}},publicKey),/SIGNATURE/)});
test('Legacy EXEs and other origins cannot enter the V2 updater',()=>{const m=manifest();assert.throws(()=>validateUpdate({...m,asset:{...m.asset,name:'legacy.exe'}},publicKey),/ASSET/);assert.throws(()=>validateUpdate({...m,asset:{...m.asset,url:'https://example.com/package.zip'}},publicKey),/ORIGIN/)});
test('The real release workflow produces metadata accepted by the signed updater',()=>{
 const workflow=readFileSync(new URL('../../../.github/workflows/release.yml',import.meta.url),'utf8');
 const archiveLine=workflow.split('\n').find(line=>line.includes('run: python tools/archive_frontend_v2.py '));
 assert.ok(archiveLine,'Release archive command is present');
 const nameTemplate=archiveLine.match(/deliverables\/release\/([^/]+\.zip)\s*$/)?.[1];
 const signedPath=workflow.match(/"deliverables\/release\/([^"\r\n]+\.zip)"/)?.[1];
 const urlTemplate=workflow.match(/"(https:\/\/github\.com\/[^"\r\n]+\.zip)"/)?.[1];
 assert.ok(nameTemplate&&urlTemplate,'Release archive name and signed URL are present');
 assert.equal(signedPath,nameTemplate,'Signer consumes the archive that was built');
 const m=manifest();
 const expand=(value:string)=>value.replaceAll('${{ steps.version.outputs.value }}',m.version).replaceAll('${{ github.repository }}','Master-Bayesian/Nioh3-Scroll-Generator');
 m.asset.name=expand(nameTemplate);m.asset.url=expand(urlTemplate);
 assert.equal(new URL(m.asset.url).pathname.split('/').at(-1),m.asset.name);
 m.signature=sign(null,Buffer.from(signedUpdatePayload(m)),keys.privateKey).toString('base64');
 assert.equal(validateUpdate(m,publicKey),m);
});
test('Update version ordering refuses downgrades and separates prerelease stability',()=>{assert.ok(compareVersion('0.7.0','0.7.0-beta.5')>0);assert.ok(compareVersion('0.7.0-rc.1','0.7.0-beta.5')>0);assert.ok(compareVersion('0.6.10','0.7.0-dev.0')<0);assert.equal(compareVersion('0.7.0','0.7.0'),0)});
