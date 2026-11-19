----------------------- MODULE LoadoutSearchControl -----------------------
EXTENDS Naturals
CONSTANTS DropQueuedCancel, RestartAfterCancel, ConfirmBeforeCleanup,
          KeepCancelledRank, ShowCancelledRank, DropWriteReceipt
VARIABLES mode, worker, client, ack, want, stop, observed, active, cleaned,
          chunks, stopAt, retained, committed, backend
vars == <<mode,worker,client,ack,want,stop,observed,active,cleaned,
          chunks,stopAt,retained,committed,backend>>
Init == /\ mode="none" /\ worker="idle" /\ client="idle"
        /\ ack=FALSE /\ want=FALSE /\ stop=FALSE /\ observed=FALSE
        /\ active=FALSE /\ cleaned=FALSE /\ chunks=0 /\ stopAt=0
        /\ retained=FALSE /\ committed=FALSE /\ backend="none"
Start == /\ worker="idle" /\ mode'\in {"read","write"}
         /\ worker'="running" /\ client'="submitting" /\ backend'\in {"cpu","card"}
         /\ UNCHANGED <<ack,want,stop,observed,active,cleaned,chunks,stopAt,retained,committed>>
Acknowledge == /\ ~ack /\ client\in {"submitting","cancelling"}
               /\ ack'=TRUE /\ client'=IF want /\ ~DropQueuedCancel THEN "cancelling" ELSE "running"
               /\ UNCHANGED <<mode,worker,want,stop,observed,active,cleaned,chunks,stopAt,retained,committed,backend>>
RequestCancel == /\ mode="read" /\ client\in {"submitting","running"} /\ ~want
                 /\ want'=TRUE /\ client'="cancelling"
                 /\ UNCHANGED <<mode,worker,ack,stop,observed,active,cleaned,chunks,stopAt,retained,committed,backend>>
DeliverCancel == /\ mode="read" /\ ack /\ want /\ ~stop /\ worker\in {"running","cleanup"}
                 /\ stop'=TRUE
                 /\ UNCHANGED <<mode,worker,client,ack,want,observed,active,cleaned,chunks,stopAt,retained,committed,backend>>
WriteEOF == /\ mode="write" /\ worker="running" /\ ~stop /\ stop'=TRUE
            /\ UNCHANGED <<mode,worker,client,ack,want,observed,active,cleaned,chunks,stopAt,retained,committed,backend>>
BeginChunk == /\ mode="read" /\ worker="running" /\ ~active /\ ~stop /\ chunks<2
              /\ active'=TRUE /\ chunks'=chunks+1
              /\ UNCHANGED <<mode,worker,client,ack,want,stop,observed,cleaned,stopAt,retained,committed,backend>>
EndChunk == /\ active /\ active'=FALSE
            /\ UNCHANGED <<mode,worker,client,ack,want,stop,observed,cleaned,chunks,stopAt,retained,committed,backend>>
ObserveCancel == /\ mode="read" /\ worker="running" /\ stop /\ ~active
                 /\ observed'=TRUE /\ stopAt'=chunks
                 /\ worker'=IF ConfirmBeforeCleanup THEN "cancelled" ELSE "cleanup"
                 /\ UNCHANGED <<mode,client,ack,want,stop,active,cleaned,chunks,retained,committed,backend>>
WrongFallback == /\ RestartAfterCancel /\ observed /\ worker="cleanup"
                 /\ worker'="running" /\ backend'="cpu" /\ active'=TRUE /\ chunks'=chunks+1
                 /\ UNCHANGED <<mode,client,ack,want,stop,observed,cleaned,stopAt,retained,committed>>
FinishRead == /\ mode="read" /\ worker="running" /\ ~active /\ chunks=2 /\ ~stop
              /\ worker'="cleanup"
              /\ UNCHANGED <<mode,client,ack,want,stop,observed,active,cleaned,chunks,stopAt,retained,committed,backend>>
CommitWrite == /\ mode="write" /\ worker="running" /\ ~committed
               /\ committed'=TRUE /\ worker'="cleanup"
               /\ UNCHANGED <<mode,client,ack,want,stop,observed,active,cleaned,chunks,stopAt,retained,backend>>
CleanupAndPublish == /\ worker="cleanup" /\ ~active /\ cleaned'=TRUE
                     /\ worker'=IF mode="read" /\ stop /\ ~KeepCancelledRank THEN "cancelled"
                                ELSE IF mode="write" /\ stop /\ DropWriteReceipt THEN "failed" ELSE "completed"
                     /\ retained'=IF mode="read" THEN ~(stop /\ ~KeepCancelledRank) ELSE ~(stop /\ DropWriteReceipt)
                     /\ UNCHANGED <<mode,client,ack,want,stop,observed,active,chunks,stopAt,committed,backend>>
DeliverTerminal == /\ ack /\ worker\in {"cancelled","completed","failed"} /\ client\in {"running","cancelling"}
                   /\ client'=IF mode="write" THEN "receipt"
                              ELSE IF want /\ ~ShowCancelledRank THEN "cancelled"
                              ELSE IF retained THEN "results" ELSE "cancelled"
                   /\ UNCHANGED <<mode,worker,ack,want,stop,observed,active,cleaned,chunks,stopAt,retained,committed,backend>>
Done == /\ client\in {"cancelled","results","receipt"} /\ UNCHANGED vars
Next == Start \/ Acknowledge \/ RequestCancel \/ DeliverCancel \/ WriteEOF \/ BeginChunk \/ EndChunk \/ ObserveCancel
        \/ WrongFallback \/ FinishRead \/ CommitWrite \/ CleanupAndPublish \/ DeliverTerminal \/ Done
Spec == Init /\ [][Next]_vars
TypeOK == /\ mode\in {"none","read","write"} /\ worker\in {"idle","running","cleanup","cancelled","completed","failed"}
          /\ client\in {"idle","submitting","running","cancelling","cancelled","results","receipt"}
          /\ ack\in BOOLEAN /\ want\in BOOLEAN /\ stop\in BOOLEAN /\ observed\in BOOLEAN
          /\ active\in BOOLEAN /\ cleaned\in BOOLEAN /\ retained\in BOOLEAN /\ committed\in BOOLEAN
          /\ chunks\in 0..3 /\ stopAt\in 0..2 /\ backend\in {"none","cpu","card"}
QueuedCancelRetained == want /\ ack => client # "running"
CancelConfirmedAfterCleanup == worker="cancelled" => cleaned /\ ~active
NoWorkRestartAfterObservedCancel == observed => chunks<=stopAt
CancelledReadHasNoRank == mode="read" /\ stop /\ worker\in {"cancelled","completed","failed"} => ~retained
ProtectedWriteKeepsOutcome == mode="write" /\ committed /\ worker\in {"cancelled","completed","failed"} => worker="completed" /\ retained
CancelledUIHasNoRank == want => client # "results"
=============================================================================
