import type {JobSnapshot} from '../../../packages/contracts/responses';
import type {ProtectedResult} from './operations-api';
export type CandidateView=JobSnapshot['candidates'][number];
export type GameFeatureSupport='supported'|'experimental'|'unsupported'|'unavailable';
export interface GameInstallationInspection {
 executable:string|null;
 file_version:string|null;
 restart_required:boolean;
 source:'selected'|'automatic';
 identity_error:{code:string;message:string}|null;
 compatibility:{
  status:'known'|'unknown'|'unavailable';
  display_version:string|null;
  data_version:string|null;
  resource_directory:string|null;
  runtime_profile:string|null;
  features:{offline_scroll_generation:GameFeatureSupport;character_read_edit:GameFeatureSupport;native_scroll_add:GameFeatureSupport;native_equipment_add:GameFeatureSupport};
  reason:string;
 };
}
export interface ReviewApi {
 favorites(params:{action:'list'|'add'|'remove';sample?:import('../../workshop/model').Sample;reference_id?:string;key?:string}):Promise<import('../../workshop/model').Sample[]>;
 update(params:{action:'status'|'check'|'download'|'apply';channel:'stable'|'beta'}):Promise<import('./update-state').UpdateState & {canApply:boolean}>;
 auxiliary(params:{seed:number;playthrough:number}):Promise<NonNullable<CandidateView['auxiliary']>&{initial_challenge_capacity:number}>;
 dataDirectory(action:'inspect'|'set'|'reset'|'open'):Promise<{data_directory:string;restart_required:boolean}|null>;
 gameInstallation(action:'inspect'|'select'|'reset'):Promise<GameInstallationInspection|null>;
 openSaveFolder(params:{save_id:string;snapshot_id:string}):Promise<void>;
 log(message:string):Promise<void>;
 copyLog():Promise<void>;
 /** Write one feedback file (diagnostics and recent log) and show it in Explorer. */
 exportFeedback():Promise<{path:string}>;
 windowAction(action:'minimize'|'maximize'|'close'):Promise<void>;
 retain(params:{job_id:string;candidate_id:string;source?:'runtime'|'search'}):Promise<{reference_id:string}>;
 release(referenceId:string):Promise<void>;
 preview(params:{seed:number;rarity:3|4|5;level:number;retain?:boolean;playthrough?:1|2|3}):Promise<{candidate:CandidateView;reference_id:string|null}>;
 prepareCart(params:{mode:'save'|'live';save_id:string;snapshot_id:string;references:string[];recommended_level:number;transfer_count:number}):Promise<ProtectedResult>;
 openBackupFolder():Promise<void>;
 openLink(name:'github'|'qq'|'updates'):Promise<void>;
 copyText(text:string):Promise<void>;
}
declare global {interface Window {review:ReviewApi}}
