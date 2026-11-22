---------------------------- MODULE HeldActionRoute ----------------------------
EXTENDS Integers, Sequences
CONSTANT Fault
VARIABLES budget, upgraded, ki, current, counter, path
vars == <<budget, upgraded, ki, current, counter, path>>

\* Finite resource units exercise exact zero and the last permitted overdraft.
\* Native f32 costs and the real initial Ki are tested by the original-code replay.
Cost(a) == IF a \in {25329,25338} THEN 3 ELSE IF a % 2 = 0 THEN 1 ELSE 2
Init == /\ budget \in {3,4,5,10,14,20}
        /\ upgraded \in BOOLEAN
        /\ ki = budget /\ current = 25329 /\ counter = 0 /\ path = <<>>

Dest(a) == CASE a = 25329 -> 25330
             [] a = 25330 -> 25331
             [] a = 25331 -> IF upgraded THEN 25332 ELSE 25330
             [] a = 25332 -> 25333
             [] a = 25333 -> 25334
             [] a = 25334 -> 25335
             [] a = 25335 -> IF counter >= 1 \/ Fault = "AlwaysUpgrade" THEN 25336 ELSE 25334
             [] a = 25336 -> 25337
             [] a = 25337 -> 25336

Advance == /\ current \in 25329..25337
           /\ LET nextKi == ki - Cost(current)
                  dest == Dest(current)
              IN /\ ki' = nextKi
                 /\ current' = IF nextKi <= 0 THEN
                                    IF Fault = "DropFinish" THEN 0 ELSE 25338
                               ELSE dest
                 /\ counter' = IF current = 25333 THEN 0
                               ELSE IF current = 25335 /\ dest = 25334 /\ Fault # "LoseIncrement"
                                    THEN 1 ELSE counter
           /\ path' = Append(path,current)
           /\ UNCHANGED <<budget,upgraded>>
Finish == /\ current = 25338
          /\ ki' = ki - Cost(current) /\ path' = Append(path,current)
          /\ current' = 0 /\ UNCHANGED <<budget,upgraded,counter>>
Spec == Init /\ [][Advance \/ Finish]_vars

Expected(i) ==
    IF i = 1 THEN 25329
    ELSE IF ~upgraded THEN IF i % 2 = 0 THEN 25330 ELSE 25331
    ELSE IF i <= 9 THEN <<25329,25330,25331,25332,25333,25334,25335,25334,25335>>[i]
    ELSE IF i % 2 = 0 THEN 25336 ELSE 25337

RECURSIVE Spent(_)
Spent(seq) == IF seq = <<>> THEN 0 ELSE Cost(Head(seq)) + Spent(Tail(seq))
Balance == ki = budget - Spent(path)
SustainRoute == \A i \in 1..Len(path): path[i] # 25338 => path[i] = Expected(i)
TerminalCharge == current = 0 => /\ Len(path) >= 2
                                  /\ path[Len(path)] = 25338
                                  /\ ki + Cost(25338) <= 0
                                  /\ ki + Cost(25338) + Cost(path[Len(path)-1]) > 0
=============================================================================
