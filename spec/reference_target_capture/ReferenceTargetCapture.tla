---------------------- MODULE ReferenceTargetCapture ----------------------
EXTENDS Naturals
CONSTANTS MixTarget, IgnoreMissing, UseFailedReplay
VARIABLES first, last, phase, known, replay
vars == <<first,last,phase,known,replay>>
Ids == {1,2}
Init == /\ first \in Ids /\ last = first /\ phase = "sampled"
        /\ known = FALSE /\ replay = FALSE
Finish == /\ phase = "sampled" /\ last' \in Ids
          /\ known' \in BOOLEAN /\ replay' \in BOOLEAN
          /\ phase' = "recorded" /\ UNCHANGED first
Materialize == /\ phase = "recorded"
               /\ (first = last \/ MixTarget)
               /\ (known \/ IgnoreMissing) /\ (replay \/ UseFailedReplay)
               /\ phase' = "bound" /\ UNCHANGED <<first,last,known,replay>>
Next == Finish \/ Materialize
Spec == Init /\ [][Next]_vars
SameOpponent == phase = "bound" => first = last
RequiredInputsKnown == phase = "bound" => known
RoleReplayComplete == phase = "bound" => replay
=============================================================================
