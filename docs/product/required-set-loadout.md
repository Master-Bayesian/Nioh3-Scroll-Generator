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

The PC202 damage parameter bank resolves 11,909 direct row references across
11,885 catalog actions. The lookup key includes the resource, action number,
parameter digest and row selector. An event plan separately specifies the
ordered landed contacts, projectile ownership, resource use and buff phases.
The cross-weapon candidate includes Ninja kusarigama strong and six Samurai
mid-stance choices: katana quick, strong and Sword Ki, odachi quick and
strong, and dual swords quick. These first-use scenarios require the selected
weapon type in slot one and no temporary status. Their public bundles contain
the attack layout, benchmark opponent, tables and scorers. Both source maps,
the held weapon, numerical buffers and progression are supplied by the role
reader. Per-scene current-player acceptance is recorded separately from
retained-role numerical parity.

PC202 helpers at RVAs 2E98F0 and 2E9948 also inspect the attack parameter
record and the action's damage-row list via the actor's action controller.
Each encounter installs the selected 336-byte parameter and its static
resource row order in virtual storage. A player's idle animation is not a
source of the selected skill's parameters. For the expanded kernels each
contact selects its row through the mutable packet descriptor pointer.

To compile an existing event sequence, build the loadout example
`compile_action_rows` and run `tools/compile_loadout_event_inputs.py` with the
scene directory, example executable, canonical descriptor address and a new
output directory. Initialize that descriptor from `damage_descriptor.bin`.
The generated `hits.txt` keeps writes at the addresses consumed by the
evaluator. Preserve equipment and phase inputs from the current role. An
earlier pointer-relocation experiment scored katana mid strong as 761 versus
the original 1196, despite containing the correct row bytes at the new address.
The expanded Samurai families declare this packet field writable and
recheck the native values. Qualify a new scene with its original per-contact components and the actual
device before exposing it as a ranking choice.

Run the native harness with a PC202 fixture that preserves the original
save and its compiled scene identity. The harness launches the actual
Windows WebView2 host and protected worker with the replay source.
It compares the published ranks against the same-domain standalone
result and has separate checks for cancellation, preview and equipment
assignment. The package pipeline and actual live character refresh for
this new worktree are separate acceptance stages.

[Native verification harness](../../apps/tauri/verify-required-set-loadout.mjs)


## Prepared CUDA packet reuse

The desktop uses direct CUDA submission with persistent device buffers.
The standalone runner's `--cuda-reuse` flag enables experimental full
packet reuse. Across the retained 86,160-outfit set search, that option
saved 482 submissions and added about 0.56 seconds of cache handling.
CPU read-certificate reuse stays enabled. Both GPU modes preserve the
same eligibility, damage and ranking contract.
