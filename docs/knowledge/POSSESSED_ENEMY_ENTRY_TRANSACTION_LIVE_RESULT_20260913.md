# PC v2.01 possessed-enemy entry-transaction live result — 2026-09-13

## Scope

This document records the bounded `mode-transaction-join` capture linking the
mode-transaction request, generated descriptors, and materializer input in one
entry transaction. It is evidence, not a product oracle.

## Accepted live evidence

The `parameterized_session_branch` return at RVA `0x21DC438` consumed request
`A8912D05B700010301010200` uniquely, with `Q[9]=1`. The linked generator
returned 10 descriptors across waves `2/2/3/3` (`class0=6`, `class1=4`).
`source_flag=1` occurred only on spawn `0xF3F` (decimal 3903). The materializer
received the same 10 descriptors byte-for-byte. The result has exact parity
with prior Run D.

The prior Sequence C six-descriptor `owned_scroll_branch` call was not observed
materializing in this transaction. It must not be interpreted as a C 6→10
transformation. Cleanup was verified.

## Evidence and limits

Raw capture, cleanup, and analysis are preserved under
`audit/possessed_enemy_capture/86872488/20260913-mode-transaction-join-a/` as
`mode-transaction-join.json`, `mode-transaction-join.cleanup.json`, and
`mode-transaction-join.analysis.json`. The validator reported 50 signatures.
Focused research tests reported 460 passed and 3 skipped. Documentation and
handoff validation added 9 passing tests.

This capture does not establish a native UI mode enum, a product oracle, or
all-writers coverage. The entry-transaction sub-question is closed; no further
live entry capture is needed for it. The product goal remains explicit One
Difficulty, possessed, and expedition-only enemy choices in filter/preview.
Native UI enum is not a blocker because solo/expedition can be selected
explicitly. Remaining research is offline class1 append replay, the scoped-LCG
starting point for `+8F`, and the deterministic/random boundary for `+E9`.

The prior v1 product-research handoff is superseded by v2:
`deliverables/Nioh3_PC_v2.01_Possessed_OneDifficulty_Expedition_Product_Research_Pro_Handoff_20260913_v2/`
and its ZIP, 359,669 bytes, SHA-256
`A9A42333ABFBD8CC78D122EB0F3E45BF1416341723DEFA9C236048C987500FE5`, with
111 files, 110 hashes, and 111 ZIP entries verified.

The v1 package is preserved only as a transaction-only historical archive at
`deliverables/Nioh3_PC_v2.01_Entry_Transaction_Live_Result_Pro_Handoff_20260913_v1/`
with a matching ZIP and sidecar. It contains 41 files with 40 payload hashes;
directory and ZIP validation both passed. The ZIP is 106,496 bytes with
SHA-256
`D8AB99E4BE0EBF02E03BDB6AB289CB59B43ED34DE174CDBF705250A4E7AF3847`.
It is not the current task to send; send only the v2 product-research package.
