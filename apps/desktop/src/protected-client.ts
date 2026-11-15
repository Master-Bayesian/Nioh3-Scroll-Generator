import type { ProtectedRequest } from '../../../packages/contracts/protected-requests';

export type ProtectedMethod = ProtectedRequest['method'];
export type ProtectedParams<M extends ProtectedMethod> = Extract<ProtectedRequest, { method: M }>['params'];
