# v0.8.1 generation-legality response acceptance

Status: bounded offline acceptance complete (2026-09-21). This record accepts
the supplied Pro-response artifact as a reproducible research package only; it
does not enable generation or legality behavior.

## Artifact identity

The input ZIP is
`D:\Downloads\Nioh3_v202_Generation_Legality_Pro_Response_20260921.zip`,
105,434 bytes, SHA-256
`EECDFA8BAE01AE1D4829C46D852B0096A684F566F99778F988089324E4657886`.
The extracted `pro-response/MANIFEST.sha256` contains 33 payload entries; all
33 hashes pass, and the archive contains 34 members below one `pro-response`
root. The named input archive identities from `evidence/INPUT_ARCHIVES.json`
were checked, including the sibling source-intake ZIP at
`D:\Nioh3_v080_deliverables\deliverables\v081-source-intake-20260921\pro-handoff\Nioh3_v081_Integration_Contracts_Pro_20260921.zip`.

## Checks and evidence

- `acceptance/policy-tests.stdout.txt`: 40/40 synthetic policy tests passed.
- `acceptance/static-recheck.rerun.json`: 14/14 static checks passed against
  the exact equipment and source-intake ZIPs; the upstream source digest is
  `dbc5e0d8...7d681`, matching `analysis/STATIC_FINDINGS.json`.
- `acceptance/delivery-checks.rerun.json`: 33/33 package checks passed in an
  isolated copy with `PYTHONUTF8=1`.
- `acceptance/manifest-and-archive.stdout.txt` and
  `acceptance/input-and-source-identities.stdout.txt` preserve hashes and
  member-count evidence.

## Reviewed boundary

The supplied source uses `random.shuffle` for unused inventory-index selection
(`main.py:531-543`), while `spawn_equipment` clones a fixed template and
rewrites identifiers (`582-618`). The effect-list fallback (`101-111`) is a
UI list fallback, not a legality rule. `NATIVE_RESEARCH_ANCHORS.json` keeps the
native add RVA unknown and marks `0x5513C8`/`0x557F34` and their edges as
candidate profile roles, not proven equipment paths. `SHARED_RULES_DRAFT.json`
sets enabled equipment hard-terminal rules and enabled gameplay rules to zero.

No native/game/save execution, natural-generation acceptance, complete
terminal-legality proof, or production safety claim follows from these checks.

## Offline P0/P1 supplement

The accepted intake now has a staged handoff at
`D:\Nioh3_v080_deliverables\deliverables\v081-generation-p0p1-20260921`.
P0 retains 16 files / 1,323,524 bytes with the installed v2.02 executable
identity and 12 matching resource identities; retained R5 evidence is
effect-slot parity only, not full-record parity or 添画. P1 retains 10 complete
`.pdata`-bounded bodies (8 anchors plus old-add caller/target), 12,013 raw
bytes, and 25 checksum entries. The AOB site `0x3EEDA3` resolves to
`call 0x3EEDB9` (`E8 66 E5 15 00`) in `0x3EEA64..0x3EF171`, targeting
`0x54D324..0x54E13B`; it is an insertion-boundary candidate, not a proven
normal producer. Caller coverage is incomplete (319 raw / 64 validated,
budget exhausted) with 264 explicit unknown edges.

The structural verifier passed through the project wrapper (`ok=true`, 95 ZIP
members verified, exit 0). The external archive is
`D:\Nioh3_v080_deliverables\deliverables\Nioh3_v202_Equipment_Generation_P0P1_20260921.zip`,
1,483,874 bytes, SHA-256
`67D3AE15FCA0EF8B1F45402965F0882E5DBE94E604B21471B076EBD105492B5D`.
This remains offline P0/P1: no P2 live tracing, generator implementation,
legality enablement, game/save write, or publication.
