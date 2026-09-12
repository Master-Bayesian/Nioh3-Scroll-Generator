# Title-screen save ownership research plan

> The [2026-09-12 approach reset](TITLE_SAVE_APPROACH_RESET_20260912.md)
> supersedes this document's mandatory C0 writer-hit prerequisite and selected
> implementation route. The original plan below is retained as research history.
> Direct-exit/cold-start safety and the unresolved concurrency concern remain.

## Product requirement

The application must be able to append one or more generated scrolls while
Nioh 3 is running at the title screen. The operation must remain safe when the
player exits immediately without loading a character, and the appended scrolls
must survive the next cold start.

The current external transaction already provides automatic backup, atomic
replacement, encrypted readback, structural validation and related-file
generation checks. Those checks do not control the game's older in-memory save
generation. A later native flush can still replace the accepted disk image.

Publication remains blocked until the application joins the game's native save
lifecycle and receives an acknowledgement bound to the same process instance,
account, slot and save generation.

## Questions to answer

1. Which native object owns the character save while the title screen is
   active?
2. Which queue, lock, dirty flag and generation fields govern title-screen
   serialization and exit-time writes?
3. Which native call loads or invalidates the cached generation?
4. Can the application append to the game-owned plaintext object and request a
   native save, or must it commit externally and then request a native reload?
5. What acknowledgement proves that the new generation has been accepted and
   that an older queued write cannot replace it?

## Evidence already available

- PC v2.01 executable identity and raw `.text`, `.rdata` and `.pdata` section
  dumps are stored under
  `audit/runtime_sections/v2.0.1.0_20260902_title/`.
- PC v2.00.02 runtime evidence found the complete temporary-file write at game
  RVA `0x61E5EF` and a serialization transform at `0x61E8AC..0x61EF48`.
  These addresses are stale leads, not v2.01 signatures.
- The canonical plaintext scroll record layout has already been correlated with
  the native serializer input.
- The current application has a tested protected-operation model for process
  creation identity, durable receipts, no replay after uncertain acceptance and
  native inventory verification. The title protocol should reuse those
  contracts without treating the loaded-character insertion hook as a save API.

## Phase 1: static recovery

Use the v2.01 section dumps to recover the current equivalents of the old save
dispatcher, save wrapper, buffer/write coordinator, temporary-file writer and
serialization transform. Start from imported file APIs, `.tmp` path references,
the old function shapes and unwind boundaries. Return stable RVAs, byte
signatures with relocation masks, call graphs and every referenced global or
manager field.

Static output must identify a small set of read-only runtime observation points.
It must not claim that any candidate field is a lock, generation or dirty flag
without dynamic evidence.

## Phase 2: controlled runtime observation

Prepare and inspect the observer before asking the user to trigger a save. The
first pass uses Windows File I/O tracing to record write chronology and native
stacks. A Cheat Engine observer then records only the narrowed v2.01 game RVAs.
It uses `research/owned_breakpoint_lifecycle_ce.lua`, removes only its own
breakpoints and verifies cleanup.

For every native save event, capture:

- process ID and process creation time;
- thread ID and monotonic event order;
- target path, handle, byte count and temporary-file rename sequence;
- direct and game-module callers as stable RVAs;
- serializer input object, plaintext pointer and byte count;
- save-manager pointer and all candidate queue, lock, dirty and generation
  fields before and after the call;
- return value, completion callback and the thread on which completion runs.

Private save bytes remain local. The research package records hashes, sizes,
timestamps and sanitized structure summaries only.

## Controlled run matrix

Each run starts from a cold game process and uses a new run ID. Record hashes,
sizes and nanosecond modification times for the main character save,
`BACKUP.BIN` and the system save at every stage.

| Run | Sequence | Purpose |
| --- | --- | --- |
| C0 | Start to title, wait, exit, start again | Baseline title and exit writes without application mutation |
| C1 | Start to title, append one known test scroll, do not load a character, exit, start again | Required failure reproduction |
| C2 | Load the character, return to title, append, do not load again, exit, start again | Detect ownership differences after a character was loaded |
| C3 | Start to title, append, load the character, save normally, return to title, exit, start again | Positive control for a generation accepted by the game |

The test scroll must have a unique operation ID and generation serial. Before
each mutation the application creates and verifies the normal three-file
automatic backup. Do not restore during an uncertain native write; preserve the
entire local evidence directory for analysis.

## Phase 3: native protocol

Implement the smallest protocol supported by the recovered control flow.

Preferred route:

1. Dispatch to the game's owning thread.
2. Acquire or join the native save queue for the selected account and slot.
3. Append the canonical `0xE8` record to the game-owned plaintext inventory.
4. Mark the real dirty/generation state through the same native routine used by
   the game.
5. Request the native save.
6. Wait for the native completion callback and verify disk plus native
   inventory.

Fallback route, only if native object mutation cannot be made safe:

1. Wait for native save quiescence and acquire the same writer ownership.
2. Commit the existing external transaction.
3. Invoke the game's native reload or cache-invalidation path on its owning
   thread.
4. Wait for acknowledgement of the exact committed generation.
5. Verify that no older save request remains queued.

Do not call a game function from an arbitrary remote thread. Do not substitute
sleeping, process suspension, repeated overwrite, an exit hook or a requirement
that the player loads the character.

## Application boundary

Expose a dedicated title-save native adapter. It must not share the current
loaded-character insertion adapter's acceptance receipt.

The protected operation is:

```text
prepare
  -> prove title state, process instance, account, slot and source generation
  -> create and verify the automatic related-file backup
execute
  -> acquire native ownership
  -> append exactly once
  -> request native save or native reload
verify
  -> match native acknowledgement to operation and generation
  -> verify disk generation and native scroll inventory
release
  -> release resources only after cleanup is proved
```

Any disconnect after native acceptance returns `unknown`. The operation remains
the owner and is never replayed under a new ID. A later receipt may reconcile
the result, but it must not cause another append.

## Acceptance gates

1. C1 and C2 both retain the append after direct exit and the next cold start.
2. C3 retains the append after a normal game save and reload.
3. Existing scroll records and all three game-managed save generations remain
   intact.
4. Rejection before acceptance is retryable; lost response after acceptance is
   not replayed.
5. Game exit during execute produces a durable `unknown` result with a verified
   backup and no automatic rollback over a newer game generation.
6. Process replacement, PID reuse and stale receipts cannot attach an operation
   to a new game instance.
7. Every owned breakpoint, allocation, handle and callback is cleaned up or
   remains explicitly owned by an unresolved operation.

Only matching PC v2.01 live evidence can satisfy these gates. Static analysis,
synthetic save tests and packaged startup remain bounded supporting evidence.
