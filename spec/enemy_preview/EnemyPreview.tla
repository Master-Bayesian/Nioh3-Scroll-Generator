------------------------------ MODULE EnemyPreview ------------------------------
EXTENDS Naturals
CONSTANTS PreserveKnownRoster, RequireResolvedRosterForEmpty, PreserveUnknownStates
VARIABLES base, detailed, resolved, display, visible, claimsSpecialState
vars == <<base, detailed, resolved, display, visible, claimsSpecialState>>
Init == /\ base = 0 /\ detailed = 0 /\ resolved = FALSE
        /\ display = "unknown" /\ visible = 0 /\ claimsSpecialState = FALSE
ReceivePreview ==
    /\ base' \in 0..2 /\ detailed' \in 0..2 /\ resolved' \in BOOLEAN
    /\ display' = IF detailed' > 0 THEN "detailed"
                  ELSE IF base' > 0 THEN IF PreserveKnownRoster THEN "base_only" ELSE "unknown"
                  ELSE IF resolved' \/ ~RequireResolvedRosterForEmpty THEN "empty" ELSE "unknown"
    /\ visible' = IF display' = "detailed" THEN detailed'
                  ELSE IF display' = "base_only" THEN base' ELSE 0
    /\ claimsSpecialState' = ((detailed' > 0) \/ (~PreserveUnknownStates /\ base' > 0))
Spec == Init /\ [][ReceivePreview]_vars
TypeOK == /\ base \in 0..2 /\ detailed \in 0..2 /\ resolved \in BOOLEAN
          /\ display \in {"detailed", "base_only", "empty", "unknown"}
          /\ visible \in 0..2 /\ claimsSpecialState \in BOOLEAN
KnownEnemiesRemainVisible == (base > 0 \/ detailed > 0) => visible > 0
OnlyResolvedEmptyRosterIsEmpty == display = "empty" => (resolved /\ base = 0 /\ detailed = 0)
BaseRosterDoesNotCertifySpecialStates == detailed = 0 => ~claimsSpecialState
=============================================================================
