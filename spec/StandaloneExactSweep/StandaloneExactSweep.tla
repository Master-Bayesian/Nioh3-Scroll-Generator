-------------------------- MODULE StandaloneExactSweep --------------------------
EXTENDS Naturals, FiniteSets, Sequences
CONSTANT Fault

Outfits == 1..4
MinimumTier == 3
Worn == <<2, 1, 3, 2>>
Reducer == <<1, 0, 0, 0>>
UpperPieces == <<2, 1, 3, 3>>
UpperReducer == IF Fault = "DropReduction" THEN <<0, 0, 0, 0>> ELSE Reducer
Tier(pieces, reduction) == IF pieces = 0 THEN 0 ELSE pieces + reduction
Eligible == {i \in Outfits : Tier(Worn[i], Reducer[i]) >= MinimumTier}
BoundedPlan == SelectSeq(<<1, 2, 3, 4>>,
    LAMBDA i: Tier(UpperPieces[i], UpperReducer[i]) >= MinimumTier)
Members(seq) == {seq[i] : i \in 1..Len(seq)}

Empty == [covered |-> 0, answered |-> 0, refused |-> 0, outside |-> 0]
VARIABLES phase, identity, plan, committed, pending, published
vars == <<phase, identity, plan, committed, pending, published>>

Init ==
    /\ phase = "Closed"
    /\ identity = "Original"
    /\ plan = <<>>
    /\ committed = Empty
    /\ pending = Empty
    /\ published = FALSE

Open ==
    /\ phase = "Closed"
    /\ phase' = "Ready"
    /\ identity' = "Original"
    /\ plan' = BoundedPlan
    /\ UNCHANGED <<committed, pending, published>>

WrongResume ==
    /\ Fault = "WrongDomain"
    /\ phase = "Closed"
    /\ committed.covered > 0
    /\ phase' = "Ready"
    /\ identity' = "Other"
    /\ UNCHANGED <<plan, committed, pending, published>>

Calculate ==
    /\ phase = "Ready"
    /\ committed.covered < Len(plan)
    /\ LET outfit == plan[committed.covered + 1]
           outcomes == IF outfit \in Eligible THEN {"Answered", "Refused"} ELSE {"Outside"}
       IN \E result \in outcomes :
            pending' = [covered |-> committed.covered + 1,
                        answered |-> committed.answered + IF result = "Answered" THEN 1 ELSE 0,
                        refused |-> committed.refused + IF result = "Refused" THEN 1 ELSE 0,
                        outside |-> committed.outside + IF result = "Outside" THEN 1 ELSE 0]
    /\ phase' = "Computed"
    /\ UNCHANGED <<identity, plan, committed, published>>

Commit ==
    /\ phase = "Computed"
    /\ committed' = IF Fault = "CursorOnly"
                    THEN [committed EXCEPT !.covered = pending.covered] ELSE pending
    /\ phase' = "Ready"
    /\ UNCHANGED <<identity, plan, pending, published>>

Publish ==
    /\ phase = "Ready"
    /\ committed.covered = Len(plan)
    /\ (committed.refused = 0 \/ Fault = "IgnoreRefusal")
    /\ published' = TRUE
    /\ UNCHANGED <<phase, identity, plan, committed, pending>>

Close ==
    /\ phase # "Closed"
    /\ phase' = "Closed"
    /\ pending' = committed
    /\ UNCHANGED <<identity, plan, committed, published>>

Next == Open \/ WrongResume \/ Calculate \/ Commit \/ Publish \/ Close
Spec == Init /\ [][Next]_vars

NoEligibleExcluded == phase = "Closed" \/ Eligible \subseteq Members(plan)
CoverageMatchesOutcomes == committed.covered = committed.answered + committed.refused + committed.outside
CompleteRequiresEveryAnswer == published => (
    /\ committed.covered + Cardinality(Outfits \ Members(plan)) = Cardinality(Outfits)
    /\ committed.answered = Cardinality(Eligible)
    /\ committed.refused = 0
)
ResumeUsesOriginalDomain == phase # "Closed" => identity = "Original" /\ plan = BoundedPlan
=============================================================================
