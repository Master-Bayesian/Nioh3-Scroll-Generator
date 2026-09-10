import type {CandidateTransfer} from '../../../packages/contracts/responses';
/** Bounded broker-owned records. Renderer references never contain record bytes. */
export class CandidateRegistry {
 private records=new Map<string,CandidateTransfer>();
 constructor(private maximum=350){}
 retain(value:CandidateTransfer){const key=value.candidate_id;if(!this.records.has(key)&&this.records.size>=this.maximum)throw new Error('CART_CAPACITY_REACHED');this.records.set(key,structuredClone(value));return {reference_id:key}}
 clear(){this.records.clear()}
 release(key:string){this.records.delete(key)}
 resolve(keys:string[]){if(!Array.isArray(keys)||keys.length<1||keys.length>Math.min(this.maximum,200)||new Set(keys).size!==keys.length)throw new Error('INVALID_CART_SELECTION');return keys.map(key=>{const value=this.records.get(key);if(!value)throw new Error('CART_REFERENCE_EXPIRED');return structuredClone(value)})}
}
