--------------------------- MODULE KnownIdPreview ---------------------------
EXTENDS Naturals
CONSTANTS BindRequestedSettings, RejectStaleUse
None == 3
VARIABLES current, requested, generated, displayed, busy, known, used, target
vars == <<current, requested, generated, displayed, busy, known, used, target>>
Init == /\ current = 0 /\ requested = 0
        /\ generated = None /\ displayed = None
        /\ busy = FALSE /\ known = FALSE /\ used = None /\ target = None
ChangeSettings == /\ current' \in 0..2 /\ current' # current
                  /\ UNCHANGED <<requested, generated, displayed, busy, known, used, target>>
BeginView == /\ ~busy /\ requested' = current /\ busy' = TRUE
             /\ UNCHANGED <<current, generated, displayed, known, used, target>>
CompleteView == /\ busy /\ generated' = requested
                /\ displayed' = IF BindRequestedSettings THEN requested ELSE current
                /\ known' = TRUE /\ busy' = FALSE
                /\ UNCHANGED <<current, requested, used, target>>
FailView == /\ busy /\ busy' = FALSE
            /\ UNCHANGED <<current, requested, generated, displayed, known, used, target>>
UsePreview == /\ known /\ ~busy /\ generated # None
              /\ (~RejectStaleUse \/ displayed = current)
              /\ used' = generated /\ target' = current
              /\ UNCHANGED <<current, requested, generated, displayed, busy, known>>
LeavePreview == /\ ~busy /\ known' = FALSE
                /\ UNCHANGED <<current, requested, generated, displayed, busy, used, target>>
Next == ChangeSettings \/ BeginView \/ CompleteView \/ FailView \/ UsePreview \/ LeavePreview
Spec == Init /\ [][Next]_vars
TypeOK == /\ current \in 0..2 /\ requested \in 0..2
          /\ generated \in 0..3 /\ displayed \in 0..3
          /\ used \in 0..3 /\ target \in 0..3
          /\ busy \in BOOLEAN /\ known \in BOOLEAN
PreviewDescribesRequestedSettings == known => displayed = generated
OnlyCurrentPreviewIsUsed == used # None => used = target
=============================================================================
