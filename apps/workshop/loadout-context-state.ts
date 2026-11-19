import type { LoadoutContextReady, LoadoutSearch, SaveCharacter } from "../../packages/contracts/protected-responses";

/** Every result belongs to the exact save bytes, bundle and player context that produced it. */
export interface LoadoutScope {
  saveId: string;
  sourceSha256: string;
  bundleId: string;
}

export interface LoadoutWearBinding extends LoadoutScope {
  contextId: string;
  inputKey: string;
  requestId: number;
}

type Base = { scope: LoadoutScope | null; inputKey: string };
type WithContext = Base & { scope: LoadoutScope; context: LoadoutContextReady };
export type LoadoutContextState =
  | (Base & { kind: "Missing" })
  | (Base & { kind: "Reading"; scope: LoadoutScope; requestId: number })
  | (WithContext & { kind: "Ready" })
  | (WithContext & { kind: "Searching" | "Cancelling"; requestId: number; requestInputKey: string })
  | (WithContext & { kind: "Interrupted"; requestId: number; requestInputKey: string; intent: "resume" | "stop"; message: string })
  | (WithContext & { kind: "Cancelled"; requestId: number })
  | (WithContext & { kind: "Results"; requestId: number; result: LoadoutSearch; source: SaveCharacter })
  | (Base & { kind: "Failed"; stage: "read" | "search"; message: string; context?: LoadoutContextReady });

export type LoadoutContextEvent =
  | { type: "ScopeChanged"; scope: LoadoutScope | null }
  | { type: "InputsChanged"; inputKey: string }
  | { type: "ReadStarted"; scope: LoadoutScope; requestId: number; inputKey: string }
  | { type: "ReadReady"; scope: LoadoutScope; requestId: number; context: LoadoutContextReady }
  | { type: "ReadFailed"; scope: LoadoutScope; requestId: number; message: string }
  | { type: "SearchStarted"; scope: LoadoutScope; contextId: string; requestId: number; inputKey: string }
  | { type: "SearchCancelRequested"; scope: LoadoutScope; contextId: string; requestId: number; inputKey: string }
  | { type: "SearchInterrupted"; scope: LoadoutScope; contextId: string; requestId: number; inputKey: string; message: string }
  | { type: "SearchResumed" | "SearchRetired"; scope: LoadoutScope; contextId: string; requestId: number; inputKey: string }
  | { type: "SearchCancelled"; scope: LoadoutScope; contextId: string; requestId: number; inputKey: string }
  | { type: "SearchReady"; scope: LoadoutScope; contextId: string; requestId: number; inputKey: string; result: LoadoutSearch; source: SaveCharacter }
  | { type: "SearchFailed"; scope: LoadoutScope; contextId: string; requestId: number; inputKey: string; message: string };

export const initialLoadoutContextState: LoadoutContextState = { kind: "Missing", scope: null, inputKey: "" };

export function sameLoadoutScope(a: LoadoutScope | null, b: LoadoutScope | null): boolean {
  return a === b || !!a && !!b && a.saveId === b.saveId && a.sourceSha256 === b.sourceSha256 && a.bundleId === b.bundleId;
}

export function contextMatchesScope(context: LoadoutContextReady, scope: LoadoutScope): boolean {
  return context.state === "ready" && !!context.context_id && context.save_id === scope.saveId &&
    context.source_sha256 === scope.sourceSha256 && context.bundle === scope.bundleId && context.attrs.length === 7 &&
    context.attrs.every(value => Number.isInteger(value) && value >= 0) &&
    (context.source === "live" || context.source === "captured");
}

export function currentLoadoutContext(state: LoadoutContextState, scope: LoadoutScope | null): LoadoutContextReady | null {
  if (!scope || !sameLoadoutScope(state.scope, scope) || !("context" in state) || !state.context) return null;
  return contextMatchesScope(state.context, scope) ? state.context : null;
}

export function canWearLoadout(state: LoadoutContextState, binding: LoadoutWearBinding, scope: LoadoutScope | null, inputKey: string): boolean {
  return state.kind === "Results" && sameLoadoutScope(state.scope, scope) && sameLoadoutScope(state.scope, binding) &&
    state.context.context_id === binding.contextId && state.result.context_id === binding.contextId &&
    state.requestId === binding.requestId && state.inputKey === binding.inputKey && inputKey === binding.inputKey &&
    state.result.bundle === binding.bundleId && state.source.save_id === binding.saveId && state.source.source_sha256 === binding.sourceSha256;
}

export function loadoutContextReducer(state: LoadoutContextState, event: LoadoutContextEvent): LoadoutContextState {
  switch (event.type) {
    case "ScopeChanged":
      return sameLoadoutScope(state.scope, event.scope) ? state : { kind: "Missing", scope: event.scope, inputKey: state.inputKey };
    case "InputsChanged": {
      if (event.inputKey === state.inputKey) return state;
      if (state.kind === "Interrupted") return { ...state, inputKey: event.inputKey, intent: "stop" };
      if (state.kind === "Searching") return { ...state, kind: "Cancelling", inputKey: event.inputKey };
      if (state.kind === "Results" && state.scope)
        return { kind: "Ready", scope: state.scope, context: state.context, inputKey: event.inputKey };
      return { ...state, inputKey: event.inputKey };
    }
    case "ReadStarted":
      return { kind: "Reading", scope: event.scope, requestId: event.requestId, inputKey: event.inputKey };
    case "ReadReady":
      if (state.kind !== "Reading" || state.requestId !== event.requestId || !sameLoadoutScope(state.scope, event.scope)) return state;
      if (!contextMatchesScope(event.context, event.scope))
        return { kind: "Failed", scope: state.scope, inputKey: state.inputKey, stage: "read", message: "角色上下文与当前存档不一致，请重新读取。" };
      return { kind: "Ready", scope: event.scope, inputKey: state.inputKey, context: event.context };
    case "ReadFailed":
      if (state.kind !== "Reading" || state.requestId !== event.requestId || !sameLoadoutScope(state.scope, event.scope)) return state;
      return { kind: "Failed", scope: state.scope, inputKey: state.inputKey, stage: "read", message: event.message };
    case "SearchStarted": {
      const context = currentLoadoutContext(state, event.scope);
      if (!context || context.context_id !== event.contextId || event.inputKey !== state.inputKey || (state.kind === "Searching" || state.kind === "Cancelling" || state.kind === "Interrupted")) return state;
      return { kind: "Searching", scope: event.scope, inputKey: event.inputKey, context, requestId: event.requestId, requestInputKey: event.inputKey };
    }
    case "SearchCancelRequested":
      if ((state.kind !== "Searching" && state.kind !== "Interrupted") || state.requestId !== event.requestId || !sameLoadoutScope(state.scope, event.scope) ||
          state.context.context_id !== event.contextId || state.requestInputKey !== event.inputKey) return state;
      return { ...state, kind: "Cancelling" };
    case "SearchInterrupted":
      if ((state.kind !== "Searching" && state.kind !== "Cancelling" && state.kind !== "Interrupted") || state.requestId !== event.requestId ||
          !sameLoadoutScope(state.scope, event.scope) || state.context.context_id !== event.contextId || state.requestInputKey !== event.inputKey) return state;
      return { ...state, kind: "Interrupted", intent: state.kind === "Cancelling" || state.kind === "Interrupted" && state.intent === "stop" ? "stop" : "resume", message: event.message };
    case "SearchResumed":
    case "SearchRetired":
      if (state.kind !== "Interrupted" || state.requestId !== event.requestId || !sameLoadoutScope(state.scope, event.scope) ||
          state.context.context_id !== event.contextId || state.requestInputKey !== event.inputKey) return state;
      if (event.type === "SearchRetired") return { kind: "Ready", scope: state.scope, context: state.context, inputKey: state.inputKey };
      return { kind: state.intent === "stop" ? "Cancelling" : "Searching", scope: state.scope, context: state.context,
        inputKey: state.inputKey, requestInputKey: state.requestInputKey, requestId: state.requestId };
    case "SearchCancelled":
      if ((state.kind !== "Searching" && state.kind !== "Cancelling" && state.kind !== "Interrupted") || state.requestId !== event.requestId ||
          !sameLoadoutScope(state.scope, event.scope) || state.context.context_id !== event.contextId || state.requestInputKey !== event.inputKey) return state;
      return { kind: "Cancelled", scope: state.scope, context: state.context, inputKey: state.inputKey, requestId: state.requestId };
    case "SearchReady":
    case "SearchFailed": {
      if ((state.kind !== "Searching" && state.kind !== "Cancelling" && state.kind !== "Interrupted") || state.requestId !== event.requestId || !sameLoadoutScope(state.scope, event.scope) ||
          state.context.context_id !== event.contextId || state.requestInputKey !== event.inputKey) return state;
      if ((state.kind === "Cancelling" || state.kind === "Interrupted" && state.intent === "stop") && event.type === "SearchReady")
        return { kind: "Cancelled", scope: state.scope, context: state.context, inputKey: state.inputKey, requestId: state.requestId };
      if (state.inputKey !== event.inputKey) return { kind: "Ready", scope: state.scope, context: state.context, inputKey: state.inputKey };
      if (event.type === "SearchFailed")
        return { kind: "Failed", scope: state.scope, inputKey: state.inputKey, stage: "search", context: state.context, message: event.message };
      if (event.result.context_id !== event.contextId || event.result.bundle !== event.scope.bundleId ||
          event.source.save_id !== event.scope.saveId || event.source.source_sha256 !== event.scope.sourceSha256)
        return { kind: "Failed", scope: state.scope, inputKey: state.inputKey, stage: "search", context: state.context, message: "搜索结果已过期，请重新读取角色后搜索。" };
      return { kind: "Results", scope: state.scope, inputKey: state.inputKey, context: state.context,
        requestId: event.requestId, result: event.result, source: event.source };
    }
  }
}
