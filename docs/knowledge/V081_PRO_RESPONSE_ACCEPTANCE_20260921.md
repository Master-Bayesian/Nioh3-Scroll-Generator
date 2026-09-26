# V081 Pro response acceptance and first internal slices (2026-09-21)

Bounded record of what was reproduced, what was only measured, and what is
pending. "Implemented internal" means code exists, compiles and is exercised by
its own offline artifact - not user-facing, not shipped, not game acceptance.

## Bounded acceptance (independently re-run)

- Response archive 105,972 bytes, SHA-256
  `816fc9dc386ff2e05fb9ea57b2dab7c131d0b5552f671b47170011993c40e70e` (verified);
  11 manifest entries verified, archive members match disk.
- `validation/audit_inputs.py`: 22 checks passed, 86 manifest entries.
  `validation/validate_response.py`: 43 checks passed, 0 failed. Both re-runs are
  deep-equal to the shipped outputs, so these are artifact and static checks, not
  product tests and not PC v2.02 runtime acceptance.
- First re-run hit a host defect with the response scripts unmodified: an unencoded
  `Path.read_text()` used this host's GBK default; re-ran with `PYTHONUTF8=1`.
  Exact record and digests: [REPRODUCTION.json](D:/Nioh3_v080_deliverables/deliverables/v081-pro-response-20260921/acceptance/REPRODUCTION.json)
  (`d537013c…a1b7c`, `56e279db…d6dbe`).

## Decisive upstream conflicts (upstream = external editor and CT)

- C1: `0x33F41E = 0x302832 + 1072*0xE8 + 0x6C`: the hard-coded counter aliases the
  third effect u32 of usable record 1072, so re-serialising it erases the increment.
- C2: the used-index collector never scans the scroll region, so every remapped
  scroll receives the same value (synthetic two-slot run returned `[0x101,0x101]`).
- C3: quantity has a 2-byte field plus a 4-byte alias over `[0x04,0x08)`; `+0x1A`
  has 2-bit and 10-bit views from bit 0; effect views are 4-byte overlays spanning
  the auxiliary word. Decisive source conflicts, not product corruption.

## Product branch finding (separately owned, pending)

`replaced_by_this_commit=false` skips rollback yet can emit `not_committed` with
"checkpoint restored" (`transaction.rs:1385-1418`, `:1412`, `:2927-2932`): a narrow
classification/message defect, not a demonstrated data defect. Whether a partially
replaced file is possible is unresolved; the fix is implemented and the remaining
evidence is the after-replace external-write fault gate in its own ticket.

## P2 disk identity and pattern results

- Installed `D:\Steam\steamapps\common\Nioh3\Nioh3.exe`, 77,830,112 bytes, file
  version 2.0.2.0, SHA-256 `E22C4A635E4EC1E27A177B76E27D7F6A637F426C0ED3928B60F5693BC52AE130`
  (unchanged by the scan).
- All five declared patterns returned 0 disk matches: a scope result, not a
  compatibility result. The on-disk `.text` is high-entropy with an executable
  `.bind` wrapper stub, and the repository's own disk-vs-runtime section manifest
  disagrees for `.text` while matching `.pdata`, so disk bytes can neither confirm
  nor deny the CT AOB or any owner/getter signature.
- Reusing the captured 2026-09-19 runtime sections (manifest SHA-256
  `FDBAD200A835A72974D80F150B7D9766CFBCB70C7A9BC89C720BF124B0BC8382`, executable
  SHA-256 matching the installed build) gave one hit each for CT AOB `0x22AEB73`,
  bag manager `0x23E61F0`, getter `0xF464C` and add-backpack `0x3EEDA3`, and 0 for
  add-warehouse. The getter `imul` immediate is `0xF0`; no item stride is asserted.
- Boundaries of that reuse step: historical capture rather than a live read at the
  time, with no module base or PID recorded; a hit proves no ownership, insertion
  acceptance or persistence. See
  [p2-disk/REPORT.md](D:/Nioh3_v080_deliverables/deliverables/v081-pro-response-20260921/p2-disk/REPORT.md) and
  [runtime-reuse/REPORT.md](D:/Nioh3_v080_deliverables/deliverables/v081-pro-response-20260921/p2-disk/runtime-reuse/REPORT.md).

## T2 offline catalog adapter (implemented internal, not shipped)

- Code: `crates/nioh3-data/src/equipment_catalog.rs` (1,020 / 948 non-blank) and
  `examples/equipment_catalog_report.rs` (750 / 705), plus a two-line `lib.rs`
  declaration. The product API is role/namespace vocabulary, `CatalogInput`,
  `load_catalog_set`, `CatalogRow`, `RowState`, `CatalogSet` row queries, the four
  normalizers and `CatalogError`; report-only helpers live in the example only.
- Reproduces: save item byte-order keys (`EB9E` -> `0x9EEB`) vs numeric effects
  (`E99A`); CT 2/4-byte tokens keeping the high word; seven malformed save-item keys
  quarantined; sentinels named; duplicate names kept; null type/max never defaulted;
  Pro intersections 953 / 984 / 1154 / 749 / 723 / 742 and base overlaps 24 / 24.
- Does not prove legality, obtainability, types, maxima, drop pools, RNG, version
  compatibility, or any UI, IPC or write path; no writer, command, product resource
  or external database is added. Product-side input is reported only as
  `no_input_supplied` or `input_unreadable`; there is no readiness API.
- Durable artifact:
  [EQUIPMENT_CATALOG_REPORT.json](D:/Nioh3_v080_deliverables/deliverables/v081-pro-response-20260921/catalog-adapter/report/EQUIPMENT_CATALOG_REPORT.json)
  SHA-256 `c88d178fcb8173eef03e98a6930ab4767f61dec2904773d7697f3904adb87d60`, with
  `catalog-adapter/report/ACCEPTANCE_RESULT.json` holding its 220 offline checks.

## Pending and unchanged scope

- Transaction outcome repair is complete. Positive checkpoint-byte equality
  permits `not_committed` with no restore claim; otherwise non-owned/unproven
  bytes remain untouched and `uncertain`. Two focused E2E cases pass (including
  the existing post-replace fault matrix); dedicated external-write/read-error
  cases were not run. Evidence: `acceptance/repair/AFTER_receipt_capture.json`.
- P3 completed one bounded read-only candidate observation (`p3-readonly/`):
  identity 2.0.2.0, four of four sites, count 2500 before and after, byte-equal
  re-reads, one read-only handle; the three candidates matched catalog names at the
  declared item/level offsets and the chain stays a candidate hypothesis. The owner
  then confirmed by UI (answer "有" plus one supplied screenshot) that raw slot 0's
  level-180 +20 千鸟十字枪 (`0xF6E8`) is in the backpack, so one sample's basic
  fields are UI-confirmed; four of the five displayed effect names also match the
  decoded non-sentinel slot-0 ids in order, with position 3 (`0x7B14`) unresolved
  and display values not validated against raw scalars. Soul-core fields,
  rarity/effects semantics, widths above 255, inventory bounds/identity, writes and
  persistence stay unproven.
- Current-version equipment read/edit/create acceptance is not started; it needs
  version-matched live evidence and explicit owner authorization, and one sample UI
  confirmation does not implement a feature. Read-only equipment browsing is the
  next direction, with its integration design decided separately.
- Pro proposals stay proposals, not blanket approvals; the adapter is offline
  internal only. Scope unchanged: SETTINGS, statistics, TITLES and the 131
  placeholders stay tracked.
