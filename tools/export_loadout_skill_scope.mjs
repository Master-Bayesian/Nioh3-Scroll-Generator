/** Identify native records outside the planner menu without guessing their effects. */
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {resolve} from 'node:path';
const source=process.argv[2];
assert(source,'Pass the retained skill-catalog native-extraction directory');
const root=new URL('../',import.meta.url);
const catalog=JSON.parse(await readFile(new URL('apps/workshop/loadout-skills.json',root),'utf8'));
const digest=b=>createHash('sha256').update(b).digest('hex');
const captures=[],sources=[];
for(const name of ['native-skills-03','native-skills-04']) {
 const dir=resolve(source,name), manifest=JSON.parse(await readFile(resolve(dir,'capture.json'),'utf8'));
 assert.equal(manifest.status,'complete_native_table_capture');
 assert.equal(manifest.executable_sha256,catalog.executable_sha256);
 assert.equal(manifest.game_writes,0);assert.equal(manifest.native_function_calls,0);
 const tables={};
 for(const table of manifest.tables.filter(row=>['skill','skill_text'].includes(row.name))) {
  assert(table.stable&&table.hash_index_and_row_keys_agree);
  const verified=async entry=>{const b=await readFile(resolve(dir,entry.filename));assert.equal(b.length,entry.size);assert.equal(digest(b),entry.sha256.toLowerCase());return b;};
  const bytes=await verified(table.rows_blob), context=await verified(table.hash.context_blob), index=await verified(table.hash.entries_blob);
  assert.equal(table.row_count,1101);assert.equal(bytes.length,8+table.row_count*table.row_size);
  const sentinel=context.readUInt32LE(4), keys=new Map();
  for(let offset=0;offset<index.length;offset+=8){const key=index.readUInt32LE(offset),i=index.readUInt32LE(offset+4);if(key!==sentinel&&i<table.row_count)keys.set(i,key);}
  assert.equal(keys.size,table.row_count);
  const rows=new Map();
  for(let i=0;i<table.row_count;i++){const row=bytes.subarray(8+i*table.row_size,8+(i+1)*table.row_size),id=row.readUInt16LE(table.key_offset);assert.equal(id,keys.get(i));assert(!rows.has(id));rows.set(id,row.toString('hex'));}
  tables[table.name]=rows;
  sources.push({capture:name,table:table.name,sha256:digest(bytes)});
 }
 captures.push(tables);
}
assert.deepEqual(captures[0],captures[1],'Native rows must match across restart');
const outside=[];
for(const [id,hex] of captures[0].skill) {
 const row=Buffer.from(hex,'hex'),meta=Buffer.from(captures[0].skill_text.get(id),'hex');
 const category=row.readInt8(0xac),subtype=row[0xb9],hidden=meta[0x7c];
 if(![21,22,23].includes(category)||![1,3].includes(subtype)||hidden)outside.push(id);
}
assert(catalog.skills.every(row=>!outside.includes(row.id)));
await writeFile(new URL('apps/workshop/loadout-native-skill-scope.json',root),JSON.stringify({
 schema:'nioh3-native-skill-scope/v1',game_version:'PC v2.02',executable_sha256:catalog.executable_sha256,
 scope:'Native category, subtype and hidden-menu filters only. Non-planner records are retained unchanged, never added to planner costs or effects.',sources,outside_planner_ids:outside.sort((a,b)=>a-b),
},null,2)+'\n');
console.log(`Projected ${outside.length} native records outside the planner menu`);
