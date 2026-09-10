# Live insertion and localization follow-up

Latest update (2026-09-08): the single native scroll addition and normal save/reload
passed bounded acceptance. The optional protected-worker/CE integration has now
been implemented after game closure. Read `LIVE_ADD_ENGINEERING.md` and
`CURRENT_HANDOFF.md` before using the earlier research-status sections below.
No further game experiments are requested while waiting for Figma.


## Verified work on 2026-09-08 UTC

Read-only access works without a CE session. The current process is PC v2.01,
PID 7744, and passes native profile identity checks. The double-read inventory
capture contains no occupied scroll slots and counters of 10. This does not
establish the current game screen or a loaded character inventory. Old process
addresses and thread IDs must not be reused.

`research/capture_live_scroll_inventory_readonly.py` verifies executable identity,
the insertion signature, capacity, owner pointers, counters and two copies of the
400-slot container. It exports targeted scroll records and duplicate serial
diagnostics. It neither reserves serials nor invokes insertion. Double reads are
consistency checks, not an atomic engine snapshot.

## Localization

`research/capture_special_rule_item_names.py` resolves all 32 native item keys at
row +0x152 to their name text IDs at +0x68 (row stride 0x1A0). Locale-specific
native effect anchors identify the text pool. Missing or ambiguous names fail
the capture. Captures are read-only and use exclusive output creation.

All 32 Chinese names were captured successfully. Item 0x729D was corrected to
the captured name in the bundled resource; the other 31 match. Native text IDs
and locale-specific provenance are now retained separately from translations.
After the user loaded English, all 32 existing English names were verified
unchanged at 2026-09-08T02:22:26Z. Their native text IDs match both other locales.
All three qualifier locales now have PC v2.01 capture provenance. After the user loaded
Japanese, all 32 Japanese names were captured at 2026-09-08T02:10:21Z. Every
native text ID matches the earlier Chinese capture. Display names use the
existing native ruby/style cleaner; original markup is retained separately in
the resource and unmodified capture. No synthetic official translations were added.

The five missing English effect names share native text ID 0x03441371 and the
captured Chinese/Japanese placeholder labels. Coverage auditing now groups
these as one known placeholder text gap while retaining the original five
missing effect IDs. It does not count fallback text as native English coverage.

Evidence: `deliverables/frontend-v2/localization-followup/qualifiers.zh-CN.json`
and `qualifiers.ja-JP.json`, `qualifiers.en-US.json`, plus `coverage.json`.
Focused localization and auxiliary catalog validation:
16 tests passed. The changes are now included in `portable-v2-localized-r3`,
which passed actual Electron, source/packaged IPC and strict GPU parity checks.
The older r2 remains unchanged. See `verification-localized-r3.json`.

## Live insertion readiness

User attribution for the serial-write positive control is now confirmed:
map pickups and enemy-drop pickups, with no shop purchases. The insertion
writer includes a quantity-overflow remainder allocation; the other observed
copy path is consistent with quantity extraction. These are bounded code-path
findings, not per-item attribution or proof of thread-safe external insertion.
See the scheduling review and `serial-writer-analysis.json` for exact limits.

Subsequent scheduling work is recorded in
`deliverables/frontend-v2/live-add-followup/SCHEDULING_REVIEW.md`: seven code
ranges and five call references match the current process. A bounded hardware
observer captured eight empty-pickup dispatch calls at entry 0x12E6840, with
stack ownership mapped to thread 43964. All observer breakpoints were removed
and cleanup confirmed. This identifies a scheduling candidate, not safe writing.

The successful DLC acquisition in
`deliverables/frontend-v2/live-acceptance/20260907T225708Z/DLC_SMALL_HELL_03_ACQUISITION.md`
supersedes the earlier insertion-contract report's missing natural acquisition
observation. It establishes builder serial allocation, pickup source identity,
destination flags and acquisition order for one attributed experimental sample.
Existing first-reveal and save checks remain separate evidence.

The native builder and pickup ran on different observed engine threads.
Exclusive serial-counter ownership remains unproven. Later no-call and read-only
native-query dispatches succeeded; safe mutation is still unproven. The insertion
return record is an unaccepted remainder;
an empty remainder alone is not proof of successful insertion. Destination slot,
full uint64 serial, lookup consistency and persistence must also be checked.

Next bounded research step: inspect the observed builder/pickup callers and their
scheduling boundary, then design a one-shot engine-context experiment with exact
cleanup and duplicate-serial checks. Do not invoke the insertion routine from an
arbitrary remote thread or reuse a copied serial. No live insertion capability
has been enabled in the application. Earlier observation was read-only. Later
dispatch probes wrote private code/marker allocations and used the game thread's
stack, without patching original code or mutating inventory records or saves.
See `deliverables/frontend-v2/live-add-followup/DISPATCH_PROBE_ACCEPTANCE.md`
for actual execution, cleanup, quantity-hook findings and remaining prerequisites.

The initial read-only baseline was
`deliverables/frontend-v2/live-add-followup/inventory-before-research.json`.
It is not a populated-character baseline for a future write experiment.
The later `inventory-after-readonly-call.json` captures 29 occupied scroll slots;
refresh it before any mutation experiment.
