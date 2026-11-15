---------------------------- MODULE SaveCommit ----------------------------
(* One save write across the outer operation ledger and the core receipt.
   target: Source before the write, Output once this commit landed, Foreign once
   the game saved over it. A crash can happen at every step, either ledger write
   can fail, and recovery runs when the client next reads the operation.
   RecordOuterCommit = FALSE removes the step that releases a commit whose core
   record never landed and fails ResolutionPossible; FixedRollback = FALSE drops
   the digest a failed rollback must keep and fails OwnBytesProvable. *)
EXTENDS TLC

CONSTANTS RecordOuterCommit, FixedRollback

Unresolved == {"pending", "uncertain"}
CoreTerminal == {"committed", "not_committed", "acknowledged"}

CoreTransitions ==
    { <<"none", "pending">>, <<"pending", "pending">>,
      <<"pending", "uncertain">>, <<"pending", "committed">>,
      <<"pending", "not_committed">>, <<"pending", "acknowledged">>,
      <<"uncertain", "committed">>, <<"uncertain", "not_committed">>,
      <<"uncertain", "acknowledged">> }

VARIABLES target, core, digest, outer, step, landed

vars == <<target, core, digest, outer, step, landed>>

Init ==
    /\ target = "Source"
    /\ core = "none"
    /\ digest = "none"
    /\ outer = "none"
    /\ step = "idle"
    /\ landed = FALSE

SetCore(next) ==
    /\ <<core, next>> \in CoreTransitions
    /\ core' = next

OuterIntent ==
    /\ step = "idle" /\ outer = "none"
    /\ outer' = "executing" /\ step' = "core"
    /\ UNCHANGED <<target, core, digest, landed>>

CoreRefuses ==
    /\ step = "core" /\ core = "none"
    /\ outer' = "not_committed" /\ step' = "done"
    /\ UNCHANGED <<target, core, digest, landed>>

CoreIntent ==
    /\ step = "core" /\ core = "none"
    /\ SetCore("pending") /\ digest' = "output" /\ step' = "intent"
    /\ UNCHANGED <<target, outer, landed>>

Install ==
    /\ step = "intent"
    /\ target' = "Output" /\ landed' = TRUE /\ step' = "installed"
    /\ UNCHANGED <<core, digest, outer>>

FailBeforeReplace ==
    /\ step = "intent"
    /\ SetCore("not_committed") /\ digest' = "none" /\ step' = "report"
    /\ UNCHANGED <<target, outer, landed>>

CoreRecordsCommit ==
    /\ step = "installed"
    /\ SetCore("committed") /\ step' = "report"
    /\ UNCHANGED <<target, digest, outer, landed>>

CoreRecordFails ==
    /\ step = "installed"
    /\ step' = "report_warning"
    /\ UNCHANGED <<target, core, digest, outer, landed>>

ReadbackFailsRollbackHolds ==
    /\ step = "installed"
    /\ target' = "Source" /\ landed' = FALSE
    /\ SetCore("uncertain") /\ digest' = "none" /\ step' = "report"
    /\ UNCHANGED outer

ReadbackFailsRollbackFails ==
    /\ step = "installed"
    /\ SetCore("uncertain")
    /\ digest' = IF FixedRollback THEN "output" ELSE "none"
    /\ step' = "report"
    /\ UNCHANGED <<target, outer, landed>>

OuterWord ==
    CASE core = "committed" -> "committed"
      [] core = "not_committed" -> "not_committed"
      [] OTHER -> "unknown"

OuterRecords ==
    /\ step = "report"
    /\ outer' = OuterWord /\ step' = "done"
    /\ UNCHANGED <<target, core, digest, landed>>

OuterRecordsWarning ==
    /\ step = "report_warning"
    /\ outer' = "committed_with_warning" /\ step' = "done"
    /\ UNCHANGED <<target, core, digest, landed>>

Crash ==
    /\ step \in {"core", "intent", "installed", "report", "report_warning"}
    /\ step' = "done"
    /\ UNCHANGED <<target, core, digest, outer, landed>>

GameSaves ==
    /\ step = "done" /\ target # "Foreign"
    /\ target' = "Foreign"
    /\ UNCHANGED <<core, digest, outer, step, landed>>

\* save.operation projects a stale intent onto the core authority.
Project ==
    /\ step = "done" /\ outer = "executing"
    /\ outer' = OuterWord
    /\ UNCHANGED <<target, core, digest, step, landed>>

Proof ==
    CASE target = "Output" /\ digest = "output" -> "committed"
      [] target = "Source" -> "not_committed"
      [] OTHER -> "none"

\* settle_unresolved from the bytes on disk.
Settle ==
    /\ step = "done" /\ outer = "unknown" /\ core \in Unresolved
    /\ Proof # "none"
    /\ SetCore(Proof) /\ outer' = Proof
    /\ UNCHANGED <<target, digest, step, landed>>

\* No core record: the reviewed source still on disk proves nothing landed.
SettleWithoutCore ==
    /\ step = "done" /\ outer = "unknown" /\ core = "none"
    /\ target = "Source"
    /\ outer' = "not_committed"
    /\ UNCHANGED <<target, core, digest, step, landed>>

Acknowledge ==
    /\ step = "done" /\ outer = "unknown" /\ core \in Unresolved
    /\ Proof = "none"
    /\ SetCore("acknowledged") /\ outer' = "acknowledged"
    /\ UNCHANGED <<target, digest, step, landed>>

SettleFromOuter ==
    /\ RecordOuterCommit
    /\ step = "done" /\ outer = "committed_with_warning" /\ core = "pending"
    /\ SetCore("committed")
    /\ UNCHANGED <<target, digest, outer, step, landed>>

Next ==
    \/ OuterIntent \/ CoreRefuses \/ CoreIntent \/ Install \/ FailBeforeReplace
    \/ CoreRecordsCommit \/ CoreRecordFails
    \/ ReadbackFailsRollbackHolds \/ ReadbackFailsRollbackFails
    \/ OuterRecords \/ OuterRecordsWarning \/ Crash \/ GameSaves
    \/ Project \/ Settle \/ SettleWithoutCore \/ Acknowledge \/ SettleFromOuter

Spec ==
    /\ Init /\ [][Next]_vars
    /\ WF_vars(Project) /\ WF_vars(Settle) /\ WF_vars(SettleWithoutCore)
    /\ WF_vars(Acknowledge) /\ WF_vars(SettleFromOuter)

\* A published success is never false, and a published refusal never hides bytes.
OuterTruthful ==
    /\ outer \in {"committed", "committed_with_warning"} => landed
    /\ outer = "not_committed" => ~landed

CoreTruthful ==
    /\ core = "committed" => landed
    /\ core = "not_committed" => ~landed

TypeOK ==
    /\ target \in {"Source", "Output", "Foreign"}
    /\ core \in {"none"} \cup Unresolved \cup CoreTerminal
    /\ digest \in {"none", "output"}
    /\ outer \in {"none", "executing", "committed", "committed_with_warning",
                  "not_committed", "unknown", "acknowledged"}
    /\ landed \in BOOLEAN

\* Once the client is reading, a save can leave the fence: every open state
\* still has a recovery step.
ResolutionPossible ==
    (step = "done" /\ core \in Unresolved) =>
        \/ outer \in {"executing", "unknown"}
        \/ (outer = "committed_with_warning" /\ RecordOuterCommit)

\* Bytes this commit left on disk stay provable as this commit's.
OwnBytesProvable == (core = "uncertain" /\ target = "Output") => digest = "output"

TerminalStable == [][(core \in CoreTerminal) => core' = core]_vars

EventuallyReleased == (step = "done") ~> (core \notin Unresolved)
=============================================================================
