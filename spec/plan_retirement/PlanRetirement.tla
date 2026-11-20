-------------------------- MODULE PlanRetirement --------------------------
CONSTANTS KeepOwner, CheckReceipt, CheckRemoval, RespectDispatch
VARIABLES phase, owner, proof, kind, journal
vars == <<phase, owner, proof, kind, journal>>
Init == /\ phase = "prepared" /\ owner = TRUE /\ proof = "pending"
        /\ kind \in {"durable", "batch"}
        /\ journal \in (IF kind = "batch" THEN {"empty","dispatch"} ELSE {"empty"})
Begin == /\ phase \in {"prepared", "retry"} /\ owner
         /\ phase' = "cancelling" /\ UNCHANGED <<owner, proof, kind, journal>>
WriteCancelClaim == /\ kind = "batch" /\ phase = "cancelling" /\ journal = "empty"
                    /\ journal' = "cancelClaim" /\ UNCHANGED <<phase, owner, kind, proof>>
CancelChild == /\ kind = "batch" /\ phase = "cancelling" /\ journal = "cancelClaim"
               /\ journal' = "childCancelled" /\ UNCHANGED <<phase, owner, kind, proof>>
Respond(result) ==
    /\ phase = "cancelling"
    /\ result \in (IF kind = "batch" THEN {"matching", "foreign", "error"}
                   ELSE {"removed", "absent", "error"})
    /\ (result = "matching" => (journal = "childCancelled" \/ (~RespectDispatch /\ journal = "dispatch")))
    /\ proof' = result /\ UNCHANGED <<kind, journal>>
    /\ IF result \in {"matching","removed","absent"}
          \/ (result = "foreign" /\ ~CheckReceipt)
          \/ (kind = "durable" /\ result = "error" /\ ~CheckRemoval)
       THEN /\ phase' = "retired" /\ owner' = FALSE
       ELSE /\ phase' = "retry" /\ owner' = KeepOwner
Next == WriteCancelClaim \/ CancelChild \/ Begin \/ (\E r \in {"matching","foreign","removed","absent","error"} : Respond(r))
        \/ (phase = "retired" /\ UNCHANGED vars)
Spec == Init /\ [][Next]_vars
DispatchNeverCancelled == journal = "dispatch" => ~(phase = "retired")
RetryOwnsPlan == phase = "retry" => owner
ReleasedOnProof == phase = "retired" => proof \in {"matching","removed","absent"}
=============================================================================
