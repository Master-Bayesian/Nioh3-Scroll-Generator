------------------------------ MODULE SelectorCapture ------------------------------
EXTENDS Naturals
CONSTANTS RequireIdentity, RequireCompleteTrace, RequireSeedReplay,
          RequireCleanup, RequireNativeEvidence
VARIABLES phase, identityOK, traceComplete, seedParity, cleanupOK, native, certified
vars == <<phase, identityOK, traceComplete, seedParity, cleanupOK, native, certified>>
Init == /\ phase = "prepared"
        /\ identityOK = FALSE /\ traceComplete = FALSE /\ seedParity = FALSE
        /\ cleanupOK = FALSE /\ native = FALSE /\ certified = FALSE
Admit == /\ phase = "prepared"
         /\ identityOK' \in BOOLEAN /\ native' \in BOOLEAN
         /\ phase' = IF identityOK' \/ ~RequireIdentity THEN "armed" ELSE "rejected"
         /\ UNCHANGED <<traceComplete, seedParity, cleanupOK, certified>>
Capture == /\ phase = "armed"
           /\ traceComplete' \in BOOLEAN
           /\ phase' = IF traceComplete' \/ ~RequireCompleteTrace THEN "captured" ELSE "rejected"
           /\ UNCHANGED <<identityOK, seedParity, cleanupOK, native, certified>>
Compare == /\ phase = "captured"
           /\ seedParity' \in BOOLEAN
           /\ phase' = IF seedParity' \/ ~RequireSeedReplay THEN "compared" ELSE "rejected"
           /\ UNCHANGED <<identityOK, traceComplete, cleanupOK, native, certified>>
Clean == /\ phase = "compared"
         /\ cleanupOK' \in BOOLEAN
         /\ phase' = IF cleanupOK' \/ ~RequireCleanup THEN "cleaned" ELSE "rejected"
         /\ UNCHANGED <<identityOK, traceComplete, seedParity, native, certified>>
Certify == /\ phase = "cleaned"
           /\ certified' = (native \/ ~RequireNativeEvidence)
           /\ phase' = IF certified' THEN "certified_one_entry" ELSE "owned_fixture_only"
           /\ UNCHANGED <<identityOK, traceComplete, seedParity, cleanupOK, native>>
Next == Admit \/ Capture \/ Compare \/ Clean \/ Certify
Spec == Init /\ [][Next]_vars
TypeOK == /\ phase \in {"prepared", "armed", "captured", "compared", "cleaned",
                        "rejected", "certified_one_entry", "owned_fixture_only"}
          /\ identityOK \in BOOLEAN /\ traceComplete \in BOOLEAN /\ seedParity \in BOOLEAN
          /\ cleanupOK \in BOOLEAN /\ native \in BOOLEAN /\ certified \in BOOLEAN
IdentityRequired == certified => identityOK
TraceRequired == certified => traceComplete
SeededParityRequired == certified => seedParity
VerifiedCleanupRequired == certified => cleanupOK
OwnedFixtureNeverCertifiesNative == certified => native
=============================================================================
