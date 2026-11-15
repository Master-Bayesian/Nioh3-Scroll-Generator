import type { Query } from "./model";
import type { ResultOrigin } from "./result-source";

/** Seed generation plus the output settings shown with the known-ID action. */
export type KnownIdSettings = Pick<Query, "ng" | "rarity" | "level" | "recommended" | "transfers">;

export type KnownIdPreviewState =
  | { kind: "not_preview" }
  | { kind: "empty" }
  | { kind: "pending" }
  | { kind: "current" }
  | { kind: "stale" };

export function isKnownIdPreview(origin: ResultOrigin): boolean {
  return origin === "known_id" || origin === "demo_known_id";
}

export function parseKnownScrollId(input: string): number | null {
  if (!input.trim()) return null;
  const seed = Number(input);
  return Number.isInteger(seed) && seed >= 0 && seed <= 0xffffffff ? seed : null;
}

/** Search-only filters and the result count cannot invalidate a known-ID preview. */
export function knownIdPreviewState({ origin, input, candidateSeed, displayed, current, pending }: {
  origin: ResultOrigin;
  input: string;
  candidateSeed: string | undefined;
  displayed: KnownIdSettings;
  current: KnownIdSettings;
  pending: boolean;
}): KnownIdPreviewState {
  if (!isKnownIdPreview(origin)) return { kind: "not_preview" };
  if (pending) return { kind: "pending" };
  if (candidateSeed === undefined) return { kind: "empty" };
  const sameSeed = parseKnownScrollId(input) === Number(candidateSeed);
  const sameSettings = displayed.ng === current.ng && displayed.rarity === current.rarity &&
    displayed.level === current.level && displayed.recommended === current.recommended &&
    displayed.transfers === current.transfers;
  return { kind: sameSeed && sameSettings ? "current" : "stale" };
}

export function knownIdUseBlocked(state: KnownIdPreviewState): boolean {
  switch (state.kind) {
    case "not_preview":
    case "current":
      return false;
    case "empty":
    case "pending":
    case "stale":
      return true;
  }
}
