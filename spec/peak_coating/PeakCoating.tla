---------------------------- MODULE PeakCoating ----------------------------
EXTENDS Naturals, Sequences
CONSTANT Fault
VARIABLES phase, held, ready, charges, recipePower, choice, projection, scored
vars == <<phase,held,ready,charges,recipePower,choice,projection,scored>>

None == <<"none",0>>
Init == /\ phase = "idle" /\ held = FALSE /\ ready = FALSE /\ charges = 0
        /\ recipePower = 0 /\ choice = "none" /\ projection = None /\ scored = <<>>

Pick(h,r,q,p,c) ==
    LET a == IF h THEN 30 ELSE 0
        b == IF r /\ (q > 0 \/ Fault = "SpendEmptySlot") THEN p ELSE 0
        power == IF Fault = "StackSameElement" THEN a + b ELSE IF a >= b THEN a ELSE b
    IN IF c = "water" /\ power > 0 THEN <<"water",power>> ELSE None

Bind(h,r,q,p,c) ==
    /\ phase \in {"idle","finished"} /\ phase' = "first"
    /\ held' = h /\ ready' = r /\ charges' = q /\ recipePower' = p /\ choice' = c
    /\ projection' = IF phase = "finished" /\ Fault = "StaleOutfitPower"
                      THEN projection ELSE Pick(h,r,q,p,c)
    /\ scored' = <<>>

Contact ==
    /\ phase \in {"first","second"}
    /\ phase' = IF phase = "first" THEN "second" ELSE "finished"
    /\ scored' = Append(scored, IF phase = "second" /\ Fault = "DropCoating" THEN None ELSE projection)
    /\ UNCHANGED <<held,ready,charges,recipePower,choice,projection>>

Next == (\E h,r \in BOOLEAN, q \in {0,1}, p \in {25,43,44}, c \in {"none","water"}:
          Bind(h,r,q,p,c)) \/ Contact
Spec == Init /\ [][Next]_vars

Eligible == {0} \cup (IF held /\ choice = "water" THEN {30} ELSE {})
                \cup (IF ready /\ charges > 0 /\ choice = "water" THEN {recipePower} ELSE {})
PeakCoating == phase = "finished" =>
    /\ Len(scored) = 2
    /\ \A hit \in {scored[1], scored[2]}:
        /\ hit[2] \in Eligible
        /\ \A offered \in Eligible: hit[2] >= offered
        /\ hit[1] = IF hit[2] = 0 THEN "none" ELSE "water"
=============================================================================
