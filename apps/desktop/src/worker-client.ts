import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import Ajv from 'ajv';
import requestSchema from '../../../packages/contracts/request.schema.json';
import responseSchema from '../../../packages/contracts/response.schema.json';
import type { WorkerRequest } from '../../../packages/contracts/generated';
import type { WorkerResponse, JobSnapshot, Handshake, SearchCatalog, RecommendedLevelResolution } from '../../../packages/contracts/responses';
import type { WorkerDiagnostic } from './support-api';

export type StartParams = Extract<WorkerRequest, { method: 'search.start' }>['params'];
const ajv = new Ajv({ strict: false });
const validRequest = ajv.compile(requestSchema);
const validResponse = ajv.compile(responseSchema);
const MAX_FRAME = 4 * 1024 * 1024;
const MAX_PENDING = 8;

export class WorkerClient {
  private child: ChildProcessWithoutNullStreams;
  private pending = new Map<string, { resolve: (value: WorkerResponse) => void; reject: (error: Error) => void; timer: NodeJS.Timeout }>();
  private buffer = Buffer.alloc(0);
  private dead: Error | null = null;
  private closing = false;
  private handshakeValue: Handshake | null = null;
  private submitted: { jobId: string; params: StartParams } | null = null;
  readonly stderr: string[] = [];

  constructor(readonly root: string, python: string, executable = false, log?:(message:string)=>void) {
    this.child = spawn(python, executable ? [] : ['-u', '-m', 'nioh3_scroll_editor.search_worker'], {
      cwd: root, shell: false, windowsHide: true, stdio: 'pipe',
      env: { ...process.env, PYTHONIOENCODING: 'utf-8', PYTHONUTF8: '1' },
    });
    this.child.stdout.on('data', (chunk: Buffer) => this.receive(chunk));
    this.child.stderr.on('data', (chunk: Buffer) => {
      const text=chunk.toString('utf8').slice(-8192);log?.(text);this.stderr.push(text);
      if (this.stderr.length > 16) this.stderr.shift();
    });
    this.child.on('error', (error) => this.fail(error));
    this.child.stdin.on('error', (error) => this.fail(error));
    this.child.on('exit', (code, signal) => this.fail(new Error(`WORKER_EXITED: ${code ?? signal}`)));
  }

  get pid() { return this.child.pid; }

  diagnostics(): WorkerDiagnostic {
    return { role: 'offline_search', connection: this.closing ? 'closed' : this.dead ? 'unavailable' : this.handshakeValue ? 'ready' : 'starting',
      contextDigest: this.handshakeValue?.context.context_digest ?? null, contractDigest: this.handshakeValue?.contract_digest ?? null };
  }

  private fail(error: Error) {
    if (this.dead) return;
    this.dead = error;
    for (const item of this.pending.values()) {
      clearTimeout(item.timer); item.reject(error);
    }
    this.pending.clear();
    // This role is offline-only. Never reuse this kill policy for a save/runtime host.
    if (this.child.exitCode === null) this.child.kill();
  }

  private receive(chunk: Buffer) {
    if (this.dead) return;
    this.buffer = Buffer.concat([this.buffer, chunk]);
    while (this.buffer.length >= 4) {
      const size = this.buffer.readUInt32LE(0);
      if (size === 0 || size > MAX_FRAME) return this.fail(new Error('INVALID_FRAME_SIZE'));
      if (this.buffer.length < size + 4) return;
      const bytes = this.buffer.subarray(4, size + 4);
      this.buffer = this.buffer.subarray(size + 4);
      try {
        const value: unknown = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
        if (!validResponse(value)) throw new Error('INVALID_WORKER_RESPONSE');
        const response = value as WorkerResponse;
        const item = response.id ? this.pending.get(response.id) : undefined;
        if (!item) throw new Error('UNEXPECTED_RESPONSE_ID');
        this.pending.delete(response.id!); clearTimeout(item.timer); item.resolve(response);
      } catch (error) { return this.fail(error instanceof Error ? error : new Error(String(error))); }
    }
  }

  private async request(method: WorkerRequest['method'], params: unknown): Promise<WorkerResponse> {
    if (this.dead) throw this.dead;
    if (this.closing && method !== 'shutdown') throw new Error('WORKER_CLOSING');
    if (this.pending.size >= MAX_PENDING) throw new Error('TOO_MANY_REQUESTS');
    const id = randomUUID();
    const payload = { protocol: 1, id, method, params };
    if (!validRequest(payload)) throw new Error('INVALID_REQUEST');
    const bytes = Buffer.from(JSON.stringify(payload), 'utf8');
    const header = Buffer.alloc(4); header.writeUInt32LE(bytes.length);
    return new Promise((resolveResponse, reject) => {
      const timer = setTimeout(() => this.fail(new Error('WORKER_TIMEOUT')), 30_000);
      this.pending.set(id, { resolve: resolveResponse, reject, timer });
      this.child.stdin.write(Buffer.concat([header, bytes]), (error) => { if (error) this.fail(error); });
    });
  }

  private async result(method: WorkerRequest['method'], params: unknown) {
    const response = await this.request(method, params);
    if (!response.ok) throw new Error(`${response.error.code}: ${response.error.message}`);
    return response.result;
  }

  async handshake(): Promise<Handshake> {
    if (this.handshakeValue) return this.handshakeValue;
    const result = await this.result('handshake', {});
    if (!('role' in result) || result.role !== 'offline_search') throw new Error('ROLE_MISMATCH');
    const digest = createHash('sha256')
      .update(readFileSync(resolve(this.root, 'packages/contracts/request.schema.json')))
      .update(readFileSync(resolve(this.root, 'packages/contracts/response.schema.json'))).digest('hex');
    if (result.contract_digest !== digest) { this.fail(new Error('CONTRACT_MISMATCH')); throw this.dead; }
    this.handshakeValue = result;
    return result;
  }

  private async job(method: 'search.start' | 'job.snapshot' | 'job.cancel', params: unknown): Promise<JobSnapshot> {
    await this.handshake();
    const result = await this.result(method, params);
    if (!('job_id' in result)) throw new Error('JOB_RESPONSE_EXPECTED');
    return result;
  }
  async start(params: StartParams) {
    const frozen = structuredClone(params);
    const job = await this.job('search.start', frozen);
    this.submitted = { jobId: job.job_id, params: frozen };
    return job;
  }
  async current() {
    await this.handshake();
    const result = await this.result('job.current', {});
    if (!('job' in result)) throw new Error('CURRENT_SEARCH_EXPECTED');
    return { job: result.job, submitted: result.job && this.submitted?.jobId === result.job.job_id
      ? structuredClone(this.submitted.params) : null };
  }
  async catalog(rarity: 3 | 4 | 5, locale: 'en-US' | 'zh-CN' | 'ja-JP'): Promise<SearchCatalog> {
    await this.handshake();
    const result = await this.result('search.catalog', { playthrough: 3, rarity, locale });
    if (!('ordinary_effects' in result)) throw new Error('CATALOG_RESPONSE_EXPECTED');
    return result;
  }
  snapshot(jobId: string) { return this.job('job.snapshot', { job_id: jobId }); }
  async resolveRecommendedLevel(displayedLevel: number): Promise<RecommendedLevelResolution> {
    await this.handshake();
    const result = await this.result('recommended_level.resolve', { displayed_level: displayedLevel });
    if (!('requested_displayed_level' in result)) throw new Error('RECOMMENDED_LEVEL_RESPONSE_EXPECTED');
    return result;
  }
  cancel(jobId: string) { return this.job('job.cancel', { job_id: jobId }); }
  async exportCandidate(jobId: string, candidateId: string) {
    await this.handshake();
    const result = await this.result('candidate.export', { job_id: jobId, candidate_id: candidateId });
    if (!('record_hex' in result)) throw new Error('CANDIDATE_TRANSFER_EXPECTED');
    return result;
  }
  async previewSeed(params: {seed:number;rarity:3|4|5;level:number}) {
    await this.handshake(); const result=await this.result('candidate.preview',params);
    if(!('transfer' in result))throw new Error('PREVIEW_RESPONSE_EXPECTED');return result;
  }
  async registerCache(cacheJson: string) {
    await this.handshake();
    const result = await this.result('cache.register', { cache_json: cacheJson });
    if (!('cache_id' in result)) throw new Error('CACHE_REFERENCE_EXPECTED');
    return result;
  }

  async close() {
    if (this.dead || this.closing) return;
    this.closing = true;
    const force = setTimeout(() => this.fail(new Error('OFFLINE_SHUTDOWN_TIMEOUT')), 5_000);
    try { await this.result('shutdown', {}); this.child.stdin.end(); }
    catch { /* A stopped offline worker is safe; requests already have explicit errors. */ }
    finally { clearTimeout(force); if (this.child.exitCode === null) this.child.kill(); }
  }
}
