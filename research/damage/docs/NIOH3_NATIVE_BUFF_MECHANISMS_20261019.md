# Native buff mechanisms and batch catalog, PC 2.0.2

## Purpose and current authority

The product goal is a native damage/candidate evaluator driven by equipment,
skills and current state, followed by actionable combat statistics. The owner
requested direct reverse engineering, including REA, shared stacking/lifecycle
mechanisms and reusable version recovery. A handoff package is a reproducibility
checkpoint; it is not completion of an authorized mechanism investigation.

Current evidence root:
F:/Nioh3_r16_build/deliverables/r16-native-buffs-continue-20261019/.
Start with natural_combined/REPORT.md, form_migration/REPORT.md and the accepted
passive_sources receipts. The adjacent r16-native-buffs-20261019/ is the frozen
earlier checkpoint, including catalog/, stacking/ and timers/ proofs referenced
below. Both earlier controlled records and the new combined trial retain their
own observation grades. New code proofs do not turn polling into an entry trace.

Executable SHA256 is e22c4a635e4ec1e27a177b76e27d7f6a637f426c0ed3928b60f5693bc52ae130;
loaded.text is4cec8fb6ad867417a76df8201c1d4f54172443884910463c1953acad91ae6c29.
The original section pointer epoch7FF67BCE0000 differs from historical30880
module7FF730120000 and the latest completed44540 collection module7FF790180000.
The owner has closed the game; these are recorded epochs, not current addresses.
Do not mix their absolute pointers or assume equal allocator/profile states.

## Counterattack Deflect creation and time

Configured Samurai skillA7AA (`反攻格挡`) joins materializer group2673 to D+9F0.
Native236BBC4 reads that gate at236C0C5, searches source-definition1AE through
20852D8/2080F30, creates through3EFB90 or refreshes through2096068. Source1AE
is manager7A8 row272; its status key312 joins manager7A0 row1, flags1F8. Native
3F0324 selects factory207CEB0, constructor2073100 and type0 vtable3AA8120.
Generic constructor3F1B18 initializes duration+20/+24;3EFFA0 stores source1AE
at status+74. The current multiplier is binary32 1.100000023841858.

For this source's finite domain, every indicated operation rounds to binary32:

```text
e     = f32(f32(STAT[1AC] * 100) - 100)
extra = f32(f32(e * definition[24]) * HPowner[2DDC])
scale = f32(f32(1 + D[6F8]) - D[770])
T     = f32(f32(300 + extra) * scale)
```

Retained current inputs are STAT1AC=1.0929999351501465, definition24=1,
HPowner2DDC=1 and D6F8=D770=0. Native execution yields
extra9.299995422363281 and initial309.29998779296875 (bits439AA666).
The upstream producer is now independently closed for the selected state in
catalog/STAT_1AC_PROVENANCE.md and stat-1ac-proof-02/STAT_1AC_PROVENANCE.json.
Original818B4C selects Q(STAT+7D8)+34, the synchronized attribute array. Its
index5 is20 in bytes already captured at selectedSTAT+858. Native818AEC clamps
the signed index1..999; table90 row20.u16+14 is930. Original818B68 stores
f32(f32(930*f32(0.0001))+1)=1.0929999351501465 at81904E. Later819327 adds D+A8
only if positive; current selected D+A8=0. Twelve native seed/clamp controls and
36 native late-D controls pass, including negative-D and wrong-scale checks.
The retained table90 corpus is hash-pinned; this is not a new live table90 header
capture or full actor rebuild. The continuation additionally captures both
root-bound attribute sources and closes their upstream producer: progress94's
seven values[25,150,150,40,50,20,50] flow through5561D0 to the synchronized source.
Both core and selected index5=20 independently reproduce6ce78b3f. The Chinese
UI name of that positional attribute is not established. Do not attribute the
9.3 residual to a guessed1.031 duration multiplier.

If the first matching source1AE object has remaining>0, native2096068 performs:

```text
n = f32(old_remaining + D[9F0])
if n > old_bound: bound = n
remaining = clamp(n, 0, bound)
```

Current refresh adds300 raw units. It neither resets elapsed time nor creates
another1.1 multiplier. This wrapper has no fixed ceiling at the initial bound;
that is a finite-domain code statement, not a global infinity claim. Eleven
original-instruction timer controls pass without helper-return substitution;
allocation is an explicit stop boundary before factory allocation.

Native39E520..39E533 computes60/f32(frame_context.u32+10). Engine state46518
gates active/zero time.39F370/396240 packs it verbatim,396330 publishes the
worker context, and B0B70 adds owner+8E0 before A56A4/A648C/A481C. The generic
tick also applies status2C/30 and a native rate helper. Controls30/60/120 produce
2/1/0.5. The continuation closes the main SHORT producer and App614/624 inputs,
with37 native cases, then the stable MAIN/LONG pacing producers with38 cases.
An independent quiet201-endpoint calibration supports about60 actor input
units/second at current rate30/factor1. The combined trial's own timestamp
brackets bound fresh1AE residence5.0991950..5.2006814 seconds. Preserve native
timer units, rate modifiers, pause/reset paths and sampling uncertainty; this
does not establish an unconditional duration for every frame schedule.

## Competition, multiplication and dilution

AggregateA=HPowner+2DC0 is initialized/reset each update. Class0 E8A708D0 is
19 original bytes: multiply statusA8 into aggregate+7C. A648C first groups
low-u16 node keys and selects greatest virtual+50 strength. Losing objects
still tick with suppression flag15 mask08 and use virtual+F0; winners may use
E8 subject to activation gates. Presence and numeric contribution differ.

All40 positive class0 creation rows in this complete captured7A8 payload use
key312. A7AA is one of them. A stronger312 status can reduce its marginal gain
to zero while its object/timer remains. Five negative class0 definitions use9F.
Other class0 dispatcher keys17D/354 have no rows in this payload; direct/script
creation is not thereby disproved. Synthetic312/313 insertion/rebuild controls
demonstrate different-key multiplication, not natural availability of a second
positive class0 key. Other status classes also write aggregate+7C; the312 rule
is not exclusive ownership of the entire outgoing factor.

MAIN sends this aggregate through6267D0 into an early boost/cut pair:

```text
if incoming >= 1: boost = f32(boost + f32(incoming - 1))
else:             cut   = f32(cut * incoming)
factor = f32(boost * cut)
```

Class172 (vtable3AA11B0, E8A70EE0, aggregate74/78) enters that same pair when
its consumer gates pass. Current native descriptors22C/2B2/3 carry the label
`千手千变`; its specific natural coexistence with A7AA remains separate and it
was not among the owner's25 configured skills.

Eight whole original MAIN counterfactuals retain the documented R2 getter and
inactive-row cuts and pass eight initial-score erase controls:

| Enabled route on the retained entry | Early factor | Final displayed damage |
| --- | ---: | ---: |
| Baseline |1|732|
| Class0 1.1 |1.1|805|
| Class172 1.2 |1.2|879|
| Class0 1.1 plus class172 1.2 |1.3|952|
| Class132 1.2, consumer gate false |1|732|

Thus1.1 is neither universally independent nor universally diluted. Same-key
competition, contributions to the same aggregate, shared additive pools,
attack applicability and later stages determine marginal gain. The ideal
1.2→1.3 relative gain is8.33%; the actual native result is879→952. Do not multiply
already truncated UI damage numbers. These are offline counterfactuals on a
retained entry, not a newly observed live loadout combination.

## Batch completeness and actual REA use

The batch reads all406 creation rows and238 descriptors, with complete native
hash-key lookups, exact resource parity and native UI text-field consumers.
227 nonzero unique text IDs have bounded native locale21 lookup provenance.
There are197 Common/Samurai/Ninja definitions and25 configured skills;182
source descriptors have accepted simple materializer recipes. The original
static join covered375 creation rows; accepted continuation preparation reaches
399 actual factories and seven nativeNULL outcomes across all406 definitions,
without unavailable preparation inputs in the declared vector. Its canonical
CSV is dispatch/preparation-e2e-01/prepared/PREPARED_CREATION_CATALOG.csv;
prepare-all-01 is a preserved failed intermediate, not acceptance. The census
covers166 vtables sharing the declared generic-method family, not all conceivable
status systems in the game.79 straight-line class entries have158 original
application controls;166 type getters are executed independently. Later complex
proofs originally accepted79 of87 declared E8 control domains, so158 of166
entries had accepted linear or complex domains. The October20
[Pro-return acceptance](NIOH3_R16_PRO_RETURN_ACCEPTANCE_20261020.md) adds seven
independently reviewed finite private full-positive domains: current86/87
complex and165/166 combined. Type86's positive transition remains unproved.
Prefixes/helper returns do not increase that full-E8 count. These are scoped
numerical/control domains, not all natural activation/applicability paths.

The complex-handler follow-up also executes full original class38/E8=208D7C0,
descriptor47 (`空手攻击增强`), through its original virtual/component predicates.
The admitted unarmed selector3C2E with no blocking predicate applies1.2;
wrong-selector3C2F and blocking-predicate controls remain neutral. Original
type0 plus this original class38 method yields1.3200000524520874 in either
order. This is a concrete other-class contributor to aggregate7C, conditional
on its domain; it does not apply to the owner's katana baseline. See stacking/
complex-index/TYPE38_METHOD_E2E.json and the87-handler semantic index. No unknown
helper return is substituted and no natural concurrent live activation is claimed.

Official local REA6.4.0/Ghidra12.1.4 actually decompiled232 distinct functions,
including all87 initially complex E8 handlers. The old136-function acceptance
and continuation receipts through phase07 remain separate immutable records.
Phase08's70,440-byte dispatcher fails Ghidra's response-buffer limit and adds
zero decompiled functions; its original switch/case CPU evidence is separate.
Carrier.text/.rdata/.pdata equal the pinned original sections;
the carrier is analysis-only and never executed. A task-only Node import adapter
uses pdata-resolved explicit seeds and-noanalysis, leaving original provider
files unchanged. Decompiler signatures are not trusted over original ABI/ASM.
REA_NATIVE_ACCEPTANCE_02.json records all batches, exports, original byte parity,
independent Capstone/Ghidra comparison of type0/pair instructions and process
closure. Complex-handler decompilation is not all-handler numerical closure.

Canonical CSV is catalog/e2e-batch-01/localized/BUFF_DEFINITIONS.csv. Its
E2E_RECEIPT.json records actual CLI execution, six mutant refusals and47 unchanged
source/input pins. Repeat commands are in catalog/README.md, timers/RUN_NATIVE.ps1
and stacking/REPLAY.ps1; use the project Python wrapper and a fresh F: output.

## Source identity correction and future metrics

OriginalFFA290 stores incomingR9W at status+18. Node creationFFB209 writes only
u16 key at+20 and Qobject at+28. A six-byte poison99ABCCDDEE7F at node+22..27
survives native insertion and is not read by the native rebuild. The old OS
collector's source_metadata_u16 is therefore a misleading name for raw padding
on this proven route. Preserve sealed bytes; deprecate that field for attribution,
grouping and lifecycle identity. Creation definition is status+74; skill source
requires the caller/materializer join. A generic Chinese status label is not a
unique source name.

Current ANALYTICS_STATUS_CONTRACT_V4.json preserves the earlier configured, eligible, present,
unsuppressed, applied, attack-effective and marginal gain. Effective buff uptime
needs that chain plus coverage and clock epochs; a surviving weak312 object
does not certify beneficial uptime. Perfect guards/minute require unique native
contact episodes and known eligible combat time, including failed actions and
recovery. Buff creation/refresh counts are not certified perfect-guard counts.
Keep unknown intervals, left-censoring, pointer reuse, pauses and non-atomic
OS reads visible. No production combat metric is accepted by this research alone.

## Continuation and boundaries

Continue the eight complex positive-path frontiers by shared native helpers,
with allocation/registry boundaries explicit. Creation preparation and the main
clock producer are now closed in their declared domains; a universal physical
time conversion and every natural trigger are separate acceptance questions.
Resolve contact episode/coverage semantics before player-facing guard rates.
Do not ask for a separate skill-toggle experiment for each definition.

The earlier debugger arm fatal remains unresolved and debugger reuse suspended.
This continuation uses bounded OS-only reads and offline original-instruction
interpreters, including the owner's completed combined natural trial. Handles
and owned REA processes close with individual receipts. No game call/write,
save change, external Pro submission, publication or new-version acceptance
occurs. See versioning/profile-04 and the update runbook for100 same-version
anchor controls and seven independently recovered RIP slot candidates.
## Active continuation after the first checkpoint

The owner rejected stopping at the earlier report while useful native work was
still available. Current evidence is under
`F:/Nioh3_r16_build/deliverables/r16-native-buffs-continue-20261019/`.
The earlier sealed directory and its final-state file remain an immutable
checkpoint. The following continuation supersedes its narrower coverage.

Creation preparation now executes original3EFB90/3EF5CC/3F0324 for all406
definitions in the declared current-player, neutral-options context:399 reach
an actual class factory, seven return nativeNULL, and no missing preparation
input remains. Alternate existing-object placement is separately proved for
six genericNULL keys. This does not turn a NULL factory into a second buff or
prove all natural triggers and coexistence. See dispatch/preparation-e2e-01.

The actual main SHORT frame is now bound to the normal state vtable400D908,
slot30, through5AC664→5AC9EC→564F40→564F60→565198→5651D0→39E4E4.
Its rate field comes from App+614, with the previous App+618 value transferred
by5AD5F8;5AF604 maps enum0/1/2 to30/60/120. A returned LONG stack-frame pointer
is not a current timing sample. Genuine current App rate30 gives native delta2.
At initial timer309.29998779296875, update155 still applies type0 then clamps
remaining to0; update156 reaches removal before another E8 apply. Zero-remaining
but still-linked1AE also blocks both refresh and replacement on the traced
caller ordering. The37-case clock wrapper passes. These are exact native
updates, not a universal physical wall-clock expiry claim.

The global pending-event9 counter is now traced through4F54C4/844138/844154.
It increments once per normal qualified primary action-transition invocation,
after owner-index, timed-window, contact and action-definition gates. A local
CFG audit finds no second direct key9 execution in that normal invocation.
It is not one increment per queued contact node. A different eligible owner
can increment the same queue, all15 counters can reset, and the reset serial
can repeat. Equal serial/count endpoints therefore cannot certify no reset
or identify a current-player guard. event_identity/wrapper-e2e-01 accepts30
original-instruction cases and an explicit admission contract. No additional
manual guard or skill-toggle experiment is requested for these code facts.

Timed-window9F32C8 is executed with genuine230 parameters: ordinary bounds are
inclusive[0,9], extended to[0,12] under D2FA; the alternate-contact route uses
[0,10]. EC8 is a repeat selector with further shortening, not a boolean.
The alternate selector comes from contact+13C mask400 through native virtual
query3E1; it is not assigned a projectile/form label without separate evidence.

The source-admission continuation runs197 candidates in both forms394 times.
For the25 configured positive-cost rows, the ordinary core-source path admits
Common8+Samurai8 in form0, Common8+Ninja9 in form1. Four zero-cost Samurai
foundations are separate and expand no nonzero groups on that path. The default
0x88 descriptor is constructor-derived and its ordinary caller path is proved;
this is not a captured transient descriptor or a full selected-D reconstruction.
Source/materializer contribution attribution remains active in passive_sources/.

The new combined OS-only recorder is acquisition/RUN_CAPTURE.ps1. It records
the166 authenticated status-family headers, source74, suppression, type0 math
fields, all15 pending counters and reset serial, genuine App timing, and the
current actor-bound movement/action object and local transition serials.
Six actual-CLI/unknown-input cases and61 live quiet endpoints pass for each
accepted recorder stage. These transition serials are not guard counts.
The next79-class typed method-input extension is being validated separately;
complex classes retain explicit unknown payload semantics. All OS handles close.

Cross-sign stacking is also covered by five retained full-MAIN counterfactuals.
Positive312×negative9F gives0.9900000095367432 before6267D0 classifies it as a
cut; another class172 positive1.2 enters the boost pool. The corresponding
same-entry damage is870, versus791 without the positive312 and952 without
the negative9F. These synthetic aggregate controls preserve the existing MAIN
getter/inactive-row domain; natural coexistence is not claimed. A fixed
"all buffs add" or "every10% is independently multiplicative" rule is invalid.

Actual REA/Ghidra continuation accepts92 successful requests,85 distinct
functions in the new batches and219 combined with the earlier136. Six explicit
unseeded-entry refusals are retained and excluded. Provider files and target
bytes remain unchanged; all owned clients/providers are closed. The79-anchor
same-version migration CLI and four independently rederived RIP data roots
have repeatable controls; another game version remains unapproved.

## Combined natural trial and native insertion refinement

The owner's combined trial is preserved under the continuation root's
acquisition/linear-live-combined-01. The owner reports two successful Samurai
perfect guards, one Ninja counter, intervening hits/ordinary guards and a final
rest. Treat this as an untimed annotation, not a mechanism for choosing or
labeling raw transitions. No redo was requested.

The accepted natural_combined/e2e-03 processor independently reconstructs all
2,820 endpoints (about141 seconds). All status trees, factors, counters and
linear-input components are usable; three endpoints have only local-activity
gaps. Its seven actual CLI controls retain partial component quality and refuse
changed epoch, E8, source74 and contradictory empty-tree data. Source/payload,
raw line hashes, masks, read brackets and timer rereads remain inspectable.

- Source1AE appears, ends, and appears again. The two guards did not produce a
  sampled positive renewal: the first source had expired before the second.
  First-source residence is bounded5.0991950..5.2006814 seconds by this trial's
  own read timestamps. This does not change the separately proved native
  remaining+300 refresh rule.
- At1821->1822, form0 changes to1. Source/key/node remain1AE/312/same node,
  object changes, remaining221.30->217.30, bound remains309.30 and elapsed88->2.
  This is a carry/rebuild observation, not a fresh perfect guard or renewal.
- At1860->1861, source31 becomes the312 occupant while1AE still has positive
  remaining103.30. Source31 residence is bounded5.0989079..5.2002380 seconds.
  All275 occupied endpoints have factor1.1, all2,545 empty endpoints factor1;
  no sampled coexistence, suppression or1.21 occurs. Between-poll states remain
  unavailable.

Root's equal-strength-insertion-02 adds nine original-instruction controls over
the actual exported fields. Type0 policy2082BB4 reads the genuine7A0 context
and returns mode0. FFA5E8's existing-node predicate compares the same key/type.
For actual A0=-1, equal virtual+50 strength and equal raw28 select the old
lifecycle branch atFFA67A, without reading source74. Different strengths or
raw28 survive toFFB1C6. Nonnegative A0 instead supplies an explicit identity
match. Unknown A0 or strength refuses. A separate declared successful-node-
allocation slice inserts a weaker same-key object into the native red-black
tree. Heap allocation/destruction and fullFFA290 entry are not executed.

This refines the old selection result: equal-strength insertion replacement
and strongest-per-key application are different stages. The natural1AE->31
transition is compatible with the executed matching rule; it is not natural
evidence of simultaneous objects losing a strongest-selection contest.

Root's ninja-counter-source-01 executes the D9ED gate on843DA8..843E2A through
the actual3EFB90 caller options, then native preparation/dispatch for source31.
The provenance is D18D/反攻识破 ->7DC8 ->literal D9ED=1 ->source31 ->312/type0.
The raw7DC8 value300 is unused by that literal materializer. Actual options
include word0100, Q+8=F and+26=FFFF; they are retained separately from the
neutral-options all406 census. Outer8437A0 gameplay-trigger admission remains
separate from the executed gate/factory path.

## Current passive origins, clock pacing and recorder acceptance

The source continuation now attributes exact current D35C=af47613e to integer
220 formed before float scaling: configured Common8E80 contributes120 and
genuine manager7E0 row17/progress5EA8 key47A4 contributes100. Progress-map value9
is not pending event-counter9. The current6A7C attack source113 is likewise
25 from the two checked attack passives plus native progression88 (34 rows*2
and four admitted gates*5). Accepted actual CLI receipts are current-e2e-02
and progression-e2e-01. These origins do not establish the whole current D.

STAT1AC is now traced upstream through native5561D0: genuine progress94's
seven integers are[25,150,150,40,50,20,50], index5=20 reaches the synchronized
source, and table90[20].u16+14=930 gives exact6ce78b3f after native scaling.
Both root-bound core and selected contexts agree. No UI attribute name is
guessed from its positional index. Full ordinary source compilation continues
through native source families and item collectors, with observed maps used
only for comparison, not as reconstruction inputs.

clock_pacing accepts38 original cases and a separate silent calibration.
The stable main-phase accumulator is Q(Q(module4751860))+6918; its only aligned
direct2BEBFC caller is5ACCD7. Native quantum is the separately rounded/truncated
microsecond expression from frame rate/factor, with saturating u64 addition.
At rate30/factor1 the quantum is33333. Across a quiet ten-second capture, its
delta9,999,900 yields300 conditional main points, LONG completion also changes
by300, and actor8E4 changes by600. All201 actor endpoints match twice the main
point delta; conservative main-point rate bounds are29.9987829494..29.9990577296
per second. These are current quiet observations with explicit no-hidden-reset
and sampled-control preconditions, not universal FPS or a transferred exact
continuous cadence for the combined trial. Actor8E4 is floating point and can
stall/reset, so it is not a unique event counter.

The V3 collector is acquisition/RUN_CAPTURE_V3.ps1, with79 typed linear plans
authenticated against actual E8 read footprints and17 actual CLI controls.
Known complex classes retain headers with unknown numerical effects. The
combined natural trial validates the observed type0 view; it does not naturally
validate all79 classes. Preserve the V3 raw A68 field but supersede its legacy
action-key label: native item68+58 is a category-dispatch key, and no exact
swing identity follows from the sampled1909/2D3F values.

REA_CONTINUATION_ACCEPTANCE_02 accepts the six actual phase06 exports plus all
prior immutable pins, for225 distinct decompiled functions. It verifies official
provider files, carrier identity, seed commitment and owned process closure.
Any subsequent live REA phase is a new checkpoint requiring its own closure.
Decompilation coverage remains distinct from numerical/native-path coverage.

## Closed form-transfer and Ninja command cases

form_migration/e2e-wrapper-01 accepts14 native controls. The changed-form caller
845970 writes HPowner+2C through846B3C, then dispatches846514/846538. Eligible
source/options are recreated with a temporary A0 ordinal. A53BFC saves consumed
time as f32(old.bound-old.remaining); constructor writer blocks reset elapsed0
and initialize a new bound;3F00B5/3EF5A8 restores new.remaining from that bound
minus consumed time. Migration sets flags16 bit0, then clears A0 toFFFF.
This also matches the genuine flags16 change60->61. One controlled old delta2
and new delta2 tick exactly reconstruct217.2999878/elapsed2; old elapsed500
does not affect the carry result. This is a block composition across explicit
allocation/container boundaries, not a complete callback or witnessed tick order.

ninja_trigger/e2e-wrapper-01 accepts12 controls and exports all471 command-table
entries. Opcode0x140 (index320) is the only registration targeting67DB1F inside
66FC28. It calls8437A0 on the preserved receiver and then844138 with key9 after
normal return. Even componentNULL, HPNULL, zero component1C0 or disabledD9ED
can return without source31 creation and still reach the counter. This is a
second proven9 producer beyond the Samurai primary transition route. It does
not certify that every invocation was a failed/successful Ninja action; the
actual command packet and gameplay-success admission are separate witnesses.

The complete70,440-byte dispatcher failed actual Ghidra decompilation with a
response-buffer error; phase08 adds no export. REA_PHASE08_OUTCOME_02.json
records316 prior accepted pins and322 total evidence pins, preserves that failure
and confirms owned closure. The sealed original outcome remains unchanged. The accepted
REA_CONTINUATION_ACCEPTANCE_03 total remains232. The original command-case
proof does not require fabricated decompiler output or the unrelated preamble.

## Full current active source-map reconstruction

passive_sources/item-guardian-e2e-01 accepts three actual CLI executions and
23 named checks. It derives all56 active-form1/bank1 source records from
genuine progress/profile, configured-skill graph, resource tables, weapon and
eight context-item records plus guardian inputs. Both current core and selected
maps match every semantic group/value/counter triple and exact insertion order;
there are zero missing/extra records and zero value/counter gaps. Observed
active maps are comparison-only, never reconstruction seeds.

The native collectors run for all eight context items; per-affix masks are
127,127,0,0,0,0,0,0. A zero mask does not reject the entire item or its independent
side-source paths. A zero-valued51ED side source is preserved. Source70 is the
compiler's owner-form copy and selects the guardian pair; source71 gates
numeric addition, while source72 supplies a requirement threshold. These are
original-writer/consumer facts, not fitted active-index choices or weapon-bank
coupling. Unknown effects, attributes and guardian fields refuse.

Slot15, set expansion and original819400 follow-up remain separately scoped.
Set expansion inspects the inactive-form cached map too; proving a cached
group's absence may consume only genuine inactive count/IDs, not the active
output oracle or invented zeros. Complete source triples do not themselves
claim full actor/STAT/D reconstruction or a new blind damage prediction.

## Masked D and the retained MAIN consumer

Native37574C/8731E4 reset plus all56 original819400 calls independently predict
2672 known D bytes. Both current D comparison images match them exactly;
144 untouched bytes remain unknown. The corrected address audit proves49
writing groups/193 D bytes with nonempty exact store/recipe coverage. Old
region-name audit metadata was empty in the reset wrapper and is superseded;
it did not affect the separate known-mask comparison or binary predictions.

main_consumer/consumer-e2e-03 passes five actual CLI controls without changing
the request contract, VM or established MAIN getter/inactive-row cuts. Baseline
and independent masked D both compute732 with full score/return/initial-score-
erasure acceptance. All old core/selected D region overlaps are erased by
address before appending only predicted bits/masks.64 actual D read sites
(229 selected bytes and one core byte) touch no unknown prediction bit.
An all-unknown D reaches reader78E69D/D+7DC and blocks at78E6A5 without a score.
Noncanonical unknown CD bits are rejected before MAIN; portable CD poison is
canonically serialized and yields the identical valid masked request.

This is current-Ninja predicted D in a retained old-Samurai scene, an explicit
mixed-context consumer check. Old STAT/attack/target context remains input;
it is not a new coherent live hit or blind item prediction. Read
main_consumer/REPORT_MASKED_D.md for source pins, masks, sidecar boundaries and
the exact conditional execution domain.

passive_sources/actor-numeric-e2e-01 additionally accepts23 checks/four actual
CLIs for whole HP/KI multipliers,90/230 seeds, actor200, guardian/context-item
numeric contributions and all56 materializers. Current HP3809, KI1370,
STAT1C4=413 and1C8=469 match; all materializer-written STAT bytes match. Expected
virtual pointer relocations, weapon records and post-agility are separated
from numeric residuals. The captured900-byte windows include adjacent data;
do not treat every raw mismatch as a numeric STAT field.

## Integrated current numerical state

The later passive_sources/integrated-actor-e2e-01 supersedes that numerical
frontier with23 checks/four actual CLIs. Complete original818248 performs six
real shadow allocations of8/16/24/32/48/72 bytes, all initially UNKNOWN, then
complete2FB2BC armor, guardian/context numeric stages, the native pre-source
weight fraction, all56 original819400 source calls and the native post stage
execute. No accepted baseline STAT/D output image seeds this combined producer.
Unknown allocation registry, free-query or weapon key inputs refuse; poisoning
UNKNOWN allocation backing preserves every accepted final bit and known mask.

All2672 known D bytes match both current images. Every known numerical STAT
body byte also matches both. There are1936 known STAT bytes in total;348 of384
weapon-record bytes are known. The remaining36 record bytes stay UNKNOWN,
including3E/3F in all six records and20..37 in record4. Virtual pointer bindings
and selected structural byte10 are separate, not numerical discrepancies.

post_agility/e2e-02 independently passes44 checks/12 actual CLIs. Original
8191A2..81920E derives STAT7A0 as0.75 from weight0/capacity264 and actual230
DC22/D84F parameter getters; original819251..8193C9 derives798/79C as80/80.
The post getter uses an explicit hot TLS ABI, with unknown-TLS refusal; this
does not establish the actual running thread's TLS selection. Five supported
fields match both comparison images after prediction outputs are saved.

post_agility/score-equation-e2e-02 adds256 complete original81789C comparisons
against a readable equation, using native percentage parameters20/70/30 and
threshold scale10. The controls cover both form families, threshold edges,
unlock/modifier behavior, zero capacity and declared unsigned overflow. These
are bounded equation controls, not legal-item or natural combat acceptance.
The full from-equipment prediction still needs a new coherent damage scene
for a fresh live hit; the retained MAIN D-only consumer check remains mixed.

Whole current final3F9FC0 is separately accepted in final3f9fc0-e2e-01 with24
checks/six actual CLIs. Its actual slot array is13/9/10/15, bank array0/1, and
all eight selected indices areFFFFFFFF. Three group queries execute24 native
NULL-return paths without an inventory record. All STAT/D and both source
spans stay unchanged. A clearly artificial minimally known record with genuine
effect key4039 appends a57th source after materialization. That control proves
a late emission, not natural occupancy or a same-call numerical update.

The follow-up late-group-phase-e2e-02 passes53 checks, including six complete
positive/wrapping materializer cases and six strict unknown-input refusals.
1EE2 adds aggregateV to D7C at81A33B; C437 adds to D78 at82037D; D12B adds to
D74 at820BA5. These are direct numerical recipes. The actual kind-zero
377394 ->85B6C ->9E45B0 posthelper returns without a numerical/source change;
377440 is a separate source-compiler wrapper. A later natural reuse/rebuild
or damage effect is unknown. Do not infer it from a controlled appended record.

## Refreshed bound across migration

migration_refresh/acceptance-01 accepts27 local native controls:16 refresh/
preparation/restore, two inserted-bound dependency and nine notification-body
controls. Created bound and actual post-insertion bound are separate inputs.
The prepared bound is309.29998779296875; restore uses the returned object's
bound minus old consumed time. Under the declared preserved-bound fixture,
refreshed550/550 restores309.29998779296875. Erasing the inserted bound refuses
at3F00B5; controlling that bound to550 instead restores550. Thus unconditional
natural truncation is not claimed across the uncomposed heap/container path.

The actual type0 copy hook is a no-op, its postlinkD8 method modifies magnitude
A8 rather than timer fields, and the genuine current progress notification
body makes zero non-stack writes. These close narrow mutation leads; OS lock
calls, old-object destruction, allocation and the whole insertion sequence
remain explicit boundaries. No additional owner action is requested by this
proof. Read migration_refresh/REPORT.md for exact formulas and source pins.

## Joint STAT and D consumption at MAIN

integrated_consumer/parameter-e2e-01 accepts five actual CLIs. The retained
baseline computes732; importing independently derived current STAT and D
computes714 with normal-return and independent initial-score-erasure gates.
The portable poison request canonicalizes to identical bits/digest/readset;
required STAT and D unknown controls refuse with no accepted old score.
All160 watched sites avoid unknown prediction bits and unclassified tails.

STAT ownership is7E0, not the raw900-byte read window. Core+7E8 is the separate
HP+2D90 selected pointer; core+818 is the distinct HP aggregate. Only proved
owner/D/source pointer lanes are rebound. Derived byte10 and unknown records
are preserved. An uncaptured old718 vector entry is completed with the genuine
same-version662-row corpus;594 known old bytes across25 logical rows agree.
Native2FA678/D248F0 executes key8E2D, coordinate100, column10. No curve return,
current character output or additional execution cut is supplied.

This proof retains old source/HP/buff/attack/target scenario inputs and lists
their actual reads. It is mixed-context consumption, not a coherent live714
observation. source_export separately derives a masked SOURCE envelope from
the frozen native producer. Its quarantined mask removes the unrebuilt inactive
map, unused active capacity and unclassified record padding, leaving2528 known
bytes and56 active-form records. Current active-map knowledge cannot fill a
different attack selector's inactive map. New source-followup controls remain
separate from the sealed STAT/D result.

SOURCE independence here concerns aggregate group/value/counter outputs.
The frozen producer still accepts captured raw guardian/context-prefix and
context8 item inputs; it does not regenerate every source-context byte from
raw profile storage. source_export/INPUT_SCOPE_CLARIFICATION.json narrows the
ambiguous output-seed flag without modifying sealed artifacts. source_variants
is tracing the original profile selectors and copy routines to close that
remaining context provenance for separately declared form/bank variants.

## Masked SOURCE consumption and complete type0 creation

The additive `integrated_consumer/source_followup/DELIVERY_SOURCE.json` seal
passes seven actual CLIs. The unchanged retained MAIN selector0 genuinely
requires the still-unknown inactive SOURCE map and refuses at921BE7, without
an old-score fallback. An explicitly artificial selector1 uses the independent
active56 map. Its separately captured actorADB8=0 input closes the next real
reader; normal and score-erasure runs then return464, with identical SOURCE
poison results. Sixty actual SOURCE read sites avoid unknown map bytes and
unclassified tails. This464 is a declared mixed-context scenario, not a new
live Ninja hit. The earlier714 STAT/D seal remains unchanged.

`status_factory/e2e-01` passes32 checks/ten actual CLIs. Whole original207CEB0
executes real allocation and2073100 in1357 instructions. Native bitmap choice
selects an initially UNKNOWN C0 cell; only one guarded free-cell flag byte is
captured as a required pre-existing input. Six exact AND-memory-zero sites
obtain their known values only from original writes. The factory output has
162 known bytes and30 unknown; duration is309.2999878 and magnitude1.1.
Wrong-site, unknown required inputs and poisoned backing controls preserve
strictness. The initial factory receiver is a declared fixture, not a live
trigger acceptance.

The subsequent `status_factory/creation-e2e-01` passes36 checks/ten actual
CLIs. Continuous original3EFB90 computes the arguments, executes that complete
factory, stamps source1AE and reaches the actual FFA290 entry in2054
instructions. It preserves the native3F0032 return, full registers, flags,
stack and known masks. Genuine owner binding is HP+2DC0, whose+340 points to
the actor; the actor's nonnull component is retained. Whole ordinary insertion
and migration's returned-object bound remain separate work until accepted.

## Shared copy and external leaf support

`handlers/pass-08/VEX_COPY_INSTRUCTION_E2E.json` closes the reached backend
instruction gap with a new exact-site VMOVDQU/VZEROUPPER adapter. Six whole
original B94250 copy controls cover33/120/256 bytes, both overlap directions
and the32-byte SSE boundary; all16 upper YMM lanes and unknown/site refusals
are checked. Genuine ISA selection, native branches and known masks are
preserved. This helper proof does not increase the full E8-handler total.

Root `external_api/SYNC_LEAF_AUTHENTICATION_01.json` also authenticates the
reached IAT38E2430 as the loaded KERNEL32 GetCurrentThreadId export. Its exact
ten-byte MOV GS:[48]/RET leaf matches disk and captured memory. PE header
differences are restricted to ImageBase, whose live value equals independently
queried module metadata. Seven actual CLI controls reject changed code,
unknown bytes, wrong entry/epoch/base and unrelated header changes. A separate
offline adapter may declare a logical-thread GS+48 input; this is not evidence
of a natural game thread ID or any OS-lock outcome. No system or game function
was called by this authentication.

## Raw context reconstruction and unchanged-selector consumption

`source_variants/DELIVERY_MANIFEST.json` seals123 artifacts and1209 upstream
pins. Original eight-slot profile selection and typed copying derive1808 of
1872 SOURCE context bytes; all1808 later match both comparison images, with64
unclassified bytes remaining UNKNOWN. Independent form0/bank0 compilation
produces60 semantic triples from native reset of initially UNKNOWN cache state.
Later inactive-cache comparison matches values/counters/order. Raw-context
tests pass34 checks/five actual CLI cases; pipeline tests add16 checks.
Form0/bank1 requires other-cache prestate at5328: collector context and owner
form are different native variables. Raw prefix/guardian scope stays explicit.

Root `source_composition/e2e-02` passes seven checks/six actual CLIs with
original MAIN selector0. Combining separate semantic60 and active56 maps with
independent STAT/D yields normal-return714 and score-erasure714. Canonical
poison has identical request/score/readset. Unknown count/record controls
refuse at921BE7/921BEF. Wrong numerical binary digest/length refuses before
program preparation; V3 checks full artifacts against the pinned producer
receipt before slicing. Read-only review confirms the repaired admission gate.

The SOURCE shadow has3012 known bytes. Its64 actual read sites avoid UNKNOWN
bytes; declared old owner/+38/map guard spans and unclassified STAT tails have
zero reads. Old-SOURCE claims are limited to those monitored spans. The seal
source_composition/DELIVERY_MANIFEST.json covers52 artifacts/2272 upstream pins,
SHA256 de6da12c58a46d30072bb732660ac8e14a6cb10d25f58e71cb29928f48a7fcf7.
This is a retained mixed scenario with separately compiled per-form caches,
not a simultaneous two-cache rebuild or new live hit. Form0 numeric rebuilding
is not obtained by relabeling captured form1 numerical seeds.

## Ordinary insertion and recovery profile06

`status_factory/insertion-e2e-01` passes41 checks/11 actual CLIs. The same
creation CPU executes1164 original insertion instructions, including the
ordinary world gate, real class30 allocation and empty-tree linking, to the
unexecuted EnterCriticalSection call9B97EA. Status/node masks remain162/36
known bytes with30/12 unknown. Under explicit normal-lock-return ABI,
separate original registration-body/epilogue slices have zero nonstack writes
and reach real3F0032. Full OS execution and migration's actual returned-object
bound remain distinct extensions, not unconditional natural claims.

Recovery profile06 adds eight context/shared-helper locations to profile05:
114 text plus6 rdata anchors and nine individually decoded RIP roots. Actual
same-version migration and four root-recovery CLI controls pass, including
independent code/data shifts and opcode/unresolved-site refusals. Profiles01
through05 remain preserved. These120 byte locations do not approve an
unobserved game update or product enablement.

## Constructor flags without an old heap-byte input

`factory_flag_masks/DELIVERY_MANIFEST.json` adds a stronger, separate factory
domain:34 artifacts/80 pins, six actual CLI cases/nine checks. Whole original
207CEB0 executes with all192 status payload bytes initially UNKNOWN and no old
payload byte file/value admitted. Exact3F1B27 AND-FC establishes only two known
zero bits, leaving six preserved bits UNKNOWN. Its flag effects are dead until
original XOR ESI,ESI3F1B35; the two intervening LEA/MOV instructions consume no
flags. Every later memory read remains subject to the original strict policy.

The resulting status has161 fully known bytes,30 unknown and one partial byte
at16 (knownmask03). Duration and magnitude match the earlier one-byte-input
proof on all new known bits. Poison, registry/unit/wrongsite/out-of-cell
controls pass;1280 isolated original-AND mask cases verify the bit/flag policy.
The earlier one-byte proof is unchanged. This removes an incidental backend
input requirement for the declared factory path; it does not prove every later
flag consumer, natural trigger or whole migration with partial flags.

## Derived post-insertion migration bound

The later status_factory/migration_bounds/DELIVERY_MANIFEST.json closes the
previous explicit Bafter input for a declared two-object scenario. Its seal
SHA256 is9708f6f93da55b1a87b670823ef8ab920c88ead21c99878db46600b9dca28868;
50 checks/16 actual CLIs and1332 transitive pins pass. Both objects are real
native allocations/constructors. The old250 state is set through3EF5A8 and
original refresh produces550/550. Native serializer/options, migration creator,
old destructor/frees, tree replacement and ordinal lookup then execute.

At3F0051 RAX and at3F00B5 RBX identify the second actual C0 allocation. Its
bound is derived309.29998779296875. Original restore reaches3F00C6 with:

| Old remaining/bound | Consumed | Actual new bound | Restored remaining |
| --- | ---: | ---: | ---: |
|550/550|0|309.2999878|309.2999878|
|530/550|20|309.2999878|289.2999878|
|200/550|350|309.2999878|0|
|550/550, duration option off|0|300|300|

Unknown actual new bound refuses at3F00B5; no completed result binary is
emitted. These are native-unit controls under explicit normal-OS-return slices
and a controlled old-object serializer entry. Upstream natural callback
admission, post3F00C6 HP callback/ordinal cleanup, uninterrupted OS completion
and natural duration-loss frequency remain unclaimed. The new whole-object
dataflow does not retroactively turn the earlier no-renewal natural trial
into a positive renewal observation.

The serializer's exact MOVSD/MOVUPS/shift/shuffle movement adapter preserves
unclassified unknown lanes. Its separate16-check proof checks original bit/flag
and upper-lane behavior, poison and real required-field refusals. No timer,
post-insertion bound, old-payload padding or callback return is supplied.

## October20 natural renewal and migration vector

The guided live01 capture has726 complete frames, normal stop and a closed
read handle. Its strict file analysis passes11 actual CLI cases and is sealed
by guided_capture/live-01-analysis/DELIVERY_MANIFEST.json,
SHA3b0836153cd4abbc6c677e5989436cba4fe240fee08a93f0e9750fefa52ddc5a.
The owner later reported an extra Ninja counter attempt; the sidecar is pinned.
No repeat recording is required for the initial migration comparison.

At393->394, the same sampled source1AE object/node has elapsed118->122,
remaining191.2999878->487.2999878 and bound309.2999878->489.2999878. Raw
D9F0 is300. Original refresh is compatible with2countdown units before adding
300, then2after; this split is inferred from endpoints, not witnessed calls.
The bound expands from the current remaining, not by adding300 to old bound.

At424->425 Samurai changes to Ninja, source1AE gets a new payload pointer and
elapsed212->2, remaining397.2999878->215.2999878, bound489.2999878->309.2999878.
The consumed term is92=oldB-oldR, not old elapsed212. Observed newB minus92
minus a declared2-unit postcreation countdown matches the new remaining f32
bits exactly. NewB is observed in this comparison; this is not independent
regeneration of the complete current duration producer or natural callback.
The previous native two-object construction/restore proof remains separately
scoped under its original caller and synchronization assumptions.

Source1AE first becomes sampled-absent at498. Source31 is first sampled at588,
8.1503625seconds after first Ninja425. The extra counter therefore does not
automatically invalidate the earlier1AE vector; the later no-counter/rest
protocol was not clean. First observation does not certify the actual action
instant or exclude transient between-poll states. All248 occupied endpoints
show unsuppressed type0 A8/AC and aggregate1.1, and478 empty ones show1.0.
At terminal497/691, remaining is0 while sampled aggregate is still1.1; the
next sampled empty endpoints return1.0. Timer positivity alone is not an
attack-effective uptime rule. No attack/damage hit was collected in this run.

Historical793 remains unreplayed:720*1.1 is792. Complete prefinal context,
pool placement and native rounding are required to explain a displayed793;
this natural timer record does not certify that damage value. Same-key
competition, inner type0 aggregate multiplication and downstream boost/cut
pool arithmetic remain separate layers, not one global stacking shortcut.

The October20 offline follow-up closes the marginal arithmetic question in a
retained selector0 scene: actual MAIN714->785 with only aggregate+7C1->f32(1.1),
23checks, independent28artifact/39assertion review. The separate sealed879->952
example demonstrates same-positive-pool dilution; these are not current hits.

The current observer input is nonempty and genuine native continuation reaches
A2DD02/15instructions, missing first-node+18. Twelve CLI checks and a173receipt
bounded audit confirm the saved current-epoch raw gap. New code-only334-byte
export identifies A2DCE0 as a status-association lookup, not callback dispatch.
Its immediate consumer conditionally ORs status.byte14 with1. Sixty isolated
original-block controls establish that the selected retained prefix's known01
flag is unchanged across15 observer/descriptor alternatives. Full lookup and
creator remain unexecuted; the scoped bit result does not authorize skipping
UNKNOWN reads in strict replay. See offline_continue_20261020/OBSERVER_LOOKUP_SCOPE.md.

Progress projection now reaches the actual flag reader374C41 after132026 native
instructions. The captured36-byte key3 node ends before byte+24. Declared0/1
alternatives both reach a genuine further key273/bucket31 gap. Neither partial
HP55 nor new-record initializationflag0/level1 establishes full HP or the actual
level50 flag. Independent file-only review verifies55frozenfiles/14checks and
zero target-range overlap in173receipts. No further game action is requested.

Latest profile09 passes actual migration and four root-recovery CLIs with
126text+6rdata locations/ten independently decoded roots. New reached flag and
factory-bank anchors do not certify input layouts or an unobserved update.
