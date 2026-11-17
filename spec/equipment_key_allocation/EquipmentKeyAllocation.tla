----------------------- MODULE EquipmentKeyAllocation -----------------------
EXTENDS Naturals, Sequences

(* TLC integer arithmetic is signed. MaxKey and BeforeMaxKey are the exact
   u32 boundary values 0xFFFFFFFF and 0xFFFFFFFE, represented symbolically.
   Their little-endian bytes and allowed successor are stated explicitly. *)
CONSTANTS MaxKey, BeforeMaxKey, ForeignCounter, NoValue,
          InheritedU16LimitFault, AllowZeroKeyFault, AllowMaxKeyFault,
          WrapCounterFault, SkipAdvanceFault, TruncateKeyFault, RenumberKeyFault,
          IgnoreSourceFault, IgnoreCounterFault, IgnoreSlotFault,
          RewriteEffectsFault, RewriteOtherRecordsFault, WriteOnRejectionFault

BoundaryKeys == {0, 65534, 65535, 65536, 255761, BeforeMaxKey, MaxKey}
ValidKeys == {65534, 65535, 65536, 255761, BeforeMaxKey}
HighKeys == {65535, 65536, 255761, BeforeMaxKey}
CounterValues == BoundaryKeys \cup {1, 2, 65537, 255762, ForeignCounter}
Phases == {"ready", "prepared", "applied", "rejected"}
SlotStates == {"reviewed-free", "foreign-free", "occupied", "new-record"}
FreeBytes == <<0, 0, 0, 0>>
Bytes32 == [1..4 -> 0..255]

KeyBytes(k) ==
    CASE k = BeforeMaxKey -> <<254, 255, 255, 255>>
      [] k = MaxKey -> <<255, 255, 255, 255>>
      [] OTHER -> <<k % 256, (k \div 256) % 256,
                    (k \div 65536) % 256, (k \div 16777216) % 256>>
Low16Bytes(k) == <<KeyBytes(k)[1], KeyBytes(k)[2], 0, 0>>
Successor(k) ==
    CASE k = 1 -> 2
      [] k = 65534 -> 65535
      [] k = 65535 -> 65536
      [] k = 65536 -> 65537
      [] k = 255761 -> 255762
      [] k = BeforeMaxKey -> MaxKey
      [] OTHER -> NoValue

VARIABLES phase, key, source, counter, serialCounter, slot,
          allocatedKey, storedKeyBytes, effects, otherRecords,
          appliedFromCounters, appliedFromSlot, writes, rejection
vars == <<phase, key, source, counter, serialCounter, slot,
          allocatedKey, storedKeyBytes, effects, otherRecords,
          appliedFromCounters, appliedFromSlot, writes, rejection>>

Init ==
    /\ phase = "ready" /\ key \in BoundaryKeys
    /\ source = "reviewed-source" /\ counter = key /\ serialCounter = 1
    /\ slot = "reviewed-free" /\ allocatedKey = NoValue
    /\ storedKeyBytes = FreeBytes /\ effects = "reviewed-effects"
    /\ otherRecords = "original-other-records"
    /\ appliedFromCounters = NoValue /\ appliedFromSlot = NoValue
    /\ writes = 0 /\ rejection = "none"

RangeAllowed ==
    /\ (key \in ValidKeys) \/ (AllowZeroKeyFault /\ key = 0) \/
        (AllowMaxKeyFault /\ key = MaxKey)
    /\ ~(InheritedU16LimitFault /\ key \in HighKeys)
Prepare ==
    /\ phase = "ready"
    /\ phase' = IF RangeAllowed THEN "prepared" ELSE "rejected"
    /\ rejection' = IF RangeAllowed THEN "none" ELSE "range"
    /\ UNCHANGED <<key, source, counter, serialCounter, slot, allocatedKey,
                    storedKeyBytes, effects, otherRecords, appliedFromCounters,
                    appliedFromSlot, writes>>

(* Ambient source/counter/slot drift is separate from writes by this operation.
   Each changed value remains within this finite domain; no actual file exists. *)
ChangeSource ==
    /\ phase = "prepared" /\ source = "reviewed-source"
    /\ source' = "changed-source"
    /\ UNCHANGED <<phase, key, counter, serialCounter, slot, allocatedKey,
                    storedKeyBytes, effects, otherRecords, appliedFromCounters,
                    appliedFromSlot, writes, rejection>>
ChangeKeyCounter ==
    /\ phase = "prepared" /\ counter = key /\ counter' = ForeignCounter
    /\ UNCHANGED <<phase, key, source, serialCounter, slot, allocatedKey,
                    storedKeyBytes, effects, otherRecords, appliedFromCounters,
                    appliedFromSlot, writes, rejection>>
ChangeSerialCounter ==
    /\ phase = "prepared" /\ serialCounter = 1 /\ serialCounter' = 2
    /\ UNCHANGED <<phase, key, source, counter, slot, allocatedKey,
                    storedKeyBytes, effects, otherRecords, appliedFromCounters,
                    appliedFromSlot, writes, rejection>>
ChangeSlot(next) ==
    /\ phase = "prepared" /\ slot = "reviewed-free"
    /\ next \in {"foreign-free", "occupied"} /\ slot' = next
    /\ UNCHANGED <<phase, key, source, counter, serialCounter, allocatedKey,
                    storedKeyBytes, effects, otherRecords, appliedFromCounters,
                    appliedFromSlot, writes, rejection>>

OwnershipAllowed ==
    /\ source = "reviewed-source" \/ IgnoreSourceFault
    /\ (counter = key /\ serialCounter = 1) \/ IgnoreCounterFault
    /\ slot = "reviewed-free" \/ IgnoreSlotFault
Apply ==
    /\ phase = "prepared" /\ OwnershipAllowed
    /\ LET chosen == IF RenumberKeyFault /\ key \in HighKeys THEN 1 ELSE key
       IN /\ allocatedKey' = chosen
          /\ storedKeyBytes' = IF TruncateKeyFault THEN Low16Bytes(chosen) ELSE KeyBytes(chosen)
          /\ counter' = IF WrapCounterFault /\ chosen = BeforeMaxKey THEN 0
                         ELSE IF SkipAdvanceFault THEN chosen ELSE Successor(chosen)
    /\ serialCounter' = 2 /\ slot' = "new-record"
    /\ appliedFromCounters' = <<counter, serialCounter>>
    /\ appliedFromSlot' = slot /\ writes' = 1 /\ phase' = "applied"
    /\ effects' = IF RewriteEffectsFault THEN "changed-effects" ELSE effects
    /\ otherRecords' = IF RewriteOtherRecordsFault THEN "changed-other-records" ELSE otherRecords
    /\ UNCHANGED <<key, source, rejection>>
RejectChangedOwnership ==
    /\ phase = "prepared" /\ ~OwnershipAllowed
    /\ phase' = "rejected" /\ rejection' = "ownership"
    /\ storedKeyBytes' = IF WriteOnRejectionFault THEN KeyBytes(key) ELSE storedKeyBytes
    /\ writes' = IF WriteOnRejectionFault THEN 1 ELSE writes
    /\ UNCHANGED <<key, source, counter, serialCounter, slot, allocatedKey,
                    effects, otherRecords, appliedFromCounters, appliedFromSlot>>

Next == Prepare \/ Apply \/ RejectChangedOwnership \/ ChangeSource \/
        ChangeKeyCounter \/ ChangeSerialCounter \/
        (\E next \in {"foreign-free", "occupied"} : ChangeSlot(next))
Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ phase \in Phases /\ key \in BoundaryKeys
    /\ source \in {"reviewed-source", "changed-source"}
    /\ counter \in CounterValues /\ serialCounter \in {1, 2} /\ slot \in SlotStates
    /\ allocatedKey \in BoundaryKeys \cup {1, NoValue} /\ storedKeyBytes \in Bytes32
    /\ effects \in {"reviewed-effects", "changed-effects"}
    /\ otherRecords \in {"original-other-records", "changed-other-records"}
    /\ appliedFromCounters \in (CounterValues \X {1, 2}) \cup {NoValue}
    /\ appliedFromSlot \in SlotStates \cup {NoValue}
    /\ writes \in 0..1 /\ rejection \in {"none", "range", "ownership"}
ValidU32KeysAreAccepted == (phase = "rejected" /\ rejection = "range") => key \notin ValidKeys
ZeroKeyIsRejected == phase \in {"prepared", "applied"} => key # 0
ExhaustedKeyIsRejected == phase \in {"prepared", "applied"} => key # MaxKey
NoCounterWrap == phase = "applied" => counter # 0
AllocationUsesSourceKey == phase = "applied" => allocatedKey = key
ExactKeyBytesPreserved == phase = "applied" => storedKeyBytes = KeyBytes(key)
NextKeyIsSuccessor == phase = "applied" => counter = Successor(key)
SerialAdvancesOnce == phase = "applied" => serialCounter = 2
SourceOwnershipPreserved == phase = "applied" => source = "reviewed-source"
CounterOwnershipPreserved == phase = "applied" => appliedFromCounters = <<key, 1>>
SlotOwnershipPreserved == phase = "applied" => appliedFromSlot = "reviewed-free"
RejectedOperationNeverWrites == phase = "rejected" => writes = 0
BeforeApplyNeverWrites == phase \in {"ready", "prepared"} =>
                           /\ writes = 0 /\ storedKeyBytes = FreeBytes
EffectsAreUnchanged == effects = "reviewed-effects"
OtherRecordsAreUnchanged == otherRecords = "original-other-records"
=============================================================================
