# Nioh3 damage and constrained-loadout research: Claude handoff

Owner handoff date: 2026-10-17, America/New_York. The owner requested acceptance
of the returned R15 source, then a temporary transfer of continuation to Claude.
This document is the operational starting point. Original reports and older
handoffs are evidence with a date/scope, not instructions to repeat their plan.

## Goal and current decision

Build a constrained optimizer for legitimate theoretical equipment or combinations
of the user's actual inventory. Constraints include minimum HP, agility and
toughness ranks, slots, instance uniqueness, requirements and sets. The proposed
default objective is fixed-reference relative damage for a specified weapon/
action/scenario, optionally a weighted set of scenarios. Timed DPS needs an
explicit action/resource/state sequence model; no universally valid damage
multiplier is assumed for every enemy, move and conditional state.

All 3,609 effects, 1,152 native groups and 392 orphan groups remain in scope. The affixes
used earlier as examples have no special priority. Do not ask the player to
test individual affixes. Reuse the existing native provider, semantic IR,
catalog, collectors and optimizer algorithms. The independent R15 strategy
review selected a thin verified input bridge plus existing native rules, not
a generic memory translator, second graph runtime or another solver/GPU project.

The next product milestone remains a real baseline and a legal active equipment
variation with complete same-candidate damage, max/currentHP, agility, toughness
and constraints, followed by a complete small actual-domain exhaustive baseline.
Current complete real candidate scores and legal full-score contrasts are0.

## Exact starting directories

The local machine is Windows. The repository is `F:/Nioh3_ScrollEditor`.
Use PowerShell7 and read its current `AGENTS.md`. Talk to the owner in Simplified
Chinese; write source/tests/comments/docs in English. Python MUST run through
`tools/run_python_tests.ps1`. Output/cache/build/test temp belongs under
`D:/Nioh3_v080_deliverables` (or the explicitly resolved build root), not the
checkout volume or C:system temp. Preserve the unrelated dirty/untracked work.
No stash/clean/reset, release, push, tag or global model/provider changes.

| Role | Absolute path |
| --- | --- |
| **Continue from the repaired working source** | `D:/Nioh3_v080_deliverables/deliverables/damage-pro-return-audit-r15-20261017/working/nioh3-r15-accepted-working-20261017` |
| Immutable extracted Pro return | `D:/Nioh3_v080_deliverables/deliverables/damage-pro-return-audit-r15-20261017/provided/nioh3-r15-semantic-evaluator-pc202-20261017` |
| Original downloaded source ZIP | `D:/Downloads/nioh3_r15_native_scoring_slice_source_20261017.zip` |
| **Final local acceptance status** | `D:/Nioh3_v080_deliverables/deliverables/damage-pro-return-audit-r15-20261017/FINAL_ACCEPTANCE.json` |
| Actual repaired public replay | `D:/Nioh3_v080_deliverables/deliverables/damage-pro-return-audit-r15-20261017/repaired-replay-01` |
| Reuse this verified original/cache | `D:/Nioh3_v080_deliverables/deliverables/damage-pro-return-audit-20261016-r10/r12-extracted-replay-01/calculation-verification/cache` |
| Materialized original input | `D:/Nioh3_v080_deliverables/deliverables/nioh3-r10-pc202-20261016` |

Read the final local acceptance JSON and its proof receipts before relying on
a passing/accepted label. It distinguishes the original verifier failure,
repaired execution and independent numerical/source/API checks. The source ZIP
is73,869,101bytes, SHA256
`3ba7e019e1b145460a329087fc39bdd142ef1f7bcdbc422e252f8680d044e832`.
CRC and all2,550members/2,549payload hashes passed local extraction. The separate
downloaded result Markdown matches packaged `RESULTS.md` byte-for-byte.

**Do not confuse this return with the older sent R15 package** at
`D:/Nioh3_v080_deliverables/deliverables/nioh3-r15-semantic-evaluator-pc202-20261017`.
They reuse a historical basename but have different parent directories/content.
Also do not assume the repository's `research/loadout_optimizer` already has
the returned registry/provider installed. The working standalone package does;
repository/product installation remains separate work with explicit resource
binding. Do not mechanically copy delivery-relative paths into the repository.

## Accepted numerical slice and its limits

Local acceptance is complete: the repaired working copy passes the entire
public verifier on Windows, including regenerated result-core equality and
positive native-provider binding. Independent numerical, source-invariance
and API-admission reviews support the restricted slice below. The unmodified
return's failed Windows comparison is preserved; it is not relabeled as a pass.

The exact domain fixes16of17native slots. Active bank0slot05 is baseline chest
regular780/key0x1BA5 or the observed bank1slot05 chest regular1029/key0x85C6.
Weapon, other equipment, active selectors, progression, scenario and independent
baseline reference stay fixed. The original552DD0copy writes224of232bytes,
preserving destination offsets24..27andE4..E7. Keep actual equipped E8 bytes
distinct from regular F0pool records. The20observed occurrences/22placements
include shared aliases; they are not a full backpack or permanent game UIDs.

| Output | Baseline | Chest replacement |
| --- | ---: | ---: |
| Body weight, native units |347|275|
| Body/final projected toughness |325|281|
| Fresh HP seed |3469|3469|
| Maximum HP |4137|4137|
| Current HP under explicit full-HP policy |4137|4137|
| Agility score / rank |50 / B|50 / B|
| Toughness rank |AA|A|
| Damage / complete candidate score |null|null|
| Full equip/generation legality, optimizer admission |unproved|unproved|

These are fixed-scene, normal-return numerical projections, not live equipment
acceptance or arbitrary-loadout formula coverage. The numerical witness uses
agility>=B and toughness>=A. It does not satisfy the owner's illustrative AA
agility requirement or implement every configurable constraint.

HP uses a fresh2696+370+403=3469seed, source+5880->STAT+18contribution0, and
final-consumer B1B6=668. No668-328inverse produces a fictional pristine source.
The source itself is not replayed through collectors as a clean base. Separate
restricted invariance proves the relevant final consumer fields unchanged for
this exact two-chest domain. The other present relevant group is4329=150.

Independent review recomputed the17possibly changed direct/set-reward groups
from original tables, checked source controls/capacity/thresholds and relevant
native write spans, and ran actual dispatch paths. The17groups are
`0BF9 369F 3A2A 43B5 4D1C 5B5D 82A5 8C3A 99FB AB47 B00A C5D3 EB7A F32F F670 F671 FFEC`.
They do not intersect the seven metric groups or38BA/8508cross controls.
Fifteen write only their small D fields; B00Aupdates STAT+1C8andC5D3updates
six prepared fields. **The HP/rank proof does not prove damage invariance.**
Read `source-proof-review/` for the exact proof scope, not just that group list.

Independent arithmetic/table decoding reproduced all listed numbers. New
private Windows controls executed29original-byte cases, including the22tail
leaves, the real copy/padding cases, HP arithmetic/rounding joins and source5880
writer controls. These are not full BE108/MAIN hardware runs. The supplied
Windows public verifier skips Linux-only hardware leaves; separate local CPU
evidence is recorded rather than treating that skip as hardware acceptance.

## Two local engineering repairs

The original Windows public verifier ran all component/research/native commands
but failed its final results-core equality check. All metric values were equal;
13core differences were proof/IR/assignment identities. Windows backslashes in
artifact/model paths caused them. The working copy uses POSIX relative protocol
paths in `metric_projection.py` and provider identities. The original Linux
proof IDs are restored for both candidates.

Independently, mutating a caller-owned research result contaminated later
candidate source evidence through cached nested objects. `NativeProduction`
now deep-copies its return. The same-instance mutation/reuse control verifies
the narrow fix. Fresh-process published numbers were not invalidated.

Exact diffs and before/after hashes are under `repair/` and the working copy's
`evidence/local-repair/`. Original reports remain in
`evidence/pro-delivered-results/`. Root reports were regenerated from actual
repaired execution. Strict core equivalence permits only the three compiler/
assignment fingerprints that necessarily change with provider source bytes;
all metric, proof, domain, constraint and unsupported fields remain exact.
No identity check or validation requirement was removed to make tests pass.

## Run and inspect

Choose a fresh output directory and reuse the existing cache; disk headroom is
limited, so do not create another full dependency/cache copy unnecessarily.
The explicit1.5GiBfloor below is for this bounded Python-only replay, not a
general waiver of the project's build-space policy.

```powershell
$r15Working = 'D:/Nioh3_v080_deliverables/deliverables/damage-pro-return-audit-r15-20261017/working/nioh3-r15-accepted-working-20261017'
$r15Cache = 'D:/Nioh3_v080_deliverables/deliverables/damage-pro-return-audit-20261016-r10/r12-extracted-replay-01/calculation-verification/cache'
& 'F:/Nioh3_ScrollEditor/tools/run_python_tests.ps1' `
  -MinimumFreeGiB 1.5 -ScriptPath "$r15Working/run.py" `
  -ScriptArgument @('--operation','research','--cache-dir',$r15Cache,
    '--output-dir','D:/Nioh3_v080_deliverables/deliverables/claude-r15-research-01')
```

For the full public verifier use `$r15Working/verify_delivery.py` with
`--work-dir <fresh-external-dir> --cache-dir <same-cache> --native-baseline`.
Expected research outputs include `CANDIDATE_INPUTS.json`,
`CANDIDATE_RESEARCH.json`, `NATIVE_COMPILE.json`, `NATIVE_SOLVE.json` and receipt.
The solve must remain `unsupported` with no certificate until full scoring and
the submitted domain/reference are actually admitted.

Read these current files in the working root:

1. `STRATEGY_REVIEW.md`, `UNRESOLVED_INPUTS.md`, `DEPENDENCY_SLICE.json` and
   `LOCAL_REPAIRS.md` for current direction, exact demands and fixed boundaries.
2. `project-source/semantic-bridge/semantic_bridge.py`: `build_context` and
   `run_research` preserve the real crosswalk/domain/reference/scenario.
3. `project-source/native-evaluator/nioh3_native_evaluator/provider.py`:
   `research_candidate` invokes actual production and per-output metrics.
4. `project-source/candidate-metrics/candidate_metrics.py`: `CandidateMetrics.evaluate`
   and its explicit unsupported damage record around the original lines283..289.
5. `project-source/native-production/production.py`, `factory-tail/factory_tail.py`
   and `native-evaluator/nioh3_native_evaluator/kernel.py` for existing production
   stages and the installed `producer.selector_getter` operation.

## Next task: same-candidate prepared -> packet -> damage

Start by wiring the retained prepared-record producer and explicit action inputs
for these two candidates into the actual selector/getter, builder/copy and damage
kernel. Recompute candidate-sensitive fields or prove their invariance. Do not
combine new STAT/D with captured packet+08=3082 without such proof, infer selected record from equal
prepared0/1values, or replace the prepared selector with active-category401B34.

`2B5322` is currently an **unexecuted integration prerequisite**, not an observed
first missing reader. Run the chosen path and report the real first unsupported
read/producer. Do not request an arbitrary384-byte object as if it were already
a consumed dependency. Keep both declared candidates and the independently
evaluated fixed reference; do not drop an unknown alternative to certify the
original query. Separate numerical closure, legality and optimizer admission.

Native production has194ordered submissions/35checkpoint groups/7,425writes
through the checkpoint before5565E2, not one continuous complete factory replay.
The scoped default0x88descriptor is constructed for the2B595A..2B597Acaller;
historical hit membership in that caller is unproved. Of5,888selected cache
WORDs,5,000final zeros are proved on normal return;888remain unknown. Missing
guardian node+24at374C41, skill bucket at557031, manager598/5A0bindings and
global passive indices/unlock holder retain their own exact roles/conditions.
Do not redo the available level/attribute/rank/counter/feature census.

The separate fresh861calibration uses process63048/capture791ab043...02ec,
HP77730->76869, and stops after4,462attempts at78C5E7reading source status6F's
8-byte link. Computed damage and normal MAIN return remain absent. Candidate
items/profile use process30808/capture2e9b32cc...f5b1d. Never splice their heap
contents or use observed861as missing computation input. This calibration is
supporting work, not a universal gate on every independent output.

## Working preferences and stop boundaries

Use REA (`morluto/rea`) where it helps a specific native contract. Existing
byte-checked exports are available through the source/native-focus maps; this
acceptance did not launch a new REA session. Record real tool/version/command
use, not a claimed session. Reuse the existing original resolver and resources.

The owner prefers information-rich Pro packages for major unresolved work.
Keep them self-contained and independently verify decisive claims on return.
Do not spend another round producing only integrity reports or instruction counts.
Do not build a new broad collector before exhausting output-relevant existing
rules. No Computer Use. For any necessary runtime evidence, first prepare/test
a bounded observer; ask for a player action only once recording is actually
ready. No per-affix manual campaign or premature standby requests.

No game/CE/CUA session or target/save write was used for this acceptance. No
release or shipped product change is authorized. Existing exhaustive/random/
set-pattern/PG/CEM/BnB lab remains available; native scoring comes before another
solver/GPU investment. Incomplete bounds/candidate domains give no optimum gap
or certificate. AA-agility, minimumHPand every legitimate inventory/generation
requirement remain part of the final scope.
