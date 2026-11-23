---------------------- MODULE ImmediateDamageSources ----------------------
EXTENDS Naturals
CONSTANT Fault
VARIABLES phase, event, outfit, absorb, skill, strong, gain
vars == <<phase,event,outfit,absorb,skill,strong,gain>>

Events == {"absorb","evade","deflect"}
Outfits == {"neutral","absorbAffix","absorbSet","unqualified","weak","both","otherBuff"}
Init == /\ phase = "select" /\ event = "absorb" /\ outfit = "neutral"
        /\ absorb = FALSE /\ skill = FALSE /\ strong = FALSE /\ gain = 0

Select(e,o) == /\ phase \in {"select","scored"}
               /\ phase' = "materialized" /\ event' = e /\ outfit' = o
               /\ absorb' = IF Fault = "StaleBank" THEN absorb
                            ELSE o \in {"absorbAffix","absorbSet"}
               /\ skill' = (o \in {"weak","both"}) /\ strong' = (o = "both")
               /\ gain' = 0

Score == /\ phase = "materialized" /\ phase' = "scored"
         /\ gain' = CASE event = "absorb" ->
                         IF absorb \/ (Fault = "WrongChannel" /\ outfit = "otherBuff")
                         THEN 10 ELSE 0
                    [] strong -> IF Fault = "StackSameKey" /\ skill THEN 25 ELSE 15
                    [] skill -> 10
                    [] OTHER -> 0
         /\ UNCHANGED <<event,outfit,absorb,skill,strong>>
Next == (\E e \in Events, o \in Outfits: Select(e,o)) \/ Score
Spec == Init /\ [][Next]_vars

CandidateBank == phase # "select" => (absorb = (outfit \in {"absorbAffix","absorbSet"}))
ExpectedGain == CASE event = "absorb" -> IF outfit \in {"absorbSet","absorbAffix"} THEN 10 ELSE 0
                [] outfit = "both" -> 15
                [] outfit = "weak" -> 10
                [] OTHER -> 0
DamageChannel == phase = "scored" => gain = ExpectedGain
=============================================================================
