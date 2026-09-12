# Proposed product protocol: native ownership, not a cache-refresh guess

## Current implementability

**Not yet callable as a production protocol. Release BLOCK.** This patch adds
observation and evidence tools only. It does not write game memory, submit a
save, bypass title checks or alter SaveInstaller/SaveApplication success rules.

Recovered functions are observation anchors, not exported safe-call APIs. An
H+0x38 busy check, the snapshot mutex, task completion and a successful rename
are individually insufficient to acquire exclusive ownership of all future
writes. The title-state owner and invalidation acknowledgement remain unknown.

## Preferred route: append through authoritative native objects

The evidence supports preferring this route because new save snapshots are
serialized from registered game objects. However, the actual scroll-inventory
object at title must first be identified in the registry callbacks; the queued
snapshot or serializer staging cannot substitute for it.

1. **Prepare, outside target mutation.** Bind an operation to process ID plus
   creation FILETIME, observed title phase, canonical account/slot, executable
   profile and the source three-file fingerprint. Generate an immutable digest
   of the exact records to append. Create and verify automatic local backups.
   Preserve existing records and native generation-serial/index invariants.
2. **Join native owner context.** Dispatch only through the actually observed
   owner-thread scheduling path. Prove no old accepted writer can later replace
   the new generation. The snapshot-only mutex is not this proof. Admission must
   be coordinated with the native request controller and outstanding snapshots.
3. **Durable claim before ambiguous mutation.** The host records the operation's
   claim before handing off mutation. If the acknowledgement channel is lost
   during handoff, the result is unknown; absence of a host response must not be
   interpreted as absence of native execution.
4. **Append exactly once.** Use the verified native inventory/serial-index
   insertion routine on the registered object. Do not append arbitrary bytes
   to the 0x900028 snapshot or S+0xE0. No such title routine is provided yet.
5. **Request native persistence.** Through the native controller, mark state
   using the real native transition and enqueue serialization. Identify the
   resulting queued snapshot/task; task pointer reuse is not generation ID.
6. **Completion and verification.** Observe worker success and controller
   consumption on their owning threads. Bind that result to this account,
   slot, operation and appended generation. Verify the native inventory/index,
   prior-record invariance and disk group. Prove no older queued/in-flight task
   can overwrite it. Do not return committed before this acknowledgement exists.
7. **Cleanup.** Release only owned callbacks, handles, allocations and claims
   after their final use is proved. If cleanup is incomplete, ownership remains
   explicit and admission of another operation stays blocked.

## Conditional fallback: external commit + native reload

This route is acceptable only after a shared writer-ownership mechanism is
recovered. Wait/join native pending saves, acquire the same ownership used by
all native writers, commit the existing external transaction, initiate native
read/load on the real owner thread, and wait for its generation-specific
acknowledgement before releasing ownership.

`0x1349610` is a load-side application routine with broad dependencies. Its final
checksum comparison occurs after object deserialization/rebuild; calling it
alone is not a defensible reload implementation. It also does not prove removal
of older queued snapshots or old staging buffers. Both must be addressed by the
native controller before this fallback can be used.

## Required receipt identity

A separate title-save receipt is required; do not reuse loaded-character live-
add acceptance as title-save acceptance.

```text
operation_id
process_id + process_creation_filetime
executable_profile + owner_thread_observation
account + slot (verified native/disk binding)
source_disk_fingerprint_group
candidate_record_digest + expected_generation_serials
native_object_identity + request/snapshot/task association
accepted_state / completed_state / verification_state
native_generation_ack (not yet discovered)
cleanup_ownership_state
```

A generation identity need not be a native integer field. A proven exclusive
request association plus verified payload identity and proof that older queued
writes cannot run may define an adequate acknowledgement. That proof has not
yet been obtained; renaming arbitrary changing bytes a "generation" would not
supply it.

Pointers and thread IDs can be reused. Bind each association to process lifetime,
operation and native request sequence. A checksum salted from native randomness
is not a monotonic generation counter; the observer's MD5 is only investigative.

## Failure and recovery semantics

| Failure boundary | Required result and action |
|---|---|
| Before handoff, native non-acceptance positively proved | `not_committed`; retry can be admitted after cleanup |
| Claim/handoff or accepted request loses response | `unknown`; no append replay under either the same or a new ID |
| Native file failure after acceptance | Remain owned/unknown until inventory and disk reconciliation, not automatic rollback |
| Game exits during accepted work | Durable unknown receipt, verified local backups retained; do not overwrite a newer native generation |
| PID reused / attachment switches | Reject stale receipt for new process; never clean breakpoints in the replacement process |
| Late matching completion arrives | Reconcile the old operation; do not execute append again |
| Backup no longer matches original source generation | Refuse restoration; preserve current files and evidence |

A pointer match, AL=1 at file return, queue length zero, or H+0x38 clear can be
supporting evidence only. None alone authorizes `committed` or a retry.

## Release gates

All original C0-C3 requirements remain. In particular C1/C2 must retain the
append after immediate title exit and cold restart, C3 must survive native save
and reload, prior records and the three-file group must remain game-accepted,
and post-acceptance lost responses must not duplicate additions. Native thread,
account/slot, generation and cleanup bindings must be verified on this exact
PC v2.01 executable. Current static and mock results satisfy none of those live
gates. The possessed-enemy subsystem remains out of scope and frozen.
