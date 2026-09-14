# Runtime lifecycle and evidence

Repository paths below are relative to the repository root. Use this sequence
for every live phase and retain each checkpoint in the run evidence.

## 1. Establish the CE bridge

When the CE MCP bridge is unavailable or no session is connected, run:

```powershell
<python> .agents/skills/nioh3-ce-research/scripts/ensure_ce_mcp.py
```

The helper verifies the approved Cheat Engine path, loader table, and pinned
v0.2.9 loader/core hashes. It launches visible CE with
`.tools/cheat-engine-mcp/load_ce_mcp.CT` only when the approved plugin is not
connected, prints the bridge session ID and attachment state, and leaves game
attachment to the runner. Pass the printed session ID to a runner supporting
`--session-id`. Use an isolated port when port 5556 belongs to an active backend:

```powershell
<python> .agents/skills/nioh3-ce-research/scripts/ensure_ce_mcp.py --port 5566
<python> <research-runner> --port 5566 --session-id <printed-session-id> ...
```

Use `--validate-only` for a static installation check. Keep one owning CE
session for consecutive phases and verify its cleanup state before changing
sessions.

## 2. Fresh phase initialization

Treat each phase as a new transaction. After attaching, record PID, process
birth identity, executable hash/size/version, module base, and approved profile.
Run the observer's fresh-initialization handshake before loading its script. The
handshake proves the previous observer is inactive, cleanup is complete, the
debug event is running, and the global breakpoint inventory is explicitly empty
when debugging is active. It then clears only the observer-owned namespace and
phase globals. Foreign debugger state remains available for its owner.

Re-read module signatures and resolve all addresses from the current module.
Record expected and actual bytes separately. A phase requiring four hardware
registers proceeds only after the empty-inventory checkpoint is proven.

## 3. Design and arm the observer

Inspect `research/owned_breakpoint_lifecycle_ce.lua`, the selected observer
adapter, and `research/ce_main_thread_timer.lua` when bridge execution is off
the GUI thread. Define a run ID, trigger, controls, duration, hit/event limits,
output bounds, and cleanup command before requesting the trigger.

For Nioh 3 user-mode observers, start a fresh CE debugger with
`debugProcess(2)` (VEH) when no debugger is active, then verify
`debug_isDebugging()` before arming. Use another debugger interface only when a
specialist probe has already proved and documented that requirement.

Derive registers, stack operands, and object identity from the exact native
instruction and caller. Model the full CE register surface (`RAX` through
`R15`) in mocks and mask explicitly for low-8 or low-32 instruction operands.
Validate stack-slot liveness at the observation instruction and use a live key
or table lookup when an earlier alias has expired.

Arm only owned breakpoints through the lifecycle helper. After bootstrap, verify
the observer identity, active state, exact owned/global breakpoint set, and
running debugger state. Store this arm proof in `capture_metadata`.

Complete bridge, target-attachment, debugger, and cleanup readiness before
asking the owner to hold a confirmation screen. After the owner reports ready,
either arm within the bounded setup window and send a distinct `enter now`
message, or release the owner from waiting while setup is repaired. Start the
capture window only for the successful arm transaction.

Every execute callback resumes the stopped event with
`debug_continueFromBreakpoint(co_run)` on active, inactive, error, and bounds
paths. Mock tests execute every configured callback and assert continuation
counts, cleanup paths, full-width registers, and poisoned stale aliases.

## 4. Collect and preserve evidence

Use positive and negative controls that isolate the question. Preserve ordered
events, zero-event timeouts, intermediate snapshots, and final snapshots. For a
named control, read back its exact seed or stable identifier and compare the
native mission/candidate fingerprint before assigning labels.

When visual spawn counts are reconciled, retain the complete mission-record
list and the filtered candidate view. Treat field-to-visual correspondence as a
scoped correlation until independent repeat or native writer/consumer evidence
supports its semantic interpretation.

For possessed-enemy late-mask salvage, use the post-spawn snapshot collector
only with a validated same-process owner hint:

```powershell
<python> -m research.possessed_enemy_capture.capture_postspawn_snapshot \
  --pid <pid> --port <port> --session-id <session-id> \
  --source-capture <prior-late-mask.json> --output <new-postspawn.json> \
  --expected-candidate-count <count> --expected-spawn-id <spawn-id> \
  --expected-enemy-key <enemy-key> --owner-observation <observation>
```

Classify salvage as post-spawn state evidence and use a fresh pre-trigger run
for timing or causal claims.

## 5. Stop, clean, and verify

On completion, timeout, partial arm, callback error, cancellation, or
attachment change, stop the observer and clean only resources owned by that
run. Keep owned addresses until a fresh inventory proves their removal. Record
cleanup errors, pending state, timer state, and authorized byte restoration.

CE timer removal may require a GUI-thread interval. Request cleanup once,
release the bridge, reconnect to the same CE session, and capture both the
initial pending snapshot and final inventory proof. A successful cleanup proof
contains an inactive observer, `cleanup_pending=false`, an explicit empty
owned/global breakpoint inventory, and a running debugger state. A process exit
is recorded as cleanup-unverified for that process until its state is proven.

## 6. Static analysis and handoff

For broad binary analysis, prefilter raw bytes, stream bounded `.pdata`
functions, and enforce per-function byte, total decoded-byte, and output-count
limits. Record ranges and skipped limits in the audit output. Run broad scans
with the game closed unless their resource behavior is already measured.

Persist raw observations before analysis with capture time, CE/tool version,
source revision and dirty state, identities/signatures, inputs, expected and
actual outputs, mode, controls, event order, read/write classification, and
cleanup result. Hash authoritative files and keep private saves, account data,
credentials, and complete game executables out of handoffs.

Route unresolved causal analysis, reverse engineering, RNG mathematics, and
inverse-solver design through `RESEARCH_HANDOFF_WORKFLOW.md`. Build the
self-contained package in `deliverables/` with `README.md`, `TASK_FOR_PRO.md`,
`ENVIRONMENT.json`, `SHA256SUMS.txt`, `evidence/`, and `project-source/`; verify
the package and keep the final ZIP hash outside the archive. Reproduce Pro
conclusions with independent evidence before integration, and label mocked,
static, and packaged checks as bounded evidence.
