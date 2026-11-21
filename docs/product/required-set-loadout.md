# Required-set loadout search

The [working plan](loadout-plan.md) tracks current priorities, evidence and pending acceptance.

## Beta product goal

Given the current character and owned equipment, find the legal outfit
that maximizes the score of **one player-selected attack**. The player
chooses a form, weapon type, stance where applicable, quick attack,
strong attack or a named skill variant, and at least one set tier. Every
owned weapon eligible for that type belongs to the search domain.

The primary score is the selected attack's final damage against the
stated reference opponent. Keep target and hit assumptions consistent
across outfits. Rebuild the character's attack, learned skill effects,
set bonuses and eligible buffs for each outfit. For theoretical peak
damage, apply the maximum jointly attainable state under the stated
conditions, respecting prerequisites, stacking, caps and consumption.
Show which buff assumptions the evaluator supports. A raw motion value
is a property of a damage row; an effective multiplier used to compare
outfits needs an explicit common reference and equipment-dependent effects.

Resolve a selection to the native actions and damage events that belong
to it. Count its physical and elemental damage in native order. Use a
specific charge level, hit count or resource-stop condition where it
changes the score. For a skill triggered after another attack, score the
selected skill alone unless the selection explicitly includes the lead-in.

A player-facing selection is verified when its label resolves to the
intended move and the ordinary App search returns the native-checked damage
and slot assignments over the requested domain. Report any unanswered
candidates. A single-contact fixture verifies that contact's arithmetic;
whole-move coverage also needs evidence for its damaging events. Validate
shared rules across weapon types and request a player observation only for
a remaining fact that can change the score or the winning outfit.

### Follow a search through the implementation

| Question | Production entry point | Evidence that answers it |
| --- | --- | --- |
| What did the player select? | `LoadoutOptimizer.tsx` submits the save digest, context and chosen bundle. `loadout_context::search_binding` joins them. | A WebView search selecting the intended named move and variant. |
| Which outfits compete? | `loadout::slot_allows` and `Search::build_plan_with_sets` construct the owned-item domain. | Coverage accounting and matching ranked slot assignments, including tied scores. |
| What changes when an item changes? | `source_rebuild::Program::pipeline` rebuilds candidate sources. `Engine::prepare_frame` produces native inputs. | Predict a held-out outfit from a starting role and compare its sources and stats with an independent game observation. |
| Which damage belongs to the move? | `Engine::execution_events` and `assemble_execution` bind the ordered contacts to MAIN. | Check event identity and multiplicity against the move's evidence, then compare physical and elemental results with original MAIN. |

Counterattack Deflect was a convenient way to obtain a known active status
in training. Keep its observed damage as a regression control. For the
general buff objective, track the source of each modifier, whether the
candidate can receive it under the selected assumptions, how native code
combines competing effects, and when MAIN observes the result. Several
triggers may produce the same status key. A table of definitions establishes
identities and parameters; simultaneous eligibility needs separate evidence.

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

## Shared form and action inputs

The ordinary reader binds a direct encounter by form and native packet ABI.
All compatible direct records contribute known opponent and actor-field
locations. Game observations supply the actor, and the resource identity
supplies the action controller. PC202 has multiple virtual layouts for the
same damage function. A retained missing byte stays unknown until an
executed path needs it.

The controller bank contains a validated parameter and the ordered local
damage-row references for every catalog identity. Regenerate it with
`tools/import_loadout_action_states.py`. The runtime checks it against the
existing damage-row bank. The controller also covers actions with no direct
rows. Those actions can refer to other event producers.

For beta scoring, the picker resolves the selected attack name, form,
weapon type, stance and variant to a native action. Controller button
bindings are outside the scoring contract. For a single hit, select its
damage row and declare the hit conditions. For a whole move, score the
ordered contacts, including owned projectiles and their elemental damage.
When charge or hold duration affects the result, specify the desired
outcome, such as full charge or continuous hits until Ki runs out.
The number of rows alone does not determine the number of hits.

Ordinary gear refresh rebuilds the active form's source entries and retains
the observed other form's entries. The search can project the requested
weapon class from the same form's progression. The owned slot record, its
category-dependent skills, its numerical effects and the attack descriptor
are separate inputs. A cross-type search cannot retain a weapon of the
wrong type or publish an equipped damage baseline for it.
`WeaponTypeProjection` checks these publication and ownership conditions.

## PC202 encounter verification

An acceptance read with `first_use_export --reference-target selected` records
the player counter, the membership list and the selected opponent's ordered
status objects. It samples every native weapon category filter in the same
role read. The target action's current first index and row count determine
which target descriptors it reads. The target reference can briefly be empty
while idle. On a changed read, retain the ordered log. For a known target,
`tools/capture_loadout_consecutive.py` can capture two identical read-only
sweeps for a full role replay.

`tools/materialize_observed_loadout_target.py` binds the target and role
observations to the same process, roots and source inputs and emits an
independent benchmark. The fixed asset contains sparse normalized opponent
bytes. Missing ancillary operands stay unknown; the source observations
and save stay private. A scene that uses the asset declares
`benchmark_target_sha256` in its descriptor and ships `benchmark_target.bin`.
The ordinary worker verifies the digest and the numerical runtime checks the
packet ABI and private target ranges. Changing the opponent creates a new
context identity. No opponent selection is required to replay the fixed scene.

The PC202 Samurai reader obtains the stance from the player's style object at
`+0x85c`. High, middle and low were observed as 0, 1 and 2. The stance in an
ordinary character snapshot and the action selected for scoring are separate
inputs. A replay can run a middle-stance action on a retained character.

The native packet layout and opponent determine the fixed elemental STAT
entries, owner modifiers and status records. Candidate equipment can reach
reads that the currently worn weapon never executes. The player's
materializer, category, source maps and ordinary membership list come from
the character read. The scorer preserves the executed order of contacts and
exposes physical and elemental totals separately.

The Odachi branch at `791A70` reads the candidate's held record. Its public
plan identifies that record and the compiled input domain covers words at
`+0`, `+0x14` and `+0x18`. To regenerate the kernel, apply
`tools/pc202_native_mul32.patch` to the native translator and run
`tools/extend_loadout_pc202_kernel.py` on the original group listing,
known-byte image, scene, and verified PC202 text. Compile the resulting
inputs in `generated/held-record/<group>` for both host and device. The
extension preserves the base trace weights and includes both return paths
of the item helper. The normal source prefix is recompiled for every
included weapon category whose plan binds the actor category and source.

For native acceptance, `standalone_search --audit-native-inputs` exports
the ordered candidate and contact words for each answered outfit, together
with its physical and elemental result bits. Replaying these inputs in
original MAIN tests the numerical calculation. A separate unchanged-source
WebView2 run tests the first read, search, coverage and published gear slots.
Measurements combined across weapon changes are labeled as controls.

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
mid-stance direct choices: katana quick, strong and Sword Ki, odachi quick
and strong, and dual swords quick. Water Tsubame Gaeshi adds an eighth choice.
Its action-only objective excludes the prerequisite quick attack and retains
the direct contact, zero-motion occurrence and damaging owner projectile.
The unimbued actor packet begins with elemental power 100, from original
PC202 2B42C6 / 2B444C. Each projectile has its own source and damage row and
links back to the role being scored. Ordinary direct-contact scenes can project owned weapons of the selected TYPE
from a slot-one observation in the same form, with no temporary status. An
incompatible held type produces no current-outfit damage comparison. Projectile
and older fixed contexts retain their declared input requirements. Their public bundles contain
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
