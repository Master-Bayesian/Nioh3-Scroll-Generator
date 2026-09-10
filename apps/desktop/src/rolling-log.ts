import {appendFileSync, mkdirSync, renameSync, rmSync, statSync, readFileSync, existsSync} from 'node:fs';
import {join} from 'node:path';
/** Bounded support log: five files of 4 MiB with operation diagnostics. */
export class RollingLog {
 constructor(readonly directory:string, readonly limit=4*1024*1024, readonly files=5){mkdirSync(directory,{recursive:true})}
 write(event:string, detail:unknown='') {try {
 const path=join(this.directory,'runtime.log');
 const line=JSON.stringify({time:new Date().toISOString(),event,detail:String(detail).slice(0,16384)})+'\n';
 if(existsSync(path)&&statSync(path).size+Buffer.byteLength(line)>this.limit){
 rmSync(join(this.directory,`runtime.${this.files-1}.log`),{force:true});
 for(let i=this.files-2;i>=1;i--){const source=join(this.directory,`runtime.${i}.log`);if(existsSync(source))renameSync(source,join(this.directory,`runtime.${i+1}.log`));}
 renameSync(path,join(this.directory,'runtime.1.log'));
 }
 appendFileSync(path,line,'utf8');
 }catch{/* A logging failure must never interrupt a protected operation. */}}
 tail(){try{return readFileSync(join(this.directory,'runtime.log'),'utf8').slice(-160000)}catch{return ''}}
}
