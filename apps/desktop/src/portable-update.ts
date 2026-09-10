import {createHash, createPublicKey, verify, randomUUID} from 'node:crypto';
import {mkdir, writeFile, open, readFile, unlink} from 'node:fs/promises';
import {join} from 'node:path';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {verifyPortable} from '../../../packages/packaging/integrity.mjs';
import {removeUpdateCache} from './update-cleanup';

const PUBLIC_KEY='c6oPCnJE4B+7ZnDUkZRJzUo3PZQmlM/eMlFqRC1h3dU=';
const RELEASES='https://api.github.com/repos/Master-Bayesian/Nioh3-Scroll-Generator/releases?per_page=20';
export interface UpdateManifest {schema:'nioh3-v2-update/v1';version:string;channel:'stable'|'beta';platform:'win32-x64';notes:string;asset:{name:string;url:string;size:number;sha256:string};signature:string}
export interface UpdateState {phase:'idle'|'checking'|'current'|'available'|'downloading'|'ready'|'failed';version?:string;notes?:string;downloaded?:number;total?:number;error?:string}
export function signedUpdatePayload(value:UpdateManifest){return JSON.stringify({schema:value.schema,version:value.version,channel:value.channel,platform:value.platform,notes:value.notes,asset:{name:value.asset.name,url:value.asset.url,size:value.asset.size,sha256:value.asset.sha256}})}
function safeZipAssetName(name:unknown):name is string {
 if(typeof name!=='string'||name.length>180||!/^.+\.zip$/i.test(name)||name!==name.trim()||/[<>:"/\\|?*\u0000-\u001f\u007f]/.test(name))return false;
 return !/^(?:con|prn|aux|nul|com[1-9¹²³]|lpt[1-9¹²³])$/i.test(name.split('.')[0].trimEnd());
}
export function validateUpdate(value:UpdateManifest, publicKey=PUBLIC_KEY){
 if(!value||value.schema!=='nioh3-v2-update/v1'||value.platform!=='win32-x64'||!['stable','beta'].includes(value.channel)||!/^\d+\.\d+\.\d+(?:-(?:beta|rc)\.\d+)?$/.test(value.version)||typeof value.notes!=='string'||value.notes.length>32000)throw Error('UPDATE_MANIFEST_INVALID');
 if(value.channel==='stable'&&value.version.includes('-'))throw Error('UPDATE_CHANNEL_MISMATCH');
 const a=value.asset;if(!a||!safeZipAssetName(a.name)||!Number.isSafeInteger(a.size)||a.size<=0||a.size>1024*1024*1024||!/^[a-f0-9]{64}$/.test(a.sha256))throw Error('UPDATE_ASSET_INVALID');
 const url=new URL(a.url);if(url.origin!=='https://github.com'||decodeURIComponent(url.pathname)!==`/Master-Bayesian/Nioh3-Scroll-Generator/releases/download/v${value.version}/${a.name}`||url.username||url.password||url.hash)throw Error('UPDATE_ASSET_ORIGIN_INVALID');
 const key=createPublicKey({key:Buffer.concat([Buffer.from('302a300506032b6570032100','hex'),Buffer.from(publicKey,'base64')]),format:'der',type:'spki'});
 if(typeof value.signature!=='string'||!verify(null,Buffer.from(signedUpdatePayload(value)),key,Buffer.from(value.signature,'base64')))throw Error('UPDATE_SIGNATURE_INVALID');
 return value;
}
export function compareVersion(a:string,b:string){const parse=(s:string)=>{const [base,tag]=s.split('-');const nums=base.split('.').map(Number);return [...nums,tag?tag.startsWith('rc')?2:tag.startsWith('beta')?1:0:3,Number(tag?.split('.')[1]||0)]};const x=parse(a),y=parse(b);for(let i=0;i<x.length;i++)if(x[i]!==y[i])return x[i]-y[i];return 0}
async function boundedFetch(url:string,maximum:number,fetcher:typeof fetch=fetch){
 const response=await fetcher(url,{signal:AbortSignal.timeout(180000),headers:{'User-Agent':'Nioh3ScrollEditorV2','Accept':'application/octet-stream'}});
 if(!response.ok)throw Error('UPDATE_HTTP_'+response.status);if(!response.body)throw Error('UPDATE_EMPTY_RESPONSE');
 let size=0;const chunks:Uint8Array[]=[];for await(const chunk of response.body){size+=chunk.length;if(size>maximum)throw Error('UPDATE_RESPONSE_TOO_LARGE');chunks.push(chunk)}return Buffer.concat(chunks);
}
/** Authenticated whole-package staging. No update operation touches the running package. */
export class PortableUpdate {
 state:UpdateState={phase:'idle'};private manifest:UpdateManifest|null=null;stagedDirectory:string|null=null;manifestHash:string|null=null;
 constructor(readonly directory:string,readonly currentVersion:string,readonly extractScript:string,private dependencies:{fetch?:typeof fetch;publicKey?:string}={}){}
 async check(channel:'stable'|'beta'){
  if(['checking','downloading','ready'].includes(this.state.phase))return;this.state={phase:'checking'};this.manifest=null;this.stagedDirectory=null;this.manifestHash=null;
  try{const releases=JSON.parse((await boundedFetch(RELEASES,1024*1024,this.dependencies.fetch)).toString());if(!Array.isArray(releases))throw Error('UPDATE_INDEX_INVALID');
   const candidates:UpdateManifest[]=[];
   for(const release of releases.slice(0,20)){if(release.draft||channel==='stable'&&release.prerelease)continue;const asset=release.assets?.find((a:{name:string})=>a.name==='v2-update.json');if(!asset)continue;
    const url=new URL(asset.browser_download_url);if(url.origin!=='https://github.com'||!url.pathname.startsWith('/Master-Bayesian/Nioh3-Scroll-Generator/releases/download/'))throw Error('UPDATE_MANIFEST_ORIGIN_INVALID');
    const manifest=validateUpdate(JSON.parse((await boundedFetch(url.href,128*1024,this.dependencies.fetch)).toString()),this.dependencies.publicKey);if(manifest.channel===channel)candidates.push(manifest);
   }
   const latest=candidates.sort((a,b)=>compareVersion(b.version,a.version))[0];
   if(!latest||compareVersion(latest.version,this.currentVersion)<=0){this.state={phase:'current'};return}
   this.manifest=latest;this.state={phase:'available',version:latest.version,notes:latest.notes};
  }catch(error){this.state={phase:'failed',error:String(error)}}
 }
 async download(){if(this.state.phase!=='available'||!this.manifest)throw Error('UPDATE_NOT_AVAILABLE');const manifest=this.manifest;
  this.state={phase:'downloading',version:manifest.version,downloaded:0,total:manifest.asset.size};
  const folder=join(this.directory,randomUUID());
  try{await mkdir(folder,{recursive:true});const archive=join(folder,'package.zip'),staged=join(folder,'package');
   const response=await (this.dependencies.fetch||fetch)(manifest.asset.url,{signal:AbortSignal.timeout(600000)});if(!response.ok||!response.body)throw Error('UPDATE_DOWNLOAD_FAILED');
   const file=await open(archive,'wx');const hash=createHash('sha256');let size=0;
   try{for await(const chunk of response.body){size+=chunk.length;if(size>manifest.asset.size)throw Error('UPDATE_RESPONSE_TOO_LARGE');hash.update(chunk);let offset=0;while(offset<chunk.length){const written=await file.write(chunk,offset,chunk.length-offset);offset+=written.bytesWritten;}this.state={...this.state,downloaded:size};}await file.sync();}finally{await file.close();}
   if(size!==manifest.asset.size||hash.digest('hex')!==manifest.asset.sha256)throw Error('UPDATE_HASH_MISMATCH');
   await promisify(execFile)('powershell.exe',['-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',this.extractScript,'-Archive',archive,'-Destination',staged],{windowsHide:true,timeout:180000});
   const report=await verifyPortable(staged);if(report.version!==manifest.version)throw Error('UPDATE_PACKAGE_VERSION_MISMATCH');
   await writeFile(join(folder,'verified-update.json'),JSON.stringify(manifest));
   await unlink(archive);
   this.manifestHash=createHash('sha256').update(await readFile(join(staged,'build-manifest.json'))).digest('hex');this.stagedDirectory=staged;this.state={phase:'ready',version:manifest.version};
  }catch(error){this.stagedDirectory=null;this.manifestHash=null;
   try{await removeUpdateCache(this.directory,folder)}catch(cleanup){error=Error(String(error)+'; '+String(cleanup))}
   this.state={phase:'failed',error:String(error)}}
 }
}
