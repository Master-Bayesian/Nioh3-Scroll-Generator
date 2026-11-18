------------------------- MODULE ScrollDraftOwner ---------------------------
EXTENDS TLC

CONSTANTS ExactOwner

\* The sources share a seed and slot. The browser processes one selection at a
\* time, but a rendered view can still lag the store's newest inventory.
A1 == <<"A", "snapshot-1", "record-A">>
A2 == <<"A", "snapshot-2", "record-A">>
B1 == <<"B", "snapshot-1", "record-B">>
Sources == {A1, A2, B1}
NoSource == <<"none", "none", "none">>
NoDispatch == <<NoSource, NoSource>>

VARIABLES inventory, shown, draft, dispatched
vars == <<inventory, shown, draft, dispatched>>

Init ==
    /\ inventory = A1
    /\ shown = A1
    /\ draft = NoSource
    /\ dispatched = NoDispatch

Edit ==
    /\ shown \in Sources
    /\ draft' = shown
    /\ UNCHANGED <<inventory, shown, dispatched>>

ChangeSource ==
    /\ inventory' \in Sources \cup {NoSource}
    /\ UNCHANGED <<shown, draft, dispatched>>

RenderSource ==
    /\ shown' = inventory
    /\ draft' = IF shown = inventory THEN draft ELSE NoSource
    /\ UNCHANGED <<inventory, dispatched>>

Prepare ==
    /\ inventory \in Sources /\ draft \in Sources
    /\ ~ExactOwner \/ draft = inventory
    /\ dispatched' = <<draft, inventory>>
    /\ UNCHANGED <<inventory, shown, draft>>

Next == Edit \/ ChangeSource \/ RenderSource \/ Prepare
Spec == Init /\ [][Next]_vars

OnlyReviewedSourceCanBeDispatched ==
    dispatched[1] = dispatched[2]
=============================================================================
