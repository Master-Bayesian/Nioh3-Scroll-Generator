# Offline parameter acquisition and version-update method

Status: accepted for the PC 2.0.2.0 corpus on 2026-10-14 (America/New_York).
This records how evidence was obtained, how to reproduce it, and what must be
revalidated after an update. It is not product compatibility or formula approval.

## Sources and acquisition lineage

There are two different acquisition paths. Preserve both; do not relabel a runtime
capture as an extracted resource or a later packaging date as the capture time.

1. Original native controls were read with Python `ProcessReader`, using
   `PROCESS_QUERY_INFORMATION | PROCESS_VM_READ`; no CE or game/save writes.
   `research/dump_r4_finalizer_tables.py` is named by the original capture attempts.
   Original root: `D:/Nioh3_v080_deliverables/deliverables/combat-data-intake-20261012/runtime/run-20261013-pid21384`.
   `tables/manifest.json` records capture UTC `2026-10-13T02:25:49.044101+00:00`,
   PID21384, EXE 2.0.2.0, 77,830,112 bytes, SHA256
   `e22c4a635e4ec1e27a177b76e27d7f6a637f426c0ed3928b60f5693bc52ae130`.
   Its SHA256 is `dd18be2adae4fab68a6866b2382dfbf01d93ede1adae94fab95165b0e514d357`.
   The parameter-manager pointer RVA was `0x45B9E30`. A fresh same-session section
   profile was necessary because the original profile referenced unavailable files.
   The profile remains candidate / `product_enablement_allowed=false`.
2. The 14 families comprise 13 rectangular stores and one indirect bonus store.
   All715table files carried into the R7 controls match the original files and
   their original manifest hashes. The earlier R2 builder copied `tables/tables`
   but omitted its outer manifest/run/profile. Those small provenance files are
   now preserved in the method record described below. The historical collector
   source hash was not recorded; the current named collector is only a current
   snapshot, never claimed to be the exact historical source revision.
3. base70 and power90 came from separate read-only Python collectors under the
   R8 `input-r6/evidence/research/live` tree. Selected directories are
   `base70/20261013T075742080077Z` and `power90/20261013T072755168375Z`.
   These are UTC collector directory timestamps, not JSON completion timestamps.
   Both record PID18464, the same EXE digest, exact signature checks and reread
   equality. Preserved `capture_weapon_base70.py` / `capture_power90.py` explain
   manager offsets0x70/0x90. R6 alias provenance maps the headers/rows to them.
4. New disk data was read from `D:/Steam/steamapps/common/Nioh3/archive` without
   a game process. Format reference: [MIT RDBExplorer, pinned commit](https://github.com/MrIkso/RDBExplorer/tree/22e72d8768092f8411480aaa7b8218c5003c8348).
   The relevant reference files are `Templates/hioch3_bin.hexpat`,
   `hioch3_lfm_order_bin.hexpat`, `hioch3_load_inf_bin.hexpat` and
   `RDBExplorer/Utils/CompressUtils.cs`. Pinned source and license are in S1.
   Only source was inspected; no upstream EXE, GUI, game hook or repacker ran.

The separate `package/root.rdb` and `system.rdb` / RDX indexes contain310,594entries.
Their indexing helped inventory candidates, but the accepted parameter extraction
uses the **LFM archive route**. IDRK payload decoding was not exercised. Do not
restart broad RDB/fdata extraction when the parameter archive already answers a query.

## Accepted files and format

Current baseline: Steam build25297068, EXE digest above. Archive03 is7,139,328bytes,
SHA256 `9b1ca4d7fcf317d3fec73bef5098bd146499dc11034bf412b4baa5b43edba2ad`.
Companions are `lfm_order_03.bin` (102,353bytes, SHA256
`d7e60a8e3b157f142df16dd122739a12229bb275ee38b90f66732c5207ee21cf`) and
`load_inf_03.bin` (15,544bytes, SHA256
`19e63893b8a2d9f2fb4e862dc6f131a07149c879bbc3b7af665bdf87ab56c579`).

- LFMB: little-endian32-byte `<4sIQQQ>` header: magicLFMB, version0, count,
  full archive size, alignment4096. Each directory entry is32-byte `<QQQQ>`:
  payload offset, decoded size, stored size, compressed0/1.
- LFMO: `<4s7I>` header, magicLFMO/version1, count, header32, entry array at40,
  root/name offsets and reserved0. Entries are `<III>`: raw field, ordinal,
  UTF-8 NUL string offset. Hashed resource paths are not text IDs or native keys.
- load_inf: u32count followed by12-byte `(ordinal, decoded size, compressed)`
  entries. The three files must agree, including every row's declared sizes.
- Raw payloads have equal stored/decoded sizes. Compressed payloads have aligned
  chunks: read oneu32word; unsupported upper16bits reject; bit15 means zlib,
  low15bits are **stored length**, otherwise a literal chunk. Decoded chunks
  are at most16,384bytes. Next chunk is aligned to16 with zero padding. Verify
  exact EOF, bounds, decoded total, and padding rather than accepting partial data.
- The corpus includes952zlib chunks and one literal chunk. Entry235's first word
  `0x8A91` has stored length`0xA91`; entry523 has an8-byte literal tail. Both are
  required controls. Initial decoded-length and zlib-only assumptions failed and
  remain recorded in `EXPERIMENT_FAILURE_LEDGER.md` and S1 tool-failure evidence.
- Accepted extraction:1,295payloads /14,810,175decoded bytes. For each, retain
  ordinal, archive offset, hashed resource path, sizes/codecs and both stored/
  decoded SHA256. Embedded resource strings never become output filesystem paths.

## Table identification, not filename guessing

Compare full bytes with the previously version-bound native controls, not just
counts, a header, a plausible scalar or an upstream type label. These are the
accepted current-version mappings; ordinals and manager offsets can change.

| Family | Manager offset | Rows | Stride | Archive03 ordinal |
| --- | --- | ---: | --- | ---: |
| rarity_roll | 0x9B0 | 6 | 0xF8 | 58 |
| item | 0x68 | 3,362 | 0x1A0 | 235 |
| category | 0xB0 | 17 | 0x6C | 259 |
| effect_group | 0xA8 | 1,152 | 0x70 | 278 |
| auxiliary_terrain | 0xA88 | 20 | 0x34 | 544 |
| auxiliary_rule_conflict | 0x20 | 29 | 0x18 | 582 |
| auxiliary_enemy_candidate | 0xA80 | 487 | 0x1C | 740 |
| effect | 0xC8 | 3,609 | 0xD8 | 758 |
| level_curve | 0xC0 | 501 | 0x0A | 772 |
| special_context | 0xA98 | 7 | 0x30 | 791 |
| scroll_special_rule | 0xAA0 | 301 | 0x38 | 1116 |
| category_count_multiplier | 0xB8 | 6 | 0x20 | 1137 |
| base70 | 0x70 | 502 | 0x94 | 1141 |
| power90 | 0x90 | 1,000 | 0x20 | 382 |

optional_multiplier (manager0x230) is2,954rows of0x20bytes spread over ordered
bodies `[1216,512,1121,855,902,804,1239]`, counts
`[1738,150,185,191,34,7,649]`. Common four-byte prefix plus summed count constructs
its8-byte aggregate header. Header plus bodies equals94,536captured bytes, SHA256
`df3f3f261b84c416f12dc9433bfc5427069b9e6e1ac9528eb1f4d01a3b8b7eea`.
Whole-file concatenation is wrong. Exact byte reconstruction does not prove the
loader's procedure, source order or reachability; those still need native code.

bonus_curve (manager0x718) is indirect object/pointer/per-row storage.532/662captured
byte sequences occur in payload1119;130remain unresolved in this corpus. Do not
flatten it, infer zero for a missing sequence, or invent a pointer relocation.

Native lookup keys, row indices, row-leading words, resource hashed names and
localization IDs are separate namespaces. Use captured hash entries until a
loader/key-source contract is proved. All3,362item keys and2,954optional keys are
mapped, but optional row+0 agrees with zero native keys. ItemC171 maps to row797,
whose leading low16word is276F. Byte-identical rows alone do not certify a new
version's lookup index. Names, slots, natural legality and owned quantities remain
unknown where explicitly marked; a definition is not an owned or obtainable item.

## Reusable extraction and repeatable E2E

The durable tools are [extractor](../../research/extract_lfm_parameters.py) and
[real-file E2E driver](../../research/verify_lfm_parameter_acquisition_e2e.py).
They use the standard library; no game attachment or native game code executes.
Use the project wrapper, freshD: outputs and explicit game/source/version inputs.
`--game-version` is a research label; the required EXE digest pins actual input,
not product approval. Obtain the observed version/hash before assigning the label.

```powershell
$gameRoot = 'D:\Steam\steamapps\common\Nioh3'
$gameExe = Join-Path $gameRoot 'Nioh3.exe'
$version = (Get-Item -LiteralPath $gameExe).VersionInfo.FileVersion
$exeSha = (Get-FileHash -LiteralPath $gameExe -Algorithm SHA256).Hash.ToLowerInvariant()
$baseline = 'D:\Nioh3_v080_deliverables\deliverables\nioh3-resource-parameter-intake-pc202-20261014\delivery\nioh3-resource-parameters-pc202-20261014-s1'
$freshOutput = 'D:\Nioh3_v080_deliverables\deliverables\parameter-update-NEW-VERSION-RUN1'
& 'F:\Nioh3_ScrollEditor\tools\run_python_tests.ps1' -ScriptPath 'F:\Nioh3_ScrollEditor\research\extract_lfm_parameters.py' -ScriptArgument @('--game-root',$gameRoot,'--archive','3','--game-version',$version,'--expected-exe-sha256',$exeSha,'--reference-tables',"$baseline\evidence\native-tables",'--reference-game-version','2.0.2.0','--output',$freshOutput)
```

The extractor retains original header/directory, companions, stored slices,
decoded payloads and a v2receipt. It checks source/EXE identities before/after,
references, counts, overlap, bounded decompression and unsupported formats. A
failed run has no accepted receipt; partial outputs/logs remain for diagnosis.
Comparison with an older reference is only byte equality against that reference.

Same-version E2E command, using another fresh output:

```powershell
& 'F:\Nioh3_ScrollEditor\tools\run_python_tests.ps1' -ScriptPath 'F:\Nioh3_ScrollEditor\research\verify_lfm_parameter_acquisition_e2e.py' -ScriptArgument @('--game-root',$gameRoot,'--baseline-package',$baseline,'--output','D:\Nioh3_v080_deliverables\deliverables\parameter-method-E2E-RUN2')
```

The accepted run is `method-record/e2e-20261014/E2E_ACCEPTANCE.json` under the
resource-intake task root. It verifies3,365baseline hashes, extracts real source,
compares all1,295payloads, and uses the immutable independent S1 verifier to check
14whole tables,2,954partition rows and3,362catalog keys. Six codec/lookup controls
plus five new EXE/header/metadata/chunk file controls reject. Inputs remain unchanged.
This same-version driver must fail when a different dataset is supplied; do not
weaken it to make a new version pass. Establish a new reviewed baseline instead.

## Update decisions: reuse the method, revalidate changed contracts

1. Record new EXE version/size/hash, Steam build, archive/companion identities,
   tool revision/hash and run receipt. Never overwrite an accepted baseline.
2. Run the extractor on archive03 first. Do not assume current counts/ordinals.
   If parameters moved, inspect small LFMB/LFMO/load_inf metadata across archives,
   then locate by exact bytes/hashed names/shape as candidates. Shape/name matches
   alone are not accepted semantic joins. Reindex RDB only for a concrete need.
3. Classify unchanged, added, removed, moved and changed payloads. Diff decoded
   bytes/rows separately from compression/padding/order changes. Reconstruct any
   aggregate with explicit parts and compare full bytes, not only hashes/counts.
4. Revalidate key/row mappings, table loaders/readers and affected EXE consumers.
   Changed resources can decode successfully while meanings/formulas differ;
   unchanged resources can be consumed by changed code. Keep unsupported versions
   and unknown fields out of accepted optimizer inputs until those contracts close.
5. Reuse unchanged proven stages and rerun decisive boundary controls. Export only
   changed relevant native functions/data readers. If decoded data or code cannot
   determine a runtime-only relationship, plan the smallest read-only capture with
   an exact version/profile. Do not repeat per-affix gameplay or an entire process dump.
6. When format/identity checks fail, retain failure/log/raw bounded inputs and use
   an informed Pro handoff for the exact unresolved mechanism. Format support,
   consumer parity, candidate legality and full forward-model acceptance are
   separate statuses. This offline stage does not bypass the existing
   [product version-update gates](../knowledge/GAME_VERSION_UPDATE_PIPELINE.md).

## Durable evidence and active R8

S1 ZIP:6,887,489bytes, SHA256
`4ef1f4d655e6e1680b2e6b176f6ddf79a1924db20c8e720333f01ec3664ee424`.
It remains immutable. Its payloads/native controls/pinned sources allow offline
replay without the original installed directories. The resource-intake root is
`D:/Nioh3_v080_deliverables/deliverables/nioh3-resource-parameter-intake-pc202-20261014`;
`method-record/provenance` preserves original outer manifests/profile/collectors,
and `method-record/ACQUISITION_LINEAGE.json` pins copied-source identities.
No full EXE, assets or save is copied into that method record. The R8 continuation
text is [here](R8_RESOURCE_SUPPLEMENT_CONTINUATION.md). R8 retains its existing
work/priorities; S1 updates parameter availability, not formula completion.

## Optimizer architecture clarified by the owner discussion

The goal is the strongest feasible legal or owned loadout under explicit constraints.
Recover a typed, modular **forward evaluator**: raw instances and attributes ->
qualified sources/aggregation -> final stats/prepared attack -> action/target
damage and required Ki/Break outputs -> constraints/score. Record all affixes and
moves, and recover their relevant dependencies; unrelated game functions need
not be rewritten. Do not collapse evidence or widths into one opaque giant function.

Equipment, affixes, sets, counts and ranks form a discrete, conditional search
space. This does not exclude gradient optimization. Keep gradient-based and
gradient-free methods eligible; select by measured solution quality, feasible
solution rate and total latency on actual representative loadout problems.

- A continuous or [Gumbel-Softmax relaxation](https://arxiv.org/abs/1611.01144)
  can optimize categorical selection parameters. A relaxed average of items is
  not an equipped loadout: set thresholds, eligibility, rounding and source
  interactions need an explicit surrogate, followed by exact discrete validation.
- A stochastic policy over actual complete loadouts can optimize expected reward
  with policy gradients. [Neural Combinatorial Optimization](https://arxiv.org/abs/1611.09940)
  supplies a primary-source example on discrete combinatorial problems. For a
  differentiable distribution p_theta on a fixed finite feasible domain,
  grad E[f(X)] = E[(f(X)-b) grad log p_theta(X)] with an action-independent baseline.
  The reward evaluator itself need not be differentiable. This makes exact game
  branches, lookup tables and integer rounding compatible with the search method.
- Constraint-aware sampling/masks and rejection or repair require explicit
  semantics; a finite penalty alone does not guarantee a hard constraint. Validate
  every returned discrete candidate with the exact evaluator and legal/owned rules.
  Evaluate variance, rare coordinated set activations and premature concentration.
- Compare these methods with safe pruning, local search, batched sampling and
  exact enumeration/branch-and-bound where valid bounds exist. Small tractable
  domains provide an exact quality reference. Do not preselect a winner because
  the variables are discrete, the objective is nonconvex or a GPU is available.

Nonconvexity does not prevent running gradient methods; it limits general global
convergence guarantees. GPU batches can serve forward rewards, gradient estimates
and continuous relaxations after profiling and numerical parity. Preserve the
independent exact evaluator even if the search uses a relaxed/learned model.
These are candidate designs for this game, not implemented or benchmarked results.

Use one fixed-reference weighted-action damage ratio with explicit scenarios.
Hard HP/agility/toughness constraints must hold under the declared state policy,
not averages of incompatible states. Only complete search with sound bounds over
a closed domain, or another independently justified global certificate, can
certify an optimum against the validated evaluator. Otherwise return
the best found candidate and a justified bound/gap where available, explicitly
distinguishing unsupported, timeout and no feasible solution. Full-formula,
legal-domain, complete-forward and optimizer-ready claims remain false for now.

Archived method-record documents are dated snapshots. This runbook owns the
current solver-selection guidance; the acquisition evidence and S1 bytes remain
unchanged by a later algorithm-method correction.

## Runtime fallback exercised for core300/core340 after rebuilt R8

The rebuilt R8 return left manager+300/+340 unbound. The preserved original manager
snapshot contains distinct pointers at300/340/718, but the300/340object/row ranges
were not part of the old14-family capture. S1's zero0x80 and eleven0x44shape leads
did not establish these bindings. A bounded read-only collection was therefore
used to supply the exact two tables rather than repeat a geometry-only handoff.

Evidence root:
`D:/Nioh3_v080_deliverables/deliverables/damage-pro-return-audit-20261014-r8`.
`capture_core_tables.py` reuses the repository `ProcessReader` and generic table
capture. `CORE_CAPTURE_SCOPE.md` defines the exact question, seven mock preflight
controls,16384-row/table and4MiB/read bounds, permitted two-table scope and cleanup.
The collector checks EXE SHA, five native reader signatures, Windows process birth,
module/manager identity, full current item rows and native key-index parity with
R8/S1, then rereads every captured blob. Mock preflight is not native acceptance.

The actual target was launched through installed Steam CLI under the owner's
standing authorization, without Computer Use/input emulation. The read-only
capture used `PROCESS_QUERY_INFORMATION | PROCESS_VM_READ`; no CE, game-function
call, hooks, code/register/table writes, inventory scan or save access occurred.
The process launched by this task was normally closed after an exact creation-
FILETIME check. It exited; the read handle was closed. Preserve launch/capture/
cleanup records separately, including the first safely rejected cleanup timestamp
reparse. A UTC timestamp parsed by ConvertFrom-Json may already be DateTime; use
its full-precision FILETIME rather than culture-reparsing it to compare ownership.

| Native binding | Rows | Stride | Native keys | Full row-store SHA256 |
| --- | ---: | --- | ---: | --- |
| manager+0x300 |23|0x80|23|51c77a63cd3caeca92a63af67a06ffa9f9b6482bf503f568a4d5173ec18028a6|
| manager+0x340 |7|0x44|7|fa8d7a7c2611af9bdb20573288f181c15c5c90dc5b744f8247da0ef6016712e3|

These30complete rows, table bodies and stores have no exact occurrence in the
1295S1payload corpus. This is not proof that the serialized values are absent:
pointer relocation, construction or a different serialized layout may explain it.
Keep resource-loader identity as a separate update contract. The current captured
rows/native key maps already support coefficient evaluation and need not wait
for storage-format closure. Capture addresses belong to their recorded new epoch;
do not transplant pointers or treat item-byte parity as full actor-state parity.

The portable `attack-review/real-core-reader/core_coefficient_reader.py` binds
the recovered3FA0C8contract to these pointer-free maps. It covers21real original
item-key pairs/all3362definitions:17pairs/552definitions primary hit,
3pairs/39definitions fallback, and(0,0)/2771definitions both-miss. All demanded hit
pairs return[45,20,10,0,0,0,0]. Captured zero-valued primary rows plus explicit
synthetic fallback combinations prove hit precedence, rather than value-based
fallback. The3503-check explicit-path verifier is offline and does not claim the
complete mask/source/coreX/actor or hardware execution. Two missing hash readers
were also exported from true containing-pdata ranges; remaining dependencies are
recorded, not treated as missing source bytes.

For an update, reuse this scoped table-acquisition contract only after relocating
the manager/readers and pinning the new executable/profile. Preserve context,
header, rows, native hash entries, key worklist, shared collector source hash,
code signatures, process identity and reread proof. Do not merely change the
expected EXE hash while keeping unverified old RVAs/layouts. If the loader is later
recovered, add a resource-to-native reconstruction control and retain this native
capture as the reference. No per-affix gameplay is needed for these static tables.

## CRT index-eight dispatch supplement (2026-10-15)

Approved PC v2.02 EXE SHA256:
`e22c4a635e4ec1e27a177b76e27d7f6a637f426c0ed3928b60f5693bc52ae130`.
The omitted u32 dispatch entry is RVA0x50DB070, raw bytes5d43b900, target
RVA0xB9435D. It is also file-backed in PE section `_RDATA` at offset75602544.
Earlier exports included.text/.rdata/.pdata and omitted this separate section.
Do not equate an absent exported range with absence from the installed file.

The bounded collector `capture_crt_dispatch.py` under
D:/Nioh3_v080_deliverables/deliverables/damage-pro-return-audit-20261015-r9/
parses the PE section table, verifies the approved executable, and records exact
file offsets/bytes. Runtime mode separately checks process birth, module identity,
loaded reader signature at0xB94250, repeated dispatch reads and target instructions.
On this version file.text differs from decrypted runtime instructions; the static
_RDATA bytes match runtime. Future updates should first inspect all PE sections,
then verify any required runtime equivalence instead of repeating broad captures.

Runtime evidence is crt-runtime-capture/; file evidence is crt-file-inspection/.
Target instructions begin488b0a488908c3 (eight-byte load/store then return).
Only140 target-memory bytes were read, with no writes, native calls, saves, CE or
Computer Use. The task-launched game exited normally; GAME_CLEANUP.json records it.
The derived offline VM maps the measured four-byte dispatch and executes the
original copy instructions. This closes this data dependency, not source lifetime,
final actor reconstruction, full damage or legal equipment generation.

## Side-source definition table and additional CRT entries (2026-10-15)

A second small read-only capture under damage-pro-return-audit-20261015-r9/
side-runtime-capture binds manager+138 to91flat rows of strideD8 and91native hash
keys. Its full19664-byte header/body exactly matches decoded S1payload00227;
all90group-side keys are covered. Source-row field+4 is confirmed against this
captured table's actual native key/index for this version only. This is not a
blanket rule for item/effect/other tables, and is not inferred from row appearance.

The same capture binds CRTindex4 atRVA50DB060:6a43b900 ->B9436A and index12 at
RVA50DB080:3343b900 ->B94333; index8 was rechecked. Original captured.text contains
all target prefixes. ApprovedEXE, native hash-reader signatures, process birth,
module identity, existing item-row epoch and all raw-blob rereads matched.
The capture code is capture_side_source_inputs.py with its shared reader source
identity in the receipt. No attack/action, target write, native call, CE, save or
Computer Use was required; SIDE_GAME_CLEANUP.json confirms normal exit.

Future-version reproduction should decode payload00227's stored segments using
its recorded resource index, then validate the91-key/row geometry against the
version-matched manager138 reader/loader. Resource ordinal alone is not a stable
semantic identifier. side-source-frontier/SIDE_CONTRACT.md records the resource
name/offset/chunks and native join; use that evidence before any new live capture.

## Actor parameter bindings beyond the prepared-record stage (2026-10-15)

One bounded read-only epoch captured four additional flat tables whose strides and
readers were confirmed from original instructions: manager200/stride74 (99rows),
manager3F0/stride100 (52rows), manager118/stride54 (84rows), manager970/stride1C
(34rows). All actual key/index maps were retained; EXE, reader signatures, process
birth, item-table epoch and every captured blob passed rereads. The task-launched
game exited normally (ACTOR_GAME_CLEANUP.json). No live state was written.

actor-runtime-capture/CAPTURE_RECEIPT.json and ACTOR_RESOURCE_JOINS.json under
damage-pro-return-audit-20261015-r9/ record exact inputs. Whole-table byte identity
matches S1payload01208 for manager200,00186 for manager118 and00216 for manager970.
Manager3F0 has no exact whole-payload match in the1295payload S1 corpus. A later
bounded range comparison resolved it: all13,320captured header/body bytes equal
the prefix of payload00482, leaving6,562trailing resource bytes uninterpreted.
ACTOR_RESOURCE_JOINS_RESOLVED.json records that exact range; its older
all_tables_match=false field refers specifically to complete-file equality, while
all_tables_reconstructable_from_exact_resource_ranges=true is the resolved claim.
The earlier whole-file negative is retained and does not imply missing data.

For manager200, native keys coincide with rowu32+0 on this exact snapshot; the
side-source manager138 instead matched rowu32+4. Both are verified concordances,
not interchangeable universal key rules. Future updates must retain actual
loader/hash-index validation and resource metadata provenance.

The new375D20 local consumer passes160full2048-byte and ordered-write comparisons
using the99 actual manager200 rows, its native hash lookup, twelve explicit caller
keys and the raw global-mode byte. It populates21eight-field vectors plus scalar
fields with native ordering, preserving untouched bytes. This closes the parameter
consumer, not the producer of caller owner+18 fieldsE0..10C or the full actor.

## Guardian level and constructor table recovery (2026-10-15)

The same approved PC2.0.2.0 executable/profile was used for two bounded read-only
epochs. capture_guardian_level_table.py reads manager+408 as a dense indexed store:
101rows, stride40,8-byte header, no native hash key. Reader signatures374E74,
9C45DCand8176F4, process creation FILETIME, module identity, existing item-table
epoch and every blob reread pass. The entire6,472-byte store equals decoded S1
payload00828. This closes parameter identity; it does not make a level coordinate
outside0..100valid or imply that every u16level naturally occurs.

capture_constructor_tables.py follows the actual1342E0selector: manager.byteB0A
zero selects+38, nonzero selects+40. Both contexts were captured in full, each
1,022rows/stride398and its complete native hash buckets. The actual byte was1.
All23nonzero requested guardian constructor keys hit; key0 is a genuine native
miss. Both full940,248-byte stores equal S1payload00970 in this epoch. The native
key/index maps are verified bijections and are retained independently of row
contents. No future-version or cross-mode table equivalence is inferred.

Authoritative evidence under the R9audit root is guardian-level-runtime-capture/,
constructor-runtime-capture/ and GUARDIAN_CONSTRUCTOR_RESOURCE_JOINS.json. The join
script verifies all1,295decoded payload lengths/hashes and every capture blob,
then searches exact complete/headerless ranges. Resource ordinal, original name,
stored offset/size/hash, decoded hash and native row hashes are recorded. The
task-launched processes exited normally; *_GAME_CLEANUP.json records cleanup.
No target write, call, hook, breakpoint, save, CE or Computer Use was required.

For an update, decode the recorded named resources, relocate all reader/selector
signatures and validate actual native index/row geometry. Do not merely reuse an
ordinal or swap the expected EXEhash. The guardian definition table uses8-byte
player records containing u32key,u16coordinate,u16reserved; its parameter rows
have a different stride0x100(256bytes). Keep those two structures distinct.

## Full720index and synchronization inputs (2026-10-15)

capture_actor_curve_inputs.py records manager720's48-byte context, complete337x12
flat storage and actual337x8 pointer index. Every pointer is checked against the
captured flat extent and its exact permutation is retained. Native upper_bound
orders the composite of key_u16 and biased signed x; flat storage itself is not
that sorted index. The preserved old flat capture has the same4,044-byte hash and
already contains the six A865nodes:2->5,3->7,4->10,5->15,6->25,7->40, with step
flag zero. The selected attack-curve JSON omitted them. No missing-key zero
follows from that curated file, and no complete S1match for the old flat body was
found in this corpus. Parameter values and native index proof remain distinct.

The same epoch records manager8's complete42x68dense coordinate rows and8-byte
header. D20AF0consumes ctx+0and ordered row.u16+48, with no hash map. Exact reader
signatures552B0C,552890,D24860,5507E4,D20AF0and1BA87E8, approvedEXE, PID creation,
module, item epoch and all blobs pass repeated checks. Evidence is
actor-curve-runtime-capture/CAPTURE_RECEIPT.json; this is table/index observation,
not live numerical execution of an actor rebuild or mission-eligibility proof.

## Complete 718 and 720 resource resolution

The later record [Complete curve resource reconstruction](NIOH3_CURVE_RESOURCE_RECONSTRUCTION_20261015.md)
supersedes the earlier partial curve-identity boundary without modifying its raw
evidence. All337 manager720records match S1payload00194 through an exact row
permutation, and its native sorted index is reproduced without duplicate-key
ambiguity. Manager8's complete4376-byte table equals S1payload01129.

The long-standing532/662 bonus-row match is also resolved: manager718 has an
84-byte physical stride; the old collector read88 bytes. All662consumed84-byte
rows match S1payload01119. The extra four bytes were the next physical row ID
(or outside-table bytes for the last row), not changed coefficients. All131curve
keys are retained. Strict84-byte native sample controls pass3972cases; the full
selector passes11916cases. The Stage3 resource evaluator matches all11916results
and15public CLI controls. Full damage, source qualification and legality remain
separate claims.

## Direct-index and auxiliary actor tables (2026-10-15)

Three later bounded read-only captures add exact native inputs for remaining
actor consumers. Their receipts retain executable SHA, process creation FILETIME,
module, reader signatures, existing item-table epoch, every blob hash and equal
rereads. No game helper was called and no target memory or save was written.

| Manager | Captured layout | Exact decoded S1 payload |
| --- | --- | --- |
| +298 | Eight-byte header, 1,302 rows of 16 u32 columns | 00246, all 83,336 bytes |
| +158 | Four rows of stride 0x52, complete u32-key native hash index | 01048, all 336 bytes |
| +010 | Six rows of stride 0xDC, direct u8 index, no hash | 00351, all 1,328 bytes |

`DENSE298_TABLE158_RESOURCE_JOINS.json` and `TABLE010_RESOURCE_JOIN.json` under the
R9 audit root record the precise payload descriptor, comparison range, native
row geometry and input hashes. `join_dense298_table158_resources.py` and
`join_table010_resources.py` reproduce the comparisons. The latter accepts
explicit capture/resource roots for portable replay. Native158 key width is
32 bits; treating it as the more common u16 key would be wrong. Native010's
85A14 reader indexes the dense row body directly; do not invent a hash identity.

The corresponding collectors are `capture_dense298.py`, `capture_table158.py`
and `capture_table010.py`; raw inputs remain in their named runtime-capture
directories. `STATIC_BATCH_GAME_CLEANUP_FINAL.json` confirms the final task-owned
process was absent by the recorded deadline. Its old PID/birth never authorizes
closing another game process. Future updates require matching consumer semantics
and native row/index validation, not reuse of a payload ordinal alone.

## Nine remaining source-family parameter tables (2026-10-15)

The bounded collector `capture_source_parameter_tables.py` adds nine source-family
tables. All277,464 observed bytes pass executable/process-birth/module identity,
32-byte consumer signatures, the accepted item-table epoch and equal rereads.
The capture uses read-only handles, no game calls, hooks, breakpoints, target
writes, save access or Computer Use. `source-parameter-runtime-capture/` owns
the raw files and CAPTURE_RECEIPT.json. The task-owned process exited normally;
SOURCE_TABLE_GAME_CLEANUP_FINAL.json confirms all Nioh3 processes absent at
2026-10-15T20:05:42Z. Never act on that historical PID after the task ends.

| Manager offset | Rows | Row stride | Exact complete S1 header/body |
| --- | --- | --- | --- |
|7C0|6|70 hex|00061|
|7C8|32|78 hex|00940|
|7E0|36|110 hex|00656|
|278|34|D8 hex|00603|
|268|4|38 hex|00957|
|3F8|340|5C hex|01030|
|400|6|24 hex|00234|
|130|1,101|C0 hex|00294|
|260|55|D0 hex|00342|

`join_source_parameter_resources.py` verifies all1,295 stored decoded payloads
then reproduces all nine exact header/body identities and each row's native
index, resource byte offset and SHA256. SOURCE_FAMILY_RESOURCE_JOINS.json binds
the capture receipt SHA0913c19f260b4ab26cae96b3042e9f60a8e49c574b36841be92c6747cd6a9c81.
Seven row stores have outer dense-iteration consumers; this does not prove that
all nested readers are direct-indexed. In particular, later278 and130 consumers
require their actual hash indices, captured below. Do not manufacture row keys
from a leading value. Tables400 and260 use actual u8 hash keys:ctx+20 holds
the hash context, bucket begin/end are+8/+10, empty-key byte is+4, bucket stride
is8, key is byte0 and row index is u32+4. Opaque padding and equality-before-empty
behavior are retained. The captured key-to-index metadata is not a replacement
for executing all256 native lookup inputs.

These records remove repeat runtime capture of the same-version parameter data.
They do not recover dynamic progress maps or prove producer/eligibility rules.
For a future update, rebind the named resource descriptors, verify decoding and
whole-body/row hashes, relocate exact readers and validate native geometry/index
semantics before accepting values. Payload ordinal and matching row prefixes
alone are insufficient. Keep original captures and failed/partial evidence;
invalidate only the contracts whose consumer, data, type or order changes.

## Source278 and source130 native index supplement (2026-10-15)

The later source-family reader audit identified two required static indices.
The root froze19 exact32-byte consumer signatures before a single bounded
read-only capture. `capture_source_parameter_indices.py` and
`run_source_index_capture.ps1` reproduce the acquisition; the frozen worklist is
`source-family278-frontier/CAPTURE_WORKLIST.json`, SHA256
`21404c78184112f60fdf8aeda8c4f46e35f297acb87d34fa6c83050a26ff65fe`.
The capture checked the approved executable, process birth/module, accepted item
epoch, every signature and every blob reread. It collected237,072 bytes in
`source-parameter-index-runtime-capture-01/attempt-01/`, including both unchanged
row bodies and their previously missing raw index storage. No game function,
target write, hook, debugger, save or Computer Use was involved.

| Manager | Native key | Rows / stride | Buckets / stride | Empty key | S1 body |
| --- | --- | --- | --- | --- | --- |
|278|u32|34 / D8|69 / 8|FFFFFFFF|00603|
|130|u16|1,101 / C0|2,203 / 8|FFFF|00294|

Each40-byte table context has the row-store pointer at+0 and hash-context pointer
at+20. In the32-byte hash context, bucket begin/end are+8/+10 and the empty key
is at+4 with the consumer's actual width. A bucket has its key at+0 and u32 row
index at+4; source130's bytes+2/+3 are opaque padding. Preserve original
equality-before-empty and bucket-probe semantics. Derived key catalogs are
provenance witnesses, never substitute return values for the native lookup.

All34 and1,101 unique indexed keys cover every corresponding row, with no invalid
nonempty entry. Every34 source278 caller selector is found. The receipt SHA256 is
`bc56800b2d428b208e4e81297d8e39ec5f42bd55092daac9bcf39d1c30e56bb1`.
`join_source_index_resources.py` rechecks all raw blobs and both complete S1
payloads, producing1,135 key/row/bucket/resource-offset/SHA256 witnesses in
`SOURCE_INDEX_RESOURCE_ROW_BINDINGS.json`, SHA256
`07999c384b95912a5dceb89db0a9a652cc9ea216d1422343485600d5fc730ffd`.

In this snapshot, source278's native keys match row.u16+88 exclusively, and
source130's keys match row.u16+18 exclusively. The former index still consumes
u32 keys. These are observed row correlations, not a recovered index-constructor
rule or a guarantee that a future version can rebuild indices from those fields.
Future updates must revalidate the exact lookup, key width and raw index before
using a row correlation as reconstruction input. Table268's nested lookup reuses
the278 index; the static3F8 reader reuses the already captured guardian3F0 index.
Dynamic progress maps remain a separate input family.

`source-parameter-index-runtime-capture-01/CLEANUP.json` confirms normal exit of
the task-owned process and no remaining Nioh3 processes at22:15:59Z. Historical
PID41428/birth134365761394729137 must never authorize acting on a later process.

## Two fixed-RVA tree-curve parameter bodies (2026-10-15)

Actual REA/Ghidra queries of FBED80/FBEDA0 and shared FC0360 identify signed
counts45B9E90/94 and32-byte rows at46D7B10/46D7C50. The old analysis carrier has
no initialized.data here: REA reads0/8 bytes with complete=false. The on-disk
PE also has no raw mapping for these virtual.data locations. Further decoding
of those absent bytes cannot establish their initialized values.

`method-audit-20261015/rea/CAPTURE_PLAN.md` scopes the resulting read-only
capture. `tree-curve-runtime-capture-01/attempt-01` contains both counts (8/8),
both256-byte row bodies,28 threshold bytes and the1.0/100.0 constants,556 bytes
total. Each row is coordinate+seven binary32 coefficients. Both full row bodies
hash to `ec3bfead968ea8cbe9575dfb4cb2bd4ea5fc47ed5f9fb2f2dfeb13893ad9501d`;
their equality is observed, not a semantic or future-version identity rule.
The collector verifies the approved executable and process birth/module, both
24-byte wrappers, all504 shared bytes,64 caller-window bytes and stable rereads.
Both finite/increasing-coordinate checks pass. No writes, target calls, hooks,
debugger, save access or CUA occur. CLEANUP.json records normal closure of the
task-owned process and no remaining game. Historical PID48168/birth
134365830872967331 is not authority for a later process.

The installed REA profile and repeated offline verifier are documented in
`method-audit-20261015/rea/README.md`. These counts/rows can now be reused for
this version's local consumers. Their asset resource or static initializer has
not yet been identified, and source/caller binding plus whole-hit/optimizer
acceptance remain separate. A future update requires new consumer/data identity
checks; it must not silently reuse this version's fixed RVAs or fill missing
runtime values with zero.
