---------------------- MODULE NinjaEvadeFirstHit ----------------------
EXTENDS Naturals, Sequences
CONSTANT Fault

\* PC202 command 0x140 offers definitions in this order for key 0x312.
\* Percent gains represent the three known class-0 binary32 contributions.
Strong == 502
Shared == 203
Skill == 49
NativeOrder == <<Strong, Shared, Skill>>
Candidates == {"skill", "weapon82", "unqualified167", "shared", "all", "empty"}
Expected(c) == CASE c = "skill" \/ c = "unqualified167" -> {Skill}
                [] c = "weapon82" -> {Skill, Strong}
                [] c = "shared" -> {Shared}
                [] c = "all" -> {Strong, Shared, Skill}
                [] OTHER -> {}
HasAffix(c) == c \in {"weapon82", "unqualified167", "all"}
GearEligible(c) == c \in {"weapon82", "all"}
Prepared(c) ==
    (IF HasAffix(c) /\ (GearEligible(c) \/ Fault = "IgnoreRequirements") THEN {Strong} ELSE {})
    \cup (IF c \in {"shared", "all"} THEN {Shared} ELSE {})
    \cup (IF c \in {"skill", "weapon82", "unqualified167", "all"} THEN {Skill} ELSE {})
Offers(order, eligible) == SelectSeq(order, LAMBDA src: src \in eligible)
Strength(src) == IF src = Strong THEN 15 ELSE 10
FirstGain(offers) == IF offers = <<>> THEN 0 ELSE Strength(Head(offers))

\* These are observed or explicitly assumed starting views, before the command.
\* A weak survivor can exclude a later stronger offer during native insertion.
\* Retaining this input on rejection makes no prediction about subsequent gameplay.
Views == {"emptyRebuilt", "weakPresent", "weakRemovedPending", "neutralPending"}
StartMembers(v) == IF v = "weakPresent" THEN {Skill} ELSE {}
StartGain(v) == IF v \in {"weakPresent", "weakRemovedPending"} THEN 10 ELSE 0
AcceptView(v,eligible) == v = "emptyRebuilt"
                 \/ (v = "weakPresent" /\ Strong \in eligible /\ Fault = "AdmitOldWeak")
                 \/ (v = "weakRemovedPending" /\ Fault = "TrustEmptyMembers")
                 \/ (v = "neutralPending" /\ Fault = "TrustNeutralValue")

\* Native A648C compares the members actually present when rebuilding. It
\* cannot decide which new offers replace existing nodes during insertion.
PoolGain(s) == IF Strong \in s THEN 15 ELSE IF s # {} THEN 10 ELSE 0
Aggregate(s) == IF Fault = "SumSameKey"
                THEN (IF Strong \in s THEN 15 ELSE 0)
                     + (IF Shared \in s THEN 10 ELSE 0) + (IF Skill \in s THEN 10 ELSE 0)
                ELSE PoolGain(s)
\* For the clean-start sequence the first eligible factor survives. Membership
\* of later, weaker/tied offers is intentionally left open. A later rebuild
\* uses the actual surviving membership chosen for this run.
PossiblePools(offers) ==
    IF offers = <<>> THEN {{}}
    ELSE {m \in SUBSET {offers[i]: i \in 1..Len(offers)} : PoolGain(m) = FirstGain(offers)}

VARIABLES outfit, gates, view, phase, admitted, offered, originalPool,
          members, removed, gain, hits, results
vars == <<outfit, gates, view, phase, admitted, offered, originalPool,
          members, removed, gain, hits, results>>
Init == /\ outfit = "skill" /\ gates = Prepared(outfit) /\ view = "emptyRebuilt"
        /\ phase = "initial" /\ admitted = FALSE /\ offered = <<>> /\ originalPool = {}
        /\ members = {} /\ removed = {} /\ gain = 0 /\ hits = 0 /\ results = <<>>

Select(c,v) ==
    /\ c \in Candidates /\ v \in Views /\ (c # outfit \/ v # view)
    /\ outfit' = c /\ gates' = IF Fault = "KeepOldGates" THEN gates ELSE Prepared(c)
    /\ view' = v /\ phase' = "initial" /\ admitted' = FALSE /\ offered' = <<>>
    /\ originalPool' = {} /\ members' = StartMembers(v) /\ removed' = {}
    /\ gain' = StartGain(v) /\ hits' = 0
    /\ results' = IF Fault = "KeepOldResult" /\ c # outfit THEN results ELSE <<>>

\* The success fact includes the eligible command recipient. Ordinary reads
\* did not observe component+1C0. A dry-run reject preserves the input view.
Trigger(ok) ==
    /\ phase = "initial" /\ ok \in BOOLEAN /\ admitted' = ok
    /\ LET apply == AcceptView(view, gates) /\ (ok \/ Fault = "AttemptIsSuccess")
           order == IF Fault = "WeakOfferedFirst" THEN <<Skill, Shared, Strong>> ELSE NativeOrder
       IN /\ phase' = IF apply THEN "active" ELSE "rejected"
          /\ offered' = IF apply THEN Offers(order, gates) ELSE <<>>
          /\ originalPool' \in IF apply THEN PossiblePools(offered') ELSE {{}}
          /\ members' = IF apply THEN originalPool' ELSE members
          /\ gain' = IF apply THEN Aggregate(members') ELSE gain
    /\ hits' = 0 /\ UNCHANGED <<outfit, gates, view, removed, results>>

\* Expiry/removal is separate from rating a hit. A timed member can persist.
Remove(s) ==
    /\ phase = "active" /\ s \in members
    /\ phase' = "pending" /\ removed' = {s}
    /\ members' = IF Fault = "RemoveAllPeers" THEN {} ELSE members \ {s}
    /\ gain' = IF Fault = "EarlyRebuild" THEN Aggregate(members') ELSE gain
    /\ UNCHANGED <<outfit, gates, view, admitted, offered, originalPool, hits, results>>
Rebuild ==
    /\ phase = "pending" /\ phase' = "rebuilt"
    /\ gain' = IF Fault = "SkipRebuild" THEN gain ELSE Aggregate(members)
    /\ UNCHANGED <<outfit, gates, view, admitted, offered, originalPool, members, removed, hits, results>>

Hit ==
    /\ phase # "initial" /\ hits < 2 /\ hits' = hits + 1
    /\ LET publish == (phase = "active" \/ (Fault = "ScorePending" /\ phase = "pending"))
                      /\ (hits = 0 \/ Fault = "ScoreAgain") /\ Fault # "SkipScore"
       IN results' = IF publish THEN Append(results, [owner |-> outfit, ordinal |-> hits',
                                                 phaseAtHit |-> phase, appliedGain |-> gain])
                     ELSE results
    /\ UNCHANGED <<outfit, gates, view, phase, admitted, offered, originalPool, members, removed, gain>>

Next == (\E c \in Candidates, v \in Views: Select(c,v)) \/ (\E ok \in BOOLEAN: Trigger(ok))
        \/ (\E s \in members: Remove(s)) \/ Rebuild \/ Hit
Spec == Init /\ [][Next]_vars

CandidateGates == gates = Expected(outfit)
ResultOwner == \A i \in 1..Len(results): results[i].owner = outfit
TriggerRequired == phase \in {"active", "pending", "rebuilt"} => admitted
CleanStartOnly == view # "emptyRebuilt" => phase \in {"initial", "rejected"} /\ results = <<>>
NativeOfferOrder == phase \in {"active", "pending", "rebuilt"} => offered = Offers(NativeOrder, Expected(outfit))
Membership == CASE phase \in {"initial", "rejected"} -> members = StartMembers(view)
              [] phase = "active" -> members = originalPool
              [] OTHER -> members = originalPool \ removed
CleanStartGain == phase = "active" => gain = FirstGain(Offers(NativeOrder, Expected(outfit)))
PendingAggregate == phase = "pending" => gain = PoolGain(originalPool)
RebuiltAggregate == phase = "rebuilt" => gain = PoolGain(originalPool \ removed)
UnmodifiedInput == phase \in {"initial", "rejected"} => gain = StartGain(view)
FirstContactOnly == /\ Len(results) <= 1
                    /\ \A i \in 1..Len(results): results[i].ordinal = 1
ImmediateOnly == \A i \in 1..Len(results): results[i].phaseAtHit = "active"
CorrectScore == \A i \in 1..Len(results):
                      results[i].appliedGain = FirstGain(Offers(NativeOrder, Expected(results[i].owner)))
FirstContactAnswered == (phase = "active" /\ hits > 0) => Len(results) = 1
=============================================================================
