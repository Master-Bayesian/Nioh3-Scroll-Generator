# Combat/passive research recovery after a game update

This extends GAME_VERSION_UPDATE_PIPELINE.md and OFFLINE_PARAMETER_ACQUISITION.md.
It is a research recovery procedure, not product compatibility approval.
Current package: F:/Nioh3_r16_build/deliverables/r16-live-passives-20261016/.

The newer controlled guard vector is
F:/Nioh3_r16_build/deliverables/r16-counterattack-live-20261016/.
Read [its accepted evidence](../research/NIOH3_COUNTERATTACK_DEFLECT_LIVE_ACCEPTANCE_20261016.md)
before using old NEXT_COLLECTION instructions. The old source-debugger route
is suspended after an arm-time fatal error; the successful continuation used
OS-only reading. Reusing old mocked or zero-callback results does not approve
another debugger attempt.

The current native mechanisms and bulk catalog are
[recorded here](../research/NIOH3_NATIVE_BUFF_MECHANISMS_20261019.md), with evidence
at F:/Nioh3_r16_build/deliverables/r16-native-buffs-20261019/. This follow-up owns
exact1AE/312 creation, duration arithmetic, growing refresh bound, strongest-key
selection, mixed damage pools and the node-padding correction below.

## Minimize owner-operated experiments

Build the native skill/group/D-field/status-class/source catalog first. Recover
common creation, application, extension and removal consumers, then validate a
small set of representative mechanisms. Do not repeat a skill-toggle/action
sequence separately for every buff merely because the first calibration used
one. Unknown source edges stay unknown until the catalog/control evidence closes
them; a common magnitude is not a common source identity.

Prepare the collector, classifier and offline analysis before asking the owner
to act. Batch independent actions into one concise protocol when possible.

For the equipment-evaluator continuation, use the current
[October20 collection record](../research/NIOH3_REAL_GEAR_LIVE_COLLECTION_20261020.md).
Its actual constructor exposed action flags beyond the earlier MAIN-only read
set and HP-owner fields beyond3300. The next collector composes the frozen core
with the bounded action/profile/owner-tail helper in one call: actual E8 header,
flags prefix400, modifier pointer array with native start/count (not a linked
list), profile byte45 and owner tail through4000. Keep byte/read caps and refuse
oversized geometry; a larger raw prefix is not a claim that every byte belongs
to a typed object. Validate the actual core/action pointer join as well as each
component's own equal rereads. Two individually stable snapshots with different
E8 addresses cannot be joined by role name. Before another owner request, use
authenticated extracted static resources and explicit offline OS lifetime
semantics to expose the next genuine mathematical-input frontier.

Maintain explicit bounded coverage and give a clear ready/stop signal; an
owner-confirmed action after a silently expired window is uninformative, not a
negative. Prefer bounded rotating recording for longer action batches once its
clock/identity/coverage semantics are verified. Do not relax resource limits or
interpolate across gaps to avoid an owner request.

## Versioned semantic contracts

Keep semantic identities stable: configured skill key/name/cost, source
eligibility, actual emission, guarded contact, perfect deflection, status
creation/refresh/end, outgoing multiplier and coverage state. Bind each to
executable/section identities, native anchors, layouts, tables, raw controls
and evidence grades. An address match alone does not establish semantics.

Machine records are analytics-versioning/RESEARCH_TRACKING.json,
ANALYTICS_CONTRACT.json and live-discovery-extension/LIVE_DISCOVERY_EXTENSION.json.
The cloned R16_LIVE_DISCOVERY_BASELINE.research.json uses the existing migration
profile schema and remains an unapproved candidate.

## Recovery sequence

1. Preserve old sealed inputs and failed controls. Record new file version/hash
   and dump loaded.text/.rdata/.pdata with the existing read-only tools. Do not
   assume disk.text equals loaded code. Image bounds use SizeOfImage, not file size.
2. Run prepare_game_version_update.py against the previous research profile.
   Preserve ambiguous/changed anchors and explicit overrides. Do not apply
   one guessed RVA delta to all pointers or structures.
3. Derive data roots from matched code. Current text MOV at5A4B07 ends5A4B0E;
   its signed displacement at5A4B0A yields45C8450. Recompute from target bytes,
   then independently validate heap geometry, locale and ownership. Literal
   old offsets do not become approved target roots through code relocation.
4. Re-capture manager130 and598. Derive target stride/count/key/rank through
   native consumers instead of forcing1101/C0/94. Compare payload hashes and
   key sets, and verify the complete pointer permutation independently.
5. Re-capture character/player/owner/form and the skill graph. Current6020 has
   839 nodes for this character only. Check list/sentinel/buckets/counts/keys,
   bounded reads and rereads. Learned, configured, source-active and actual
   descriptor-effective states remain different claims.
6. Resolve explicit full u32 text IDs. Current chain5A4B04 ->63D9C4 ->63DDA0
   reads map atroot+1CD0, locale+1CE4 and pack count+1CE8; node key+20 and
   UTF16 pointer+28. Reconfirm target layout and locale bounds. Use bounded
   lookup and reread consumed nodes/strings; do not scan the whole process
   or call a game function to bypass an unknown contract.
7. Rebuild metadata/name joins. Current130.u16+18 joins598.u16+0/rank+4;
   name ID is598.u32+C, cost is130.f32+84. The same offset in another table
   has no implied shared meaning. Zero IDs, empty strings, UNUSED masks,
   missing keys and optional text stay distinct;598+10 is not a rank suffix.
8. Revalidate UI order and a complete checked menu control. Current order is
   enabled first, cost descending,A0 descending,598+8 ascending,rank descending.
   Do not guess an unresolved final tiebreak. Current25 rows close31/40/40
   capacity and match direct names; retain images and transcription corrections.
9. Capture actual source emission. Current four sites556118/5569B9/556A0E/
   556B04 joinRCX/RDX/R8, target thread/frame, selected/core/source owner,
   descriptor+48 and complete selected/submitted events. Zero events do not
   prove an empty override. Temporary/preview contexts remain separately scoped.
10. Run numerical, event and lifecycle controls for the intended domain.
    Byte parity, UI attribution, causal trigger, clock calibration, recorded
    replay and new-version live acceptance are separate gates. Preserve all
    product/save/legal-domain/release approval boundaries.

The extension's UPDATE_CHECKLIST.md contains exact current vectors. Bundled
RUN_OFFLINE.ps1 tests same-version relocation through the existing tool; it
does not accept another game version.

## Controls to retain

- HP300 template/value reproduces the selected Common description.
- Every configured name has full ID/node/UTF16 provenance. Preserve native
  U+3000; ASCII spacing is presentation-only. Do not normalize similar glyphs.
- All25 costs agree across source130, native UI order and screenshot totals.
  The four zero-cost Samurai foundations stay separate from configured skills.
- Native598 pointer/key/rank parity and whole payload hash bind actual rows.
- F4C5 total220 versus skill contribution120,2673/value300 versus D9F0 and
  A628/presence0 versus D2FA are local consistency controls. They do not prove
  a clean nongear basis or all origins.
- The newer236BBC4/236C0C5 path joins D9F0 to source1AE, manager7A8 row272,
  descriptor312 and class0. Preserve older2366098 as a separate field reader.
  Raw300 is still not an unconditional wall-clock duration.

## Recover buff definitions and shared consumers in batches

1. Derive the manager global from admitted code; current is Q(module+45B9E30).
   Recover context+7A8 (406 rows, stride2C) and+7A0 (238 rows, stride1C), their
   8-byte payload headers, complete native hash contexts and index vectors.
   Current vectors contain813 and477 entries. Re-read context/payload/hash data
   and retain missing/sentinel-key controls. Counts are version facts, not
   compatibility assumptions.
2. Re-execute original getters3F0170/3F01C0/3F0250 and78CC90/78CCE0/78CD70
   against every captured key. Verify row7A8.u16+2A to descriptor-key joins.
   Current payload SHA256 values are bfea2fdec5a6278166400e092fd0379c95cdbc4343279ddd2f611daa1ef40538
   and b9a39a7df9ca431adf0227222de5c97a49a672e5de8eae5a4ac4bf9dacbbcd2b.
   They exactly match retained resource00080.bin and01072.bin, respectively.
3. Re-derive primary/help text-field consumers. Current1F15EDC reads descriptor
   u32+0 before5A4B04;1F09162 reads u32+4 before5A4B04. Preserve zero text IDs,
   raw IDs, actual locale and generic-label/source distinctions. Batch explicit
   nonzero IDs; do not scan the whole process or call game functions.
4. Resolve original vtable pointers with at least independent known-method
   base controls. Seed functions from.pdata boundaries, not interior labels.
   The current shared-method family contains166 classes; do not assume classes
   overriding all seed methods are enumerated. Run dispatch with declared
   contexts and keep null/unknown dependency cases distinct from invalid rows.
5. Recover reset, strongest-key selection, creation/source lookup, active/suppressed
   application, refresh and final damage consumers. Validate representative
   classes in original-instruction interpreters with known-memory admission;
   never substitute an unknown helper return merely to complete a model.
6. Preserve decompiled-but-unverified handlers as such. Current native catalog
   joins375/406 definitions,79 straight-line class recipes,87 complex decompiled
   handlers and31 explicit creation-definition frontiers. Organize continuation
   by shared aggregate destinations and gates, not individual menu activations.

## Identity, effectiveness and time corrections

On the proven native insertion route, node.u16+20 is the grouping key,
node.Q+28 is the object and node+22..27 is padding. Historical
source_metadata_u16 is a deprecated raw-padding field, not source identity.
IncomingR9W is stored at status.u16+18; creation definition is status.u16+74.
Only a native caller/materializer join identifies a skill. Preserve old sealed
captures and apply versioned interpretation rather than rewriting raw evidence.

A648C picks greatest virtual+50 strength per low16 key. Suppressed objects can
remain/tick while contributing nothing; flags15 mask08 controls the suppressed
branch. Current40 positive class0 definitions share312, while other classes can
also write the same outgoing aggregate. Presence, unsuppressed application,
attack eligibility and marginal damage gain require separate records. Use
ANALYTICS_STATUS_CONTRACT_V2.json from the native evidence root for new work.

For source1AE, use the separately rounded STAT1AC/definition24/HP2DDC and
D6F8/D770 creation formula; do not guess a modifier from309.3/300. Refresh adds
D9F0 to remaining, growing bound if needed; it does not reset elapsed time.
Recover the raw engine clock upstream as well:39E520 computes60/ctx.u32+10,
396240 packs it verbatim, B0B70 adds owner8E0, and A481C applies status/rate
modifiers. Distinguish native timer units, observed normal-play units/second,
pause/slow-motion behavior and certified wall-clock seconds.

Re-derive STAT1AC as an input producer too: current818B4C selects synchronized
attributes at Q(STAT+7D8)+34; index5 feeds818AEC's signed1..999 clamp and
table90.u16+14. Original818B68/81904E uses f32(.0001), then819327 adds only
strictly positive D+A8. The selected current source is embedded atSTAT+810 and
already covered by the900-byte capture; core's source is at+12C8 and is not.
Retain this pointer/range distinction, native constant bits38D1B717 and table90
hash cd2ab342a202fd8dde5eebb459212293f497e52ffa7f05a35b5d69e6fde230f2. Do not
guess a character UI name from an attribute array position.

## Actual REA recovery on this Windows host

Use the official pinned REA6.4.0/Ghidra12.1.4 toolchain and JDK21. This host's
F: source volume is exFAT; Windows REA requires NTFS native authority for targets
and private runtimes. Stage immutable target/tool copies under a named D: build
scratch directory and set child TEMP/TMP there. Keep canonical evidence on F:.
Do not bypass path/DACL/Job Object admission or change global model settings.

The accepted task uses an analysis-only PE carrier containing exact original
.text/.rdata/.pdata and uninitialized uncaptured globals. It is never executed.
Its task-only Node import adapter selects explicit.pdata seeds and-noanalysis;
report that profile instead of claiming broad default analysis. Native code
remains the authority where inferred decompiler ABIs disagree. The REA seed
commitment hashes canonical lowercase-address/LF text, while raw files may be
CRLF; keep both hashes and do not confuse them. Close the binary, close MCP and
independently verify only the owned analysis processes are gone.

rea/README.md in the native evidence root records commands, original provider
hashes,136 actual decompilations and independent original instruction checks.

## Combat statistics acceptance

Perfect guards per minute require unique certified native contact/episode
IDs and known eligible combat time. Count failed actions and recovery inside
the encounter window. Exclude verified menu/loading/death or explicit
out-of-combat periods; unknown eligibility is not silently excluded. Never
use only time between successful guards. A zero denominator is null.

Join incoming guarded packet, action generation, native thread/actor and
appropriate outgoing deflect marker, or a separately validated alternate rule.
Equal timestamps do not justify merging packets. The historical4/20 labels
remain separate from3 complete patterns,1 incomplete candidate and25 packets.

Buff uptime requires configuration/form/source eligibility, status-instance
identity, creation, refresh/extension/stacking, end and coverage boundaries.
An already-active object is left-censored. Do not carry state through missing
callbacks or join different four-slot phases as one continuously observed
segment. Keep known active/inactive and unknown intervals, coverage and bounds.
A shared aggregate multiplier cannot alone name its buff or source.

Use a monotonic clock with explicit epochs/discontinuities. Preserve raw game
timer coordinates separately and calibrate pause/slow-motion/simulation-time
relationships before conversion. Player feedback must account for opportunities,
configuration, encounter mix and data coverage; an unavailable skill or
ineligible form is not poor buff management.

## Lifecycle and reproducible handoff

Prepare/inspect/mock before requesting an owner trigger. Use at most four
owned hardware slots, bounded resources and fresh PID/birth/module/site checks.
Every callback continues, including error/timeout/inactive paths. Stop only
owned resources and preserve first pending plus independent final proof.

CE APIs can return non-boolean functions. Prove running/stopped state with
explicit inventories and current-context evidence. Delayed removal can outlast
short timer retries: release/reconnect the same session and reconcile its
inactive owner. Pin CE child TEMP/TMP under the build root when the wrapper's
temporary directory will disappear. Preserve source/input hashes, masks,
controls and stop/cleanup receipts; verify archive/extraction and recorded
replay. No publication follows automatically from research or relocation.

## R16 continuation: current recovery and collection sequence

Canonical continuation evidence is
F:/Nioh3_r16_build/deliverables/r16-native-buffs-continue-20261019/. Earlier
sealed packages remain immutable. Use PowerShell7 and the project Python
runner, with explicit NIOH3_BUILD_ROOT=F:/Nioh3_r16_build. Child Python uses
-E -s -B. Analysis scratch may use the named NTFS REA stage, but canonical
evidence and all retained replay outputs stay on F:.

1. Run versioning/e2e_recovery.py with --profile pointing to profile-05/
   NATIVE_BUFF_COUNTER_CLOCK.research.json and a fresh --output-dir. It invokes
   the actual repository migration CLI against the retained manifest, checks
   106 text and6 rdata anchors, preserves candidate-only approval and leaves
   every runtime data site unresolved. The unsupported-schema refusal remains
   part of acceptance. For a new game, use a newly captured version-matched
   loaded-section manifest; the same-version control does not approve it.
2. Recover each supported global slot from its own relocated RIP instruction
   using recover_data_roots.py and profile-05/RECOVERY_CONTRACT.json. Do not
   transfer a uniform code/data delta. Signed displacement recipes separately
   derive parameter manager45B9E30, pending-counter manager45BBE38, world state
   reference45B9E40, engine root4759060, progress4751860, source-level4751848,
   cache474F898, inventory4751530 and allocator registry4B50348. roots-e2e-03
   and e2e_data_roots_v3.py verify independent
   code/data shifts and refuses changed opcodes or unresolved instructions.
   A code-derived slot is still not an initialized live pointer/layout claim.
3. Revalidate process birth, executable and loaded-text hashes, root ownership,
   parameter table key/count/stride/hash layouts and original consumer bytes.
   Source130/598/7E0/7C0/7C8 are different tables. Progress5EA8 uses a u16 key
   and value+18 in32-byte nodes; progress5DE8 uses u32 key+10/value+14 in24-byte
   nodes. The source-level maps under the separate4751848 root have their own
   byte-key/column layouts. Never reuse a nearby map's layout by analogy.
4. Replay passive admission with form and weapon bank as independent inputs.
   Preserve the default descriptor+48 override, source reset and compiler
   order. Compare output maps only after reconstruction. Keep captured source
   arrays, current progress and immutable parameter resources qualified by
   their individual capture epochs/brackets; they are not one atomic state.
5. Use acquisition/RUN_CAPTURE_V4.ps1 with a new output folder, at most180
   seconds, interval at least50ms and explicit output budget (default128MiB,
   hard256MiB). READY means the actual window/limits are armed. Inspect the
   actual stop reason and unknown components, not the requested duration.
   The collector reads166 family headers,79 authenticated linear views and
   six native current/previous action-storage slots with resource banks and
   serials. Native18, actualCLI26 and a genuine41-endpoint quiet audit pass.
   Complex numerical effects and atomic/postcommit/event identity remain
   unknown. Frozen V3 remains available for its accepted legacy schema.
6. Decode the legacy combined V3 capture with natural_combined/summarize_trial.py.
   Preserve valid status/counter components when only activity is unknown.
   Use actual monotonic read brackets for timing bounds. Keep form rebuilds,
   pointer reuse, same-source refresh and different-source replacement distinct.
   The combined trial's status source is payload+74; node+22..27 is padding.
   V4 has a distinct schema and ordered read transcript. Do not send it through
   a V3-only decoder or promote the quiet-only audit to general combat
   acceptance. action_identity/SCOPED_ACCEPTANCE.json records the supported
   fields and exact boundaries; a general V4 combat/event evaluator is separate.

Namespace examples must remain explicit: skill A7AA/反攻格挡 ->group2673 ->
D9F0 ->creation1AE ->status312 ->type0/vtable3AA8120 ->aggregate7C. Ninja
D18D/反攻识破 ->group7DC8 ->D9ED ->creation31 joins the same status/type.
Progress key47A4 value9 is not pending event9. Native resource labels10158 and
18706 in this analysis are hexadecimal keys65880 and100102. Source A0 is an
insertion/migration identifier, not the source definition.

Entity+A68 retains its frozen V3 historical name only for compatibility; sampled
1909/2D3F correlate with item68+58 category dispatch and do not certify action
IDs25036/etc. Pending counter9 is global/resettable and can increment for a
different eligible owner. Use neither field alone for perfect guards/minute.
V4 calls it raw_entity_A68_u32. Actual stored action keys are u32 at headers
Q(entity+E8+8*layer) and Q(entity+118+8*layer), with six constructor-proven slots.
35BC68 publishes pointer/bank fields before incrementing serial318+4*layer;
stable polling can therefore see new fields with an old serial. Certification
requires an executed postincrement35C291 witness and owner/layer/thread/frame
join, rather than assuming that serial brackets provide an atomic journal.
Separate creator eligibility, transfer eligibility, existing-status application
and attack effectiveness. The combined trial preserves Samurai-created1AE
after form0->1 with factor1.1 even though the ordinary Ninja source compiler
does not admit A7AA's creator gate. Do not delete an existing status or exclude
all of its possible uptime solely because the current form cannot create it.
That observation is not a buffed Ninja-hit replay; relevant attack consumers
still require their own evidence.
The38-case native pacing proof and201-endpoint silent calibration support a
conditional current main-phase rate; they do not turn sampled counters into
lossless event observations or certify every intervening frame.

For equal-magnitude312 statuses, apply the mode0 insertion predicate before
the strongest-per-key rebuild. Under negative A0, equal strength/raw28 can
replace the old object independently of source74; unequal strengths can
coexist. Keep successful allocation as an explicit boundary unless its native
allocator/lifecycle has been executed too. The nine-case receipt is
equal-strength-insertion-02/E2E_RECEIPT.json.

versioning/README.md supplies exact current and new-manifest commands. The
same-version wrapper has a fixed retained manifest; invoke the repository CLI
directly for an actual updated game. Apply SEMANTIC_ERRATA.json to the legacy
3FAA98 anchor label; signature parity does not authenticate an old semantic
description. ANALYTICS_STATUS_CONTRACT_V4.json adds the proved insertion and
migration rules, corrected aggregate/tree layout, component-quality handling,
typed math inputs, timing boundaries and raw A68 interpretation.

For numeric prediction, use the accepted integrated-actor and final3f9fc0
producers with their known masks. The900-byte STAT read is a transport window,
not an object extent: core+7E8 is HP+2D90's selected-STAT pointer, core+818
begins the HP aggregate, and selected+810 is a separate source object. Replace
only authenticated owned bytes; keep adjacent scene/HP/source context in its
own namespace and audit every retained dependency. Never copy an observed
selected byte10 or unknown weapon-record bytes to complete a prediction.

For refreshed migration, distinguish Bcreated from actual Bafter_insertion.
The native restore demands the latter. Preserve migration_refresh's27-control
scope and heap/OS-lock boundaries when rebuilding after an update; matched
factory arguments alone do not prove an unconditional whole-path result.

The current complete factory and continuous creation proofs are separately
versioned under status_factory. Preserve UNKNOWN allocator backing and derive
the actual selected cell from current bank/bitmap metadata; a guarded required
free-cell byte is not a constructed-object image. Do not reuse the classC0
allocator bank for the distinct ordinary node's size30 bank. Shared world
collections contain bucket heads and linked NEXT nodes; compare complete
ancestry, topology, phase and shared fields across bounded packets before
composition. Sequential reads alone do not establish an atomic snapshot.

The reached copy helper retains genuine ISA selection. Its new adapter admits
only authenticated original VEX instruction sites, with explicit upper-lane
and known-mask controls. If a reached import crosses the game module, identify
the import and exact nonforwarded export, query loaded-module ownership, pin
the DLL file, compare captured leaf bytes and PE headers, and declare any
logical-thread input separately. external_api/REPORT.md records the current
GetCurrentThreadId proof and its only permitted ImageBase header difference.
An OS update is also an identity change; do not assume the same external leaf
or supply a helper return to get past a new boundary.

Current recovery candidate is versioning/profile-08 (122text+6rdata anchors,
ten RIP roots). Its README explicitly passes profile08 and e2e08 paths to
the frozen root-recovery wrapper; that wrapper's defaults still select05.
Keep01..07 and accepted/rejected receipts. Same-version e2e08/roots-e2e06 are
bounded relocation checks; they do not approve an actual update. Profile07
found all locations but its new root recipe refused a24-byte-versus7-byte
signature mismatch. Profile08 preserves the exact one-instruction contract.
The new root independently decodes45C84F8 from2086D4; its rawbyte1DC value,
meaning and target owner layout remain separate runtime evidence.

ANALYTICS_STATUS_CONTRACT_V5.json adds the separately compiled form0/bank0
semantic cache and unchanged-selector consumer. Preserve actual native map
initialization and unknown counts; owner form and explicit collector context
are independent. Raw context copy known masks exclude64 unclassified bytes.
For numeric exports, authenticate the producer receipt and each full binary's
expected length/hash before slicing; hashing whatever file exists is not
upstream artifact admission. source_composition/e2e-02 has actual rejection
controls. The retained-scene714 remains separate from a fresh coherent hit.

ANALYTICS_STATUS_CONTRACT_V7.json also records the actual returned migration
bound, explicitly private empty-observer creator tail, authenticated disk3F0
prefix and current recovery profile. Do not merge private tail completion,
cross-epoch startup-gate projection and natural whole callback acceptance.

Before collecting another parameter table, check the exact disk/native range
joins in OFFLINE_PARAMETER_ACQUISITION.md. Manager3F0 is the13,320-byte prefix
of payload00482,52x256-byte rows; its6,562-byte resource trailer is unclassified.
The earlier whole-file mismatch is superseded by that explicit prefix proof.
form0_numeric/disk3f0 validates the full19882-byte resource digest before
slicing, preserves unknown trailer, and requires an explicit context binding.
Old pointer values are provenance, not current addresses.

After any game restart, bind the guided file watcher to a fresh authenticated
identity using run_guided_capture_v5.py --identity, retaining the frozen V4
collector and reviewed state_v3 semantics. Confirm actual training-state
roots and a successful genuine quiet run before READY/combat instructions.
Startup NULL actor/counter preflight is failed-closed, not an empty buff state.
No source growth, sound, action or absence is inferred from a refused run.

October20 latest recovery candidate is profile09 (126text+6rdata anchors,
10independentRIP roots); e2e09 and roots-e2e07 pass actual migration/root CLIs.
It adds the actual progress-flag caller/read and distinct factory class32
bank instruction. Keep profile08 and earlier receipts immutable. A recovered
code location does not supply node+24, progresskey273, a factory bank value,
callback packet or updated-version acceptance.

For future minimal progress captures, first inspect the actual stalled caller,
not just another wrapper around the same hash helper. The key3 u32 value getter
reads+20, while the actual HP flag getter reads+24. A36-byte node prefix ends
before the latter byte. Capture a code-proved bounded value-object extent and
quarantine opaque bytes; never infer adjacent flags from the known level.
The two declared flag alternatives both require further key273 inputs, so a
new collection plan must consider the caller's full bounded query sequence.
Current session is closed; this is preparation, not a request to reopen it.

For coverage, preserve remaining0 endpoints whose independent aggregate factor
is still1.1. Separate sampled object presence, suppression, aggregate effect,
attack eligibility and actual marginal damage. The complete MAIN factor
counterfactual714->785 and separate879->952 same-pool example verify arithmetic
placement, not natural current hits or universal independent multiplication.

October20 Pro-return acceptance adds a Windows portability gate: relative
manifest keys must use.as_posix(), never platform-native path spelling. Keep
original returned bundles immutable and record any local port's exact diff and
original hashes. File/model tests do not become original-instruction proof.

The accepted finite E8 control-domain count is now86/87 complex and165/166
combined; see NIOH3_R16_PRO_RETURN_ACCEPTANCE_20261020.md and the additive
POSITIVE_DOMAIN_ACCEPTANCE.json. Preserve the79/87 seals and distinguish full
positive RET, a helper prefix, a rejected application and a write followed by
UNKNOWN. Current-player/natural/all-branches acceptance is still separate.

For collection minimization, all six current manager400 rows have effectID0:
under the normal-return/stable-map/disjoint-memory contract,5565E2..556689
preserves source bytes/masks without demanding its progress flag or bucket.
Do not generalize this to the later556854->817740 selected numeric levels.
Descriptor1909FC8's default16zero bytes are derived from original writes; the
private initialized-CRT domain does not certify a current dynamic descriptor.
Ordinary marker key20F7 already exists in the retained parameter230 map; asset
code/data must be checked before asking for a new game session.

Root section admission now checks actual PE metadata, raw dump extents and
VirtualSize separately. The5AC71E root instruction is a STORE. Literal profile
signature collisions do not establish a migration bug: the existing CLI uses
unique neighboring context votes. Its successful current14-site control and
10root ownership check do not approve a future changed binary or its pointees.
