--------------------------- MODULE ProtectedClose ---------------------------
EXTENDS TLC

CONSTANTS RequireExitAfterLoss, AcceptExitedAfterLoss

VARIABLES transport, process, closeResult

vars == <<transport, process, closeResult>>

Init ==
    /\ transport = "serving"
    /\ process = "running"
    /\ closeResult = "waiting"

TransportLost ==
    /\ transport = "serving"
    /\ transport' = "lost"
    /\ UNCHANGED <<process, closeResult>>

ChildExits ==
    /\ process = "running"
    /\ process' = "exited"
    /\ transport' = IF transport = "acknowledged" THEN transport ELSE "lost"
    /\ UNCHANGED closeResult

ShutdownAck ==
    /\ transport = "serving"
    /\ transport' = "acknowledged"
    /\ UNCHANGED <<process, closeResult>>

CanClose ==
    \/ transport = "acknowledged"
    \/ (transport = "lost" /\
        IF process = "exited" THEN AcceptExitedAfterLoss ELSE ~RequireExitAfterLoss)

RequestClose ==
    /\ closeResult # "allowed"
    /\ closeResult' = IF CanClose THEN "allowed" ELSE "refused"
    /\ UNCHANGED <<transport, process>>

Next == TransportLost \/ ChildExits \/ ShutdownAck \/ RequestClose
Spec == Init /\ [][Next]_vars

NoCloseWithoutRelease ==
    closeResult = "allowed" => (transport = "acknowledged" \/ process = "exited")

ExitedWorkerCanClose == transport = "lost" /\ process = "exited" => CanClose
=============================================================================
