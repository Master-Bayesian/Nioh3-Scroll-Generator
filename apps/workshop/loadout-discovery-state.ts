/** Installed optimizer data has its own explicit, recoverable discovery owner. */
export type LoadoutDiscoveryData = { bundles: unknown[] };
export type LoadoutDiscoveryState<T extends LoadoutDiscoveryData> =
  | { kind: 'Idle'; requestId: number }
  | { kind: 'Loading'; requestId: number }
  | { kind: 'Ready'; requestId: number; data: T }
  | { kind: 'Absent'; requestId: number }
  | { kind: 'Failed'; requestId: number; message: string };
export type LoadoutDiscoveryEvent<T extends LoadoutDiscoveryData> =
  | { type: 'Requested'; requestId: number }
  | { type: 'Resolved'; requestId: number; data: T }
  | { type: 'Failed'; requestId: number; message: string };

export function initialLoadoutDiscoveryState<T extends LoadoutDiscoveryData>(): LoadoutDiscoveryState<T> {
  return { kind: 'Idle', requestId: 0 };
}

export function loadoutDiscoveryReducer<T extends LoadoutDiscoveryData>(
  state: LoadoutDiscoveryState<T>, event: LoadoutDiscoveryEvent<T>,
): LoadoutDiscoveryState<T> {
  switch (event.type) {
    case 'Requested':
      return event.requestId > state.requestId ? { kind: 'Loading', requestId: event.requestId } : state;
    case 'Resolved':
      if (state.kind !== 'Loading' || event.requestId !== state.requestId) return state;
      return event.data.bundles.length > 0
        ? { kind: 'Ready', requestId: event.requestId, data: event.data }
        : { kind: 'Absent', requestId: event.requestId };
    case 'Failed':
      if (state.kind !== 'Loading' || event.requestId !== state.requestId) return state;
      return { kind: 'Failed', requestId: event.requestId, message: event.message };
  }
}

/** A failed read is an available retry route,not evidence of absent content. */
export function loadoutDiscoveryVisible<T extends LoadoutDiscoveryData>(state: LoadoutDiscoveryState<T>): boolean {
  switch (state.kind) {
    case 'Idle':
    case 'Absent': return false;
    case 'Loading':
    case 'Ready':
    case 'Failed': return true;
  }
}
