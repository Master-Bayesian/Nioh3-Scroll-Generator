------------------------- MODULE FixedContactRanking ------------------------
EXTENDS Naturals, Sequences
CONSTANTS WrongOwner, ReplacePrefix, ForgetFailure, PublishCancelled
VARIABLES phase, next, sums, published, failedLane
vars == <<phase, next, sums, published, failedLane>>
Owners == <<"first", "first", "second">>
Candidates == {"first", "second", "miss"}
Empty == [c \in Candidates |-> <<>>]
Init == /\ phase = "running" /\ next = 1 /\ sums = Empty /\ published = Empty
        /\ failedLane \in 0..2
Result(prev, i) == IF i = failedLane THEN <<0>>
                   ELSE IF prev = <<0>> THEN IF ForgetFailure THEN <<i>> ELSE <<0>>
                   ELSE IF ReplacePrefix THEN <<i>> ELSE Append(prev, i)
Step == /\ phase = "running" /\ next \in DOMAIN Owners
        /\ LET owner == IF WrongOwner /\ next = 2 THEN "second" ELSE Owners[next]
           IN sums' = [sums EXCEPT ![owner] = Result(@, next)]
        /\ next' = next + 1 /\ UNCHANGED <<phase, published, failedLane>>
Commit == /\ phase = "running" /\ next = Len(Owners) + 1
          /\ phase' = "committed" /\ published' = sums /\ UNCHANGED <<next, sums, failedLane>>
Cancel == /\ phase = "running" /\ phase' = "cancelled"
          /\ published' = IF PublishCancelled THEN sums ELSE Empty
          /\ UNCHANGED <<next, sums, failedLane>>
Spec == Init /\ [][Step \/ Commit \/ Cancel]_vars
OwnershipAndOrder == phase = "committed" =>
   /\ published["first"] = (IF failedLane = 0 THEN <<1,2>> ELSE <<0>>)
   /\ published["second"] = <<3>> /\ published["miss"] = <<>>
NoPartialRank == phase # "committed" => published = Empty
=============================================================================
