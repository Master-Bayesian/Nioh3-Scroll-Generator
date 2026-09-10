import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, readFile, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {FavoritesStore} from '../src/favorites-store';
import {data,type Sample} from '../../workshop/model';
import type {CandidateTransfer} from '../../../packages/contracts/responses';
function pair(seed:number) {
  const sample={...data.samples[0],seed:String(seed),playthrough:3,level:180} as Sample;
  const transfer={candidate_id:'candidate-'+seed,seed,rarity:sample.rarity,playthrough:3,level:180} as CandidateTransfer;
  return {sample,transfer};
}
test('Favorites survive restart, retain exact candidate data and cap concurrent additions at 50',async()=>{
  const path=join(await mkdtemp(join(tmpdir(),'nioh3-favorites-')),'favorites.json');
  const store=new FavoritesStore(path);
  const outcomes=await Promise.allSettled(Array.from({length:51},(_,i)=>{const p=pair(i);return store.add(p.sample,p.transfer)}));
  assert.equal(outcomes.filter(v=>v.status==='fulfilled').length,50);
  const loaded=await new FavoritesStore(path).list();assert.equal(loaded.length,50);
  assert.equal(loaded[3].transfer.candidate_id,'candidate-3');
  const p=pair(3);await store.add(p.sample,p.transfer);assert.equal((await store.list()).length,50);
  await store.remove(`3:${p.sample.rarity}:180:3`);assert.equal((await store.list()).length,49);
});
test('Favorites reject mismatched candidates and preserve a damaged file for recovery',async()=>{
  const path=join(await mkdtemp(join(tmpdir(),'nioh3-favorites-')),'favorites.json');
  const store=new FavoritesStore(path),p=pair(5);
  assert.throws(()=>store.add({...p.sample,seed:'6'},p.transfer),/INVALID/);
  await writeFile(path,'broken');await assert.rejects(store.list());
  await assert.rejects(store.add(p.sample,p.transfer));assert.equal(await readFile(path,'utf8'),'broken');
});
