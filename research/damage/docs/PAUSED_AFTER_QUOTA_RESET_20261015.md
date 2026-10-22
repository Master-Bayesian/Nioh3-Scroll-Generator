# Pause after observed quota-window reset (2026-10-15)

State: RESUMED after the owner explicitly authorized continuation with GPT-6.1 Sol / Max. The quota API changed from primary used54%, reset1792501630 to
used10%, reset1792649618. Ordinary usage is still allowed; this is not a claim
that the new window is exhausted. Work stopped to protect the new quota window.
This is a different event from the earlier premature protective pause, which
had been rescinded. Wait for explicit owner authorization to use the new window.
Do not automatically restart agents or continue toward exhausting new quota.

All three running child agents were formally interrupted: lab_experiments_v2,
lab_v2_review and r9_code_audit. No new research/tests were started afterward.
No root-owned long command remains running. Preserve interrupted outputs before
any replay. The game had already exited normally after the last parameter capture;
ACTOR_GAME_CLEANUP.json records it. No CE/CUA, hooks, native calls or writes were used.

## Completed immutable deliveries

- Optimizer lab v2:23,307,137bytes,3045recorded searches,80E2E plus relocated replay.
  D:/Nioh3_v080_deliverables/deliverables/loadout-optimizer-lab-v2-20261015/finalization/delivery/loadout-optimizer-lab-v2-20261015.zip
  SHA2567076d181ba7cce10adba314d088c2a63cbd6018ac1e2fba08707337d041e3ea2.
- First native extension checkpoint:185,764,632bytes, actual extraction/1025hashes,
  relocated205parent/9CLI,107actorCLI and19152CRTchecks pass.
  D:/Nioh3_v080_deliverables/deliverables/damage-pro-return-audit-20261015-r9/packaging/delivery/nioh3-r9-local-native-checkpoint-20261015.zip
  SHA256e89f1c00a59c0e31948c474a7fcd3d601448525f038dbe695403803d358fc896.

The first working model working/nioh3_r9_extended and both sealed packages remain
unchanged. Further development is in working/nioh3_r9_stage2 under the audit root:
D:/Nioh3_v080_deliverables/deliverables/damage-pro-return-audit-20261015-r9/.
Stage2 is NOT yet a sealed or integrated full-actor delivery.

## Latest stage2 source and acceptance

1. Side collector integration is complete in stage2/src/r9_model.py,
   r9_native.py, source_collectors.py, side_sources.py and side_bindings.py.
   side-source-frontier/parent-integration/{FINAL,FINAL-OLD205,FINAL-CLI}:
   146new whole-parent cases,205old cases+9CLI,6sideCLI pass. Side state retains
   opaque trailing stack padding via known masks; no zero invention. Fixed-slot
   u16counts are modeled beyond7; free-slot count>7 remains explicit unsupported,
   not a legality restriction. The recorded native parent padding varied across
   histories, proving a fixed zero fill would be wrong.
2. set_expansion.py is in stage2/src and separately accepted:420native cases,
   13CLI, all91actual side definitions,1011aggregate submissions and3444writes.
   A full ordinary map can reject insertion while the native cross-family reward
   still sets7D1. 38BA is tested by presence, includingzero/negative values, cached
   once per family. Actual table has a CB4B reward; dormant D7BF/flag1 controls are
   explicitly synthetic. See set-expansion-frontier/final-evidence/ACCEPTANCE.json.
3. body_stage.py is in stage2/src. body-source-integration had52CLIcases pass,
   including42whole2FB2BC VM comparisons and10negative controls. It reuses accepted
   R8 aggregate arithmetic, adds nine per-slot defense stores, and compares all
   2048stat bytes, call ordering/interleaving and ordinary/side source poststate.
   Final compact receipt/provenance writeout was interrupted or just finishing;
   inspect before claiming a final package. Raw object identities are normalized
   offline ABI inputs with extent/nonoverlap checks, not product address inputs.
4. slot15_stage.py and base_actor_parameters.py are new root-owned stage2 modules.
   slot15-source-frontier/controls-05:128whole81744C comparisons, ordered numeric
   writes and an observed active-source checkpoint at1A1EC8B. Active source trace
   now deep-copies its poststate before later reserve mutation. controls-01/02
   preserve decoder-width/R12 fixture failures; controls-04 predates trace repair.
   base-actor-frontier/controls-01:160whole375D20/2048byte/order comparisons.
   root-stage2-cli-02:10publicCLI controls pass. root-stage2-cli-01 is a retained
   development mismatch after source-scope metadata changed, preceding trace repair.
5. actor_local_stages.py (first checkpoint) closes17factor defaults/optional raw
   override and8175F4 explicit context deltas. actor-rebuild-frontier separately
   closes37574C/8731E4reset and attribute initialization with real guards;214cases
   and9CLI pass. Existing R6 synchronization arithmetic is reused, not rederived.
   Its integration into the full prefix remains pending; cap helper5507E4 and
   network-mode1peer producer1C9A088 remain explicit frontiers.

## Interrupted active lanes

- lab_v2_review: ordered-rebuild-integration and NEW stage2/src/rebuild_chain.py.
  Join actual caller slice81910F..819192: prepared -> body -> slot15 -> set
  expansion -> conditional7D1clear/refresh ->8175F4 context deltas. Actual native
  contiguous calls must execute; typed initial style/source/context identity stays
  explicit. It was still in progress; no complete chained acceptance is claimed.
- lab_experiments_v2: final body integration receipts/ABIproofs, then available for
  subsequent work. It owns body_stage.py only among current stage2 model modules.
- r9_code_audit: special-weapon-frontier and NEW stage2/src/special_weapon.py.
  Main40000scalar branch passed4200wrapper vectors and10925checks, including
  all3362items and34special970keys. BUT817DA8/817EA4 record+10/+14 also have special
  branches. Their auxiliary implementation/verification was still in progress.
  Do not remove the parent flag40000 refusal yet. Key nuance: main cap link/key
  comes from rarity-selected row; auxiliary link/key comes from original row,
  while auxiliary base comes from selected row. Module may contain partial edits.

## Additional verified parameter data

actor-runtime-capture contains native key/index maps and raw rows for:
manager20099rows/stride74, manager3F052rows/stride100, manager11884rows/stride54,
manager97034rows/stride1C. All EXE/code/process/item-epoch/reread checks passed.
Stage2 copies are evidence/actor-tables/capture; actor_tables.py validates them.
ACTOR_RESOURCE_JOINS_RESOLVED.json proves all four recoverable from S1 ranges:
01208,00482prefix,00186,00216respectively. Guardian00482 has13320table bytes followed
by6562uninterpreted trailing bytes; the earlier whole-file negative is retained.
Side manager13891rows is exactS1payload00227. CRTindices4/8/12 are measured.
All acquisition scripts, exact source metadata and cleanup receipts are preserved.

## Next safe continuation

Read current sources, interrupted lane outputs and
`docs/research/NIOH3_DAMAGE_DIRECT_CONTINUATION_20261015.md` before changes. Resume
only the same bounded tasks after owner approval. Finish special auxiliaries before
parent integration, complete the ordered native chain, independently verify it,
then prepare a second self-contained checkpoint without altering the first one.
Current full-formula, final-actor, full-source-lifetime, native-legality and game
optimizer-ready flags remainfalse. Source closure, final damage/HP, Ki/Break,
actions and complete legal/owned candidate domains still need work. All3609effects
and1152groups remain in scope; no example effect has priority. Pro handoffs remain
suspended until the owner explicitly restores them.

Use PowerShell7 and tools/run_python_tests.ps1, PYTHONDONTWRITEBYTECODE=1 before
imports, D:/Nioh3_v080_deliverables temp/output, fresh replay directories. Do not
stash/reset/clean the shared checkout, discard failed evidence, or run CUA.

