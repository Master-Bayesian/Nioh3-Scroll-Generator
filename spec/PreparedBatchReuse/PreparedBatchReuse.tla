-------------------------- MODULE PreparedBatchReuse --------------------------
EXTENDS Naturals, FiniteSets, Sequences, TLC
CONSTANTS Fault, MaxBatches
Packets == {"A", "B", "C"}
Requests == {<<"A">>, <<"B">>, <<"C">>, <<"A", "A">>, <<"A", "B", "A">>}
Members(seq) == {seq[i] : i \in 1..Len(seq)}
Truth(p) == CASE p = "A" -> "DamageA" [] p = "B" -> "DamageB" [] OTHER -> "DamageC"
\* A and B collide deliberately. Correct lookup still compares whole packets.
Hash(p) == IF p \in {"A", "B"} THEN 0 ELSE 1
EmptyCache == [p \in Packets |-> "Empty"]
EmptyResults == [p \in Packets |-> "Unwritten"]
VARIABLE s
vars == <<s>>
Init == s = [phase |-> "Idle", batch |-> 0, request |-> <<>>,
    pending |-> {}, done |-> {}, results |-> EmptyResults, cache |-> EmptyCache,
    inFlight |-> FALSE, bufferLive |-> FALSE, generation |-> 0, flightGeneration |-> 0,
    cancelled |-> FALSE, failed |-> FALSE, poisoned |-> FALSE,
    cancelledBatches |-> {}, failedBatches |-> {}, published |-> <<>>]

Begin == /\ s.phase \in {"Idle", "Committed"} /\ s.batch < MaxBatches /\ ~s.poisoned
    /\ \E r \in Requests : s' = [s EXCEPT !.phase = "Prepared", !.batch = @ + 1,
         !.request = r, !.cancelled = FALSE, !.failed = FALSE,
         !.pending = {p \in Members(r) : s.cache[p] = "Empty"},
         !.done = {p \in Members(r) : s.cache[p] # "Empty"},
         !.results = [p \in Packets |-> IF p \in Members(r) /\ s.cache[p] # "Empty"
                                        THEN s.cache[p] ELSE "Unwritten"]]

\* Generation changes when host/device staging storage is rewritten or grown.
Stage == /\ s.phase = "Prepared" /\ ~s.inFlight /\ ~s.cancelled /\ ~s.poisoned
    /\ s' = [s EXCEPT !.phase = "Reserved", !.bufferLive = TRUE, !.generation = @ + 1]
Submit == /\ s.phase = "Reserved" /\ s.pending # {} /\ ~s.cancelled /\ ~s.poisoned
    /\ s' = [s EXCEPT !.phase = "InFlight", !.inFlight = TRUE, !.flightGeneration = s.generation]

\* Unique lanes may finish in any order; duplicate request rows share a lane.
DeviceStep == /\ s.phase = "InFlight" /\ ~s.failed /\ s.pending # {}
    /\ \E p \in s.pending : s' = [s EXCEPT !.results[p] = Truth(p),
         !.pending = @ \ {p}, !.done = @ \cup {p}]
FailDevice == /\ s.phase = "InFlight" /\ ~s.failed
    /\ s' = [s EXCEPT !.failed = TRUE, !.poisoned = TRUE,
         !.failedBatches = @ \cup {s.batch}, !.cache = EmptyCache]

\* A successful join or a joined failure is needed before reuse/free. A lost
\* CUDA context is abstracted here; actual cleanup remains driver-dependent.
Join == /\ s.phase = "InFlight" /\ (s.pending = {} \/ s.failed)
    /\ s' = [s EXCEPT !.phase = "Joined", !.inFlight = FALSE]
CacheOnly == /\ s.phase = "Reserved" /\ s.pending = {} /\ ~s.inFlight
    /\ s' = [s EXCEPT !.phase = "Joined"]
Cancel == /\ s.phase \in {"Prepared", "Reserved", "InFlight", "Joined", "CommitReady"}
    /\ ~s.cancelled
    /\ s' = [s EXCEPT !.cancelled = TRUE, !.cancelledBatches = @ \cup {s.batch}]

\* A successful immutable answer may stay cached after cancellation. This is
\* separate from publishing a checkpoint/rank to the caller.
FinishCache == /\ s.phase = "Joined" /\ ~s.failed /\ ~s.poisoned /\ s.pending = {}
    /\ s' = [s EXCEPT !.phase = "CommitReady",
         !.cache = [p \in Packets |-> IF p \in s.done THEN s.results[p] ELSE s.cache[p]]]
Publish == /\ s.phase \in {"CommitReady", "Joined"} /\ ~s.inFlight
    /\ (~s.cancelled \/ Fault = "PublishCancelled")
    /\ ((~s.failed /\ ~s.poisoned /\ s.pending = {}) \/ Fault = "PublishFailed")
    /\ s' = [s EXCEPT !.phase = "Committed",
         !.published = Append(@, [id |-> s.batch, input |-> s.request,
             output |-> [i \in 1..Len(s.request) |-> s.results[s.request[i]]]])]
Abort == /\ s.phase \in {"Prepared", "Reserved", "Joined", "CommitReady"}
    /\ ~s.inFlight /\ (s.cancelled \/ s.failed \/ s.poisoned)
    /\ s' = [s EXCEPT !.phase = "Aborted"]
Close == /\ s.phase # "Closed" /\ ~s.inFlight
    /\ s' = [s EXCEPT !.phase = "Closed", !.bufferLive = FALSE, !.cache = EmptyCache]

UnsafeReuse == /\ Fault = "ReuseInFlight" /\ s.inFlight
    /\ s' = [s EXCEPT !.generation = @ + 1]
UnsafeDestroy == /\ Fault = "DestroyInFlight" /\ s.inFlight
    /\ s' = [s EXCEPT !.bufferLive = FALSE]
CachePartial == /\ Fault = "CacheIncomplete" /\ s.phase = "InFlight" /\ s.pending # {}
    /\ \E p \in s.pending : s' = [s EXCEPT !.cache[p] = s.results[p]]
HashOnlyLookup == /\ Fault = "HashOnly" /\ s.phase = "Prepared"
    /\ \E p \in s.pending, q \in Packets :
         /\ s.cache[q] # "Empty" /\ q # p /\ Hash(p) = Hash(q)
         /\ s' = [s EXCEPT !.results[p] = s.cache[q], !.pending = @ \ {p}, !.done = @ \cup {p}]

Next == Begin \/ Stage \/ Submit \/ DeviceStep \/ FailDevice \/ Join \/ CacheOnly
        \/ Cancel \/ FinishCache \/ Publish \/ Abort \/ Close
        \/ UnsafeReuse \/ UnsafeDestroy \/ CachePartial \/ HashOnlyLookup
Spec == Init /\ [][Next]_vars
TypeOK ==
    /\ s.phase \in {"Idle", "Prepared", "Reserved", "InFlight", "Joined",
                     "CommitReady", "Committed", "Aborted", "Closed"}
    /\ s.batch \in 0..MaxBatches /\ s.request \in Requests \cup {<<>>}
    /\ s.pending \subseteq Members(s.request) /\ s.done \subseteq Members(s.request)
    /\ s.pending \cap s.done = {} /\ s.pending \cup s.done = Members(s.request)
    /\ s.results \in [Packets -> {"Unwritten", "DamageA", "DamageB", "DamageC"}]
    /\ s.cache \in [Packets -> {"Empty", "Unwritten", "DamageA", "DamageB", "DamageC"}]
    /\ s.inFlight \in BOOLEAN /\ s.bufferLive \in BOOLEAN
    /\ s.cancelled \in BOOLEAN /\ s.failed \in BOOLEAN /\ s.poisoned \in BOOLEAN
    /\ s.cancelledBatches \subseteq 1..MaxBatches /\ s.failedBatches \subseteq 1..MaxBatches
    /\ s.generation \in 0..(MaxBatches + 1) /\ s.flightGeneration \in 0..MaxBatches
BufferLivesUntilJoin == s.inFlight => s.bufferLive
NoReuseBeforeJoin == s.inFlight => s.generation = s.flightGeneration
CachedAnswersExact == \A p \in Packets : s.cache[p] # "Empty" => s.cache[p] = Truth(p)
CompletedAnswersExact == \A p \in s.done : s.results[p] = Truth(p)
NoCancelledCommit == \A i \in 1..Len(s.published) : s.published[i].id \notin s.cancelledBatches
NoFailedCommit == \A i \in 1..Len(s.published) : s.published[i].id \notin s.failedBatches
PublishedOrderAndDuplicatesExact == \A i \in 1..Len(s.published) :
    s.published[i].output = [j \in 1..Len(s.published[i].input) |-> Truth(s.published[i].input[j])]
=============================================================================
