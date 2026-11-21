------------------------- MODULE WeaponTypeProjection -------------------------
EXTENDS Naturals
CONSTANTS AllowOtherForm, KeepWrongWeapon, ScoreWrongBaseline
VARIABLES held, requested, phase, owned, ranked, baseline
vars == <<held, requested, phase, owned, ranked, baseline>>
Weapons == {1,2,3,4}
School(w) == IF w <= 2 THEN 0 ELSE 1
Init == /\ held \in Weapons /\ requested \in Weapons /\ owned \in SUBSET Weapons
        /\ phase = "read" /\ ranked = {} /\ baseline = "unscored"
Bind == /\ phase = "read" /\ (School(held) = School(requested) \/ AllowOtherForm)
        /\ phase' = "bound" /\ UNCHANGED <<held, requested, owned, ranked, baseline>>
Publish == /\ phase = "bound" /\ requested \in owned
           /\ ranked' = {requested} \cup (IF KeepWrongWeapon THEN {held} ELSE {})
           /\ baseline' = IF held = requested \/ ScoreWrongBaseline THEN "scored" ELSE "not applicable"
           /\ phase' = "published" /\ UNCHANGED <<held, requested, owned>>
Next == Bind \/ Publish
Spec == Init /\ [][Next]_vars
FormMatches == phase \in {"bound","published"} => School(held) = School(requested)
RankedType == phase = "published" => ranked = {requested}
BaselineApplicable == baseline = "scored" => held = requested
=============================================================================
