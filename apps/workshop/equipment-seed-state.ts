import type { EquipmentSeeds } from "../../packages/contracts/protected-responses";

/** A response and its selection belong to exactly one search identity. */
export type EquipmentSeedState =
  | { phase: "idle"; identity: string }
  | { phase: "searching"; identity: string; requestId: number }
  | { phase: "empty"; identity: string; requestId: number; result: EquipmentSeeds }
  | { phase: "results"; identity: string; requestId: number; result: EquipmentSeeds }
  | { phase: "selected"; identity: string; requestId: number; result: EquipmentSeeds; candidateDigest: string }
  | { phase: "error"; identity: string; requestId: number; message: string };

export type EquipmentSeedAction =
  | { type: "invalidate"; identity: string }
  | { type: "start"; identity: string; requestId: number }
  | { type: "resolve"; identity: string; requestId: number; result: EquipmentSeeds }
  | { type: "reject"; identity: string; requestId: number; message: string }
  | { type: "select"; identity: string; candidateDigest: string };

export function equipmentSeedReducer(state: EquipmentSeedState, action: EquipmentSeedAction): EquipmentSeedState {
  switch (action.type) {
    case "invalidate":
      return { phase: "idle", identity: action.identity };
    case "start":
      return { phase: "searching", identity: action.identity, requestId: action.requestId };
    case "select":
      if (action.identity !== state.identity || (state.phase !== "results" && state.phase !== "selected") ||
        !state.result.outcomes.some(outcome => outcome.candidate_digest === action.candidateDigest)) return state;
      return { ...state, phase: "selected", candidateDigest: action.candidateDigest };
    case "reject":
      if (action.identity !== state.identity || state.phase !== "searching" || action.requestId !== state.requestId) return state;
      return { ...state, phase: "error", message: action.message };
    case "resolve":
      if (action.identity !== state.identity || state.phase !== "searching" || action.requestId !== state.requestId) return state;
      return {
        phase: action.result.outcomes.length ? "results" : "empty",
        identity: state.identity,
        requestId: state.requestId,
        result: action.result,
      };
  }
}
