export function hashFile(path: string, filesystem?: typeof import('node:fs')): Promise<string>;
export function verifyPortable(directory: string, filesystem?: typeof import('node:fs')): Promise<{ version: string; fileCount: number; signed: boolean; manifestSha256: string }>;
