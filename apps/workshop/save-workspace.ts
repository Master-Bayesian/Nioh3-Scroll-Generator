import {OperationController} from '../desktop/src/operation-controller';
import {SaveSession,saveGateway} from '../desktop/src/save-session';
import {desktop,previewSeed} from './desktop-bridge';
import {data,type Sample} from './model';
import type {SaveInventory,SaveReference} from '../../packages/contracts/protected-responses';
export const saveObserver=desktop?new OperationController('save',window.operations):null;
export const runtimeObserver=desktop?new OperationController('runtime',window.operations):null;
export const saveSession=desktop?new SaveSession(saveGateway(window.operations,saveObserver!)):null;
export async function selectSave(){const result=await saveObserver!.run(()=>window.operations.selectSave());if(result&&'save_id' in result&&'account_id' in result&&'path' in result)await saveSession!.select(result as SaveReference)}
export function entrySample(entry:SaveInventory['entries'][number]):Sample{return {seed:String(entry.header.seed),rarity:entry.header.rarity,level:entry.header.level,playthrough:entry.header.playthrough,saveEntry:entry,effects:entry.effects.filter(e=>e.effect_id!==0xFFFFFFFF).map((e,i)=>({id:String(e.effect_id),name:data.editorEffects.find(v=>v.id===String(e.effect_id))?.name||'未知词条',raw:e.value,roll:e.metadata&255,role:i===0?'主词条':'副词条'})),capacity:entry.derived.initial_challenge_capacity,enemyKeys:[],enemySlotKeys:[],enemies:[],terrainKeys:[],rules:[]}}
export async function enrichEntry(sample:Sample){const aux=await window.review.auxiliary({seed:Number(sample.seed),playthrough:sample.playthrough||3});return {...sample,capacity:aux.initial_challenge_capacity,enemyKeys:aux.enemy_groups.flatMap(g=>g.map(e=>e.lookup_key)),enemySlotKeys:aux.enemy_groups.map(g=>g[0]?.lookup_key).filter(k=>k!==undefined),enemies:aux.enemy_groups.flatMap(g=>g.map(e=>data.enemies.find(v=>v.keys.includes(e.lookup_key))?.name||String(e.lookup_key))),terrainKeys:aux.terrain.display_effect_keys,rules:aux.special_rules.map(rule=>({key:rule.key,name:data.rules.find(r=>r.keys.includes(rule.key))?.name||String(rule.key),value:rule.display_value===null?rule.display_grade||'':String(rule.display_value)+(rule.display_unit==='percent'?'%':rule.display_unit==='seconds'?' 秒':'')}))}}

let discovery:Promise<SaveReference[]>|null=null;
export function discoverSaves(refresh=false){if(refresh)discovery=null;if(!discovery)discovery=(async()=>{const result=await saveObserver!.run(()=>window.operations.execute({method:'save.discover',params:{}}));if(!result||!('saves' in result))throw Error('未能检测存档。');if(result.saves.length===1&&!saveSession!.getSnapshot().selected)await saveSession!.select(result.saves[0]);return result.saves})().catch(error=>{discovery=null;throw error});return discovery}
