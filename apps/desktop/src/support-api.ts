export interface WorkerDiagnostic {
  role: 'offline_search' | 'save' | 'runtime';
  connection: 'starting' | 'ready' | 'unavailable' | 'closed';
  contextDigest: string | null;
  contractDigest: string | null;
}
export interface DiagnosticReport {
  schema: 'nioh3-v2-diagnostics/v1';
  version: string;
  packaged: boolean;
  locale: string;
  platform: string;
  arch: string;
  runtimeVersions: { electron: string; node: string; chrome: string };
  packageVerification: { version: string; fileCount: number; signed: boolean; manifestSha256: string } | null;
  workers: WorkerDiagnostic[];
}
export interface SupportApi {
  diagnostics(): Promise<DiagnosticReport>;
  exportDiagnostics(): Promise<{ saved: boolean }>;
}
declare global { interface Window { support: SupportApi } }
