# Protective pause: damage research and loadout optimizer (2026-10-15)

State: RESUMED on 2026-10-15 after explicit owner instruction conveyed by the coordinating task. The historical pause details below remain as evidence.
The resume instruction has now been received; the three bounded lanes are active again. A coordinating task conveyed the
owner-authorized protective pause to avoid consuming the next quota window.
All three active child agents were interrupted through formal collaboration
controls: lab_experiments_v2, lab_v2_review, r9_code_audit. No new tests or research
were started after the pause. Preserve all existing sources and artifacts.

## Implemented optimizer work

Source: research/loadout_optimizer/. Added policy_gradient, cross_entropy,
integer-only branch_and_bound, independent verify-result, incumbent trajectories,
stress fixtures and multi-budget experiments. Native/full-formula/legality flags
remain false. Existing35 E2E cases plus45 new cases form80 total. Development
passes and independent small enumeration succeeded; final stable combined replay,
large experiments, packaging and relocated verification were in progress when
interrupted. Do not claim a final v2 ZIP or completed large run without inspection.

Primary work directory:
D:/Nioh3_v080_deliverables/deliverables/loadout-optimizer-lab-v2-20261015/.
Finalization lane: finalization/ (benchmark, experiments, review, delivery).
The prior v1 delivery under loadout-optimizer-lab-20261015 stays immutable.
Root had frozen CONTRACT.md and LOADOUT_OPTIMIZER_LAB.md before finalization.
Packaging tools were being extended by lab_experiments_v2; inspect their current
state and existing receipts before resuming or rerunning anything.

## R9 acceptance and new evidence

Received ZIP D:/Downloads/nioh3_r9_core_prepared_delivery_20261015.zip:
179152026 bytes, SHA256
658d7fecb3fd4dc8b05cccedecff826121ac09dec672bfe8604998eb6bb69da6.
Audit root:
D:/Nioh3_v080_deliverables/deliverables/damage-pro-return-audit-20261015-r9/.
Received source is immutable under received/nioh3_r9_core_prepared/.
provided-replay-receipt.json confirms all11 supplied runs and157 payload hashes
before/after. Fresh caches retain nested original R9 and R8 inputs.
independent-code-audit/ reports13835 independent checks,212 complete384-byte
poststates,96 whole-function VM comparisons and16 owner/slot controls. This is
native-byte/interpreter and independent arithmetic evidence, not game hardware
execution or complete actor/legality closure.

The missing CRT index8 was read from the approved live module, without Computer
Use, CE, target writes, native calls or save access. crt-runtime-capture/ contains
identity and raw4bytes 5d43b900 -> RVA0xB9435D. Reader signature/process birth/EXE
hash/repeat reads matched. Total target data read140bytes. GAME_CLEANUP.json
confirms the task-launched game exited normally; no force termination.
The same bytes are in the EXE _RDATA section at file offset75602544. The earlier
section export omitted _RDATA. Disk.text remains encrypted/different from loaded
instructions; retain the exact data-vs-code provenance distinction.

Derived source: working/nioh3_r9_extended/. Added dispatch_evidence.py, actual
read-only VM mapping at0x50DB070 and guarded high-level branch support. The old
missing_copy_entry CLI example now succeeds (crt-restored-example.json).
Original received source remains unchanged. Extension-wide mask/order/native
regression was in progress in independent-code-audit/crt-extension/; do not assume
completion. Review the final or partial artifacts before continuing.

## Remaining direct work

The owner explicitly suspended Pro handoffs until they say to restore that flow.
Continue direct analysis only after explicit resume. source-frontier/ is the
other child lane: actual ordinary active0x55764C and reserve0x5557DC collectors,
feeding0x557BA0 in native order. Side-key paths and manager+0x138 producer contracts
are separate unresolved dependencies. No source no-op or final actor closure is
allowed. Root had inspected caller0x818B68 and post-stage0x8175F4 only; those were
not newly implemented. Source collection, intermediate actor state, complete
final damage/HP, Ki/Break and legal candidate domain still require work.

## Resume procedure

1. Inspect this note, current git status and existing lane receipts/logs. Check
   whether any interrupted child command finished or left partial output; use
   fresh output directories when replaying. Do not discard prior evidence.
2. Finish v2 finalization and verify its actual artifact before claiming delivery.
3. Finish CRT extension controls, then integrate ordinary source collection only
   with exact poststate/write-order parity. Continue actor producers afterward.
4. Update acquisition/research documents and the failure ledger for substantive
   integration failures; preserve all unresolved flags.

All Python must run through tools/run_python_tests.ps1, with
PYTHONDONTWRITEBYTECODE=1 before imports and D:/Nioh3_v080_deliverables temp/output.
No game/CE/CUA is needed for the current remaining offline controls.

