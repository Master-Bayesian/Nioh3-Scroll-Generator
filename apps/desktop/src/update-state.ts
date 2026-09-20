/** Updater state rendered by the review UI; the Tauri host owns the update workflow. */
export interface UpdateState {phase:'idle'|'checking'|'current'|'available'|'downloading'|'ready'|'failed';version?:string;notes?:string;downloaded?:number;total?:number;error?:string}
