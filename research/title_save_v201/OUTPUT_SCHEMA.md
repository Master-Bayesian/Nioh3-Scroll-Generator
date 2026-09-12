# Observer output schema and bounds

Format: UTF-8 JSONL; schema `nioh3.title-save-observer/v1`. There is one header,
zero or more events, and one final status. Export happens after stop; keep CE
alive until export. The observer does not persist the player's plaintext or
make a target-process call. A lost CE process can lose its in-memory observation
window; this is research instrumentation, not the product's durable receipt.

## Header

```text
kind = header
schema
run_id, profile
process.pid                         number
process.creation_filetime           decimal string, UTC Windows FILETIME
process.version                     "2.0.1.0"
process.size, process.sha256         attached executable disk identity
account_key                         caller-supplied disk pseudonym
slot                                selected disk slot, 0..99
account_binding                     "unverified_disk_to_native"
module_base                         hex process address, never a stable locator
```

Executable filesystem path and raw native account values are not exported by
the CE observer. Native account values are assigned session-local account_N
pseudonyms. Correlating those pseudonyms to the selected disk account is still
an experiment, not an assumption.

## Every event

```text
schema, run_id, profile, epoch
process.pid + process.creation_filetime
sequence                            strictly increasing local event order
thread_id                           CE debug-event THREADID, required
site                                one of the 28 verified observation names
rva                                 stable PC v2.01 module-relative hex address
address                             process address for this attachment only
tick_ms                             CE tick counter; not a disk timestamp
entry_return_rva                     true [RSP] return only at *_entry sites
stack_code_candidates[]             <=32 code-looking stack words; NOT unwound frames
globals.root, root_phase
       .context                     H pointer, staging pointers, raw control fields
       .task                        S pointer, worker wrapper/TID, operation, phases
       .queue_count, .queue[]        <=2 nodes; omitted count if unreadable
```

Use `sequence`, not `tick_ms`, as the authoritative local ordering. Tick values
can wrap and are not wall-clock nanoseconds. Independent file-I/O tracing and
controlled stage annotations establish cross-tool timing. The observer does
not claim all unrelated process writes are covered.

Missing/unreadable memory fields are absent/null, never replaced with a zero
that could masquerade as a quiescent queue or clear busy flag.

## Site-specific payloads

| Sites | Fields and semantics |
|---|---|
| request_entry | operation, slot, system, context, busy_at_entry; true function entry return |
| request_exit | matching entry_sequence by thread and frame, busy_at_entry, disposition; unreadable busy remains unknown |
| task_bind_entry | request operation/format/payload/size/slot/system, account_ref, task address |
| completion_consume | H from RBX before busy clear; explicitly not product acknowledgement |
| snapshot_entry/ready | destination or snapshot; optional bounded body digest/checksum |
| queue_capture_entry/pop | root argument, request selectors, current bounded queue |
| serializer_entry | task and output; optional body fingerprint only for supported character staging length |
| serializer_copyback | source/destination/length; transformed data copied into staging |
| serializer_exit | task, output, error_code from EBX |
| coordinator_exit | task from RDI, return_al including early failure |
| writer_entry | redacted directory/filename, buffer address/length, error pointer |
| writer_*_result | API outcome, handle where meaningful, error field, write byte count |
| writer_exit | common return_al and bounded path/error fields, including null/early failure |
| read_entry/exit | read task, return_al on common return |
| apply_entry/exit | payload/body fingerprint at entry, checksum equality at exit; not a safe standalone API |
| registry_dispatch | registration node, callback address/RVA, stream pointers and mode28 |
| worker_start/dispatch | global active task and thread identity; not title-thread proof |

Writer return values are meaningful only at the specified instruction boundary.
At the request routine return, volatile RAX is deliberately not treated as a
boolean acceptance result. The writer success arm repurposes RDI as -1 and ESI as 0x104 after close.
The observer retains the original byte count in a bounded, thread/frame-keyed
context and never reports that later ESI value as a handle. An unpaired common
return keeps the size unknown. The snapshot/control queue may already be empty
while a task retains a previously captured staging buffer.

## Final status

```text
kind = status
sequence, epoch
dropped
errors[]                            <=32 messages, each <=512 bytes/characters
cleanup_pending
cleanup_errors[]                    <=32 bounded reasons, including resolved retries
owned_breakpoints[]                 only this observer's addresses
active
stop_reason
continue_failed
live_acceptance = false
release = BLOCK
```

An empty Lua table serializes as `{}`. The analyzer permits it as an empty array
only for the explicit final error/breakpoint vector fields; it does not reinterpret
arbitrary objects. Empty event lists, mixed process identities, missing final
status, incomplete lines, invalid thread identity, loss, callback errors and
unresolved cleanup cannot report complete capture integrity.

## Resource bounds

| Resource | Limit |
|---|---:|
| Hardware execute breakpoints | 4; no pre-existing breakpoint may be evicted |
| Event count | Default 1024; configurable 4..4096 |
| Encoded event / total event bytes | 64 KiB / 8 MiB |
| Memory read per scalar/string/array read call | <=512 bytes |
| UTF-16 string | 256 code units; only redacted save-tail text exported |
| Code-looking stack words | 32 |
| Queue records | 2 |
| Outstanding request-entry pairs / writer frames | 32 each |
| Account / path pseudonyms | 16 / 32 |
| Errors | 32, messages bounded to 512 |
| Optional content hash | <=0x9001B0 per call, 64 MiB aggregate budget |
| Local identity query output | 2048 bytes maximum read before any breakpoint |
| Analyzer input / line / events | 12 MiB / 64 KiB / 4096 |

The unchanged lifecycle helper bounds cleanup history/retries and verifies the
remaining owned addresses against CE's breakpoint inventory. On attachment
change it does not inspect/remove the replacement process's breakpoint state;
old ownership remains unresolved. MainForm's prior process-open handler is
chained and restored only if no other component has replaced the observer's
handler in the meantime.

This bounds instrumentation growth. It does not supply native writer ownership,
exactly-once persistence, a generation counter or a completed C0-C3 experiment.
