# Formula-independent loadout optimizer lab

October20 continuation: the CLI now includes the bounded
[real observed-equipment native adapter](NIOH3_REAL_GEAR_OPTIMIZER_20261020.md).
It evaluates and searches the exact empty/observed-helmet domain, with independent
verification and hard constraints. The synthetic-only statements below describe
the original foundation; they do not describe this new adapter. General unseen
equipment prediction and product optimizer readiness remain open.

Current2026-10-16priority: connect the real candidate evaluator using the
[native product milestone](NIOH3_NATIVE_EVALUATOR_PRODUCT_MILESTONE_20261016.md).
The current code already includes exhaustive/random/set-pattern/PG/CEM/BnB
baselines. Their synthetic evaluator and integer-bound certificates remain
separate from native scoring. Some initial-foundation details below predate
those additions; do not rebuild already present baselines.

Status: implemented research foundation, 2026-10-15 (America/New_York).
This is a standalone Python/JSON lab. It is not connected to the shipped UI,
generation/save/native-add APIs, a live game, R9 outputs or a GPU backend. All
fixture coefficients, item identities, sets, rank rules and damage are synthetic.
The final native algorithm/backend choice remains open.

## Purpose and completion boundary

While Pro R9 recovers the actual forward model, prepare the stable parts:

1. Compile complete query inputs into an inspectable, fingerprinted IR with
   explicit coverage, ordered operations and native dependencies.
2. Represent cumulative set tiers and coordinated choices without pretending
   marginally weaker items can safely be discarded.
3. Run replaceable search baselines under recorded budgets against reproducible
   small exact-reference and larger no-oracle synthetic problems.
4. Exercise the real CLI end to end and retain inputs, outputs, source identity,
   counters, seeds and independent expected-answer evidence on the D: build volume.

The lab can support comparisons before the native formula is complete. Synthetic
speed or accuracy does not establish the best native solver or game acceptance.

## Source map

| Source | Responsibility |
| --- | --- |
| [CONTRACT.md](../../research/loadout_optimizer/CONTRACT.md) | Versioned problem/IR/API and proof/budget contract |
| [ir.py](../../research/loadout_optimizer/ir.py) | Input validation, query folding, fixed reference, coverage and deterministic IR |
| [evaluator.py](../../research/loadout_optimizer/evaluator.py) | Exact synthetic reference evaluation and batch/adapter interface |
| [synergy.py](../../research/loadout_optimizer/synergy.py) | Instance/slot/lock checks, cumulative tiers and conservative activation patterns |
| [search.py](../../research/loadout_optimizer/search.py) | Exhaustive, random and coordinated set-pattern CPU baselines |
| [fixtures](../../research/loadout_optimizer/fixtures/__init__.py) | Six analytically specified small cases and one 200-item domain |
| [benchmarks.py](../../research/loadout_optimizer/benchmarks.py) | Reproducible JSON/CSV, source snapshots, runtime identity and quality accounting |
| [CLI entry](../../tools/run_loadout_optimizer_lab.py) | Independent research command entry; no product integration |
| [E2E acceptance](../../tests/research/test_loadout_optimizer_lab_e2e.py) | CLI-only independent expected answers and adversarial scope checks |

## IR semantics

Problem schema is `nioh3-loadout-problem/v1`; compiled schema is
`nioh3-loadout-ir/v1`. Preserve the native/opaque source facts rather than forcing
an additive approximation. The compiler retains original item/effect payloads,
typed field specifications, source/evidence identities, scenario-specific ordered
operations, all coverage decisions, reference inputs, constraints and slot supply.
SHA256 fingerprints bind both original inputs and canonical compiled IR.

- `integer_sum` is exact integer arithmetic for a declared fixture. It is not a
  claim about native widths, wrapping, units or global additivity.
- `ordered_f32` rounds the initial value, operand conversion and each addition
  to binary32, in declared slot/effect/set order. Operations are not regrouped.
- `opaque_native` preserves a known reset or explicit unknown initial:null and
  binding metadata. It compiles structurally but cannot be evaluated as zero.
- `native_nodes` retain typed outputs, producer metadata and dependency references.
  Original declaration order is retained and a separate topological order is
  emitted. Cycles or unbound internal references are invalid; `external:` inputs
  are explicit unresolved anchors. No node, RVA, expression or code is executed.
  Allowed output types are u8/u16/u32/u64, i8/i16/i32/i64, f32/f64, bool, bytes
  and opaque; alternative labels require an explicit schema revision.
- Only explicit scenario-condition mismatches can fold an effect away. Missing
  conditions, unknown semantics and unbound metrics keep evaluation unsupported.
  Irrelevant-effect declarations retain their evidence and per-query scope.
- All native formula, legal-equipment, complete-inventory and optimizer-readiness
  flags remain false. Input metadata cannot promote them to accepted game evidence.

The initial numerical evaluator deliberately supports only `synthetic-v1`.
`native-pending` exposes a structural interface and returns unsupported for numeric
queries. A future R9 adapter must implement the actual native program/widths and
validate its evidence/coverage; changing a model ID is not sufficient integration.

## Loadouts, hard constraints and fixed reference

Every candidate has explicit slots and compatible items. v1 uses unique instances
with quantity1. Alternate item IDs may alias the same instance; they cannot occupy
two slots simultaneously. Identical-looking separate instances remain distinct.
Equivalence-class supply and naturally attainable generation domains are not
invented. Query locks apply to candidates, not necessarily the fixed reference.

The reference can intentionally violate candidate minimum constraints or use an
empty slot for a counterfactual baseline. Supplied reference item compatibility
and unique-instance constraints still hold. The same weighted reference denominator
is used for every candidate and must be positive. Per-scenario damage and ratios
are reported, including a null per-scenario ratio when its reference damage is zero.

Hard numeric/rank constraints are checked in every declared scenario separately.
No state averaging, finite penalty, tolerance or relaxed fractional equipment can
stand in for satisfaction. Unknown constraint metrics are rejected before search,
including an empty candidate domain; they must not become an infeasibility proof.
Fixture agility/toughness rules are configurable synthetic tier orders and do not
stand in for the recovered game thresholds/unlocks.

The later direct native research has independently recovered the actual display
selectors and all local field-materializer groups. See
[Stage3 contracts](NIOH3_DAMAGE_STAGE3_CONTRACTS_20261015.md) for numerical rank
thresholds, unlock conditions, original-byte/public evidence and open producer
dependencies. These components have not been silently substituted into the
synthetic evaluator. Complete actor/damage and legality still gate a real native
adapter and final solver/backend selection. As of 2026-10-16, direct native work
continues and the owner permits Pro packages for major unresolved obstacles.
Historical handoffs are navigation aids; current raw evidence and executable
checks determine which conclusions remain valid.

## Explicit set collaboration

Each selected item contributes once to each declared set tag. All reached tiers
activate cumulatively and exactly once. An activation pattern records count ranges
that correspond to one cumulative tier signature. Complete patterns partition the
valid supplied assignments, but their initial reachability calculation is only an
independent-slot relaxation: joint instance competition remains to be checked.

The set-pattern baseline constructs coordinated multi-slot candidates using
bounded DFS and conservative count lookahead. Joint demanded-tag deficits and
remaining upper capacities are checked without assuming an item has only one set
tag. Actual instance aliases are always forbidden by construction/evaluation.
Construction nodes/work units and pattern expansion are capped and reported.
An exhausted cap is not an infeasibility or completeness proof. There is no
arbitrary per-set top-k, unproved dominance or deletion of weaker-looking members.

## Baselines and certificate scope

`exhaustive`, `random` and `set_pattern` are initial comparators, not a selected
production algorithm. Each gets the same complete-candidate attempt budget;
duplicate or structurally rejected candidates still spend attempts. Separate
evaluator/revalidation counts and set-construction work expose unequal cost.
All improved best results are exactly re-evaluated before acceptance.

Only a fully traversed, supported, declared complete finite synthetic domain can
produce `optimal_for_fixture` or `infeasible_for_fixture`. Other runs return a best
found candidate or bounded not-found state with no fabricated gap. A known small
benchmark optimum can measure heuristic quality, but does not upgrade its search
certificate. Native legal/owned/global optimality is never claimed by these fixtures.

No branch-and-bound, convex/concave envelope, policy gradient, Gumbel relaxation,
learned surrogate or GPU evaluator has been selected or claimed here. They can be
added and compared through the same contract after their required structure and
numeric semantics are established. Random property tests can find counterexamples;
they cannot prove a global monotonicity/concavity or pruning-bound guarantee.

## Fixtures and measurement

The quick suite includes an analytic optimum, a three-slot synergy trap, a rare
feasible loadout, explicit synthetic rank/cap boundaries, state-dependent hard
constraints and an infeasible case. The standard suite adds200items across8slots
(25choices/slot). Larger quality is null because no exact optimum is supplied.
Analytical small-case proofs and exact witnesses are stored separately from
search certificates, with no hidden unbudgeted exhaustive oracle run.

Each benchmark directory includes:

- `inputs/`, `ir/`, `oracles/`, per-run `results/`, `report.json` and `results.csv`;
- source snapshots and SHA256 manifest; input/IR/evaluator/source fingerprints;
- seeds, candidate/evaluation/construction budgets and counters;
- Python/platform/backend identity, compile/search/final-validation wall times;
- per-scenario constraint witnesses, known-optimum quality only where justified;
- explicit synthetic/native-unavailable status and deferred algorithm decision.

Compilation is shared once per fixture; per-run timing records the same compilation
cost along with that run's search and additional final validation. Total benchmark
elapsed time also includes artifact/snapshot overhead. Times are observations,
excluded from deterministic score/loadout/certificate replay. This framework is
not a CPU/GPU performance conclusion for the still-unfinished native evaluator.

## Commands and retained outputs

On this host all Python runs use PowerShell7 and the prepared wrapper. Use a fresh
task-specific D: output for each run; set bytecode off before importing test modules.

```powershell
$env:PYTHONDONTWRITEBYTECODE = '1'
& 'F:\Nioh3_ScrollEditor\tools\run_python_tests.ps1' -ScriptPath 'F:\Nioh3_ScrollEditor\tools\run_loadout_optimizer_lab.py' -ScriptArgument @('benchmark','--suite','quick','--seeds','0,1,2','--budget','1024','--output-dir','D:\Nioh3_v080_deliverables\deliverables\loadout-lab-QUICK-FRESH')
& 'F:\Nioh3_ScrollEditor\tools\run_python_tests.ps1' -ScriptPath 'F:\Nioh3_ScrollEditor\tools\run_loadout_optimizer_lab.py' -ScriptArgument @('benchmark','--suite','standard','--seeds','0,1,2','--budget','512','--output-dir','D:\Nioh3_v080_deliverables\deliverables\loadout-lab-STANDARD-FRESH')
```

The CLI also exposes `compile`, `patterns`, `evaluate` and `solve`; every command
has `--help`. Unsupported models exit2; invalid input exits3; supported operations
(including a fixture no-solution result) exit0. Evaluation has no native/game I/O.

Primary task outputs are under
`D:/Nioh3_v080_deliverables/deliverables/loadout-optimizer-lab-20261015/`.
Final delivery records identify the frozen-source benchmark/E2E results; worker
review artifacts or reports with `source_stable_during_run=false` are not final
reproducibility evidence. No R9 ZIP/source is changed by this independent work.

## R9 and update integration

When R9 arrives, keep original captures, hashes and native type/guard/source order
as authority. Implement a separate adapter and first replay it against independently
verified native-stage controls. Preserve unsupported dependencies and exact reference/
state policies. Only then add real-game benchmark datasets and compare search
methods under equal end-to-end budgets. Final algorithm/backend choice remains open.

Use [the acquisition/update runbook](OFFLINE_PARAMETER_ACQUISITION.md) for data and
reader changes. Query compilation, field semantics, legality, evaluation, search
and certificate provenance stay separate so an update invalidates the affected
contracts rather than silently weakening every guarantee or requiring a restart.

## Accepted 2026-10-15 delivery

Frozen quick/standard evidence contains117seeded baseline runs. The independent
35-case E2E gate executes67real CLI operations and retains418file hashes. A fresh
actual ZIP extraction replays36benchmark runs and all35E2E cases successfully,
with the identical lab source fingerprint
`e9e8566746f9ee223403810d720ac0ec41bcfc0017d5eacbd07ecae9ffb31bcb`.

Standalone artifact is `delivery/loadout-optimizer-lab-20261015.zip` under the
task D: root above:1,562,342bytes, SHA256
`ad07f973b3f6fa7f04dca4cc7eb5956785ea8193a3bf0f714ef8e75779884141`.
All627payload hashes/628exact ZIP files, CRC, actual extraction and immutable
post-replay hashes pass. The external `delivery/DELIVERY_RECEIPT.json` owns the
final verification. `tools/package_loadout_optimizer_lab.py` and
`tools/verify_loadout_optimizer_lab_delivery.py` preserve/replay this workflow.
These checks accept the declared synthetic lab; native-game optimization and
final solver/backend selection remain explicitly unclaimed.

## Discrete learning, interval certificates and experiment ladders (2026-10-15)

The v2 laboratory adds actual score-function policy gradient and smoothed elite
categorical search, alongside the original three baselines. They operate on exact
discrete loadouts and use the unchanged exact synthetic evaluator. Differentiating
through the damage formula is unnecessary for this policy-gradient method.
Conditional instance masks prevent physical-item reuse; bounded dead ends remain
training observations, not evidence of infeasibility. All exact hard constraints
and the original fixed-reference objective are rechecked for accepted incumbents.
The bounded training reward is a search aid and can saturate in binary64; it is
not an alternate definition of strongest equipment. See
`research/loadout_optimizer/STOCHASTIC_METHODS.md` for the derivation and sources.

A separate integer-only branch-and-bound implementation uses exact interval
upper bounds. It safely relaxes correlations between fields, future instances
and optional cumulative set tiers. It does not assume concavity or generalize
integer arithmetic to native binary32. Signed contributions, bonus clamps,
floor/caps, all-scenario constraints and supported rank intervals are included.
A work cap can stop construction before a first candidate; counters report that
outcome distinctly from infeasibility. Final method selection is still empirical.

The new verify-result command checks full incumbent witnesses and scope metadata,
then independently enumerates bounded small domains to confirm a claimed exact
certificate without using the search implementation or interval bounds. Its
shared evaluator remains synthetic. It detects search-proof errors, not an
incorrect native damage formula. Exceeding the verification domain limit returns
verification_incomplete. Heuristic witnesses are never promoted to global proofs.

The experiment command adds reproducible budget ladders, per-run learning and
construction diagnostics, improvement trajectories, median/p95 cost and grouped
feasibility/quality. Every budget is a new seeded restart. Unsupported pairs are
reported separately and excluded from feasibility/quality/utility denominators;
supported failures keep null quality, with an explicitly distinct zero-contribution
utility only when a known positive optimum exists. Stress fixtures extend to
2,000 items and cover multi-tag/alias competition and ordered-f32 cancellation.
The previous six quick fixture inputs remain unchanged. All large-domain optima
remain unknown; there is no inferred native legality or selected GPU backend.

The independent v2 test suite adds45 CLI E2E cases to the35 original cases,
including12 independent exhaustive integer oracles, forged certificate rejection,
constraint/rank cliffs, signed set/bonus effects, learning replay, enormous exact
scores, UID dead ends, and independent experiment-statistic recomputation.
Final frozen run counts, source identity and relocated delivery evidence are
recorded in the task's finalization receipts, not inferred from development passes.

A read-only hardware inventory reports an NVIDIA GeForce RTX4070TiSUPER with
16,376MiB and driver616.64 on this host. No GPU backend or speed claim follows from
that inventory; CPU reference cost and numerical equivalence must guide selection.
The laboratory remains separate from the R9 native prepared-record acceptance.
The owner initially suspended further Pro handoffs and authorized direct native
analysis, then permitted Pro packages for major obstacles on 2026-10-16. Existing
evidence boundaries persist; a prior handoff does not supersede current evidence.

The later [Stage4 native contract map](NIOH3_DAMAGE_STAGE4_CONTRACTS_20261015.md)
now proves one continuous ordered field/post/final-source suffix. Its late source
updates must not be reapplied as a second field pass; inventory/source aliases,
source order and rebuild phase belong in a future native adapter's inputs/cache
identity. This evidence does not change the synthetic lab evaluator, solver
certificates or supported native-readiness flags. All-group field coverage is
distinct from complete damage, all source admission and legal candidate domains.

## Native objective and state-cut implications (2026-10-15)

The first actual ordinary-hit oracle is now retained separately from evaluator
inputs: MAIN damage1311 and actual target HP loss1311. Candidate coupling is still
unaccepted. The native path consumes a binary32 attack-coordinate curve720 and
an iterative attack-versus-defense scalar230 compression before later factors.
Those two numeric windows now have independent original-CPU comparisons. A
weapon-relative damage ratio therefore remains a ratio for an explicit action,
enemy defense and combat state, rather than a universal equipment-only multiplier.
The synthetic objective and algorithms are unchanged pending a complete scorer.

The native adapter must key caches and admission by more than the item IDs and
summed effects. Exact rebuild phase, source/STAT aliases, active weapon selection,
attack-packet producer state, versioned parameter identity, machine float domain
and selected endpoint belong in its dependency identity. The current absolute-HP
scene policy is explicit; a future full-health/percentage policy must derive
candidate current HP consistently with its maximum HP rather than retaining an
impossible baseline current HP after a capacity change.

Two integration failures are now concrete. A post-inline source used as an earlier
copy input double-counts a set HP contribution328; generated source996 conflicts
with actual668, while base3469+668=actual4137. Separately, MAIN+08 is a constructed
attack-packet input and does not automatically change when candidate STAT/D is
replaced. Other packet fields also need a native producer or a proved invariance
domain. A final score with stale packet inputs is not a native objective.

The output-driven queue remains: one real candidate-correct baseline and active
variant, independent complete endpoint comparison, same-candidate HP/agility/
toughness, then shared-mechanism all-affix and legal/owned candidate coverage.
Exact/scenario feasibility checks precede any claim about search quality. Source
phase, pointer ordering and local arithmetic counts are not completion percentages.
See [the method-reset record](NIOH3_DAMAGE_METHOD_RESET_20261015.md) for the current
evidence paths and the explicit remaining gaps.
