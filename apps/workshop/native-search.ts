import {runtimeObserver,saveSession} from './save-workspace';
import {workerQuery,candidateSample} from './desktop-bridge';
import {matches,type Query,type Sample} from './model';
import type {OperationsApi} from '../desktop/src/operations-api';
/** The existing native generator runs only after an explicit title-screen confirmation. */
export async function searchNativePage(query:Query,seed:number,cancelled:()=>boolean,onResult:(sample:Sample)=>void,onStatus:(text:string)=>void,knownSeed=false,afterTrial=0){
 const inventory=saveSession!.getSnapshot().inventory;if(!inventory)throw Error('请先选择角色存档。');
 if(!runtimeObserver!.canStart())await runtimeObserver!.recover();
 const resolution=await window.nioh.resolveRecommendedLevel(query.recommended);if(resolution.selected_internal_level===null)throw Error('推荐等级无法转换。');
 const wire=workerQuery(query);const criteria={primary_effect_ids:wire.primary_effect_ids,required_secondary_ids:wire.required_secondary_ids,required_secondary_id_groups:wire.required_secondary_id_groups,grace_effect_id:wire.grace_effect_id,auxiliary:wire.auxiliary};
 let cursor=seed,count=0,exhausted=false,trial=afterTrial;
 const unsubscribe=runtimeObserver!.subscribe(()=>{if(runtimeObserver!.getSnapshot().job?.state==='running')onStatus(`原生搜索中，已找到 ${count} / ${knownSeed?1:query.count} 张绘卷…`)});
 try{for(let page=0;page<128&&!cancelled()&&count<(knownSeed?1:query.count);page++){
  const params={save_id:inventory.save_id,snapshot_id:inventory.snapshot_id,seed:cursor,playthrough:query.ng,rarity:query.rarity,level:query.level,recommended_level:resolution.selected_internal_level,title_screen_confirmed:true,criteria:knownSeed?{}:criteria,after_trial:trial,max_seeds:knownSeed?1:Math.min(1024,0x100000000-cursor)} as Parameters<OperationsApi['searchNative']>[0];
  const result=await runtimeObserver!.run(()=>knownSeed?window.operations.generate(params):window.operations.searchNative(params));if(!result||!('candidate' in result))throw Error('原生搜索未返回结果。');
  if(result.candidate){const sample=candidateSample(result.candidate,query.level);cursor=result.candidate.seed+1;trial=result.candidate.cursor??0;if(knownSeed||matches(sample,query)){const ref=await window.review.retain({source:'runtime',job_id:'',candidate_id:result.candidate.candidate_id});onResult({...sample,backend:{...sample.backend!,referenceId:ref.reference_id}});count++}}
  else {const nextTrial='resume_trial' in result?result.resume_trial:null;const nextSeed='resume_seed' in result?result.resume_seed:cursor+params.max_seeds;if(nextTrial!==null&&typeof nextTrial==='number'){if(nextTrial<=trial){exhausted=true;break}trial=nextTrial;}else if(trial){exhausted=true;break}if(typeof nextSeed==='number')cursor=nextSeed;}
  if(cursor>0xFFFFFFFF){exhausted=true;break}if(knownSeed)break;
 }
 return {cursor,count,exhausted,trial};
 }finally{unsubscribe()}
}
