/** Public, synthetic signing seed for cross-language protocol tests only. */
import { createPrivateKey, createPublicKey, sign } from 'node:crypto';
import { mkdir, writeFile } from 'node:fs/promises';
const key=createPrivateKey({key:Buffer.concat([Buffer.from('302e020100300506032b657004220420','hex'),Buffer.alloc(32,7)]),format:'der',type:'pkcs8'});
const payload={schema:'nioh3-tauri-update/v1',version:'0.7.2',channel:'stable',platform:'win32-x64',notes:'Japanese names: 恩寵. Update cleanup.',asset:{name:'Nioh3Studio-0.7.2-win-x64.zip',url:'https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/download/v0.7.2/Nioh3Studio-0.7.2-win-x64.zip',size:1234,sha256:'a'.repeat(64)}};
const fixture={publicKey:createPublicKey(key).export({format:'der',type:'spki'}).subarray(-32).toString('base64'),manifest:{...payload,signature:sign(null,Buffer.from(JSON.stringify(payload)),key).toString('base64')}};
await mkdir('apps/tauri/test-fixtures',{recursive:true});
await writeFile('apps/tauri/test-fixtures/signed-update.json',JSON.stringify(fixture,null,2)+'\n');
