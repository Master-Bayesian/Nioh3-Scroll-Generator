# PC v2.01 title-save ownership: recovered native save pipeline

## Scope, result and confidence

This is a static recovery plus an executable, read-only observation patch. It is
not a title-screen insertion implementation and it is not live acceptance.
Publication remains **BLOCK**. No game process, real save file or account was
opened during this work. The supplied package is the sole source of native-game
facts; old v2.00.02 addresses were only search leads.

The important new result is a concrete separation of **registered game objects,
queued snapshots, mutable staging buffers, asynchronous file tasks and
completion consumption**. These are not interchangeable representations of one
save generation. In particular, an empty snapshot queue is not a writer lease,
and the serializer input is not an authoritative inventory object.

Evidence levels used below:

- **Confirmed static**: instructions, direct/indirect call bindings, imports,
  constant sizes and field accesses present in the supplied v2.01 sections.
- **Strong inference**: architectural interpretation supported by that flow,
  but not yet associated with a particular C0-C3 runtime phase.
- **Unknown**: no current native capture establishes the required identity or
  synchronization property. Unknowns are not implemented as guessed calls.

## 1. Findings, ordered by confidence and product impact

### F1 — Confirmed static, P0: snapshot dequeue precedes asynchronous I/O

The wrapper `0x5B62D8` calls the snapshot capture/coalescer `0x5B760C`, then the
submission wrapper `0x5B6BE0`. The coalescer handles operation codes 0 and 2,
searches the root's pointer vector by `Q+0x08` (system/character selector), and
has a maximum of two queued nodes. It allocates a `0x40` node and either a
`0x900028` character snapshot or a `0x39820` system snapshot.

For character work, `0x5B7787` calls `0x5B89DC` to fill the snapshot. Later,
`0x5B7518` copies the first queued snapshot into `H[0]` through `0x5B7964` and
calls `0x5B77CC` at `0x5B7587`. The latter frees the snapshot, frees its node,
shifts the remaining queue entries, and decrements the end pointer. Only after
that does `0x5B6F64` submit the staged request through `0x5B7050`.

Consequences:

1. A queue count of zero can coexist with an active task retaining old bytes.
2. Waiting for the queue to drain cannot prove stale exit-time writes are gone.
3. Coalescing is by system/character kind within this root, not an observed
   application operation ID or save-generation token.
4. The actual C1/C2 loss/corruption incident is not replayed by this static
   evidence; it establishes a concrete place where older bytes can outlive the
   queue, not the exact historical corruption bytes.

Evidence: `disassembly/queue_capture_5B760C.asm`,
`stage_copy_5B7518.asm`, `queue_pop_5B77CC.asm`; request-builder excerpt also
included in `disassembly/request_build_5B6F64.asm`.

### F2 — Confirmed static, P0: serializer staging is mutated during packaging

`0x5B7E7C..0x5B8518` accepts task `S` and an output buffer. It constructs a
`0x158` header, selects RNNUSR/RNNSYS paths using `S+0xE27`, and transforms the
payload. At `0x5B8093` it copies a transformed temporary buffer back into
`[S+0xE0]`. Subsequent work uses that staging buffer again. It is not a durable
plain inventory into which an arbitrary record can safely be appended.

At `0x5B84CE..0x5B84E5`, an error condition clears the **output** buffer; this is
not an unconditional wipe of the source inventory. `0x5B84EA` is the common
error-code return boundary and preserves `RSI=S`, `RDI=output`.

The character snapshot path has three distinguishable sizes:

| Representation | Size / relationship |
|---|---|
| Queued character snapshot | `0x900028` |
| Character staging request | `0x900058`, snapshot body staged after a `0x30` prefix |
| Packaged character image | `0x900058 + 0x158 = 0x9001B0` |

Do not transfer an offset from one of these representations to another. A
serializer plaintext pointer proves a serialization source for that moment;
it does not prove object ownership, valid insertion helpers, serial-index
maintenance or title-state lifetime.

### F3 — Confirmed static, P0: the visible mutex has a limited scope

Storage at `base+0x4B56780` is passed to the lock wrapper in `0x5B89DC` and
`0x5B7964`, and released through `0xB613F4`. The first critical section captures
objects into a snapshot; the second copies the queued snapshot into staging.
Neither spans the entire asynchronous file operation.

`H+0x38` is a request busy gate, not a proven cross-thread mutex.
`S+0xE8F8` and `S+0xE8FC` are separate controller/worker phase words, not
save-generation counters. Writing any of these directly would bypass native
state transitions. No observer code writes them.

### F4 — Confirmed static, P0: file completion and controller consumption differ

The save task is bound at `0x5B7110`. The worker-construction path at
`0x5B72C0` uses the literal `[App]SaveLoadThreadMain`, creates the worker through
`0x5B7364`, and supplies thunk `0x5B65F0`. The thread trampoline at `0x5B65A0`
waits on its event and calls its stored function at `0x5B65CD`.

`0x5B65F0` obtains the active task and dispatches its vtable `+0x30` target. For
the write-task vtable this is `0x5B6610`. The coordinator and temp-file writer
run down this worker path. The controller separately polls through
`0x9204F0 -> 0x1B2B6C`; at `0x1B2D60` it clears `H+0x38` and proceeds to task
finalization/join through `0x1B2ABC`.

The object at the completion observation site is in **RBX**, not RDI. RDI is an
operation code in that function. The observer uses RBX.

The worker family is statically identified. Actual requesting and completion
thread IDs, and whether a particular title UI phase owns either one, remain
unknown. A completion-poll observation is not a recovered callback carrying
account/slot/generation/application-operation identity.

### F5 — Confirmed static, P0: a reload/apply path exists, but is not a safe refresh API

The read task uses storage `0x47156D0`, a separate vtable, and dispatcher
`0x1FE3264`. The downstream read/decode routine is
`0x1FE112C..0x1FE1A80`, with `0x1FD6408` as a file-read helper. Common read
return `0x1FE1A4B` is distinct from `0x1FE11A2`, which is a retry-loop header.

A successful character-load handling branch reaches
`0x2979F74 -> 0x1349610`. At `0x1349610` a character payload is checksummed,
then `0x2188A7C` walks deserialization registrations. Several dependent managers
are rebuilt. Only at `0x134969D` does this routine compare the computed and
stored checksum and return equality in AL.

This function has broad mutation and initialization prerequisites. It is not
safe to call merely because its name could be described as "apply". In
particular, the final equality result is not a pre-mutation validation barrier.
Queue cancellation, title prerequisites, caller-thread ownership and the
meaning of all rebuild helpers must be established before considering it for
an external-commit/native-reload protocol.

### F6 — Confirmed static mechanism; title ownership unknown, P0

`0x5B89DC` captures character state under the mutex by calling `0x5B8ACC`.
`0x5B8ACC` sets stream mode `+0x28=1`, calls `0x9DB730`, then invokes a separate
worker serialization path through `0x1080EF0`. The inverse stream setup at
`0x2188A7C` uses mode zero and also calls `0x9DB730`.

`0x9DB730` traverses the registration list headed by `base+0x4BCCBC0`, follows
`node+0x08`, and calls `[node.vtable+0x08]` at `0x9DB752`. The observer captures
the actual registration node, callback RVA and stream mode. That is a justified
next observation point for finding which registered object handles the scroll
inventory; it is not yet the inventory producer itself.

The snapshot code also clears a field at `[[base+0x474D810]]+0x69E4`. Its name
and significance are unknown. It is not labelled a dirty flag or generation.

No single flat title-owned inventory, title-state predicate, queue generation
identifier or cache-invalidation acknowledgement has been established. The
queue root and staging buffers are real, but may not be the objects retained
in every cold-title or returned-title scenario.

### F7 — Reproducible application tooling defect, fixed

The supplied `tools/capture_title_save_lifecycle.py` counted files matching
`[0-9][0-9][0-9]_*.json` under the run root. In reality, each JSON lived under a
numbered stage directory. The next-stage number therefore repeatedly reset to
one. This damaged chronological association of otherwise useful evidence.

The patch reserves monotonically numbered directories via exclusive `mkdir`,
including gaps and incomplete previous stages, rejects traversal labels, and
caps a run at 999 stages. Concurrent collectors cannot reserve the same stage
number. Process fingerprints now use decimal-string UTC FILETIME, matching the
CE observer, rather than an incompatible locale-dependent time format.

This change repairs evidence collection only; it is not a native save lock.

## 2. Current-version recovery and call graph

All ranges are half-open and belong to the supplied current `.text` image.

| Stale v2.00.02 lead | Recovered PC v2.01 function/range | Independent current evidence |
|---|---|---|
| `0x61D044` | `0x5B6610..0x5B66C3` | Write-task vtable, phase switch, call to wrapper |
| `0x61D600` | `0x5B6BCC..0x5B6BDF` | Direct wrapper to coordinator |
| `0x61D6CC` | `0x5B6C98..0x5B6F0E` | Buffer size, serializer call, temp writer call |
| `0x61E514` | `0x5B7AE4..0x5B7CEC` | `%s.tmp` reference and resolved file API imports |
| `0x61E8AC` | `0x5B7E7C..0x5B8518` | Header/transform flow and known payload sizes |

There is no uniform relocation delta. Cold split fragments were followed as
branches, not promoted to new standalone APIs.

```text
[title/exit-specific initiating caller and thread: UNKNOWN]
  -> known request wrappers (5B62D8 and its verified callers)
     -> 5B760C: capture/coalesce queue node Q
        -> character 5B89DC -> 5B8ACC -> registry 9DB730
                                      -> worker serializer 1080EF0
        -> system 297A0EC
     -> 5B6BE0 -> 5B6F10: H+38 busy gate
        -> 5B6F64: build request
           -> 5B7518 -> 5B7964 / 2979774: copy Q into H staging
                    -> 5B77CC: free/pop Q
           -> 5B7050: select task; reject if another active task
              -> virtual binder 5B7110 -> phase setup
              -> worker creation 5B7210 -> 5B72C0 -> 5B7364

worker trampoline 5B65A0 -> 5B65F0
  -> write task virtual +30 = 5B6610
     -> 5B6BCC -> 5B6C98
        -> 5B7E7C: package/transform, including staging copy-back
        -> 5B7AE4: .tmp create/write/flush/close/rename
     -> worker phase result

controller 9204F0 -> 1B2B6C -> 1B2D60 (H+38 clear)
  -> 1B2ABC: task finalization/join, active-task cleanup
  -> queued work may subsequently start

load side: task binder -> read task 1FE3264
  -> 1FE10A8 -> 1FE112C -> 1FD6408 (read/decode)
  -> completion branch 2979F74
     -> system 2979E88
     -> character 1349610 -> 2188A7C -> registry 9DB730 (mode 0)
        -> manager rebuilds -> final checksum equality
```

Specific initiating call sites to `0x5B62D8` exist in the `0x1C2BFDC`,
`0x1C30CD4`, `0x220BCE8`, `0x220CB04` and `0x27263A0` regions. They are not
labelled "title exit" until a controlled capture selects the actual caller.

### Import and path anchors

The current import table, not an absolute pointer copied from the dump, binds:

| IAT RVA | Function |
|---|---|
| `0x38DE378` | CreateFileW |
| `0x38DE380` | WriteFile |
| `0x38DE388` | FlushFileBuffers |
| `0x38DE390` | CloseHandle |
| `0x38DE398` | GetLastError |
| `0x38DE3A0` | MoveFileExW |
| `0x38DE3B8` | ReadFile |

The UTF-16 `%s.tmp` at `0x3A964D0` is referenced by `0x5B7B40`. The writer's
post-call observation points are open `0x5B7B6A`, write `0x5B7BBF`, flush
`0x5B7BDD`, close `0x5B7BFF` and rename `0x5B7CCD`. Rename uses numeric flags 9.
After successful close the writer reuses RDI=-1 and ESI=0x104 for path formatting.
The observer preserves entry/open sizes by thread/frame and does not treat those
later register values as size/handle. The common early/success return is `0x5B7B7D`; the distant success arm jumps back
to that epilogue. `0x5B6EDE` similarly covers the coordinator's early failures.

These observations prove a per-file operation. They do not establish atomic
commit of character main, game backup and system saves as one accepted group.

## 3. Data-flow layouts recovered from actual accesses

Pointers below are 64-bit unless specified otherwise. R, H, Q and S are local
research names, not recovered native type names.

### Root R = [base+0x45C4448], context H = [R]

| Offset | Width | Evidence-backed interpretation |
|---|---:|---|
| R+0 | 8 | Operation-context pointer H |
| R+0x10 | 4 | Status word; full title semantics unresolved |
| R+0x18/+0x20/+0x28 | 8 each | Snapshot-node vector begin/end/capacity |
| H+0 / H+8 | 8 each | Character/system staging buffer |
| H+0x18 | 4 | Result field read by completion consumer |
| H+0x28 | 4 | Slot-related field used by request builder |
| H+0x34 | 4 | Request option, not a generation |
| H+0x38 | 1 | Request busy gate set before submission, cleared at consumption |
| H+0x39 | 1 | System selector |
| H+0x3A..0x42 | bytes | Result/control flags; observer retains raw offsets |
| H+0x48..0x6F | 0x28 | Copied request metadata |

### Snapshot node Q, allocation size 0x40

| Offset | Width | Meaning |
|---|---:|---|
| +0 | 4 | Operation code |
| +4 | 4 | Slot |
| +8 | 1 | System selector / coalescing key within R |
| +0x10..0x37 | 0x28 | Metadata from native helper |
| +0x38 | 8 | Snapshot pointer |

### Active task S = [base+0x45C2E80]

Write-task storage is `base+0x4706DC0`, read-task storage is `base+0x47156D0`.
The write vtable at `0x3A96F00` binds +0x20 to `0x5B7110`, +0x28 to `0x1B2F34`,
and +0x30 to `0x5B6610`. Its creation route uses `0x5B7210`/`0x5B72C0`.

| Offset | Width | Meaning |
|---|---:|---|
| +0 | 8 | Vtable |
| +8 | 8 | Worker wrapper T |
| +0x10 | object | Controller/worker synchronization object |
| +0xB0 | 8 | Account value copied from binder RDX; pseudonymized in observer |
| +0xB8 | 4 | Task error/result |
| +0xC0 | 8 | Filename pointer |
| +0xC8 | 8 | Metadata pointer |
| +0xD0 | 4 | Operation code |
| +0xD8 | 8 | Format, copied from request u32 |
| +0xE0/+0xE8 | 8 each | Mutable staging pointer / length |
| +0xF0..0x117 | 0x28 | Copied metadata |
| +0xDDC | UTF-16 buffer | Account-relative slot path component |
| +0xE1C | 4 | Slot |
| +0xE24..0xE27 | bytes | Options / system selector; not dirty/generation flags |
| +0xE8F8/+0xE8FC | 4 each | Controller and worker phase words |

Worker wrapper T has observed thread ID at +0x10, OS thread handle at +0x18,
event at +0x20, supplied parameter at +0x28 and function pointer at +0x30.

### Registry and stream

Head `[base+0x4BCCBC0]` leads to nodes with vtable at +0 and next pointer at +8.
The walker calls virtual +8. A bounded observer records the callback RVA and
stream +0/+8 pointers and +0x28 mode. It deliberately does not serialize raw
node objects or the player's plaintext body into its output.

## 4. Stable signatures and proof boundaries

`locators.json` contains 28 observation-site definitions: exact breakpoint RVA,
independent signature-start RVA, expected bytes, a byte mask, textual AOB,
`.pdata` interval and supporting decoded instructions. `locators.lua` is the
runtime form. Both are compared by an executable regression test.

Every pattern uniquely matches the current supplied `.text`; that is relocation
support, not semantic proof. Semantic claims above come from actual control and
data flow. Rel32 calls/jumps and most RIP-relative operand displacements are
masked; the registry-head displacement is intentionally retained to distinguish
otherwise identical registry walkers. Short epilogues use an earlier signature
window with an explicit offset to the breakpoint instruction.

Only the supplied disk executable SHA-256 and size are accepted. The `.rdata`
dump is an already loaded image with relocated pointers/IAT entries; its whole
runtime hash is not assumed to remain identical under different ASLR bases.
Section hashes identify the static evidence. At runtime the observer checks the
actual attached process executable's disk hash/version/size and every local
masked signature, before any breakpoint is armed.

## 5. What remains unknown and how the observer resolves it

1. **Title object:** capture registry callbacks during C0/C2 snapshot collection
   and C3 load. Inspect the callback handling inventory records, and prove its
   lifetime/availability at cold title. Flat queued/staging buffers alone are
   insufficient.
2. **Owner thread:** correlate request, registry, worker and completion thread
   IDs. Code-looking stack words are explicitly not an unwound stack. Use the
   direct entry return and independent Windows File I/O stacks for caller proof.
3. **Native generation/dirty protocol:** no versioned token or dirty-marking API
   is established. Compare object callbacks, queue requests and completion in
   one process/account/slot; do not name changing bytes by correlation alone.
4. **Cancellation/reload:** trace read request initiation and queued-write
   interaction before any attempt to call `0x1349610`. The function itself does
   not prove a safe title reload.
5. **Three-file consistency:** correlate native paths with the local fingerprint
   collector. No single rename is a group-generation acknowledgement.
6. **True title-exit writer:** the static pipeline is concrete, but a controlled
   run must establish which branches actually execute on direct exit. A missed
   breakpoint is not evidence that the process has no stale generation.

The smallest next observation is C0, not an invented runtime save call. See
`OBSERVER_README.md`, `NATIVE_PROTOCOL.md` and `OUTPUT_SCHEMA.md`. C1-C3 are
controlled research runs with automatic backups and local evidence, not release
acceptance until the recovered ownership protocol itself is implemented.
