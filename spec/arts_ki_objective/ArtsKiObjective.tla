-------------------------- MODULE ArtsKiObjective --------------------------
EXTENDS Naturals, Integers
CONSTANT Fault
VARIABLES initial, objective, phase, step, ki, state
vars == <<initial, objective, phase, step, ki, state>>

\* Normalized units for one eligible martial action. The original f32
\* producer and actual PC202 per-outfit costs are checked separately.
Budget == 20
InitialCost == CASE initial = "Absent" -> 10 [] initial = "Charging" -> 7 [] OTHER -> 0
Init == /\ initial \in {"Absent","Ready","Debited","Charging"}
        /\ objective \in {"Opening","Depletion"}
        /\ phase = initial /\ step = 0 /\ ki = Budget
        /\ state = IF objective = "Depletion" /\ initial \in {"Ready","Debited"}
                      /\ Fault # "FalseDepletion" THEN "NoEndpoint" ELSE "Running"

\* No notification 12 occurs inside this held action chain. Notification
\* 11 is not a transition to Charging. Contacts are downstream of each debit.
Advance == /\ state = "Running"
           /\ LET cost == CASE phase = "Absent" -> 10
                           [] phase = "Charging" -> IF Fault = "LoseDiscount" THEN 10 ELSE 7
                           [] OTHER -> 0
                  next == ki - cost
              IN /\ ki' = next
                 /\ step' = step + 1
                 /\ phase' = CASE phase = "Ready" -> "Debited"
                                  [] phase = "Debited" /\ Fault = "ChargeOnNextContact" -> "Charging"
                                  [] OTHER -> phase
                 /\ state' = IF (objective = "Opening" /\ step' = 3) \/ next <= 0
                             THEN "Finished" ELSE "Running"
           /\ UNCHANGED <<initial,objective>>
Spec == Init /\ [][Advance]_vars
Endpoint ==
  (objective = "Depletion" /\ initial \in {"Ready","Debited"}) <=> state = "NoEndpoint"
Debit == ki = Budget - step * InitialCost
Phase == initial \in {"Ready","Debited"} => phase \in {"Ready","Debited"}
FinitePrefix == objective = "Opening" => step <= 3
=============================================================================
