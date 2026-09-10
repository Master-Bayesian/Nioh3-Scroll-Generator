/** Sign only the Tauri update format, using the existing official Ed25519 key. */
import {readFile, writeFile} from 'node:fs/promises';
import {basename} from 'node:path';
import {createHash, createPrivateKey, createPublicKey, sign, verify} from 'node:crypto';
const [asset, version, notesFile, output] = process.argv.slice(2);
if (!output || !/^\d+\.\d+\.\d+(?:-(?:beta|rc)\.\d+)?$/.test(version)) throw Error('Usage: ZIP VERSION NOTES OUTPUT');
const raw=Buffer.from(process.env.NIOH3_UPDATE_PRIVATE_KEY_BASE64 || '', 'base64');
if(raw.length!==32) throw Error('A 32-byte release signing key is required');
const key=createPrivateKey({key:Buffer.concat([Buffer.from('302e020100300506032b657004220420','hex'),raw]),format:'der',type:'pkcs8'});
const publicKey=createPublicKey({key:Buffer.concat([Buffer.from('302a300506032b6570032100','hex'),Buffer.from('c6oPCnJE4B+7ZnDUkZRJzUo3PZQmlM/eMlFqRC1h3dU=','base64')]),format:'der',type:'spki'});
const bytes=await readFile(asset); const name=basename(asset);
if(!/^[\w.-]+\.zip$/.test(name))throw Error('Unsafe asset name');
const manifest={schema:'nioh3-tauri-update/v1',version,channel:version.includes('-')?'beta':'stable',platform:'win32-x64',notes:await readFile(notesFile,'utf8'),asset:{name,url:`https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/download/v${version}/${name}`,size:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex')}};
const payload=Buffer.from(JSON.stringify(manifest));const signature=sign(null,payload,key);
if(!verify(null,payload,publicKey,signature))throw Error('Signing key does not match the installed production public key');
await writeFile(output,JSON.stringify({...manifest,signature:signature.toString('base64')},null,2)+'\n');
console.log(output);
