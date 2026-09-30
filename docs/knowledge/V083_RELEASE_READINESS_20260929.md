# v0.8.3 release readiness — 2026-09-29

**Status: REPLACEMENT CANDIDATE BUILT; NATIVE STARTUP AND OFFLINE SEED ROUTE PASSED.**
Packaged save-UI/game acceptance, signing and publication remain separate gates.

Candidate branch: claude/v082-integration  
Intake source SHA: 2d133f7cd388bda390d542ea327b7f47398e049e  
Target version: 0.8.3

## Retained backend evidence

| Check | Result | Retained artifact | Evidence scope |
|---|---:|---|---|
| Character-edit host E2E | PASS, 282.91 s | deliverables/codex-v083-handover-20260929/backend/host-character-edit-e2e.log | Host E2E with a synthetic save |
| Rust equipment parity | PASS, 1.10 s | deliverables/codex-v083-handover-20260929/backend/rust900-equipment-generation-parity.log | 900 whole-record offline comparisons |
| Seeded equipment search | PASS, 0.58 s | deliverables/codex-v083-handover-20260929/backend/focused-seed-search.log | Focused offline search path |
| Seeded replay audit | PASS, 10.06 s | deliverables/codex-v083-handover-20260929/backend/focused-seeded-replay-audit.log | More than 1,000 generated records; no private save |

These results do not establish live-game writes, a real-user-save write, or
save/reload acceptance. The emulator research note also reports 3,600 compact
entry comparisons, 5,000 whole-record comparisons, and 1,500 difficulty /
progress comparisons; raw receipts for the latter two totals are not retained.
The Rust 900-record parity run is the retained whole-record parity evidence.

## test7 disposition

The test7 portable manifest and ZIP name the intake SHA above, version 0.8.2,
and git.dirty: true. The ZIP SHA-256 is
d258ce5d1b29de585c540649fb69ba5152a8dd0a1e4c94d3533fd89208c39cdb; the
expected EXE is absent. The original console log was not retained. The artifact
state and build_tauri_onefile.py clean-source guard support the reported
dirty-source refusal, but do not recover its original stderr. Preserve the ZIP
for diagnosis only.

## Gates and ownership boundaries

Version synchronization and canonical context verification passed; all version
metadata is 0.8.3. The early preflight allowed a dirty development observation.
The group-key audit repair passed its prewritten host integration case after
the same case reproduced the original false positive (48.19 s green), and the
existing generated-record replay check passed again (9.37 s). See
`D:/Nioh3_v080_deliverables/deliverables/codex-v083-handover-20260929/backend/repair-group-key/`.

The route-scoped browser seed workflow passed 45/45 checks using a scripted bridge, including
stale input and late-response handling. TypeScript and the 1,115-message locale
audit passed. Screenshots cover Chinese, English and Japanese at 1440 by 960.
See `D:/Nioh3_v080_deliverables/deliverables/codex-v083-handover-20260929/ui-scope/`.
These results are not native or packaged UI acceptance. Freeze the reviewed
whitelist and run clean-required preflight before building a fresh candidate;
do not reuse the dirty test7 bytes.

No real-save test has been scheduled. Do not require closing the game; if a
specific save-file workflow needs a context boundary, the owner's limit is at
most returning to the title screen. Local preparation does not imply authority
to push, tag or publish. The external candidate delivery report records the
final source identity, package hashes and completed packaged checks.

The replacement outer EXE from product source `7676ab7` passed launch/cache E2E
and seven packaged offline seed-route checks. Search and protected workers both
passed strict binary/contract/handshake verification. A report-only mismatch
was repaired in the acceptance helper and the launch E2E was rerun on the same
EXE; no product rebuild was needed. Exact hashes and bounds are in
`D:/Nioh3_v080_deliverables/deliverables/codex-v083-handover-20260929/candidate-r2/DELIVERY.md`.

The original CC conversation was reviewed after the initial intake. See
[V083_CC_CONTEXT_RECONCILIATION_20260929.md](V083_CC_CONTEXT_RECONCILIATION_20260929.md)
for the broader version scope, existing live evidence and original publication
instruction. Candidate 78e2dfe built successfully; its native launch was blocked
by an existing test6 instance. The old instance subsequently exited, and the
replacement candidate checks ran while the game remained running.
