----------------------------- MODULE SeedSearch -----------------------------
EXTENDS Naturals, FiniteSets
CONSTANTS ImplicitCpuFault, UncommittedCursorFault
VARIABLES policy, preference, cuda, directcompute, backend, state,
          cursor, pending, checkpoint, published, publishCount
vars == <<policy, preference, cuda, directcompute, backend, state,
          cursor, pending, checkpoint, published, publishCount>>
Init == /\ policy \in {"strict", "allow"}
        /\ preference \in {"auto", "cuda", "directcompute"}
        /\ cuda \in BOOLEAN /\ directcompute \in BOOLEAN
        /\ backend = "none" /\ state = "ready"
        /\ cursor = 0 /\ pending = 0 /\ checkpoint = 0
        /\ published = {} /\ publishCount = 0
Selection == IF preference = "directcompute"
                THEN IF directcompute THEN "directcompute" ELSE "refused"
             ELSE IF cuda THEN "cuda"
             ELSE IF preference = "auto" /\ directcompute THEN "directcompute"
             ELSE IF policy = "allow" \/ ImplicitCpuFault THEN "cpu"
             ELSE "refused"
Select == /\ state = "ready" /\ backend' = Selection
          /\ state' = IF Selection = "refused" THEN "failed" ELSE "selected"
          /\ UNCHANGED <<policy, preference, cuda, directcompute,
                        cursor, pending, checkpoint, published, publishCount>>
Dispatch == /\ state = "selected" /\ state' = "running"
            /\ UNCHANGED <<policy, preference, cuda, directcompute, backend,
                          cursor, pending, checkpoint, published, publishCount>>
Decide == /\ state = "running" /\ cursor < 3
          /\ pending' \in (cursor + 1)..3 /\ state' = "decided"
          /\ UNCHANGED <<policy, preference, cuda, directcompute, backend,
                        cursor, checkpoint, published, publishCount>>
Publish == /\ state = "decided"
           /\ published' = published \cup ((cursor + 1)..pending)
           /\ publishCount' = publishCount + pending - cursor
           /\ cursor' = pending /\ checkpoint' = pending
           /\ state' = IF pending = 3 THEN "complete" ELSE "ready"
           /\ UNCHANGED <<policy, preference, cuda, directcompute, backend, pending>>
Cancel == /\ state \in {"running", "decided"} /\ state' = "cancelled"
          /\ cursor' = IF UncommittedCursorFault /\ cursor < 3
                         THEN cursor + 1 ELSE cursor
          /\ pending' = cursor'
          /\ UNCHANGED <<policy, preference, cuda, directcompute, backend,
                        checkpoint, published, publishCount>>
Resume == /\ state = "cancelled" /\ state' = "ready" /\ backend' = "none"
          /\ UNCHANGED <<policy, preference, cuda, directcompute,
                        cursor, pending, checkpoint, published, publishCount>>
Failure == /\ state = "running" /\ state' = "failed" /\ pending' = cursor
           /\ UNCHANGED <<policy, preference, cuda, directcompute, backend,
                         cursor, checkpoint, published, publishCount>>
Next == Select \/ Dispatch \/ Decide \/ Publish \/ Cancel \/ Resume \/ Failure
Spec == Init /\ [][Next]_vars
TypeOK == /\ policy \in {"strict", "allow"}
          /\ preference \in {"auto", "cuda", "directcompute"}
          /\ backend \in {"none", "cuda", "directcompute", "cpu", "refused"}
          /\ state \in {"ready", "selected", "running", "decided", "cancelled", "complete", "failed"}
          /\ cursor \in 0..3 /\ checkpoint \in 0..3 /\ pending \in 0..3
          /\ published \subseteq 1..3 /\ publishCount \in 0..3
CpuRequiresOptIn == backend = "cpu" => policy = "allow"
CursorIsCommitted == cursor = checkpoint
PublishedPrefix == published = 1..checkpoint
PublishedOnce == publishCount = Cardinality(published)
GpuHasCapability == /\ (backend = "cuda" => cuda)
                    /\ (backend = "directcompute" => directcompute)
=============================================================================
