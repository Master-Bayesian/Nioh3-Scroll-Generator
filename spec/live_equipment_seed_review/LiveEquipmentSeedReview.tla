---- MODULE LiveEquipmentSeedReview ----
EXTENDS Naturals

CONSTANTS AcceptObsoleteSearch, SkipPreviewReview
VARIABLES context, search, operation
vars == <<context, search, operation>>

Epochs == 1..3
SearchState == [pending: 0..3, result: 0..3, selection: 0..3]
OperationState == [phase: {"idle", "preparing", "prepared", "cancelled", "dispatched"},
                   metadata: {"manual", "known", "invalid"},
                   preview: {"none", "matched", "different"},
                   confirmed: BOOLEAN,
                   usedSearch: BOOLEAN]
Idle == [phase |-> "idle", metadata |-> "manual", preview |-> "none",
         confirmed |-> FALSE, usedSearch |-> FALSE]

Init ==
    /\ context = 1
    /\ search = [pending |-> 0, result |-> 0, selection |-> 0]
    /\ operation = Idle

ChangeContext ==
    /\ operation.phase = "idle" /\ context < 3
    /\ context' = context + 1
    /\ search' = [search EXCEPT !.result = 0, !.selection = 0]
    /\ UNCHANGED operation

StartSearch ==
    /\ operation.phase = "idle" /\ search.pending = 0
    /\ search' = [pending |-> context, result |-> 0, selection |-> 0]
    /\ UNCHANGED <<context, operation>>

FinishSearch(hasResult) ==
    /\ search.pending # 0
    /\ search' = [pending |-> 0, selection |-> 0,
                  result |-> IF hasResult /\ (search.pending = context \/ AcceptObsoleteSearch)
                            THEN search.pending ELSE 0]
    /\ UNCHANGED <<context, operation>>

SelectSeed ==
    /\ operation.phase = "idle" /\ search.result = context
    /\ search' = [search EXCEPT !.selection = context]
    /\ UNCHANGED <<context, operation>>

EditManualSeed ==
    /\ operation.phase = "idle" /\ search.selection # 0
    /\ search' = [search EXCEPT !.selection = 0]
    /\ UNCHANGED <<context, operation>>

Prepare(metadata, useSearch) ==
    /\ operation.phase = "idle" /\ search.pending = 0
    /\ ~useSearch \/ search.selection = context
    /\ operation' = [Idle EXCEPT !.phase = "preparing", !.metadata = metadata,
                                  !.usedSearch = useSearch]
    /\ UNCHANGED <<context, search>>

Preview(value) ==
    /\ operation.phase = "preparing"
    /\ operation' = [operation EXCEPT !.phase = "prepared", !.preview = value]
    /\ UNCHANGED <<context, search>>

RestoreMalformedRequirements ==
    /\ operation.phase = "prepared" /\ operation.metadata = "known"
    /\ operation' = [operation EXCEPT !.metadata = "invalid", !.confirmed = FALSE]
    /\ UNCHANGED <<context, search>>

ReviewAllows ==
    /\ operation.phase \in {"prepared", "dispatched"}
    /\ operation.preview # "none"
    /\ operation.metadata = "manual"
       \/ (operation.metadata = "known" /\ operation.preview = "matched")

Confirm ==
    /\ operation.phase = "prepared" /\ ~operation.confirmed
    /\ ReviewAllows \/ SkipPreviewReview
    /\ operation' = [operation EXCEPT !.confirmed = TRUE]
    /\ UNCHANGED <<context, search>>

Dispatch ==
    /\ operation.phase = "prepared" /\ operation.confirmed
    /\ ReviewAllows \/ SkipPreviewReview
    /\ operation' = [operation EXCEPT !.phase = "dispatched"]
    /\ UNCHANGED <<context, search>>

Cancel ==
    /\ operation.phase = "prepared"
    /\ operation' = [operation EXCEPT !.phase = "cancelled", !.confirmed = FALSE]
    /\ UNCHANGED <<context, search>>

Next ==
    \/ ChangeContext \/ StartSearch \/ SelectSeed \/ EditManualSeed
    \/ \E result \in BOOLEAN : FinishSearch(result)
    \/ \E meta \in {"manual", "known"}, useSearch \in BOOLEAN : Prepare(meta, useSearch)
    \/ \E value \in {"matched", "different"} : Preview(value)
    \/ RestoreMalformedRequirements \/ Confirm \/ Dispatch \/ Cancel

Spec == Init /\ [][Next]_vars
TypeOK == context \in Epochs /\ search \in SearchState /\ operation \in OperationState
ResultsAreCurrent == search.result # 0 => search.result = context
SelectedSeedIsCurrent == search.selection # 0 => search.selection = search.result /\ search.result = context
SearchSeedRequiresSelection == operation.usedSearch => search.selection = context
ConfirmedPreviewIsReviewable == operation.confirmed => ReviewAllows
InvalidRecoveryCanCancel ==
    (operation.phase = "prepared" /\ operation.metadata = "invalid") => ENABLED Cancel
====
