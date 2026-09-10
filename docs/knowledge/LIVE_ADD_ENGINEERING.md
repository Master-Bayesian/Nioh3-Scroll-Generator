# Live scroll insertion: engineering and version adaptation

## Evidence as of 2026-09-08

One native insertion completed in PC v2.01, PID 7744, on the observed
`[App]MissionThread` (43964), without returning to the title screen and without
editing a save. The added scroll has seed 10030565, a newly allocated full
uint64 instance serial 2416080, and occupied slot 15. Count changed 29 to 30.
The user confirmed the new item in the game and confirmed it remained after
normal shrine saving, returning to title, and reloading the same save.

Independent verification of the complete 400-slot container reconstructed the
post-write SHA-256 from exactly one replacement of an empty slot. All previous
29 records remained byte-identical. The native serial index gained exactly one
key and retained every previous mapping. Serial and acquisition counters each
advanced once. A copied, decrypted normal save and the reloaded memory agree
on all 30 serials and defined record fields. Viewing the added item cleared
bit 1 at record +0x18; this is the only allowed, explicitly reported difference
from the immediate insertion snapshot. Undefined padding is +0x24..0x27 and
+0xE4..0xE7; no other fields are ignored.

Private evidence: `deliverables/frontend-v2/live-add-followup/single-insertion-01/`.
`independent-verification.json` proves insertion; `persistence-02/verification.json`
proves matching save/current memory. The reload itself is user-confirmed.
`persistence-01` preserves the initial strict failure on the cleared new marker.
Do not rerun `arm-once.lua`: its operation UUID has already been dispatched.

A further no-insertion preview round-tripped the installation record through
`live_add_descriptor.assembly_descriptor` and the native builder. Every defined
generation field matched and the serial remained UINT64_MAX. This removes the
dependency on a hand-copied natural descriptor for that tested record. It does
not yet accept every candidate, rarity or edited field.

## Approach and invariants

1. Obtain a ready candidate through the existing application service. R4 matching
   and display use the finalized record; installation uses `installation_record`.
   Never install a pending-finalization candidate or rewrite RNG/finalizer rules.
2. Resolve a specifically accepted game profile, verify loaded instructions and
   inventory ownership, inspect all occupied full-uint64 serials and their native
   hash index, and retain an encrypted save backup matching the live inventory.
3. Serialize a native descriptor and perform an isolated preview with serial
   allocation disabled. Compare the resulting defined record fields against the
   candidate installation record. A disagreement rejects the operation.
4. Prepare a plan bound to process, inventory owner, full container bytes,
   next serial, first empty slot, scheduler owner and selected code identities.
   Assign a unique operation UUID and durably claim it before dispatch.
5. Dispatch once at the observed empty-pickup mission-update boundary, after
   checking the expected caller, task flags, current owner/counter/container,
   empty pickup queue, stack alignment and exclusive breakpoint ownership.
6. Call the original native builder with serial allocation enabled, then original
   insertion. Guard the builder output pointer, freshly allocated serial and
   incremented counter before invoking insertion. Never fabricate a serial or
   append only an array record: insertion also updates an internal hash index.
7. Acknowledge the original continuation, preserve registers, stack and flags,
   verify the returned slot and empty remainder, compare the full container and
   native index, then remove owned breakpoints and release the allocation.
8. Persist an independent verified receipt. A timeout after redirect remains
   uncertain and retains remote memory. It must never automatically retry.
   Cancellation before dispatch competes for the same exclusive durable claim.
   Cancellation after dispatch is not rollback. Save persistence is a distinct
   normal-game acceptance step, not an offline write performed by this feature.

## Version-owned knowledge

`nioh3_scroll_editor/live_add_profile.py` owns the live-add RVAs and structure
offsets. It accepts only PC v2.01. Existing generation profiles in `native.py`
remain independent: accepting offline generation does not accept mutation.
Keep executable identity, discovery results and accepted capabilities distinct.
Do not enable a new build merely because an AOB match was found or an RVA moved.
The CE executor consumes the generated `research/live_add_layout_ce.lua`.
Regenerate it with `tools/export_live_add_layout.py`; an automated parity check
rejects drift from the Python profile. The separate selected-callee identity
resource stores hashes, not captured game code. Product preflight requires the
original quantity getter; the historical conditional hook bypass remains only
research evidence.

The dispatch entry is RVA 0x12E6840, caller return 0x20BB2C. Its upstream mission
routine is split across unwind ranges: E62EC0 eventually tail-jumps to E64480.
There is therefore no direct E8 call cross-reference for that transition.
E5DDE0 is the thread loop and vtable slot +0x10 dispatches the mission routine.
Task waits precede the insertion boundary, including flags at scheduler
+0x1408/+0x1409/+0x140B; +0x1629 records the observed ready phase. A second
serial writer was observed on `KIDSTaskScheduler0` (9248). The checked phase
and successful trial do not prove exclusion of every scheduler job in all states.

The index uses FNV-1a over all eight serial bytes, with linked nodes and reverse
bucket traversal. Occupied records must resolve to their actual slot. Unused
historical index keys can remain (31 keys for 29 records before this experiment).
Duplicate full keys are invalid; index size is not inventory count.

One externally modified quantity getter at 0x2FA636 is bypassed for the verified
nonstackable scroll path: bit 23 at record +0x18 makes 0x2FA624 return one before
the patched branch. This is a bounded exception for this scroll path, not an
allowlist for arbitrary patched functions, equipment, consumables or new builds.
The selected callee preflight checked 60 ranges. It is not a complete call graph.

## Updating for a new game build

1. Capture executable version/identity and clean runtime sections. Preserve the
   old accepted profile and evidence. Work in a candidate research profile.
2. Locate known functions by surrounding semantics and decoded instruction
   boundaries. Raw E8/RIP byte searches are candidates, not proof. Decode complete
   containing unwind ranges and record unresolved/truncated ranges explicitly.
3. Trace manager ownership, capacities, record layout, counters, index hashing,
   insertion return/remainder ABI, serial allocation and mission/task scheduling.
   Recheck relevant transitive callees and external modifications.
4. Run read-only inventory/index checks and bounded observers, then no-call
   dispatch, isolated builder roundtrip and register/cleanup tests.
5. Run one backed-up native insertion and independently compare the complete
   container, full serial index, counters, UI, normal save and reload.
6. Extend candidate parity across supported rarities and requested editable
   inputs; keep known failures capability-gated. Re-run fallback/lifecycle tests.
7. Promote only capabilities with matching evidence. Record exact signatures,
   ABI, offsets, discovery rationale, validation artifacts and unresolved limits.

## Current implementation boundary

The CE-backed experimental insertion and persistence are accepted for the single
tested sample. The integrated code now connects input serialization, preview,
backups, durable claims, the bounded local named pipe, protected-worker contracts,
Electron broker/preload and `LiveAddSession`. Read-only inspection and evidence
verification are application modules; research CLIs reuse them. Plans also bind
the Windows process creation time, preventing reuse after a PID is recycled.
Operation and receipt content have consistency digests; these are accidental
corruption checks, not signatures. Claimed operations remain uncertain across
host restart until reconciled and cannot be dispatched again.

Connection setup is explicit and optional through `configure-live-add.ps1`.
It generates a local CE bootstrap and a launcher with an ephemeral connection
token. The transport permits only ping/preview/insert/status/release/stop; it
does not interpret request text as Lua. I/O is bounded even if CE stops replying.
No arbitrary Lua, addresses or raw records are exposed to React. CE owns remote
execution independently of renderer lifetime. Unacknowledged execution retains
its allocation; process exit can make host shutdown safe without fabricating a
successful receipt. Keep CE attached until the attempt settles.

The game was closed before integration was completed. Therefore the full new
candidate-to-worker-to-pipe route, varied R3/R4/R5 candidates, shutdown during
actual native execution and all-state concurrency remain live acceptance items.
The implemented per-candidate native preview rejects unsupported reconstructions
before serial allocation. This is an optional engineering feature, not a new
stable release or a claim that every edited field is supported. Final Figma UI
can consume the existing API/session without changing this execution design.

## Preserve for a future legal equipment adder

Do not discard the natural equipment/consumable pickup captures under
`deliverables/frontend-v2/live-acceptance/20260907T225708Z`, the insertion/callee
analysis, serial-writer observations, task-wait call chain or this scroll trial.
They establish a useful research approach for legal equipment creation:

- Observe natural creation and pickup separately. Capture the source descriptor,
  constructed record, caller/thread context, destination container, returned
  remainder, counters and native index. Compare against normal saved readback.
- Reuse the documented Windows x64 insertion ABI as a hypothesis to revalidate:
  RCX manager, RDX remainder buffer, R8 source record, R9 returned slot pointer,
  and a fifth argument on the stack. It is not permission to reuse scroll layouts.
- Preserve the separation between displayed seed, global full-uint64 instance
  serial, acquisition order, per-container index and an item's generated effects.
  Never infer that copying an equipment record or assigning a new low-32-bit ID
  is equivalent to native insertion.
- Equipment requires its own legality evidence: type-specific generation inputs,
  effect pools and exclusions, star/rarity/level rules, materialization/finalization,
  category dispatch, quantities and stacking, capacity and index ownership.
  The scroll builder, descriptor offsets, 400-slot limit and quantity-hook bypass
  must not be applied to equipment without matching runtime evidence.
- Begin applicability research with the existing
  `EQUIPMENT_CATALOG_LIVE_HANDOFF_20260902.md` and preserve unresolved entries.
  UI names and spreadsheets alone do not establish native legality.
- Reuse bounded dispatch ownership, backup/plan/claim/receipt principles and
  version-capability separation. Introduce a concrete equipment application
  service only when that work starts; do not add a general remote-write or
  arbitrary-code interface to the scroll service now.

The user's stopping boundary is explicit: finish this live-add engineering
milestone, then pause for Figma. Equipment implementation and more live battles
are deferred, not silently included in the current task.
