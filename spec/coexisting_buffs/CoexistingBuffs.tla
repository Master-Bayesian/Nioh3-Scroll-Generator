----------------------- MODULE CoexistingBuffs -----------------------
EXTENDS Naturals
CONSTANT Fault
VARIABLES phase, scene, absorb, defense, earlySkill, ordinary, follow, arts, debit
vars == <<phase,scene,absorb,defense,earlySkill,ordinary,follow,arts,debit>>
Scenes == {"absorbDefense","shiftAbsorb","deflectArts"}
Init == /\ phase = "choose" /\ scene = "absorbDefense"
        /\ absorb = FALSE /\ defense = 0 /\ earlySkill = FALSE
        /\ ordinary = 0 /\ follow = 0 /\ arts = FALSE /\ debit = 1
Choose(s,a,d,k) ==
    /\ phase \in {"choose","hit"} /\ phase' = "offered"
    /\ scene' = s /\ absorb' = a /\ defense' = d /\ earlySkill' = k
    /\ ordinary' = IF Fault = "CountQueued" /\ a THEN 10 ELSE 0
    /\ follow' = 0 /\ arts' = FALSE /\ debit' = 1
Settle ==
    /\ phase = "offered" /\ phase' = "effective"
    /\ LET pickup == IF absorb /\ scene # "deflectArts" THEN 10 ELSE 0
           reaction == IF scene # "shiftAbsorb" THEN defense ELSE 0
       IN ordinary' = IF Fault = "AddSameKey" THEN pickup + reaction
                      ELSE IF reaction > pickup THEN reaction ELSE pickup
    /\ follow' = IF scene = "shiftAbsorb" /\ earlySkill
                    /\ ~(Fault = "DropIndependent" /\ absorb) THEN 6 ELSE 0
    /\ arts' = (scene = "deflectArts")
    /\ debit' = IF scene = "deflectArts" /\ ~(Fault = "ChargeFreshArts" /\ defense > 0)
                THEN 0 ELSE 1
    /\ UNCHANGED <<scene,absorb,defense,earlySkill>>
Hit == /\ phase = "effective" /\ phase' = "hit"
       /\ UNCHANGED <<scene,absorb,defense,earlySkill,ordinary,follow,arts,debit>>
Next == (\E s \in Scenes, a \in BOOLEAN, d \in {0,10,15}, k \in BOOLEAN: Choose(s,a,d,k))
        \/ Settle \/ Hit
Spec == Init /\ [][Next]_vars
QueuedNotEffective == phase = "offered" => ordinary = 0
SingleDamageGroup == phase = "hit" =>
    CASE scene = "deflectArts" -> ordinary = defense
      [] scene = "shiftAbsorb" -> ordinary = (IF absorb THEN 10 ELSE 0)
      [] defense = 15 -> ordinary = 15
      [] OTHER -> ordinary = (IF absorb \/ defense = 10 THEN 10 ELSE 0)
IndependentFollowUp == phase = "hit" => follow = (IF scene = "shiftAbsorb" /\ earlySkill THEN 6 ELSE 0)
FreshArtsDebit == phase = "hit" => arts = (scene = "deflectArts") /\ (debit = 0) = arts
=============================================================================
