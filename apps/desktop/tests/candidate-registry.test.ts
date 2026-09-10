import test from 'node:test';
import assert from 'node:assert/strict';
import {CandidateRegistry} from '../src/candidate-registry';
import type {CandidateTransfer} from '../../../packages/contracts/responses';
const record=(id:string)=>({candidate_id:id,effects:[{effect_id:1}]} as unknown as CandidateTransfer);
test('Cart references are owned, bounded, isolated and released',()=>{
 const registry=new CandidateRegistry(2),source=record('one');
 registry.retain(source);source.effects[0].effect_id=2;
 assert.equal(registry.resolve(['one'])[0].effects[0].effect_id,1);
 registry.retain(record('two'));assert.throws(()=>registry.retain(record('three')),/CAPACITY/);
 assert.throws(()=>registry.resolve(['one','one']),/INVALID/);
 registry.release('one');assert.throws(()=>registry.resolve(['one']),/EXPIRED/);
 registry.retain(record('three'));registry.clear();assert.throws(()=>registry.resolve(['three']),/EXPIRED/);
});
