---------------------------- MODULE CIValidation ----------------------------
EXTENDS Naturals, FiniteSets, TLC

CONSTANTS IgnoreUnknownFault, IgnoreGlobalInputFault, PruneReferenceFault,
          SkipIntegrationFullFault, AllowMissingInventoryFault,
          IgnoreMissingReportFault, IgnoreStaleSourceFault,
          IgnoreFailedReportFault, DuplicateAssignmentFault,
          IgnoreMissingShadowFault, IgnoreShadowMismatchFault

Tasks == {"a", "b"}
Shards == {"one", "two"}
VARIABLES phase, event, inventoryKnown, graphKnown, globalInput, decision,
          reference, selected, assignment, reports, source, successful,
          shadowPresent, shadowCases
vars == <<phase, event, inventoryKnown, graphKnown, globalInput, decision,
          reference, selected, assignment, reports, source, successful,
          shadowPresent, shadowCases>>

Init == /\ phase = "Discover"
        /\ event \in {"pr", "main", "merge_queue"}
        /\ inventoryKnown \in BOOLEAN
        /\ graphKnown \in BOOLEAN
        /\ globalInput \in BOOLEAN
        /\ decision = "Undecided"
        /\ reference = {}
        /\ selected = {}
        /\ assignment = [s \in Shards |-> {}]
        /\ reports = {}
        /\ source = "current"
        /\ successful = TRUE
        /\ shadowPresent = TRUE
        /\ shadowCases = {}

Integration == event # "pr"
ChooseFull == IF Integration THEN ~SkipIntegrationFullFault
              ELSE (~graphKnown /\ ~IgnoreUnknownFault)
                   \/ (globalInput /\ ~IgnoreGlobalInputFault)

Plan == /\ phase = "Discover"
        /\ IF ~inventoryKnown /\ ~AllowMissingInventoryFault
           THEN /\ phase' = "Blocked"
                /\ decision' = "Blocked"
                /\ UNCHANGED <<reference, selected, assignment>>
           ELSE LET chosen == IF ChooseFull THEN Tasks ELSE {"a"}
                    execution == IF PruneReferenceFault /\ ~Integration
                                 THEN chosen ELSE Tasks
                IN /\ phase' = "Collect"
                   /\ decision' = IF ChooseFull THEN "Full" ELSE "Affected"
                   /\ selected' = chosen
                   /\ reference' = execution
                   /\ assignment' = [s \in Shards |-> IF s = "one" THEN {"a"}
                                      ELSE IF DuplicateAssignmentFault THEN {"a"}
                                           ELSE execution \ {"a"}]
        /\ UNCHANGED <<event, inventoryKnown, graphKnown, globalInput,
                       reports, source, successful, shadowPresent, shadowCases>>

Collect == /\ phase = "Collect"
           /\ \E sample \in {"complete", "missing", "stale", "failed",
                              "shadow_missing", "shadow_mismatch"}:
                /\ phase' = "Ready"
                /\ reports' = IF sample = "missing" THEN {"one"} ELSE Shards
                /\ source' = IF sample = "stale" THEN "old" ELSE "current"
                /\ successful' = (sample # "failed")
                /\ shadowPresent' = (sample # "shadow_missing")
                /\ shadowCases' = IF sample = "shadow_mismatch" THEN Tasks \ selected
                                   ELSE selected
           /\ UNCHANGED <<event, inventoryKnown, graphKnown, globalInput,
                          decision, reference, selected, assignment>>

Finalize == /\ phase = "Ready"
            /\ phase' = IF (reports = Shards \/ IgnoreMissingReportFault)
                           /\ (source = "current" \/ IgnoreStaleSourceFault)
                           /\ (successful \/ IgnoreFailedReportFault)
                           /\ (Integration \/ shadowPresent \/ IgnoreMissingShadowFault)
                           /\ (Integration \/ shadowCases = selected \/ IgnoreShadowMismatchFault)
                        THEN "Passed" ELSE "Failed"
            /\ UNCHANGED <<event, inventoryKnown, graphKnown, globalInput,
                           decision, reference, selected, assignment, reports,
                           source, successful, shadowPresent, shadowCases>>

Next == Plan \/ Collect \/ Finalize
Spec == Init /\ [][Next]_vars

InventoryRequired == phase \in {"Collect", "Ready", "Passed"} => inventoryKnown
ReferenceIsFull == phase \in {"Collect", "Ready", "Passed"} => reference = Tasks
UnknownInputUsesFull == (phase \in {"Collect", "Ready", "Passed"}
                         /\ ~Integration /\ ~graphKnown) => decision = "Full"
NonImportedInputUsesFull == (phase \in {"Collect", "Ready", "Passed"}
                             /\ ~Integration /\ globalInput) => decision = "Full"
IntegrationUsesFull == (phase \in {"Collect", "Ready", "Passed"}
                        /\ Integration) => decision = "Full"
AssignmentIsUnique == phase \in {"Collect", "Ready", "Passed"} =>
                      /\ UNION {assignment[s] : s \in Shards} = reference
                      /\ \A task \in reference:
                            Cardinality({s \in Shards : task \in assignment[s]}) = 1
ReportsAreComplete == phase = "Passed" => reports = Shards
SourceIsCurrent == phase = "Passed" => source = "current"
ReportsAreSuccessful == phase = "Passed" => successful
ShadowIsPresent == (phase = "Passed" /\ ~Integration) => shadowPresent
ShadowMatchesSelection == (phase = "Passed" /\ ~Integration) => shadowCases = selected
=============================================================================
