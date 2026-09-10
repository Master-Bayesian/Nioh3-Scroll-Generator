import type {JobSnapshot} from '../../../packages/contracts/responses';
import type {ProtectedResult} from './operations-api';
export type CandidateView=JobSnapshot['candidates'][number];
export interface ReviewApi {
 favorites(params:{action:'list'|'add'|'remove';sample?:import('../../workshop/model').Sample;reference_id?:string;key?:string}):Promise<import('../../workshop/model').Sample[]>;
 update(params:{action:'status'|'check'|'download'|'apply';channel:'stable'|'beta'}):Promise<import('./portable-update').UpdateState & {canApply:boolean}>;
 auxiliary(params:{seed:number;playthrough:number}):Promise<NonNullable<CandidateView['auxiliary']>&{initial_challenge_capacity:number}>;
 dataDirectory(action:'inspect'|'set'|'reset'|'open'):Promise<{data_directory:string;restart_required:boolean}|null>;
 openSaveFolder(params:{save_id:string;snapshot_id:string}):Promise<void>;
 log(message:string):Promise<void>;
 copyLog():Promise<void>;
 windowAction(action:'minimize'|'maximize'|'close'):Promise<void>;
 retain(params:{job_id:string;candidate_id:string;source?:'runtime'|'search'}):Promise<{reference_id:string}>;
 release(referenceId:string):Promise<void>;
 preview(params:{seed:number;rarity:3|4|5;level:number;retain?:boolean}):Promise<{candidate:CandidateView;reference_id:string|null}>;
 prepareCart(params:{mode:'save'|'live';save_id:string;snapshot_id:string;references:string[];recommended_level:number;transfer_count:number}):Promise<ProtectedResult>;
 openBackupFolder():Promise<void>;
 openLink(name:'github'|'qq'|'updates'):Promise<void>;
 copyText(text:string):Promise<void>;
}
declare global {interface Window {review:ReviewApi}}
