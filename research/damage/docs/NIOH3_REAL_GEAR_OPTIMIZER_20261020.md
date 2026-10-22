# Real equipment evaluator to optimizer, PC2.02

Later same-day continuation: the
[actual new process collection](NIOH3_REAL_GEAR_LIVE_COLLECTION_20261020.md)
supplies the2A0/default role and a fresh714/720 owner contrast. Independent
current-profile production matches70 STAT bytes, both2816-byte D tables and
both SOURCE maps/counts. This does not change the pinned two-state CLI domain
below or promote its observational packet admission to complete causality.
Read the newer collection record before treating the initial input frontier
below as a current-game request.

## Accepted implementation boundary

The existing optimizer CLI now dispatches to `r16-observed-helmet-v1`, an
actual offline native-backed evaluator for exactly two retained same-character
states: the empty helmet slot and the observed equipped `6DE6` helmet. It no
longer requires synthetic damage coefficients for this bounded domain.

This is a working calibration slice, not admission of arbitrary unseen gear,
all owned inventory, legal generation, other characters/actions, or a released
optimizer. The common S0 damage packet is admitted by **observed paired
invariance**, not a complete causal packet constructor. All broad game-formula,
equipment-legality and product-readiness flags remain false.

The task delivery root is
`F:/Nioh3_r16_build/deliverables/r16-real-optimizer-20261020/`.

## One candidate, one numerical state

`native_gear_metrics.py` pins the S0 capture, original 232-byte item record,
native body/table inputs and existing producers. It derives the item body,
selected D and independent core D. Both SOURCE maps and counts are changed
with the candidate: 60 baseline rows become 64 helmet rows. The selected
serialized SOURCE is independently rematerialized and compared with D.

`native_damage_bridge.py` inserts those candidate images into the retained S0
request, erases stale aliases, checks known masks and SOURCE ownership, executes
the original MAIN path to normal return, and checks the readable formula and
initial-score-erasure control. Unmodeled reads refuse scoring. Candidate
changes are external evidence files; the strict native request field contract
is preserved. S1 is an independent comparison oracle, not a scored input.

| Metric | Empty slot | Observed helmet | Bound |
| --- | ---: | ---: | --- |
| Damage | 714 | 720 | Original MAIN and readable formula agree |
| Maximum HP | 3809 | 3809 | Same-profile observed invariant and bounded unchanged HP dependencies; not a pristine full-actor rebuild |
| Scenario current HP | 3809 | 3809 | Explicit `full_hp` policy, not a current live observation |
| Weight | 0 | 47 | Native item body and candidate STAT |
| Capacity | 566 | 566 | Fixed profile |
| Toughness | 0 | 50 | Native item body and candidate STAT |
| Raw agility | 80 | 80 | Code-equivalent native integer branch; no new letter-rank claim |

Only the retained action25036/stance1/unbuffed/full-HP/S0-training-samurai
scenario is admitted. Human rank requirements, changed actions, unknown items,
extra synthetic effects and conflicting input identities refuse rather than
becoming zero damage or an infeasibility claim.

## Search and independent verification

The compiler, evaluator, exhaustive search and result verifier use the same
registered adapter. Damage improvement is the exact fixed-reference fraction
`720/714 = 120/119`. Exhaustive search visits both supplied assignments;
independent verification starts another CLI process, enumerates the compiled
domain again, reproduces candidate calculations and compares semantic witnesses.
Evidence paths and wall timings are excluded from those semantic witnesses.

The strongest accepted scope is `optimal_for_supplied_domain` or
`infeasible_for_supplied_domain`. A complete two-entry domain is not a complete
backpack. Synthetic branch-and-bound bounds cannot certify native candidates.

The actual CLI E2E harness is
`tools/verify_native_gear_optimizer_e2e.py`. Final `e2e-03/E2E_RECEIPT.json`
passes22 actual CLI calls and79 checks, including unchanged implementation-source
pins, current-HP policy and unknown nested slot/domain/constraint input refusals.
It retains every request, command,
stdout/stderr, response and checksummed receipt. It verifies that a weight<=0
constraint selects empty despite its lower damage, toughness>=51 yields no
feasible candidate, independent certificates reproduce, and tampering and
unsupported inputs are rejected. The existing 80 synthetic CLI E2E tests also
pass after final dispatch hardening; see `synthetic-regression-final.xml` in the delivery.

To reproduce in PowerShell7 from the repository, use a fresh output directory:

```powershell
$env:NIOH3_BUILD_ROOT='F:/Nioh3_r16_build'
& ./tools/run_python_tests.ps1 -MinimumFreeGiB 0 `
  -ScriptPath 'F:/Nioh3_ScrollEditor/tools/verify_native_gear_optimizer_e2e.py' `
  -ScriptArgument @('--domain-manifest',
    'F:/Nioh3_r16_build/deliverables/r16-real-optimizer-20261020/numeric/domain/domain_manifest.json',
    '--output-dir','F:/Nioh3_r16_build/deliverables/r16-real-optimizer-20261020/e2e-reproduction')
```

## Remaining causal input frontier

The final four attack writers and both 16-byte packet stores are derived in
`damage/packet-tail/DERIVATION.md`. Original suffix execution and private
controls pass, but they do not establish the real constructor prefix. A longer
native path crosses the previously missing parameter130 resource using its
authenticated immutable role, then reaches the missing parameter2A0 role.

The saved-input audit checks S0 snapshots/fill data and 173 capture receipts.
SOURCE+B426, query+50 and manager+2A0 lack the required bytes. Missing bytes
cannot become a null pointer, an empty table or a native default. Other-process
mutable actor data cannot fill the historical S0 epoch.

Native2A0 lookup uses two **sorted pointer vectors**, signed byte selectors at
row+12/+13, and u16 result at row+0C. It is not the parameter130 hash layout.
The bounded acquisition implementation and merged plan live under
`collection-prep/` and `damage/constructor-input-audit/`. These are preparation
for one coherent fresh-session capture, not evidence of live acceptance.

The constructor takes four logical inputs (destination packet, input context,
mode, action). Old register/XMM/stack dumps are unnecessary. Its destination
can be reused and has consumed prestate, so MAIN output bytes cannot substitute
for constructor entry bytes. Caller selection, source/query paths, destination
prestate and any needed getter context must be recovered or collected coherently.
Any newly executed missing dependency remains a typed refusal.

The prepared collector passes28 actual file-backed CLI checks. The native2A0
helper has five private original-instruction controls, selector preflight has
seven private controls and its actual saved-S0 application has48 known bytes.
These checks remain separate from live acquisition acceptance. The merged owner
protocol is `NEXT_MERGED_COLLECTION.md` in the delivery.

No game was opened or attached, and no debugger, game function, game/save write,
UI change, release, publication or external Pro submission occurred in this pass.
