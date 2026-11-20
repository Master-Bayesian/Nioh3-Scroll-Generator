----------------------------- MODULE FirstUseLoadout -----------------------------
EXTENDS Naturals
CONSTANTS MaxReads, AcceptRetired, SearchWithSources
VARIABLES phase, request, scope, issued, owner, capability
vars == <<phase, request, scope, issued, owner, capability>>
Init == /\ phase = "empty" /\ request = 0 /\ scope = 0 /\ issued = {}
        /\ owner = <<0,0>> /\ capability = "none"
Start == /\ request < MaxReads
         /\ \E s \in {1,2} :
              /\ request' = request + 1 /\ scope' = s
              /\ issued' = issued \cup {<<request',s>>}
              /\ phase' = "reading" /\ owner' = <<0,0>> /\ capability' = "none"
Reply == \E id \in issued, kind \in {"unavailable","sources","damage"} :
           /\ phase = "reading" /\ (id = <<request,scope>> \/ AcceptRetired)
           /\ phase' = IF kind = "damage" THEN "ready" ELSE kind
           /\ capability' = kind /\ owner' = id
           /\ UNCHANGED <<request,scope,issued>>
Search == /\ (phase = "ready" \/ (SearchWithSources /\ phase = "sources"))
          /\ phase' = "ranked" /\ UNCHANGED <<request,scope,issued,owner,capability>>
Next == Start \/ Reply \/ Search \/ UNCHANGED vars
Spec == Init /\ [][Next]_vars
CurrentReply == phase \in {"sources","ready","ranked","unavailable"} => owner = <<request,scope>>
RankRequiresDamage == phase = "ranked" => capability = "damage"
=============================================================================
