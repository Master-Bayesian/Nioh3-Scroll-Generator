import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import Ajv from 'ajv';
import requestSchema from '../../../packages/contracts/protected-request.schema.json';
import responseSchema from '../../../packages/contracts/protected-response.schema.json';
import type { ProtectedRequest } from '../../../packages/contracts/protected-requests';
import type { ProtectedResponse } from '../../../packages/contracts/protected-responses';

export type ProtectedMethod = ProtectedRequest['method'];
export type ProtectedParams<M extends ProtectedMethod> = Extract<ProtectedRequest, { method: M }>['params'];
export interface OperationJob {
  job_id: string; kind: string; state: 'running' | 'cancel_requested' | 'completed' | 'failed';
  sequence: number; cancellable: boolean; progress: Record<string, unknown> | null;
  result: Record<string, unknown> | null; error: { code: string; message: string } | null;
}
const ajv = new Ajv({ strict: false });
const validRequest = ajv.compile(requestSchema), validResponse = ajv.compile(responseSchema);

/** Protected hosts own writes/hooks. No code path in this client kills them. */
export class ProtectedClient {
  private child: ChildProcessWithoutNullStreams;
  private buffer = Buffer.alloc(0);
  private pending = new Map<string, { resolve: (result: any) => void; reject: (error: Error) => void; timer: NodeJS.Timeout }>();
  private expired = new Set<string>();
  private dead: Error | null = null;
  private negotiated: Promise<unknown> | null = null;
  private safelyClosed = false;
  private identity: { contextDigest: string | null; contractDigest: string } | null = null;
  constructor(readonly root: string, executable: string, readonly role: 'save' | 'runtime', packaged = false, log?:(message:string)=>void) {
    this.child = spawn(executable, [...(packaged ? [] : ['-u', '-m', 'nioh3_scroll_editor.protected_worker']), '--role', role],
      { cwd: root, shell: false, windowsHide: true, stdio: 'pipe', env: { ...process.env, PYTHONUTF8: '1', PYTHONIOENCODING: 'utf-8' } });
    this.child.stdout.on('data', (bytes: Buffer) => this.receive(bytes));
    this.child.stderr.on('data',(bytes:Buffer)=>log?.(bytes.toString('utf8').slice(-8192)));
    this.child.on('error', error => this.fail(error));
    this.child.stdin.on('error', error => this.fail(error));
    this.child.on('exit', () => this.fail(new Error('PROTECTED_HOST_EXITED: verify operation receipt or runtime state')));
  }
  diagnostics(): import('./support-api').WorkerDiagnostic {
    return { role: this.role, connection: this.safelyClosed ? 'closed' : this.dead ? 'unavailable' : this.identity ? 'ready' : 'starting',
      contextDigest: this.identity?.contextDigest ?? null, contractDigest: this.identity?.contractDigest ?? null };
  }
  private fail(error: Error) {
    if (this.dead) return;
    this.dead = error;
    for (const item of this.pending.values()) { clearTimeout(item.timer); item.reject(error); }
    this.pending.clear();
    this.child.stdin.end(); // EOF lets the host finish writes and restore hooks.
  }
  private receive(bytes: Buffer) {
    if (this.dead) return;
    this.buffer = Buffer.concat([this.buffer, bytes]);
    while (this.buffer.length >= 4) {
      const size = this.buffer.readUInt32LE();
      if (!size || size > 4194304) return this.fail(new Error('INVALID_FRAME_SIZE'));
      if (this.buffer.length < size + 4) return;
      const frame = this.buffer.subarray(4, size + 4); this.buffer = this.buffer.subarray(size + 4);
      try {
        const value: unknown = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(frame));
        if (!validResponse(value)) throw new Error('INVALID_PROTECTED_RESPONSE');
        const response = value as ProtectedResponse;
        if (!response.id) throw new Error('MISSING_RESPONSE_ID');
        if (this.expired.delete(response.id)) continue;
        const item = this.pending.get(response.id);
        if (!item) throw new Error('UNEXPECTED_RESPONSE_ID');
        this.pending.delete(response.id); clearTimeout(item.timer);
        if (response.ok) item.resolve(response.result);
        else item.reject(new Error(`${response.error.code}: ${response.error.message}`));
      } catch (error) { return this.fail(error as Error); }
    }
  }
  private request(method: ProtectedMethod, params: unknown, timeout = 30000): Promise<any> {
    if (this.dead) return Promise.reject(this.dead);
    if (this.pending.size >= 8 || this.expired.size >= 64) return Promise.reject(new Error('PROTECTED_HOST_UNRESPONSIVE'));
    const id = randomUUID(), message = { protocol: 1, id, method, params };
    if (!validRequest(message)) {const branch=requestSchema.oneOf.findIndex(s=>s.properties.method.const===method);const details=validRequest.errors?.filter(e=>e.schemaPath.startsWith('#/oneOf/'+branch+'/')).map(e=>e.instancePath+' '+e.message+' '+JSON.stringify(e.params)).slice(0,3).join('; ');return Promise.reject(new Error('INVALID_REQUEST '+method+': '+details));}
    const bytes = Buffer.from(JSON.stringify(message));
    if (bytes.length > 4194304) return Promise.reject(new Error('FRAME_TOO_LARGE'));
    const header = Buffer.alloc(4); header.writeUInt32LE(bytes.length);
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id); this.expired.add(id);
        reject(new Error('PROTECTED_HOST_TIMEOUT: outcome unknown; never replay automatically'));
      }, timeout);
      this.pending.set(id, { resolve, reject, timer });
      this.child.stdin.write(Buffer.concat([header, bytes]), error => { if (error) this.fail(error); });
    });
  }
  handshake() {
    return this.negotiated ??= this.request('handshake', {}).then(result => {
      const digest = createHash('sha256').update(readFileSync(resolve(this.root, 'packages/contracts/protected-request.schema.json')))
        .update(readFileSync(resolve(this.root, 'packages/contracts/protected-response.schema.json'))).digest('hex');
      if (result.role !== this.role || result.kill_safe !== false || result.contract_digest !== digest) {
        const error = new Error('PROTECTED_CONTRACT_MISMATCH'); this.fail(error); throw error;
      }
      this.identity = { contextDigest: result.context?.context_digest ?? null, contractDigest: result.contract_digest };
      return result;
    });
  }
  async call<M extends ProtectedMethod>(method: M, params: ProtectedParams<M>): Promise<any> {
    await this.handshake(); return this.request(method, params);
  }
  async run<M extends ProtectedMethod>(method: M, params: ProtectedParams<M>): Promise<Record<string, unknown>> {
    let job: OperationJob = await this.call(method, params);
    while (job.state === 'running' || job.state === 'cancel_requested') {
      await new Promise(resolve => setTimeout(resolve, 80));
      job = await this.call('job.snapshot', { job_id: job.job_id });
    }
    if (job.state === 'failed') throw new Error(`${job.error?.code}: ${job.error?.message}`);
    return job.result!;
  }
  async close(): Promise<boolean> {
    if (this.safelyClosed) return true;
    if (this.dead) return false; // Pipe loss cannot prove a restored runtime hook.
    try {
      await this.handshake();
      const result = await this.request('shutdown', {}, 5000);
      if (result.safe_to_shutdown !== true) return false;
      this.safelyClosed = true; this.child.stdin.end(); return true;
    } catch { return false; }
  }
}
