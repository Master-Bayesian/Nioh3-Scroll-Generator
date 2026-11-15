/** A known base roster remains useful when detailed enemy states are unavailable. */
export type EnemyOccurrence = {
  lookupKey: number;
  name: string;
  waveIndex: number;
  position: number;
  availability: "base" | "expedition_only";
  possessed: "yes" | "no" | "unknown";
  curse: "guaranteed" | "possible" | "never" | "unknown";
};

export type EnemyDisplayState =
  | { kind: "detailed"; occurrences: readonly EnemyOccurrence[] }
  | { kind: "base_only"; names: readonly string[]; statesUnavailable: boolean }
  | { kind: "empty" }
  | { kind: "unknown" };

export function enemyDisplayState(source: {
  enemies: readonly string[];
  enemyOccurrences?: readonly EnemyOccurrence[];
  enemyRosterComplete?: boolean;
  enemyStatesUnavailable?: boolean;
}): EnemyDisplayState {
  if (source.enemyOccurrences?.length) {
    return { kind: "detailed", occurrences: source.enemyOccurrences };
  }
  if (source.enemies.length) {
    return {
      kind: "base_only",
      names: source.enemies,
      statesUnavailable: source.enemyStatesUnavailable === true,
    };
  }
  // Only an explicitly resolved roster can establish that there are no enemies.
  // Older saved samples without that evidence remain unknown.
  return { kind: source.enemyRosterComplete === true ? "empty" : "unknown" };
}
