# Semantic observations before further damage reverse engineering

The owner proposed a memory-to-game-content translator on 2026-10-17. Adopt
that layer as the data entrance to the existing native evaluator and optimizer.
The product milestone remains a real active equipment contrast with damage,
HP, agility and toughness produced for the same candidate. Translation does
not replace the missing rules, and native MAIN instruction coverage is not a
product completion measure.

## Decision status and falsification

On2026-10-17the owner clarified that this was an intuitive suggestion, not an
architectural requirement that the analysis must endorse. The thin semantic
entrance is a working direction supported by the identity/alias/phase problems
already observed. Its correctness as a data decoder is tested; superiority as
the fastest route to a full evaluator is still an inference. Pro should audit
that distinction and may reject or narrow the proposed direction.

Keep the translation layer small and reuse the existing parsers/IR/catalogs.
Its useful acceptance is that the next scoring slice consumes explicit semantic
objects with traceable provenance and fewer unresolved identity joins. Do not
count more labels, schema fields or decoder tests as scoring progress. Stop
expanding the adapter when it no longer removes an output-relevant blocker;
complete the shared production/calculation rules instead. A source/descriptor
gap remains a rule or evidence gap even after it has a good semantic name.

R15 has now independently selected the existing native provider plus the thin
verified input bridge, and implemented an exact two-chest fixed-scene HP/rank
slice. The [local acceptance and Claude handoff](NIOH3_DAMAGE_CLAUDE_HANDOFF_20261017.md)
own current source, repairs, evidence and the next prepared/packet/damage task.
Do not expand this historical observation proposal into a generic engine
translator or a second runtime as a prerequisite for that task.

## Responsibilities

```text
Recorded memory / resource tables / supported save records
                |
        Versioned observation adapters
                |
   Character + item occurrences + placements + action/state observations
                |
       Existing semantic IR + executable RuleCatalog
                |
   Same-candidate source / actor / prepared attack / damage evaluation
                |
         Existing constrained search and result verification
```

The adapter answers what an observation contains, which objects it belongs to,
and which phase was actually observed. It exposes exact IDs, raw fields, name
sources, masks, aliases and unresolved semantics. It does not call a late total
a non-equipment base, make an allocation bound a backpack occupancy count, or
infer an affix's unit from its label.

The rule layer answers what a different legal loadout would produce. It owns
source filtering and ordering, aggregation, sets, active-weapon selectors,
conditional states, caps, thresholds, binary32 rounding and the final consumers.
Reading an observed attack or damage number does not implement that transform.

These are separate interfaces, but they reuse the existing optimizer IR,
all-effect catalog and native rules. Do not create a second optimizer, another
blank intermediate representation, or an unrestricted whole-engine emulator.

## Implemented first adapter

[research/game_state.py](../../research/game_state.py) is a pure-Python,
read-only offline adapter for the reviewed PC2.02 executable and loaded-text
identities. It operates on one explicitly selected `MAIN_entry_prestate`
snapshot. It has no game attachment, target calls, CE, UI automation or writer.

It independently decodes the saved raw regions. It does not import the R14
provider, its VM or its precomputed inventory/profile results. The latter are
used as the separate E2E comparison target.

Current output schema: `nioh3-game-observation/v1`.

| Output | Implemented boundary |
| --- | --- |
| Process and epoch | Executable/text profile, PID/birth/module, arming identity, unique snapshot and matching MAIN-entry event sequence. |
| Player | Current-player global manager/slot chain, source object, source+3A0 owner and reciprocal owner backlink. |
| Character | Recorded level and seven native allocated-attribute slots; slot names and authenticated clean source basis remain unresolved. |
| Items | Pool/index occurrence IDs scoped to capture **and snapshot**, raw bytes/masks, item key, quantity, level, plus and rarity code. |
| Placements | Two native equipment banks, index guards, source/pool copy comparisons and separate actual E8 equipped records. Shared instances stay shared. |
| Active category slot | The recovered 401B34/401B54 bank/selector choice; this does not identify the later prepared attack record. |
| Affixes | All seven raw slots retained per observed item, exact effect IDs and values, metadata and optional localized name hints. Unknown remains null. |
| Numerical semantics | Display grade, formatted value, unit and executable calculation rule remain null until their relevant catalog/rule contract is established. |

Native copy evidence is `552DD0..552EF8`, SHA256
`60b578243ecdedcdac3dc01d2318a187380db3e27cc44688d3b74582e2cf0e84`.
Only `[0,24)` and `[28,E4)` are copied. The two padding holes and the separate
pool/owner record extents must not be compared or substituted as identical.

The shipped multilingual effect catalog has 3,609 IDs and declares version
`2.00.02`. Its exact-ID labels can be attached as **name hints with that source
version**, without claiming current-version formula, applicability or display
format equivalence. Item names and unresolved templated labels are not invented.

The initial real observation contains **20 unique occurrences and 22 resolved
placements**, recorded level 450, and active category bank0/slot1. It is the
older process30808 capture SHA `2e9b32cc...f5b1d`, not a new live read. The newer
process63048 capture SHA `791ab043...02ec` is used only as a different-lifetime
negative control; its heap state is never merged into the older observation.

## Repeatable evidence

The authoritative run is
[semantic-e2e-03](D:/Nioh3_v080_deliverables/deliverables/damage-pro-return-audit-r14-20261017/semantic-e2e-03/RECEIPT.json).
Its real baseline produces
[readable state](D:/Nioh3_v080_deliverables/deliverables/damage-pro-return-audit-r14-20261017/semantic-e2e-03/real-baseline/output/state.md)
and
[machine-readable observation](D:/Nioh3_v080_deliverables/deliverables/damage-pro-return-audit-r14-20261017/semantic-e2e-03/real-baseline/output/game_state.json).
Earlier `semantic-state-01` and `semantic-e2e-01/02` are development snapshots,
superseded by the final source and run above.

[The CLI E2E verifier](../../tools/verify_game_state_translation_e2e.py) passes
48 checks. It compares
every raw pool record and every actual E8 equipped record against R14's separate
raw revalidation, verifies the profile and original selector result, preserves
20/22 aliasing, and checks deterministic output. Explicitly mutated parser
controls test wrong versions/processes/player identities, epoch mismatch,
missing pointers, copied-field disagreement, allowed no-write padding, unknown
effect values and conflicting bank rereads. They are parser controls, not game
or numerical-formula controls.

Run the translator through the project Python wrapper, using a fresh output
directory on the build root:

```powershell
& F:/Nioh3_ScrollEditor/tools/run_python_tests.ps1 `
  -ScriptPath F:/Nioh3_ScrollEditor/research/game_state.py `
  -ScriptArgument @(
    '--capture', 'D:/Nioh3_v080_deliverables/deliverables/nioh3-r10-pc202-20261016/evidence/audit/fixed-scenario-e2e-20261015/coherent-live-01/capture-ordinary.json',
    '--effect-names', 'F:/Nioh3_ScrollEditor/nioh3_scroll_editor/data/effect_names_multilingual.json',
    '--locale', 'zh-CN',
    '--output-dir', 'D:/Nioh3_v080_deliverables/deliverables/game-observation-new-run'
  )
```

Success returns `game_state.json`, `state.md` and a source/input/output-hashed
`RECEIPT.json`. Unsupported input returns exit3 plus `REFUSAL.json`. No synthetic
damage, unknown-as-zero or scoring fallback is supplied. All original capture
and R14 files remain unchanged.

## Integration and next priorities

1. Reuse this observation entrance and add parameter/localization joins using
   exact native key namespaces and row provenance. Preserve unknown units,
   effect grades, action/state roles and unmapped IDs until resolved.
2. Establish actual backpack membership and observed/allowed placements before
   claiming an owned search domain. Current 2500/400 bounds are not occupancy.
3. Connect only authenticated source phases and typed shared native rules.
   First finish the retained full factory/default-descriptor rules or collect
   its narrow missing descriptor/phase. Do not restart another broad census.
4. Build one baseline and one legal active variation through complete candidate
   production and the existing registered evaluator; then run the existing
   small-domain exhaustive baseline and independently verify it.

The observation namespace is intentionally stronger than R14's pinned
`snapshot:<capture>:<pool>:<index>` occurrence namespace. A native bridge must
explicitly crosswalk the exact capture/snapshot/raw equipped record. It must
not rename a generic observation ID into a trusted provider ID or slice a pool
record and assume its padding equals the actual owner record. That bridge and
repository installation of the returned native evaluator are still pending.

Limit reverse engineering to specific semantic gaps: layout/ownership, shared
collectors, formula operators and output-relevant dependencies. REA remains
useful for those contracts. Do not make inspecting every unrelated engine
function, capturing every writer or reaching MAIN RET a universal prerequisite.

For an update, retain separate version profiles, original binary/resource
hashes, site/layout witnesses, raw observations, row-key maps and E2E artifacts.
Compare and revalidate the changed adapters/rules. Do not automatically reuse
offsets or start the entire discovery again.

The complete formula, full backpack, legality, real gear-to-score chain and
optimizer readiness remain unproved. A useful observation layer reduces the
amount of address plumbing in every later task; it cannot infer missing
counterfactual damage rules from a memory dump.
