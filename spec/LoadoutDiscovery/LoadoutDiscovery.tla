----------------------- MODULE LoadoutDiscovery -----------------------
\* Async bundle discovery and its visible recovery route. No backend calls.
EXTENDS Naturals, FiniteSets
CONSTANTS MaxRequests, FaultAcceptStale, FaultHideFailure
VARIABLES state, requestId, issued, outcome, acceptedResponseId,
          dataPresent, dataRequestId, errorPresent, errorRequestId
vars == <<state, requestId, issued, outcome, acceptedResponseId,
          dataPresent, dataRequestId, errorPresent, errorRequestId>>
States == {"Idle", "Loading", "Ready", "Absent", "Failed"}
RequestIds == 1..MaxRequests

Init ==
    /\ state = "Idle" /\ requestId = 0 /\ issued = {}
    /\ outcome = "None" /\ acceptedResponseId = 0
    /\ dataPresent = FALSE /\ dataRequestId = 0
    /\ errorPresent = FALSE /\ errorRequestId = 0

\* A later request retires every previous result. Old/duplicate requests stutter.
Requested(id) ==
    /\ id \in RequestIds
    /\ IF id > requestId
       THEN /\ state' = "Loading" /\ requestId' = id /\ issued' = issued \cup {id}
            /\ outcome' = "None" /\ acceptedResponseId' = 0
            /\ dataPresent' = FALSE /\ dataRequestId' = 0
            /\ errorPresent' = FALSE /\ errorRequestId' = 0
       ELSE UNCHANGED vars

CanAccept(id) == state = "Loading" /\ (id = requestId \/ FaultAcceptStale)

\* Issued requests remain possible reply sources after retirement/completion.
\* This deliberately permits old, duplicate and oppositely ordered responses.
Resolved(id, present) ==
    /\ id \in issued /\ present \in BOOLEAN
    /\ IF CanAccept(id)
       THEN /\ state' = IF present THEN "Ready" ELSE "Absent"
            /\ outcome' = IF present THEN "Present" ELSE "Empty"
            /\ acceptedResponseId' = id
            /\ dataPresent' = present /\ dataRequestId' = IF present THEN id ELSE 0
            /\ errorPresent' = FALSE /\ errorRequestId' = 0
            /\ UNCHANGED <<requestId, issued>>
       ELSE UNCHANGED vars

Failed(id) ==
    /\ id \in issued
    /\ IF CanAccept(id)
       THEN /\ state' = "Failed" /\ outcome' = "Error" /\ acceptedResponseId' = id
            /\ dataPresent' = FALSE /\ dataRequestId' = 0
            /\ errorPresent' = TRUE /\ errorRequestId' = id
            /\ UNCHANGED <<requestId, issued>>
       ELSE UNCHANGED vars

\* Recovery is another Requested(new id). Terminal generations may also idle.
TerminalStutter == /\ state \in {"Idle", "Ready", "Absent", "Failed"}
                   /\ UNCHANGED vars
Next == (\E id \in RequestIds : Requested(id))
        \/ (\E id \in issued, present \in BOOLEAN : Resolved(id, present))
        \/ (\E id \in issued : Failed(id)) \/ TerminalStutter
Spec == Init /\ [][Next]_vars

NavigationVisible == state \in {"Loading", "Ready"}
                     \/ (state = "Failed" /\ ~FaultHideFailure)
RetryVisible == state = "Failed"

TypeOK ==
    /\ state \in States /\ requestId \in 0..MaxRequests /\ issued \subseteq RequestIds
    /\ outcome \in {"None", "Present", "Empty", "Error"}
    /\ acceptedResponseId \in 0..MaxRequests
    /\ dataPresent \in BOOLEAN /\ dataRequestId \in 0..MaxRequests
    /\ errorPresent \in BOOLEAN /\ errorRequestId \in 0..MaxRequests
CurrentRequestIsNewest ==
    /\ \A id \in issued : id <= requestId
    /\ (requestId = 0 <=> issued = {})
    /\ (requestId # 0 => requestId \in issued)
ResultBelongsToCurrentRequest ==
    /\ (state \in {"Ready", "Absent", "Failed"} => acceptedResponseId = requestId)
    /\ (dataPresent => dataRequestId = requestId /\ dataRequestId \in issued)
    /\ (errorPresent => errorRequestId = requestId /\ errorRequestId \in issued)
LoadingClearsPreviousData ==
    state = "Loading" => outcome = "None" /\ acceptedResponseId = 0
                         /\ ~dataPresent /\ dataRequestId = 0
                         /\ ~errorPresent /\ errorRequestId = 0
ReadyOwnsData == state = "Ready" <=> dataPresent /\ outcome = "Present"
AbsentMeansSuccessfulEmpty ==
    state = "Absent" => outcome = "Empty" /\ ~dataPresent /\ ~errorPresent
FailureOwnsError ==
    state = "Failed" <=> errorPresent /\ outcome = "Error"
NoPayloadOutsideTerminalOutcome ==
    /\ (state # "Ready" => ~dataPresent /\ dataRequestId = 0)
    /\ (state # "Failed" => ~errorPresent /\ errorRequestId = 0)
IdleAndAbsentAreHidden == state \in {"Idle", "Absent"} => ~NavigationVisible
LoadingAndReadyAreVisible == state \in {"Loading", "Ready"} => NavigationVisible
DiscoveryFailureRemainsVisible == state = "Failed" => NavigationVisible /\ RetryVisible
=============================================================================