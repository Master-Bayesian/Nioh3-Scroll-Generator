-------------------------- MODULE PlanRetirement --------------------------
CONSTANTS KeepOwner, CheckReceipt, CheckRemoval
VARIABLES phase, owner, proof, kind
vars == <<phase, owner, proof, kind>>
Init == /\ phase = "prepared" /\ owner = TRUE /\ proof = "pending"
        /\ kind \in {"durable", "batch"}
Begin == /\ phase \in {"prepared", "retry"} /\ owner
         /\ phase' = "cancelling" /\ UNCHANGED <<owner, proof, kind>>
Respond(result) ==
    /\ phase = "cancelling"
    /\ result \in (IF kind = "batch" THEN {"matching", "foreign", "error"}
                   ELSE {"removed", "absent", "error"})
    /\ proof' = result /\ kind' = kind
    /\ IF result \in {"matching","removed","absent"}
          \/ (result = "foreign" /\ ~CheckReceipt)
          \/ (kind = "durable" /\ result = "error" /\ ~CheckRemoval)
       THEN /\ phase' = "retired" /\ owner' = FALSE
       ELSE /\ phase' = "retry" /\ owner' = KeepOwner
Next == Begin \/ (\E r \in {"matching","foreign","removed","absent","error"} : Respond(r))
        \/ (phase = "retired" /\ UNCHANGED vars)
Spec == Init /\ [][Next]_vars
RetryOwnsPlan == phase = "retry" => owner
ReleasedOnProof == phase = "retired" => proof \in {"matching","removed","absent"}
=============================================================================
