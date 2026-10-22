# Output-driven damage research reset

Current priority update: the2026-10-16owner steering adopts the
[native candidate evaluator product milestone](NIOH3_NATIVE_EVALUATOR_PRODUCT_MILESTONE_20261016.md).
Real active-equipment contrasts and same-candidate constraints are the product
acceptance gate; single-frame MAIN replay is a calibration lane. Pro handoffs
are actively authorized again. Historical suspension/ordering statements below
describe the earlier checkpoint and do not override that decision.

The owner's concern about repeated walls is justified. Local byte/table/math
contracts are useful, but the work expanded into reconstructing substantial
runtime machinery before producing one complete gear-to-objective evaluator.
Helper counts, instruction counts, CLI counts and384/418 local pair scalar
resolution are not completion percentages. The all-affix optimizer target stays
unchanged; investigation order and acceptance criteria change here.

The audit root is
`D:/Nioh3_v080_deliverables/deliverables/damage-pro-return-audit-20261015-r9`.
Supporting assessments are under `method-audit-20261015/`:
`container-impact.md`, `source-impact.md`, `oracle-and-coverage.md`, the executable
output triage and the isolated hardware arithmetic experiment. Existing sealed
evidence is preserved. Pro handoffs remain owner-suspended.

## Concrete methodological defects

1. **The numerical oracle was correlated.** The original-byte Python VM imports
   the same preserved floating-point primitives used by high-level functions.
   Agreement validates different control/dataflow descriptions and explicit
   state composition, but cannot independently prove that arithmetic's hardware
   semantics. CLI relocation and AST equality are transport/integrity evidence,
   not additional independent numerical truth.
2. **The main graph lacks some memory def/use edges.** In one original basic
   block7962BB..79636D, stores7962D1/+224 and7962F0/+228 precede reads796310/+224
   and796319/+228, then ADDSS/store796322/+218. The read nodes contain only the
   object-address dependency, without their reaching store values. An output
   slice for that +218 store therefore incorrectly ends after36 nodes/raw loads.
   The same-object, same-width, intervening pure608684 call and alias conditions
   are under a focused two-edge correction review. General heap/callee aliases
   remain unproved. Adding more local helper tests cannot repair a missing graph
   relation by itself.
3. **The queue was not consistently driven by an observable.** Tree sums reach
   unclosed FBEDA0/FBED80 consumers and then +224/+228 output stores. Deriving
   additional node destruction without first proving the sum-to-output relation
   does not establish objective impact. A pair-factor counter also misses this
   output cone. Later paths can overwrite the candidate outputs.
4. **Different outputs were conflated in scope.** A build's maximum-HP constraint
   and post-hit final current HP need different contracts. The default research
   interpretation of minimum HP is the displayed build maximum after rebuild;
   the owner can later select usable/reserved HP explicitly. Maximum HP needs
   candidate-correct STAT and capacity production, not every hit notification or
   resource reconciliation path. Complete damage application remains a separate
   requirement for an actual final-HP-loss objective.

## Observable contracts

Use a parameterized evaluator `E(build, scenario)` with typed intermediate state
and declared observables. The scenario fixes the weapon/action identity, enemy,
combat flags, player/world constants and reference build. It does not freeze
candidate-dependent D/STAT, source admission or raw context-pair values as if
they were immutable external constants.

Keep separate outputs for outgoing damage channels, applied HP loss, build
maximum/usable HP, agility and toughness. The weapon-relative score compares the
candidate and fixed reference in the same declared scenario. Scenario weights or
a robust/Pareto objective can be added later. A single equipment multiplier is
not assumed invariant across enemies, actions and activation states.

Source aggregation order, duplicate/cap behavior, scalar widths, rounding,
candidate-dependent conditions and relevant aliases remain exact. Data structures
may receive a value-level summary after proving the selected observation is
preserved, including non-escape/noninterference and the supported success domain.
Allocation addresses, colors and full heap write traces are not automatically
optimization observables. An unsupported producer is never replaced with an
invented helper answer, empty collection, inactive effect or score zero.

## First milestone

Deliver one complete, declared fixed-scenario scorer before another broad helper
campaign. Choose an existing valid recorded scenario if its consumed data is
complete; otherwise identify the exact missing capture for that one scenario.
Use a baseline and an active modifier variant with a nonzero final output delta.

Acceptance requires:

- Inputs are the build and explicit scenario state; intermediate helper answers
  are not inputs disguised as ground truth.
- Every candidate-dependent edge on the chosen output path is executable or
  has a validated value summary. The first unsupported input is named precisely.
- All writes/reads leading to the selected endpoint, branch predicates, caps,
  rounding and applicable late overwrites are joined. Pre-application channels
  and final HP loss remain visibly different endpoints.
- A separate original-code/actual-game oracle compares the final observation.
  The scenario has an identified state epoch and reachability boundary.
- One repeatable artifact runs the input through the complete selected evaluator
  and records the independent output comparison. Packaging integrity alone does
  not pass this milestone.

This is a milestone toward every affix, not a reduction of the final scope. A
normal-hit scenario does not silently certify survival, resource timing or DPS.

## All-affix extension and legal optimization

Build a row-level catalogue for all3,609 effect IDs/1,152 native groups. Existing
materializer coverage becomes one column, not a final completion flag. Each row
needs its actual source admission, applicability predicate, aggregation rule,
numeric/curve handler, output consumers, interaction constraints and oracle
coverage. Keep byte/table provenance, parameter-domain coverage and full-scorer
coverage separate. A proved inactive predicate can return zero; missing semantics
cannot. Test shared mechanisms over their parameter domains and activation/
interaction boundaries, then automatically route every actual row through them.
No manual one-affix-at-a-time campaign is required.

The candidate domain is a separate contract: legal slot/type/requirements,
joint rolls/transform rules, set combinations and owned-instance identity.
Membership in a parameter table proves none of those. Exact constraints use the
candidate-correct evaluator. Compare discrete, relaxed/gradient and GPU methods
against the same validated score and small exact optima before selecting a
solver. Re-score returned incumbents exactly and report any remaining gap.

## Experiments actually completed during this reset

`output-triage-01` enumerates35 R14 stores across24 fields, retaining value,
address and control boundaries and multiple possible writers. Three stores
target+218. Its137 target/boundary labels are triage candidates, not137 functions
that must all be reconstructed. Unknown call alias effects and the missing
memory def/use edges mean this is explicitly not a sound whole-program slice.
Receipt SHA256:
`edc78b196419368ee318f27fbac36b47db25c9efcaa50b138bd4664e91beadf7`.

`native_leaf_oracle.py` now executes the exact189-byte/40-instruction1FF09F0 leaf
in a separate child process with guarded child-owned buffers. It uses original
machine arithmetic rather than the Python arithmetic reference. The native leaf
has no calls, relative external reads or branches. The115 retained supported
contexts each use two concrete completions of opaque bits:230 comparisons,
222 actual CPU leaf calls and eight zero-flag copy-only controls. All claimed
model bits, source/guards/code and native RAX match. Actual SSE controls are
nearest-even/DAZ-off/FTZ-off/masked; arithmetic naturally changes sticky status
flags, which are logged separately from unchanged control bits.

Current evidence is `native-oracle-context115-02/RECEIPT.json` and
`HARDWARE_RESULT.json`; original leaf SHA256 is
`775c932b29b51a1d9bb60ef11ac1004007ccb4df31914efd51f1d13aaf1c2609`.
The child uses no game process, and this experiment proves one arithmetic
mechanism rather than a whole hit or actual gameplay. Earlier pilot/metadata
records remain history. No current-player instance or complete scenario was
fabricated.

## Queue and stopping rules

1. Repair/validate the two demonstrated memory def/use edges and define the
   remaining memory/alias/control obligations of the chosen endpoint.
2. Inventory existing real-hit fixtures against that endpoint. Select one
   complete scene or request only the exact missing bounded capture.
3. Advance the first unresolved value, condition or object-binding edge that
   blocks that evaluator. For the tree cone, inspect the numeric consumers
   before spending more on unobserved topology/lifetime details.
4. Extend the independent native oracle only when it closes one of those
   milestone gaps. Do not replace a helper-count campaign with an oracle-count
   campaign.
5. Publish a repeatable complete-scene result before expanding the scene family;
   retain a visible unsupported catalogue until all intended affixes are handled.

Current extra container work is inspection-only and sealed. Source268's running
matrix was completed/sealed, but3F8 expansion and promotion are stopped pending
this queue. Whole canonical guardian promotion is sealed; actual router145 gate
remains pending and is not claimed completed. The source/guardian/container
proofs remain useful inputs, with their original limitations unchanged.

## Tool basis and limitations

Ghidra's [p-code representation](https://ghidra.re/ghidra_docs/languages/html/sleigh.html)
provides an established instruction/data-flow basis. REA's
[official repository](https://github.com/morluto/rea) integrates native analysis
engines and evidence workflows; tool availability is separate from correctness.
The [angr backward-slicing documentation](https://docs.angr.io/en/v9.2.211/analyses/backward_slice.html)
requires CFG/control/data dependence inputs and its
[API caveat](https://docs.angr.io/en/v10.0.0/api/angr.analyses.backward_slice.html)
warns that a CFG-based DDG can be unsound/inaccurate. Therefore automatic graph
reach must still be checked against memory, aliases and feasible scenario paths.
Use those tools to reduce manual plumbing, not to replace independent observation.

## REA backend resumed and actual tree-curve data acquired

The existing task-local REA6.4.0/Ghidra12.1.4/JDK21 installation was available
on D: even though it was absent from PATH and this chat's direct MCP catalogue.
Earlier October13 work had already used it for native exports. The recent
method-reset work underused it; absence from the direct tool list was not a
missing-engine blocker. The public launcher/private stdio transport now works
again without installation or global configuration changes.

Actual `analyze_function` calls recovered FBED80/FBEDA0's24-byte wrappers and
FC0360's complete504-byte/31-block body. `trace_native_values` returned221 nodes
and271 edges, untruncated. Three new byte-backed entry seeds were required in
a new process-local138-entry seed file; the old135-entry profile did not define
these functions. Seeded names/entries are caller assertions. Recovered graph
coverage is not a complete memory-alias or gameplay-dependence proof.

REA `read_bytes` explicitly reported0 of8 bytes/complete=false for the two
count DWORDs, and disk PE geometry places both counts and rows in virtual-only
.data. A planned read-only capture then obtained556 bytes: both positive signed
counts, both complete8-row/256-byte tables and the fixed constants. Every
consumer/caller signature matched frozen original.text; approved executable,
PID/birth/base and all rereads matched. Both table bodies have SHA256
`ec3bfead968ea8cbe9575dfb4cb2bd4ea5fc47ed5f9fb2f2dfeb13893ad9501d`.
Equality is observed for this epoch, not an initializer/version invariant.
The task-owned game closed normally, with no remaining process or CUA access.

Evidence/reproduction is under `method-audit-20261015/rea/README.md`, complete
query02 outputs, `acceptance-01/ACCEPTANCE.json`, and the sibling audit-root
`tree-curve-runtime-capture-01/`. The prior tree-consumer report's missing-data
statement is historical: the exact counts/rows are now captured. Binding the
caller inputs and integrating these consumers into a complete fixed-scene
evaluator remain open. No all-affix, whole-hit or legal-optimizer closure follows
from the tool graph size or this successful parameter capture.

## Owner-directed REA-first execution and current milestone status

The owner explicitly requested maximum useful use of REA. Treat it as the primary
native-analysis entry: use decompilation, typed references, call-path/target
resolution, instruction inspection and value tracing before writing a custom
replacement analysis. Existing specialized replay/arithmetic remains validation
and integration machinery. Choose tools by the objective dependency; running
every advertised operation is not a completion criterion. Preserve the current
owner's direct-analysis/Pro suspension and the all-affix scope.

Current accepted product-facing stages are deliberately separate:

| Stage | Current evidence | Gate still open |
| --- | --- | --- |
| Parameter/effect indexing | All3,609 effect IDs/1,152 groups and local field dispatch mapped; actual parameter acquisition retained | Complete row-specific admission, activation, interactions and damage consumer coverage |
| Native forward components | Multiple source, prepared-attack, field/stat, rank and resource stages executable under explicit contracts | Candidate-correct end-to-end build/state composition |
| Optimization laboratory | Executable IR, set collaboration and replaceable exact/stochastic/gradient search experiments | Actual native score, real legal/owned candidate domain and game-backed benchmarks |
| First complete scorer | Objective, reference/scenario and independent endpoint acceptance defined | No complete build-to-final-score baseline/active-variant pair accepted yet |
| Usable constrained optimizer | Not accepted | Full scorer/constraint/legality integration and end-to-end result verification |

These are readiness gates, not equal fractions of the remaining work. Local
function/case/node counts do not supply a defensible whole-project percentage.
The critical path is one complete fixed-scenario scorer, then mechanism-family
coverage over every actual affix, followed by real legal/owned optimization.

### Additional actual result: typed REA dependencies and CPU-checked curves

`rea/main-input-03` uses actual REA calls to resolve78EC63/78EC82 and recover the
entire main pseudocode/instruction view. Native78EC56 loads XMM1 from[RBP-20]
before the first curve call. Both calls therefore use the same coordinate,
F32([RBP-20]*[RBP-1C]), under the stable caller-frame/normal-return domain. The
earlier5B-starting consumer window omitted this defining instruction.

The untyped pseudocode exposed a concrete analysis hazard: FBEDA0()/FBED80()
appeared without parameters and the live XMM7/XMM6 summations disappeared.
The original assembly contains both. A task-private Ghidra seed annotation now
declares precisely three byte/CPU-backed Windows fastcall signatures. No installed
REA/provider or native bytes change; a process-local adapter selects the private
bridge and includes adapter/seed/extra-ABI hashes in the analysis profile.
`main-typed-04/acceptance-01` confirms both two-argument calls and both sums are
restored. The raw typed/untyped observations remain available for comparison.

The finite local consumer model reads the actual binary counts/rows/constants
and uses independent exact integer/rational binary32 RNE operations. Its142
supported boundary/ULP/ordinary vectors all match original FC0360 CPU arithmetic
in a separate child.18 nonfinite/overflow cases remain explicit refusals. Only
four audited RIP constant-address displacement fields are relocated in the
native child; all arithmetic/control instructions and actual table bytes remain
unchanged, with guard/control-register checks. This is one local shared mechanism,
not a complete hit or real equipment scenario. Evidence is under
`tree-consumer-model` and `rea/curve-cpu-model-01`.

A separate actual whole-main trace remains partial: the provider caps High P-code
at3,000 operations even when the outer trace budget is20,000. The result correctly
reports truncation and residual unknowns. Paging cannot recover operations the
provider omitted. Keep this limit visible; do not promote the bounded graph into
a complete dataflow proof. The restored main C relationships and the independent
curve acceptance remain useful despite that larger-graph limit.

## Owner correction to historical manual Ki Pulse controls

The owner states that all earlier manual normal/perfect Ki Pulse attempts used
the guard key and did not successfully perform a manual Ki Pulse. Those historical
manual-control labels are withdrawn. Preserve raw bytes, events, original labels
and source hashes; add the correction as provenance rather than rewriting them.
Ki Pulse activation/recovery conclusions that depended only on those labels are
unsupported. Directly observed numeric writes and independent native code/math
retain their narrower scopes; action identity needs independent native evidence.
The current complete-scorer milestone uses ordinary attacks and does not require
the owner to repeat Ki Pulse experiments. The machine-readable correction is
fixed-scenario-e2e-20261015/INPUT_CORRECTION.json under the audit root.

## Actual ordinary-hit oracle and cumulative replay (2026-10-15)

The prepared observer now has one actual, associated ordinary-hit frame on the
approved PC2.02 executable and fully verified loaded.text. Four checkpoints retain
the original MAIN entry, resolved actors, MAIN final record, and actual memory
after the HP store. Root's separate `observed_hit_oracle.py` verifies original
region hashes, checkpoint RVAs, actor pointers, exact MAIN R14/RBP/RSP identity,
event order and the before/after HP address. Its output is comparison data only;
the damage model imports entry prestate, never the final/HP snapshots.

The observed MAIN+218 bits are `00e0a344` (1311.0). Target HP is75108 before
application and73797 after, with max77730. The measured loss is1311 for this hit.
This agreement is an observation, not a general damage-to-HP application rule.
The HP association lacks debug-thread identity and a complete upstream caller
proof. RET is inferred from the original final epilogue rather than directly
observed. MXCSR was not captured; finite RNE/DAZoff/FTZoff remains a declared
offline execution domain. Cross-thread atomicity is not claimed.

The first45-second combined capture retained eight temporary rebuild entries but
no MAIN event. Those temporary owner/STAT pointers and whole raw equipment/core
images do not join the later player's observed MAIN input; do not promote them
to a player rebuild witness. A fresh ordinary-only window captured the four
checkpoints and automatically stopped. Owned hardware breakpoints and timers
were removed and verified before runner exit. The user subsequently exited the
game. A later read-only supplement failed before obtaining any bytes; it supplies
no TLS/CRT/static-data evidence and does not justify reopening the user's game.

Evidence is under `fixed-scenario-e2e-20261015/coherent-live-01/` and
`observed-oracle-01/ORACLE.json` in the R9 audit delivery root. The raw ordinary
export is1753125 bytes, SHA256
`2e9b32cc0b3f875b6623146e1848d0e478abda9fc7564df212089271302f5b1d`.
The cumulative replay still reports an explicit dependency frontier; no complete
raw-equipment scorer, baseline/active-variant delta, all-affix or legal optimizer
acceptance follows from capturing this oracle.

Root and both active native lanes now use independent actual REA sessions with
task-private profiles, exact original carrier identity and retained evidence IDs.
The root's expanded High P-code export also remains partial at65536 operations;
its late endpoint stores are not covered. A retained evidence export recovered
the over-budget MCP response, not the omitted provider operations. Focus new REA
queries on the next consumed objective dependency rather than another whole-main
graph expansion.

## Coupled candidate execution exposes phase and packet inputs

Root's first actual paired integration passes emitted reference/familiarity
candidate memory through physical carving and the public MAIN interpreter, with
one source identity across both runs. Unknown producer bits replace captured
known bits; consistent aliases share one view. Both runs correctly stop after115
original instructions:78D088 reads the uncomputed source-owner maxHP and78D08F
cannot decide ZF. Actual sourceSTAT HP is4137, while the currently reconstructed
prefix is3469. These are useful dependency observations, not accepted scores.
Keep build maximum HP and explicit scene current HP distinct.

Two additional native boundaries must not be hidden by coupling plumbing:

- owner+25A8 inlineSTAT's+7D8 is the same physical pointer field as owner+2D80.
  A captured core may already contain first-pass equipment effects; it is not
  automatically pristine nongear source state. The current cumulative fixed-core
  invocation remains conditional until its original phase/invariance is proved.
- The main attack coordinate comes directly from MAIN+08, not a fresh prepared
  STAT read in the retained prefix. Updating STAT/D while leaving this field
  frozen cannot establish equipment sensitivity. The prior observed attack-source
  chain and new REA caller/getter work must rebuild this packet input separately.

The actual captured primary/flat pair2934+148 equals MAIN's3082, but numerical
agreement alone does not prove the current selected-record producer. Preserve the
current real caller return3277EB and original4F3550/builder/copy evidence. Only
proved input fields may be regenerated in MAIN; final+218 remains oracle-only.
The existing earlier-world unarmed8 bytes remain conditional inputs, not a new
same-hit observation. Any future missing-phase capture is designed offline first;
the owner has left and no new action or game launch is requested now.
