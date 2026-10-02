# DLC1 version diagnostics and reviewed compatibility policy (2026-10-02)

## Scope and source identity

Owner-authorized source work on ARASHI, branch `codex/astra-audit-20261002`,
based on `ec5575c37f340558c000d4f87ece6869802f1b96`. This batch preserves the
previous offline reliability repairs and all unrelated checkouts. It does not
change test9, publish a package, attach to a real game, or write a real save.

The goal is evidence-based compatibility across actually released DLC1-era
versions, with deliberate opt-in for independently verifiable executable
variants. This is not a claim that every historical or modified executable is
now adapted. Missing layouts, resources and native-call semantics cannot be
recovered from a version label or a matching short signature.

## Released versions and verified local evidence

The official update feed retrieved on October 2 lists four DLC1-era releases,
including the pre-DLC update. DLC1 Hell Rising was released on August 19.
Sources: [official updates](https://teamninja-studio.com/nioh3/us/update/),
[official update data](https://teamninja-studio.com/nioh3/assets/insert/update.json),
and [DLC announcement](https://www.gamecity.ne.jp/news/29141.html).
The extracted version/date receipt is `sources/official-updates.json` in the
external evidence root. A public release name is not a Windows FILEVERSION.

| Public release | Date | Observed FILEVERSION | Resource mapping | Current capability and evidence |
| --- | --- | --- | --- | --- |
| Ver2.00.01 | 2026-08-18 | Unknown; no sample found | None | Not adapted. No inferred tuple, layout or ABI. |
| Ver2.00.02 | 2026-08-21 | `2.0.0.2` | `r4_finalizer/pc_v2_00_02/resource_v1` | Existing offline scroll support; experimental character layout; no native scroll/count or seeded equipment insertion binding. |
| Ver2.01.00 | 2026-09-02 | `2.0.1.0` | Same 2.00.02 payload, supported by recorded byte equality | Existing offline scroll and native scroll/count bindings; experimental character layout; no old seeded equipment builder. |
| Ver2.02.00 | 2026-09-17 | `2.0.2.0` | `r4_finalizer/pc_v2_02/resource_v1` | Registered current character, native scroll/count and seeded equipment paths; each retains operation-specific checks. |
| Other readable version or modified image | No invented release/date | Actual file value is displayed | Never silently borrowed | Host diagnostics remain available. Known-version whole-file differences may use reviewed consent; an unknown layout/ABI remains blocked. |

Nine retained `.text`, `.rdata` and `.pdata` capture files across 2.00.02,
2.01 and 2.02 were hashed read-only and matched their manifests. The receipt
`version-section-hash-receipts.json` names every path, size and SHA256. The
installed 2.02 executable is 77,830,112 bytes, SHA256
`E22C4A635E4EC1E27A177B76E27D7F6A637F426C0ED3928B60F5693BC52AE130`.
It was read as a file only. These checks validate retained evidence identity;
they do not establish full historical parity or acceptance of an unofficial
executable. No such executable was downloaded or run.

The old 2.01 character global remains inferred from the inventory global plus
0x320. Old-version inventory pagination/menu readers still have 2.02-specific
sites. Neither limitation is upgraded to complete old-version support here.

## Gate classification

| Gate family | Policy | Result |
| --- | --- | --- |
| Whole executable SHA and an already registered older version | Advisory for scoped live addition; consent for the other documented native capabilities | Scroll/equipment addition checks its own supported binding, target, backup, preview and operation receipt. It does not require the global compatibility plan. Other opted-in capabilities retain their exact reviewed consent. |
| Version metadata without trusted resources/layout/ABI | Diagnostic, with an unresolved hard dependency | Display actual version and missing bindings. A structure-only match never grants native calls or writes. |
| Administrative profile approval | Purpose-specific | Existing explicitly accepted oracle/override/live-add purposes remain; no blanket native-write approval is invented. |
| Resources and bindings actually consumed by the requested operation | Hard for that operation | Missing record semantics or an unverified native binding blocks only the dependent operation. Do not require unrelated generation profile sites; never borrow latest data or guess RVAs. |
| Ambiguous process, changed PID/birth, address overflow, module bounds, code/layout/owner/slot/capacity mismatch | Hard | Refuse before mutation; relevant native admission must validate the same process instance. |
| The operation's missing, changed or corrupt backup; consent audit for a consent-requiring operation | Hard for that operation | Scroll addition retains its selected-save checkpoint. Equipment addition automatically prepares a selected-save checkpoint and binds it into the plan digest. A failed unrelated global consent backup does not disable either addition. |
| Stale reviewed plan or consent for an absent capability | Hard | Reprepare and review; cancellation invalidates the plan and authority. |
| Single-writer ownership, durable intent, interrupted/unknown write, readback mismatch | Hard recovery boundary | Keep operation receipts and uncertainty; never report success or automatically replay. Recovery, status, cancel and stop remain available without new consent. |

The detailed gate inventories are `version-resource-audit.json` and
`runtime-gate-audit.md` in the external evidence root.

## Implemented behavior

- Installation inspection runs in the Tauri host before worker startup. It
  reports selection/discovery source, actual FILEVERSION, precise read errors,
  resource/runtime mappings and per-feature support. Invalid selections never
  silently fall back; unsupported startup can still show diagnostics.
- Compatibility preparation creates fresh read-back-verified copies and a
  hashed manifest. All discovered sources are copied sequentially rather than
  silently stopping at 16. Earlier copies are preserved.
- The host binds the exact plan to process identity and that backup. It verifies
  backup copies and manifest again at acceptance and admission, and records
  actual bypassed soft checks in a durable consent file before success.
- The UI shows differences, allowed capabilities, mandatory checks and backup
  paths. Confirmations start unchecked. Close, Escape and cancellation discard
  authority; late preparation replies cannot reopen or reinstate an old plan.
- Known native operation admission is covered consistently, including count
  edits, temporary overrides and native generation/search/grace capture.
  The reviewed capability list must match the operation being admitted.
- The inspected identity is carried into count, override, scroll/equipment and
  oracle admission. Actual process handles verify the expected creation time;
  the oracle validates module/site/selector bounds before remote reads or
  allocation. A changed target offers reconnection and a fresh plan. The
  generation/search resource context must match the running game when that
  operation composes version-specific results. Scroll insertion no longer
  requires that broad startup tuple equality: candidate provenance, final/stage-one
  pairing, catalog legality and the actual native builder's complete business-byte
  preview comparison remain mandatory. This is not proof of cross-version RNG parity.
- Explicit unknown-version inspection can probe only the three registered
  character structures through a read-only handle. Module bounds are checked
  before globals are read; double reads, independent ownership, vtable and
  successful item-array validation are required. Unique, multiple, no-match
  and unavailable results remain diagnostics, with no write capability.

## Recovery is part of the product contract

The owner clarified that detecting an actual problem must lead to a useful
recovery path, not a dead end. Do not add a new generic refusal layer. Preserve
the caller's form and requested operation while refreshing only invalid state.

| Condition | User path and preserved state |
| --- | --- |
| Game exited or restarted | Reconnect and prepare: rediscover the sole game, resolve its current addresses and identity, and rebuild the compatibility plan. Keep entered values and selections. Never write to old addresses. |
| Multiple game processes or no readable target | The present UI still auto-selects a unique Nioh3 process; it has no manual PID picker. Report ambiguity/read failure without guessing or stopping processes. A future explicit selector must bind PID and birth to the plan and keep receipt recovery bound to its original target. |
| Plan changed, cancelled or expired before a write | Discard only old authority and rebuild from retained inputs. Review the new plan and confirm again. |
| Backup missing, inaccessible or changed | Show the affected path and cause; after the source becomes available, retry preparation. A successful new backup clears the previous failure. |
| Unrelated installed-version selection or generation resources fail | Runtime starts independently, with an explicit unloaded generation context. Equipment addition uses its own 2.02 tables and actual running binding. Dependent generation operations load the evidenced context on demand and report a resource repair when needed. |
| Unknown bindings or nonmatching structure | Show actual version/candidate results and missing evidence, retain diagnostic/selection controls and allow a supported executable to be selected. Explain the specific unsupported feature rather than blocking unrelated features. |
| Result unknown after possible write | Recover the recorded operation by receipt/status. Do not automatically repeat it, rebuild it as a new write, or imply that cancellation undid it. |

Acceptance must include failure then recovery, cancellation then retry, game
restart and re-preparation, stale-plan rebuilding, retained caller inputs and
no automatic replay of unknown writes. Compatibility confirmation itself never
executes the requested write. Repairing its feature list must preserve existing
evidenced capabilities rather than silently removing them.

## Operation-local admission follow-up

The owner requested modifier-style live addition: validate the requested target
and operation rather than requiring every feature and global version marker to
agree. The source now implements these bounded changes:

- Packaged runtime launch no longer consults the cached installed EXE version
  or starts the offline/save engine. It retains staged package/binary integrity.
  Its handshake context is explicitly null until generation is needed; no
  latest-version or legacy identity is fabricated. Save/offline roles retain
  their explicit generation identities.
- Runtime controls, compatibility diagnostics, character/count paths and
  equipment addition can run without unrelated generation resources. Failed
  generation loading leaves independent controls available for repair.
- Offline completion prediction uses its explicit offline 2.02 context rather
  than asking deferred runtime discovery for a running game. The repaired
  host_scroll_completion regression passes without a process or write, and
  prediction does not populate or replace the live-generation context.
- Scroll admission uses the actual process lifetime and one existing 2.01/2.02
  LiveAdd binding. The executor does not consume the broad generation profile,
  so its eleven unrelated research sites are no longer an admission dependency.
- Both addition capabilities are reported in operation_scoped_features and
  excluded from global consent's allowed_features. Existing backup, native code,
  owner/container/counter, single-writer, exact preview, receipt and readback
  checks still govern each operation. A global checkbox cannot grant a missing
  binding or override a local failure.
- Equipment addition backs up the explicitly supplied save, or automatically
  chooses the unique discovered save. Multiple saves require the current
  character's explicit backup path. The native preview cannot start until the
  checkpoint is verified; its source/hash/copy metadata enters the plan digest
  and is rechecked before the insertion claim. Retried backups preserve prior
  copies. This does not infer which account is loaded from a directory name.
- Old experimental character layouts remain experimental. 2.00.01 and unknown
  bindings remain unsupported; no new game/version adaptation was attempted.

Source tests use in-memory replies, owned encrypted save fixtures and a real
framed runtime worker with a nonexistent data root. They do not attach to a
game or establish packaged WebView2/native insertion/persistence acceptance.
The current unique-process behavior is retained; this batch does not add a
manual multi-process selector or automatic replay after an uncertain write.

## Long-term version retention rule

An unsupported version or capability stays explicitly unsupported until a
legitimate sample and the required resource, layout and ABI evidence exist.
A public release label, short signature, structure match or consent checkbox
cannot supply that evidence. Ver2.00.01 remains unadapted; the experimental
older character layouts and missing older equipment/native bindings listed
above retain their precise limits. Do not force an unknown version through
a newer profile or report a partial capability set as complete support.

Every newly supported version must preserve the adapters, explicit dispatch
and resource mappings, required runtime data, and regression tests for already
verified older capabilities. Validate the old and new paths together before
claiming support. Proven byte-identical inputs may remain shared when the
version-specific context and resource identity are preserved. This rule retains
maintainable compatibility implementations; it does not require adding game
executables, full section captures, complete game installations or every old
product installer to the repository or release. No binary sample was added
by this policy or space audit.

## Version retention space audit

The external `version-retention-audit.json` records the measurement timestamp,
base commit, dirty-worktree status, bounded search roots and version-token
pattern, and every included relative path, byte length and SHA256. These are
logical current file bytes, not NTFS allocated space or Git history/object
storage. Source and regression entries are whole modules containing explicit
version references in the recorded roots; shared implementations and inline
tests remain included in their whole-file sizes. This is a reproducible scoped
inventory, not an old-version-only overhead estimate or permission to remove
unlisted dependencies.

| Retained category | Files | Logical bytes |
| --- | ---: | ---: |
| Version-bearing source modules | 51 | 1,885,924 |
| Regression files and test helpers | 63 | 1,389,702 |
| Runtime version profiles | 3 | 22,952 |
| Other runtime data | 60 | 8,975,554 |
| Shared tool helper DLLs and build metadata | 3 | 1,382,148 |
| Distinct files in this scoped inventory | 180 | 13,656,280 |

The profile and other-data rows partition the complete 63-file, 8,998,506-byte
`nioh3_scroll_editor/data` tree. The existing old R4 directory
`r4_finalizer/pc_v2_00_02/resource_v1` contains 13 files / 2,480,911 bytes;
`r4_finalizer/pc_v2_02/resource_v1` contains 13 files / 2,484,879 bytes.
FILEVERSIONs `2.0.0.2` and `2.0.1.0` explicitly select the same old directory,
so 2.01 adds zero additional R4 payload bytes. The shared auxiliary generation
path is `auxiliary_generation/pc_v2_00_02/resource_v3`; enemy-state tables use
`enemy_states/pc_v2_01/native_tables.json`. Version labels do not imply a
separate copy of each shared table. The explicit dispatch lives in
`crates/nioh3-data/src/lib.rs` and `nioh3_scroll_editor/resolved_context.py`;
`crates/nioh3-data/src/selected_bundle.rs` retains regressions for the shared
legacy bundle identity and the distinct current bundle. Those tests were inspected, not rerun for this audit.

Across the two R4 trees, 10 files at matching relative paths have equal SHA256
and total 978,470 bytes per set. The three differences are `manifest.json`,
`tables/item.bin` and `tables/optional_multiplier.bin`. Across all runtime data,
44 distinct file contents total 7,999,288 bytes; repeated contents account for
999,218 additional logical bytes across 16 duplicate groups. The receipt lists
each group. No storage deduplication or resource rewrite was applied: physical
sharing was not measured, and manifests/context digests must remain valid.

`tools/package_tauri.py::stage_rust_runtime` copies the entire data tree and
two tool DLLs plus their build metadata. There is no version-based pruning in
this stage: the measured inputs are 66 files / 10,380,654 bytes before
compression. The Rust portable assembler does not copy raw adapter sources
or regression files. Their contribution to compiled hosts/workers has not
been measured. `tools/build_tauri_onefile.py` embeds the portable ZIP; no new
package or archive was built here. Consequently the actual release size and
increment attributable to retaining older versions remain unmeasured, rather
than being equated with these source or uncompressed data totals.

Reproduce the inventory by reading each receipt path under its recorded
repository root with Node `fs.readFileSync`, comparing byte lengths and SHA256,
and summing by category. Group runtime data by SHA256 for repeated-content
counts; compare the R4 trees by their relative `resource_v1` paths. This audit
only updated policy documentation and the external receipt; it did not edit
adapters, resources or packaging selection, run game/save operations, or
claim new runtime or packaged acceptance.

## Acceptance and remaining evidence

Completed checks for the operation-local source follow-up:

| Source or fixture gate | Result | External evidence |
| --- | --- | --- |
| Full runtime library, rechecked after receipt controls | 207/207 | `logs/receipt-control-runtime-full-20261002.log` |
| Policy and equipment checkpoint | 21/21 + 2/2 | `logs/operation-admission-first.log` |
| Runtime bootstrap, including framed worker without game/data | 2/2 | `logs/runtime-bootstrap-framed.log` |
| Bootstrap parser | 5/5 | `logs/runtime-bootstrap-parser.log` |
| Deferred offline prediction | 1/1; bootstrap 2/2 also rechecked | `logs/operation-offline-prediction.log` |
| Tauri packaged-resolution unit tests and offline session | 11/11 + 1/1 | `logs/operation-tauri-packaged-resolvers.log`, `logs/operation-tauri-offline-session.log` |
| Full npm | 91 pass / 1 package-dependent skip | `logs/operation-final-npm.log` |
| Runtime and protected Clippy | Pass; final protected all-target exit 0 | `logs/operation-runtime-clippy.log`, `logs/operation-final-protected-clippy.log` |
| Stored-receipt controls with no game | 4/4 | `logs/receipt-control-protected-20261002.log` |
| Public-error mapping | 17/17 | Final UI worker report; final receipt owned by root |
| Format checks | Three checked targets pass | Root final format verification |
| Frozen mocked browser workflows | Equipment 66/66; compatibility 146/146 | `equipment-browser-2026-10-02T10-16-45-267Z/equipment-ui-e2e.json`, `browser-2026-10-02T10-05-38-481Z/compatibility-ui.json` |
| Locale audit at the UI freeze | 1,312 messages | UI worker freeze report; final receipt owned by root |

The actual-error UI mapping and no-game stored-receipt status/cancel follow-ups
are verified; actual recovery retains the process identity requirements.
Earlier 201-runtime/18-policy/143-browser counts belong to the preceding batch.
Final TypeScript, locales (1,312 messages), and full npm (91 pass / 1 skip) pass; see logs/operation-final-typecheck.log and logs/operation-final-npm.log. Four package
verifiers were adjusted for deferred context, per-role logs and isolated save
discovery and pass syntax checks; matching package acceptance remains pending.
After a clean source commit, build local test10 and record the exact outer EXE
identity and acceptance in its external delivery receipt. No current game/save
acceptance or publication is claimed.

The shared save discovery helper can still skip unreadable child directories;
this batch verifies all returned sources, not files it could not enumerate.
The source-to-copy list remains available for review; root/empty-discovery
failures identify the searched location and how to retry. Owned synthetic files,
in-memory readers and mocked browser bridges cover known/unknown inputs,
identity changes, backup failure/tampering, reviewed plans and recovery without
automatic replay. Those checks do not validate a real player's save or game.

Required next evidence before extending compatibility:

1. A legitimate Ver2.00.01 sample or retained version/section/resource evidence.
   Its FILEVERSION has not been guessed.
2. Old seeded equipment builder and insertion-chain semantics, plus historical
   character/pagination/menu validation where current evidence is incomplete.
3. Trusted operation-specific full code/read-only-data evidence for any unknown
   executable variant. A module-owned vtable or short matching prologue alone
   does not prove field or ABI semantics.
4. Separately authorized packaged UI and owner-present game acceptance,
   including persistence/reload. No real force-write test was performed.

External evidence root:
`D:/Nioh3_v080_deliverables/deliverables/codex-compatibility-policy-20261002`.
The final combined patch includes the preceding
[V083 offline reliability batch](V083_OFFLINE_RELIABILITY_20261002.md).
