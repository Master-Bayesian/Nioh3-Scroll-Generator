-------------------- MODULE LoadoutPreparedWear --------------------
EXTENDS Naturals
CONSTANTS KeepStaleConfirmation, DiscardForeignPlan, CommitRetired
VARIABLES phase, view, sequence, plan, owner, servicePlan,
          discardOwner, discardTarget, receiptPlan, receiptOwner,
          receiptView, receiptPhase, receiptService
vars == <<phase, view, sequence, plan, owner, servicePlan,
          discardOwner, discardTarget, receiptPlan, receiptOwner,
          receiptView, receiptPhase, receiptService>>

Init ==
    /\ phase = "none" /\ view = 0 /\ sequence = 0
    /\ plan = 0 /\ owner = 0 /\ servicePlan = 0
    /\ discardOwner = 0 /\ discardTarget = 0
    /\ receiptPlan = 0 /\ receiptOwner = 0 /\ receiptView = 0
    /\ receiptPhase = "none" /\ receiptService = 0

StartPrepare ==
    /\ phase = "none" /\ sequence < 2
    /\ sequence' = sequence + 1 /\ plan' = sequence'
    /\ owner' = view /\ phase' = "preparing" /\ servicePlan' = 0
    /\ UNCHANGED <<view, discardOwner, discardTarget, receiptPlan, receiptOwner,
                    receiptView, receiptPhase, receiptService>>

FinishPrepare ==
    /\ phase = "preparing"
    /\ servicePlan' = plan
    /\ phase' = IF owner = view THEN "planned" ELSE "retiring"
    /\ UNCHANGED <<view, sequence, plan, owner, discardOwner, discardTarget,
                    receiptPlan, receiptOwner, receiptView, receiptPhase, receiptService>>

\* Rendering checks ownership before the asynchronous discard effect runs.
ChangeInputs ==
    /\ view < 2 /\ view' = view + 1
    /\ phase' = IF phase = "planned" /\ ~KeepStaleConfirmation THEN "retiring" ELSE phase
    /\ UNCHANGED <<sequence, plan, owner, servicePlan, discardOwner, discardTarget,
                    receiptPlan, receiptOwner, receiptView, receiptPhase, receiptService>>

\* Another view may replace the shared service plan after this UI reviewed it.
ReplaceServicePlan ==
    /\ phase \in {"planned", "retiring", "failed"} /\ servicePlan # 3
    /\ servicePlan' = 3
    /\ phase' = IF phase = "planned" /\ ~KeepStaleConfirmation THEN "retiring" ELSE phase
    /\ UNCHANGED <<view, sequence, plan, owner, discardOwner, discardTarget,
                    receiptPlan, receiptOwner, receiptView, receiptPhase, receiptService>>

CancelPlan ==
    /\ phase = "planned" /\ phase' = "retiring"
    /\ UNCHANGED <<view, sequence, plan, owner, servicePlan, discardOwner, discardTarget,
                    receiptPlan, receiptOwner, receiptView, receiptPhase, receiptService>>

RetireFailed ==
    /\ phase = "retiring" /\ phase' = "failed"
    /\ UNCHANGED <<view, sequence, plan, owner, servicePlan, discardOwner, discardTarget,
                    receiptPlan, receiptOwner, receiptView, receiptPhase, receiptService>>

RetryRetire ==
    /\ phase = "failed" /\ phase' = "retiring"
    /\ UNCHANGED <<view, sequence, plan, owner, servicePlan, discardOwner, discardTarget,
                    receiptPlan, receiptOwner, receiptView, receiptPhase, receiptService>>

RetirePlan ==
    /\ phase = "retiring"
    /\ discardOwner' = plan
    /\ discardTarget' = IF DiscardForeignPlan \/ servicePlan = plan THEN servicePlan ELSE 0
    /\ servicePlan' = IF discardTarget' # 0 THEN 0 ELSE servicePlan
    /\ phase' = "none" /\ plan' = 0
    /\ UNCHANGED <<view, sequence, owner, receiptPlan, receiptOwner, receiptView,
                    receiptPhase, receiptService>>

CommitPlan ==
    /\ plan # 0
    /\ IF CommitRetired THEN phase \in {"planned", "retiring", "failed"}
       ELSE phase = "planned" /\ owner = view /\ servicePlan = plan
    /\ receiptPlan' = plan /\ receiptOwner' = owner /\ receiptView' = view
    /\ receiptPhase' = phase /\ receiptService' = servicePlan
    /\ phase' = "none" /\ plan' = 0 /\ servicePlan' = 0
    /\ UNCHANGED <<view, sequence, owner, discardOwner, discardTarget>>

Next == StartPrepare \/ FinishPrepare \/ ChangeInputs \/ ReplaceServicePlan \/
        CancelPlan \/ RetireFailed \/ RetryRetire \/ RetirePlan \/ CommitPlan
Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ phase \in {"none", "preparing", "planned", "retiring", "failed"}
    /\ view \in 0..2 /\ sequence \in 0..2 /\ plan \in 0..2 /\ owner \in 0..2
    /\ servicePlan \in 0..3 /\ discardOwner \in 0..2 /\ discardTarget \in 0..3
    /\ receiptPlan \in 0..2 /\ receiptOwner \in 0..2 /\ receiptView \in 0..2
    /\ receiptPhase \in {"none", "planned", "retiring", "failed"} /\ receiptService \in 0..3
VisibleConfirmationOwned == phase = "planned" => owner = view /\ servicePlan = plan
DiscardOnlyReviewedPlan == discardTarget = 0 \/ discardTarget = discardOwner
CommitOnlyCurrentPlan == receiptPlan = 0 \/
    (receiptPhase = "planned" /\ receiptOwner = receiptView /\ receiptService = receiptPlan)
RetirementKeepsIdentity == phase \in {"retiring", "failed"} => plan # 0
=============================================================================
