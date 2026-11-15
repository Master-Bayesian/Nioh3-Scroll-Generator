import type { WorkerRequest } from '../../../packages/contracts/generated';

export type StartParams = Extract<WorkerRequest, { method: 'search.start' }>['params'];
