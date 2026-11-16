---------------------- MODULE EquipmentSeedSelection ----------------------
EXTENDS Naturals, FiniteSets

(* Finite safety abstraction: two search requests, two outcomes, one queued
   item, one identity change, and one host prepare attempt. A digest is an
   opaque collision-free token, not a proof of SHA-256 or native generation. *)
CONSTANTS NoValue, AcceptStaleCompletionFault, QueueWithoutSelectionFault,
          QueueIneligibleSkillFault, MutableQueueFault,
          SkipProgressCheckFault, SkipContextCheckFault, SkipDigestCheckFault,
          AllowUnboundHellFault, RejectLegacyOrdinaryFault,
          AcceptPartialBindingFault, RequireSkillForSearchFault,
          SkipHostSkillCheckFault, PlanChangesSkillFault

None == NoValue
Saves == {"save-a", "save-b"}
Modes == {"legal-save", "modded-save", "live"}
Identities == [save : Saves, mode : Modes, condition : 0..1, revision : 0..1, hell : BOOLEAN]
Choices == 1..2
Skills == 0..3
Eligible(s) == s \in {1, 2}
Context(h) == IF h THEN "hell_conversion_zero_flags" ELSE "reward_zero_flags"
Contexts == {Context(FALSE), Context(TRUE)}
Progress(s) == IF s = "save-a" THEN 0 ELSE 1
SaveForProgress(p) == IF p = 0 THEN "save-a" ELSE "save-b"
UIPhases == {"idle", "searching", "empty", "results", "selected", "error"}
HostPhases == {"idle", "received", "bound", "replayed", "prepared", "rejected"}
Bindings == {"unclassified", "legacy-ordinary", "reviewed", "rejected"}

Idle(i) == [phase |-> "idle", requestId |-> 0, identity |-> i, choice |-> 0]
Candidate(i, c) ==
    [identity |-> i, choice |-> c, hell |-> i.hell,
     context |-> Context(i.hell), progress |-> Progress(i.save),
     recordToken |-> <<i.condition, c, i.hell, Progress(i.save)>>]
Candidates == {Candidate(i, c) : i \in Identities, c \in Choices}
Digest(c) == <<c.context, c.progress, c.recordToken>>
Digests == {Digest(c) : c \in Candidates}
MissingDigest == <<"missing", 0, <<0, 0, FALSE, 0>>>>
WrongDigest == <<"wrong", 0, <<0, 0, FALSE, 0>>>>
Replay(c, p) == Candidate([c.identity EXCEPT !.save = SaveForProgress(p)], c.choice)
UIType == [phase : UIPhases, requestId : 0..2, identity : Identities, choice : 0..2]
RequestType == [id : 1..2, identity : Identities]
QueueType == [candidate : Candidates, skill : Skills, explicitlySelected : BOOLEAN]
PayloadType ==
    [candidate : Candidates, skill : Skills,
     presence : {"complete", "partial", "none"}, context : Contexts,
     progress : 0..1, digest : Digests \cup {MissingDigest, WrongDigest}]
PreparedType == [candidate : Candidates, skill : Skills]

VARIABLES current, identityChanges, ui, pending, requestCount, skill,
          queued, frozenQueue, submitted, binding, hostPhase,
          snapshotProgress, replayedCandidate, prepared
vars == <<current, identityChanges, ui, pending, requestCount, skill,
          queued, frozenQueue, submitted, binding, hostPhase,
          snapshotProgress, replayedCandidate, prepared>>

Init ==
    /\ current \in {i \in Identities : i.save = "save-a" /\
                                      i.mode = "legal-save" /\ i.condition = 0 /\ i.revision = 0}
    /\ identityChanges = 0 /\ ui = Idle(current)
    /\ pending = {} /\ requestCount = 0 /\ skill = 0
    /\ queued = None /\ frozenQueue = None /\ submitted = None
    /\ binding = "unclassified" /\ hostPhase = "idle"
    /\ snapshotProgress = 0 /\ replayedCandidate = None /\ prepared = None

(* Search skill is deliberately absent from the identity and candidate digest.
   The one-queue-item bound stops further searches after that item is queued. *)
CanSearch == /\ current.mode = "legal-save" /\ ui.phase # "searching"
             /\ requestCount < 2 /\ queued = None /\ hostPhase = "idle"
Start ==
    /\ CanSearch
    /\ ~RequireSkillForSearchFault \/ requestCount = 0 \/ ~current.hell \/ Eligible(skill)
    /\ requestCount' = requestCount + 1
    /\ ui' = [phase |-> "searching", requestId |-> requestCount + 1,
              identity |-> current, choice |-> 0]
    /\ pending' = pending \cup {[id |-> requestCount + 1, identity |-> current]}
    /\ UNCHANGED <<current, identityChanges, skill, queued, frozenQueue,
                    submitted, binding, hostPhase, snapshotProgress,
                    replayedCandidate, prepared>>

ReceiptMatches(r) == /\ ui.phase = "searching"
                     /\ r.id = ui.requestId /\ r.id = requestCount
                     /\ r.identity = ui.identity /\ r.identity = current
Complete(r, outcome) ==
    /\ r \in pending /\ outcome \in {"results", "empty", "error"}
    /\ pending' = pending \ {r}
    /\ ui' = IF ReceiptMatches(r) \/ AcceptStaleCompletionFault
              THEN [phase |-> outcome, requestId |-> r.id,
                    identity |-> r.identity, choice |-> 0]
              ELSE ui
    /\ UNCHANGED <<current, identityChanges, requestCount, skill, queued,
                    frozenQueue, submitted, binding, hostPhase,
                    snapshotProgress, replayedCandidate, prepared>>

Select(c) ==
    /\ c \in Choices /\ ui.phase \in {"results", "selected"}
    /\ ui.identity = current /\ current.mode = "legal-save"
    /\ ui' = [ui EXCEPT !.phase = "selected", !.choice = c]
    /\ UNCHANGED <<current, identityChanges, pending, requestCount, skill,
                    queued, frozenQueue, submitted, binding, hostPhase,
                    snapshotProgress, replayedCandidate, prepared>>

Queue ==
    /\ queued = None /\ current.mode = "legal-save"
    /\ ui.phase = "selected" \/ (QueueWithoutSelectionFault /\ ui.phase = "results")
    /\ ~ui.identity.hell \/ Eligible(skill) \/ QueueIneligibleSkillFault
    /\ LET c == Candidate(ui.identity, IF ui.choice = 0 THEN 1 ELSE ui.choice)
           q == [candidate |-> c, skill |-> IF c.hell THEN skill ELSE 0,
                 explicitlySelected |-> ui.phase = "selected"]
       IN /\ queued' = q /\ frozenQueue' = q
    /\ UNCHANGED <<current, identityChanges, ui, pending, requestCount, skill,
                    submitted, binding, hostPhase, snapshotProgress,
                    replayedCandidate, prepared>>

ChangeIdentity(i) ==
    /\ i \in Identities /\ i # current /\ identityChanges = 0
    /\ current' = i /\ identityChanges' = 1 /\ ui' = Idle(i)
    /\ queued' = IF MutableQueueFault /\ queued # None
                  THEN [queued EXCEPT !.candidate = Candidate(i, queued.candidate.choice)]
                  ELSE queued
    /\ UNCHANGED <<pending, requestCount, skill, frozenQueue, submitted,
                    binding, hostPhase, snapshotProgress, replayedCandidate, prepared>>

ChangeSkill(s) ==
    /\ s \in Skills /\ s # skill /\ skill' = s
    /\ queued' = IF MutableQueueFault /\ queued # None
                  THEN [queued EXCEPT !.skill = s]
                  ELSE queued
    /\ UNCHANGED <<current, identityChanges, ui, pending, requestCount,
                    frozenQueue, submitted, binding, hostPhase,
                    snapshotProgress, replayedCandidate, prepared>>

(* Complete reviewed payloads, missing bindings, and hostile transport changes
   all use the same host entry point. A save snapshot supplies actual progress. *)
SubmitQueued(variant) ==
    /\ hostPhase = "idle" /\ queued # None
    /\ variant \in {"exact", "wrong-progress", "wrong-context", "wrong-digest",
                     "wrong-skill", "partial", "unbound"}
    /\ variant # "wrong-skill" \/ queued.candidate.hell
    /\ LET c == queued.candidate
       IN submitted' =
          [candidate |-> c, skill |-> IF variant = "wrong-skill" THEN 3 ELSE queued.skill,
           presence |-> CASE variant = "unbound" -> "none"
                          [] variant = "partial" -> "partial"
                          [] OTHER -> "complete",
           context |-> IF variant = "wrong-context" THEN Context(~c.hell) ELSE c.context,
           progress |-> IF variant = "wrong-progress" THEN 1 - c.progress ELSE c.progress,
           digest |-> CASE variant = "unbound" -> MissingDigest
                        [] variant = "wrong-digest" -> WrongDigest
                        [] OTHER -> Digest(c)]
    /\ snapshotProgress' = Progress(current.save)
    /\ hostPhase' = "received"
    /\ UNCHANGED <<current, identityChanges, ui, pending, requestCount, skill,
                    queued, frozenQueue, binding, replayedCandidate, prepared>>

SubmitLegacy(h) ==
    /\ h \in BOOLEAN /\ hostPhase = "idle" /\ requestCount = 0 /\ queued = None
    /\ LET c == Candidate([current EXCEPT !.hell = h], 1)
       IN submitted' = [candidate |-> c, skill |-> 1, presence |-> "none",
                         context |-> c.context, progress |-> c.progress, digest |-> MissingDigest]
    /\ snapshotProgress' = Progress(current.save) /\ hostPhase' = "received"
    /\ UNCHANGED <<current, identityChanges, ui, pending, requestCount, skill,
                    queued, frozenQueue, binding, replayedCandidate, prepared>>

BindingFor(p) ==
    IF p.presence = "none"
    THEN IF (~p.candidate.hell /\ ~RejectLegacyOrdinaryFault) \/
             (p.candidate.hell /\ AllowUnboundHellFault)
         THEN "legacy-ordinary" ELSE "rejected"
    ELSE IF (p.presence = "complete" \/ AcceptPartialBindingFault) /\
            (p.context = Context(p.candidate.hell) \/ SkipContextCheckFault) /\
            (p.progress = snapshotProgress \/ SkipProgressCheckFault)
         THEN "reviewed" ELSE "rejected"
Bind ==
    /\ hostPhase = "received"
    /\ binding' = BindingFor(submitted)
    /\ hostPhase' = IF binding' = "rejected" THEN "rejected" ELSE "bound"
    /\ UNCHANGED <<current, identityChanges, ui, pending, requestCount, skill,
                    queued, frozenQueue, submitted, snapshotProgress,
                    replayedCandidate, prepared>>

ReplayAllowed ==
    /\ ~submitted.candidate.hell \/ Eligible(submitted.skill) \/ SkipHostSkillCheckFault
    /\ binding = "legacy-ordinary" \/ SkipDigestCheckFault \/
        submitted.digest = Digest(Replay(submitted.candidate, snapshotProgress))
ReplayRecord ==
    /\ hostPhase = "bound" /\ binding \in {"reviewed", "legacy-ordinary"}
    /\ hostPhase' = IF ReplayAllowed THEN "replayed" ELSE "rejected"
    /\ replayedCandidate' = IF ReplayAllowed
                            THEN Replay(submitted.candidate, snapshotProgress) ELSE None
    /\ UNCHANGED <<current, identityChanges, ui, pending, requestCount, skill,
                    queued, frozenQueue, submitted, binding, snapshotProgress, prepared>>
Plan ==
    /\ hostPhase = "replayed"
    /\ prepared' = [candidate |-> replayedCandidate,
                    skill |-> IF PlanChangesSkillFault
                              THEN IF submitted.skill = 1 THEN 2 ELSE 1
                              ELSE submitted.skill]
    /\ hostPhase' = "prepared"
    /\ UNCHANGED <<current, identityChanges, ui, pending, requestCount, skill,
                    queued, frozenQueue, submitted, binding, snapshotProgress,
                    replayedCandidate>>

Next == Start \/ Queue \/ Bind \/ ReplayRecord \/ Plan
        \/ (\E r \in pending, o \in {"results", "empty", "error"} : Complete(r, o))
        \/ (\E c \in Choices : Select(c))
        \/ (\E i \in Identities : ChangeIdentity(i))
        \/ (\E s \in Skills : ChangeSkill(s))
        \/ (\E v \in {"exact", "wrong-progress", "wrong-context", "wrong-digest",
                       "wrong-skill", "partial", "unbound"} : SubmitQueued(v))
        \/ (\E h \in BOOLEAN : SubmitLegacy(h))
Spec == Init /\ [][Next]_vars

TypeOK == /\ current \in Identities /\ identityChanges \in 0..1
          /\ ui \in UIType /\ pending \subseteq RequestType /\ requestCount \in 0..2
          /\ skill \in Skills /\ queued \in QueueType \cup {None}
          /\ frozenQueue \in QueueType \cup {None}
          /\ submitted \in PayloadType \cup {None} /\ binding \in Bindings
          /\ hostPhase \in HostPhases /\ snapshotProgress \in 0..1
          /\ replayedCandidate \in Candidates \cup {None}
          /\ prepared \in PreparedType \cup {None}
StaleResultRejected == ui.phase \in {"results", "selected", "empty", "error"} =>
                         /\ ui.identity = current /\ ui.requestId = requestCount
                         /\ current.mode = "legal-save"
QueueRequiresExplicitSelection == queued # None => queued.explicitlySelected
HellSkillRequiredBeforeQueue == queued # None =>
                                 (queued.candidate.hell => Eligible(queued.skill))
QueueIsFrozen == queued = frozenQueue
EffectSearchIndependentOfSkill == CanSearch => ENABLED Start
CompleteReviewedBinding == (binding = "reviewed" /\ hostPhase \in {"bound", "replayed", "prepared"}) =>
                            submitted.presence = "complete"
ReviewedProgressMatchesSnapshot == (binding = "reviewed" /\ hostPhase \in {"bound", "replayed", "prepared"}) =>
                                    submitted.progress = snapshotProgress
ReviewedContextMatchesRoute == (binding = "reviewed" /\ hostPhase \in {"bound", "replayed", "prepared"}) =>
                                submitted.context = Context(submitted.candidate.hell)
ReviewedDigestMatchesReplay == (binding = "reviewed" /\ hostPhase \in {"replayed", "prepared"}) =>
                                submitted.digest = Digest(replayedCandidate)
HellRequiresReviewedBinding == (hostPhase \in {"bound", "replayed", "prepared"} /\ submitted.candidate.hell) =>
                                binding = "reviewed"
LegacyOrdinaryIsAccepted == (submitted # None /\ submitted.presence = "none" /\ ~submitted.candidate.hell /\
                            hostPhase \in {"bound", "replayed", "prepared", "rejected"}) =>
                              binding = "legacy-ordinary"
HostHellSkillIsEligible == (hostPhase \in {"replayed", "prepared"} /\ submitted.candidate.hell) =>
                            Eligible(submitted.skill)
ReviewedPlanPreservesCandidate == (hostPhase = "prepared" /\ binding = "reviewed") =>
                                   /\ prepared.candidate = submitted.candidate
                                   /\ prepared.skill = submitted.skill
=============================================================================
