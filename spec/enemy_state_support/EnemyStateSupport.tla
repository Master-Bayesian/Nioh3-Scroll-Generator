----------------------------- MODULE EnemyStateSupport -----------------------------
EXTENDS Naturals
CONSTANTS CheckVersion, CheckSelector, CheckVariant, CheckTableHash
VARIABLES currentVersion, selector, solo, pinnedTables, supported
vars == <<currentVersion, selector, solo, pinnedTables, supported>>
Init == /\ currentVersion = FALSE /\ selector = 0 /\ solo = FALSE
        /\ pinnedTables = FALSE /\ supported = TRUE
Evaluate ==
    /\ currentVersion' \in BOOLEAN /\ selector' \in 0..2
    /\ solo' \in BOOLEAN /\ pinnedTables' \in BOOLEAN
    /\ supported' = (selector' = 0 \/
        ((currentVersion' \/ ~CheckVersion) /\ (selector' = 1 \/ ~CheckSelector) /\
         (solo' \/ ~CheckVariant) /\ (pinnedTables' \/ ~CheckTableHash)))
Spec == Init /\ [][Evaluate]_vars
TypeOK == /\ currentVersion \in BOOLEAN /\ selector \in 0..2
          /\ solo \in BOOLEAN /\ pinnedTables \in BOOLEAN /\ supported \in BOOLEAN
UnverifiedVersionRemainsUnknown == (supported /\ selector # 0) => currentVersion
UnverifiedSelectorRemainsUnknown == supported => selector \in {0, 1}
UnverifiedVariantRemainsUnknown == (supported /\ selector # 0) => solo
ChangedTablesRemainUnknown == (supported /\ selector # 0) => pinnedTables
=============================================================================
