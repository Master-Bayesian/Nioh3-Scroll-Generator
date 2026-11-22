-------------------------- MODULE PeakBuffContacts --------------------------
EXTENDS Naturals, Sequences
CONSTANT Fault
VARIABLES phase, pickup, reaction, pill, skill, arts, prepared, status312, target, followUp, cost, scored
vars == <<phase,pickup,reaction,pill,skill,arts,prepared,status312,target,followUp,cost,scored>>

Strongest(p, r, h) == CHOOSE x \in {p,r,h}: \A y \in {p,r,h}: x >= y
Supplies == [carnageSlot: BOOLEAN, carnageCharges: {0,1}, powerBonus: {0,8,13},
             targetSupplies: BOOLEAN]
Available(slot, charges) == slot /\ (charges > 0 \/ Fault = "UseEmptyPrepared")

Init == /\ phase = "choose" /\ pickup = 0 /\ reaction = 0 /\ pill = 0
        /\ skill = FALSE /\ arts = FALSE /\ status312 = 0
        /\ target = [water |-> FALSE, confusion |-> 0, weakness |-> 0]
        /\ prepared = [carnageSlot |-> FALSE, carnageCharges |-> 0, powerBonus |-> 0,
                       targetSupplies |-> FALSE]
        /\ followUp = "inactive" /\ cost = 100 /\ scored = <<>>

Bind(p,r,h,s,a,supply) ==
    LET carnage == Available(supply.carnageSlot,supply.carnageCharges)
        source == IF carnage THEN 40 + supply.powerBonus ELSE Strongest(p,r,h)
        targetReady == Fault # "GateTargetOnStock" \/ supply.targetSupplies
    IN
    /\ phase \in {"choose","finished"} /\ phase' = "ready"
    /\ pickup' = p /\ reaction' = r /\ pill' = h /\ skill' = s /\ arts' = a
    /\ prepared' = supply
    /\ status312' = CASE Fault = "AddCompeting" -> p + r + h + (IF carnage THEN source ELSE 0)
                       [] Fault = "RetainCasterPower" /\ phase = "finished" /\ carnage
                          /\ prepared.carnageSlot /\ prepared.carnageCharges > 0 -> 40 + prepared.powerBonus
                       [] OTHER -> source
    /\ target' = [water |-> targetReady,
                   confusion |-> IF ~targetReady THEN 0
                                 ELSE IF Fault = "AddUndeclaredAilments" THEN 70 ELSE 40,
                   weakness |-> IF targetReady /\ ~(carnage /\ Fault = "MergeCasterAndTarget") THEN 20 ELSE 0]
    /\ followUp' = IF s THEN "active" ELSE "inactive"
    /\ cost' = IF a THEN (IF Fault = "GrantFreshWaiver" THEN 0 ELSE 70) ELSE 100
    /\ scored' = <<>>

UnsettledWindow ==
    /\ phase = "ready" /\ phase' = "afterUnsettled"
    /\ followUp' = IF Fault = "ConsumeUnsettled" THEN "inactive" ELSE followUp
    /\ UNCHANGED <<pickup,reaction,pill,skill,arts,prepared,status312,target,cost,scored>>

FirstSettlement ==
    /\ phase \in {"ready","afterUnsettled"} /\ phase' = "awaitRebuild"
    /\ scored' = Append(scored, <<status312, target, followUp = "active", arts, cost>>)
    /\ followUp' = IF followUp = "active" THEN "removed" ELSE followUp
    /\ UNCHANGED <<pickup,reaction,pill,skill,arts,prepared,status312,target,cost>>

Rebuild ==
    /\ phase = "awaitRebuild" /\ phase' = "afterRebuild"
    /\ followUp' = IF followUp = "removed" /\ Fault = "KeepSpent"
                   THEN "active" ELSE "inactive"
    /\ status312' = IF Fault = "ExpirePersistent" THEN 0 ELSE status312
    /\ UNCHANGED <<pickup,reaction,pill,skill,arts,prepared,target,cost,scored>>

SecondSettlement ==
    /\ phase = "afterRebuild" /\ phase' = "finished"
    /\ scored' = Append(scored, <<status312, target, followUp = "active", arts, cost>>)
    /\ UNCHANGED <<pickup,reaction,pill,skill,arts,prepared,status312,target,followUp,cost>>

Next == (\E p \in {0,10}, r \in {0,10,15}, h \in {0,15}, s,a \in BOOLEAN, supply \in Supplies:
            Bind(p,r,h,s,a,supply))
        \/ UnsettledWindow \/ FirstSettlement \/ Rebuild \/ SecondSettlement
Spec == Init /\ [][Next]_vars
Expected == IF prepared.carnageSlot /\ prepared.carnageCharges = 1 THEN 40 + prepared.powerBonus
            ELSE IF reaction = 15 \/ pill = 15 THEN 15
            ELSE IF pickup = 10 \/ reaction = 10 THEN 10 ELSE 0
ExpectedTarget == [water |-> TRUE, confusion |-> 40, weakness |-> 20]
ExpectedCost == IF arts THEN 70 ELSE 100
PeakChannels == phase = "finished" =>
                scored = <<<<Expected,ExpectedTarget,skill,arts,ExpectedCost>>,
                            <<Expected,ExpectedTarget,FALSE,arts,ExpectedCost>>>>
=============================================================================
