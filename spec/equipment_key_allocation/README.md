# Equipment inventory key allocation

This is a finite safety model of the save equipment allocator. It checks the key-width correction and preserves the surrounding ownership/frame contract. It does not read a save, capture a game, establish a repair policy, or claim anything about native u32 overflow behavior.

## Domain and explicit states

The requested key domain is exactly `{0, 65534, 65535, 65536, 255761, MAX-1, MAX}`, where `MAX = 0xFFFFFFFF`. Every valid representative is accepted without an inherited 16-bit cap; zero and `MAX` reject. `MAX-1` is allowed and leaves the counter at `MAX`. A subsequent allocation at that exhausted value must reject, without wrap or renumbering.

`phase` explicitly names `ready`, `prepared`, `applied`, and `rejected`; `rejection` records a range or ownership refusal. `applied` means the accepted in-memory save transform, not a disk commit or a live-game insertion. There is one requested allocation per behavior. Source drift, either counter changing, a different empty-slot snapshot, and an occupied slot can occur after preparation. The two counters must still equal the prepared key and serial; the target must still be the exact reviewed empty slot.

TLC's integer arithmetic is signed, so `BeforeMaxKey` and `MaxKey` are explicit model values for the two upper u32 boundaries. Their exact little-endian bytes are respectively `FE FF FF FF` and `FF FF FF FF`; the allowed upper successor is explicitly `BeforeMaxKey -> MaxKey`. All smaller values use arithmetic byte extraction. No overflowing arithmetic is performed, and there is no normal successor for zero or `MAX`. Additional values such as 65537 and 255762 occur only as successful counters, not as extra requested inputs.

`effects` is the already-reviewed new record's effect payload, which allocation must copy unchanged. `otherRecords` is an opaque aggregate of every other source record; it must not change. The model does not require the formerly empty target's effects to stay empty. `appliedFromCounters`, `appliedFromSlot`, and `writes` are safety witnesses, not implementation state machines. The serial is bounded to one valid example plus a drift value; serial-width/exhaustion behavior, multiple-add chaining, held capacity, encryption and crash recovery remain outside this component model. These selected boundary representatives do not enumerate all 2^32 possible key values.

## Implementation mapping

| Contract | Existing implementation |
| --- | --- |
| Full u32 key, independent of held count | [`character.rs`](../../crates/nioh3-save/src/character.rs): `equipment_counters` reads four bytes at `NEXT_INVENTORY_KEY_OFFSET`; `equipment_fields` reads `inventory_key` from `+0x1C` as u32. The allocator uses the saved counter, not a renumbered record index. |
| Valid boundary keys accepted; zero and exhaustion reject | `apply_character_edits`, `CharacterEdit::AddEquipment`: the corrected guard is `key == 0 || key == u32::MAX`. It no longer imports the scroll allocator's u16 bound. `Prepare`, `ValidU32KeysAreAccepted`, `ZeroKeyIsRejected`, and `ExhaustedKeyIsRejected` model that branch. |
| Exact four-byte key and next counter | `build_generated_equipment_record` copies `key.to_le_bytes()` into `[0x1C..0x20]`. `AddEquipment` validates the record's stored key and serial, copies the prepared record, and stores `(key + 1).to_le_bytes()` at the next-key offset. `AllocationUsesSourceKey`, `ExactKeyBytesPreserved`, `NextKeyIsSuccessor`, and `NoCounterWrap` express those bounds. |
| Source, counters, and empty-slot ownership unchanged | [`save_app.rs`](../../crates/nioh3-protected/src/save_app.rs) binds the plan to its source hash and rejects a changed source before writing. `AddEquipment` rejects an occupied target, unequal `expected_free` bytes, or unequal key/serial counters. `SourceOwnershipPreserved`, `CounterOwnershipPreserved`, `SlotOwnershipPreserved`, and `RejectedOperationNeverWrites` preserve those checks. A range fix does not authorize retry against changed ownership. |
| No effect or unrelated-record rewrite | `AddEquipment` installs the already-built record in its reviewed slot and advances only the two counters. `EffectsAreUnchanged` preserves the reviewed payload; `OtherRecordsAreUnchanged` is the outside-record frame. The existing `new_items_go_after_the_occupied_tail_and_take_consecutive_counters` regression checks the byte frame and high-counter cases separately. |
| Existing native width evidence | [`v0.8.7 engineering record`](../../docs/product/releases/v0.8.7.md), evidence section, records historical 65533..65536 additions and four-byte saved keys. [`runtime equipment_add.rs`](../../crates/nioh3-runtime/src/mutation/equipment_add.rs), `read_state`, reads a u32 acquisition counter and documents the existing `0x54C380` setter without a 16-bit cap. These are reused width evidence, not a new live capture. This save correction still rejects zero/overflow and does not infer the native setter's behavior at u32 exhaustion. |

The supplied high-counter regression value `255761` is represented directly. The model neither opens the reported save/copy nor treats its record count as an allocation limit. The root task owns the matching high-counter host E2E and any later copy-only rehearsal.

The root task reports the matching high-counter host save/reload E2E passed in 197.98 seconds at `D:/Nioh3_v080_deliverables/deliverables/hell-weapon-seed-search-20261116/host-e2e/run-43896-1794865681632416300`. Its separate unit boundary checks accept `0xFFFF`, `0x10000`, `255761`, and `MAX-1`, reject zero/`MAX`, and preserve an existing record. That native-host evidence is separate from this component model and was not rerun or used to access a save here.

## Deliberate fault controls

Each adjacent negative configuration changes exactly one normally-false constant. The existing runner requires TLC exit **12** and the exact declared invariant error; parser errors and unrelated invariant failures do not count.

| Named control | Required invariant |
| --- | --- |
| `inherit-scroll-u16-cap` | `ValidU32KeysAreAccepted` |
| `accept-zero-key` | `ZeroKeyIsRejected` |
| `accept-exhausted-u32-key` | `ExhaustedKeyIsRejected` |
| `wrap-last-allowed-successor` | `NoCounterWrap` |
| `skip-counter-advance` | `NextKeyIsSuccessor` |
| `truncate-key-to-u16` | `ExactKeyBytesPreserved` |
| `renumber-high-key` | `AllocationUsesSourceKey` |
| `ignore-source-ownership` | `SourceOwnershipPreserved` |
| `ignore-counter-ownership` | `CounterOwnershipPreserved` |
| `ignore-slot-ownership` | `SlotOwnershipPreserved` |
| `rewrite-effects` | `EffectsAreUnchanged` |
| `rewrite-other-records` | `OtherRecordsAreUnchanged` |
| `write-after-rejection` | `RejectedOperationNeverWrites` |

## Component check

```powershell
pwsh -NoProfile -File tools/check_models.ps1 `
  -Model EquipmentKeyAllocation `
  -Java 'D:/Nioh3_v080_deliverables/tools/java/jdk-21.0.13+9-jre/bin/java.exe' `
  -Evidence 'D:/Nioh3_v080_deliverables/deliverables/hell-weapon-seed-search-20261116/key-width-model'
```

The correct model passed with **249 distinct states**, depth **7**, and zero states remaining. All **13** negative controls passed with exit 12 and their exact named invariants. This is a safety check, with no fairness/eventual-write property; terminal bounded states intentionally disable deadlock checking. The evidence directory retains the exact configurations/logs, `model-controls.json`, these four source files under `source/`, inspected implementation hashes, and a SHA-256 artifact manifest. Full repository gates and the new private package remain the root task's separate work.
