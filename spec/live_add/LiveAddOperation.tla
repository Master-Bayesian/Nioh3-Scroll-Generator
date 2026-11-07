---- MODULE LiveAddOperation ----
\* One live addition, decided by two durable files: claim.json and receipt.json.
\* Processes race and may die at any step. Preview children keep their own path.
EXTENDS Naturals

CONSTANTS
    ExclusiveClaim,
    RecoverChecksEvidence

Procs == {"a", "b"}

Settled == {"verified", "rejected_before_insertion", "dispatched_unverified"}
Receipts == {"none", "rejected_before_dispatch"} \cup Settled

Snapshots == {
    <<"none", "none", "prepared">>,
    <<"cancel", "none", "cancelled">>,
    <<"dispatch", "none", "uncertain">>,
    <<"dispatch", "verified", "verified">>,
    <<"dispatch", "rejected_before_dispatch", "rejected_before_dispatch">>,
    <<"dispatch", "rejected_before_insertion", "rejected_before_insertion">>,
    <<"dispatch", "dispatched_unverified", "dispatched_unverified">>
}

VARIABLES claim, receipt, dispatches, pc

vars == <<claim, receipt, dispatches, pc>>

State ==
    LET matching == {row \in Snapshots : row[1] = claim /\ row[2] = receipt}
    IN IF matching = {} THEN "invalid" ELSE (CHOOSE row \in matching : TRUE)[3]

Init ==
    /\ claim = "none"
    /\ receipt = "none"
    /\ dispatches = 0
    /\ pc = [p \in Procs |-> "idle"]

Claim(p) ==
    /\ pc[p] = "idle"
    /\ IF ExclusiveClaim THEN claim = "none" ELSE claim # "cancel"
    /\ receipt = "none"
    /\ claim' = "dispatch"
    /\ pc' = [pc EXCEPT ![p] = "claimed"]
    /\ UNCHANGED <<receipt, dispatches>>

Cancel(p) ==
    /\ pc[p] = "idle"
    /\ claim = "none"
    /\ claim' = "cancel"
    /\ UNCHANGED <<receipt, dispatches, pc>>

Dispatch(p) ==
    /\ pc[p] = "claimed"
    /\ dispatches' = dispatches + 1
    /\ pc' = [pc EXCEPT ![p] = "dispatched"]
    /\ UNCHANGED <<claim, receipt>>

Refuse(p) ==
    /\ pc[p] = "claimed"
    /\ receipt = "none"
    /\ receipt' = "rejected_before_dispatch"
    /\ pc' = [pc EXCEPT ![p] = "idle"]
    /\ UNCHANGED <<claim, dispatches>>

Finish(p) ==
    /\ pc[p] = "dispatched"
    /\ receipt = "none"
    /\ receipt' \in Settled
    /\ pc' = [pc EXCEPT ![p] = "idle"]
    /\ UNCHANGED <<claim, dispatches>>

Crash(p) ==
    /\ pc[p] # "idle"
    /\ pc' = [pc EXCEPT ![p] = "idle"]
    /\ UNCHANGED <<claim, receipt, dispatches>>

Recover ==
    /\ \A p \in Procs : pc[p] = "idle"
    /\ claim = "dispatch"
    /\ receipt = "none"
    /\ IF RecoverChecksEvidence
       THEN receipt' \in (IF dispatches = 0 THEN {"rejected_before_dispatch"} ELSE Settled)
       ELSE receipt' \in Receipts \ {"none"}
    /\ UNCHANGED <<claim, dispatches, pc>>

Next ==
    \/ \E p \in Procs : Claim(p) \/ Cancel(p) \/ Dispatch(p) \/ Refuse(p) \/ Finish(p) \/ Crash(p)
    \/ Recover

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ claim \in {"none", "dispatch", "cancel"}
    /\ receipt \in Receipts
    /\ dispatches \in Nat
    /\ pc \in [Procs -> {"idle", "claimed", "dispatched"}]

SnapshotDefined == State # "invalid"
AtMostOneDispatch == dispatches <= 1
CancelledNeverDispatched == claim = "cancel" => dispatches = 0
RefusalTruthful == receipt = "rejected_before_dispatch" => dispatches = 0
SettledDispatched == receipt \in Settled => dispatches = 1
OnlyPreparedDispatches == \A p \in Procs : pc[p] # "idle" => claim = "dispatch"

ReceiptFinal == [][(receipt # "none") => (receipt' = receipt)]_vars
ClaimFinal == [][(claim # "none") => (claim' = claim)]_vars
====
