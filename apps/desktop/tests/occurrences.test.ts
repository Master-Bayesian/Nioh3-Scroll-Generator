import test from 'node:test';import assert from 'node:assert/strict';
import {data,initialQuery,matchesEffectOccurrences,conditionKey} from '../../workshop/model';
import {workerQuery,formQuery} from '../../workshop/desktop-bridge';
test('Duplicate effects keep separate thresholds and restore distinct draggable choices',()=>{
 const q=initialQuery();q.unrestricted=true;q.effects=[{id:'1',choiceId:'first',name:'Life',mode:0,roll:90,cross:false},{id:'1',choiceId:'second',name:'Life',mode:0,roll:80,cross:false}];
 const wire=workerQuery(q);assert.equal(wire.effect_occurrences?.length,2);assert.deepEqual(wire.required_secondary_ids,[1]);assert.deepEqual(wire.minimum_roll_percent_by_effect_id,[[1,80]]);
 const restored=formQuery({query:wire,result_count:25} as any);assert.equal(restored.effects.length,2);assert.notEqual(conditionKey(restored.effects[0]),conditionKey(restored.effects[1]));
 const sample={...data.samples[0],effects:[{id:'1',name:'Life',role:'主词条',raw:0,roll:95}]};
 assert.equal(matchesEffectOccurrences(sample,q),false);sample.effects.push({...sample.effects[0],role:'副词条',roll:85});assert.equal(matchesEffectOccurrences(sample,q),true);
});
