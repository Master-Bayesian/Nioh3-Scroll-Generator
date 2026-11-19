# Required-set loadout search

The local 0.8.13 integration candidate adds **Loadout Optimizer** to the
desktop navigation. The player chooses a weapon type, an installed attack
scene and at least one set tier. Several rules can be combined. The
selected save remains the owner of subsequent reads and previews. The
first page read joins the existing save discovery before starting its
own protected reads.

The verified exact path scores owned records at fixed attributes in the
bound native scene. Its set feasibility bounds use qualified equipment
lines, retained gear, the piece-reduction effect and the actual displaced
weapon for a spare-slot swap. Every remaining outfit passes the same
native set predicate before scoring. An empty feasible search and an
unanswered search have distinct outcomes. The JSON result exposes the
raw domain, proven set exclusions and completed checks, as well as the
score and the exact slot assignment for every shown row.

The protected worker loads attack bundles from its data root. The
desktop renderer sends a save digest and a context identity. Equipment
bytes are read from the selected save in the protected worker. A native replay and a live process read
have separate source labels. Preparation of a wear plan uses the same
save identity and the current reviewed save snapshot. A spare weapon move displaces
the previous weapon to the slot that was used during scoring. The write
action still follows the ordinary reviewed plan, confirmation and backup
workflow.

## Ownership and formal controls

- `LoadoutDiscovery` maps request IDs to the page discovery reducer.
  A stale response cannot replace the current page state. A failed
  discovery exposes the retry action.
- `LoadoutSearchControl` maps submission to the save operation
  controller, cancellation to the protected job, and the control
  boundary to `ExactSweep::advance`. An accepted cancellation is
  checked between batches. The search and device allocations leave
  scope before the protected action publishes its terminal state.
  The result publication lock retires a late read-only success.
- `LoadoutPreparedWear` maps preview ownership to the current result,
  the selected save snapshot and the service plan ID. Cancelling and
  changing the inputs retire only the reviewed plan.
- `StandaloneExactSweep` maps planned and excluded assignments to the
  frozen set-bound census and the persisted standalone sweep. The app
  reuses that planner and score loop inside an ordinary protected job.

## Verification scope

Run the native harness with a PC202 fixture that preserves the original
save and its compiled scene identity. The harness launches the actual
Windows WebView2 host and protected worker with the replay source.
It compares the published ranks against the same-domain standalone
result and has separate checks for cancellation, preview and equipment
assignment. The package pipeline and actual live character refresh for
this new worktree are separate acceptance stages.

[Native verification harness](../../apps/tauri/verify-required-set-loadout.mjs)
