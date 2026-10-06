# PC v2.02 stamina-to-equipment-capacity research (2026-10-06)

Status: bounded read-only evidence; the native capacity rule is unresolved. This
record does not change product behavior or authorize an offline capacity formula.
The detailed process captures and the local Pro handoff remain under ignored
`deliverables/` and are not part of this public repository change.

## Controlled UI observations

The operator used normal game controls to set stamina (`刚`) and switch styles,
reporting no equipment or additional capacity effects. The same PC v2.02
process was used throughout. The read-only player-object capture confirmed the
stamina field at `player + 0x9C` for each value. UI capacity values are operator
observations, not memory-derived values. The machine-readable grid is
[`research/load_capacity_v202/ui_grid.json`](../../research/load_capacity_v202/ui_grid.json).

| Stamina | Samurai capacity UI | Ninja capacity UI |
| ---: | ---: | ---: |
| 5 | 29.5 | 14.1 |
| 40 | 48.4 | 22.8 |
| 80 | 53.7 | 24.7 |
| 99 | 55.8 | 25.2 |

The executable file version was `2.0.2.0`, with SHA-256
`e22c4a635e4ec1e27a177b76e27d7f6a637f426c0ed3928b60f5693bc52ae130`.
The existing `module + 0x4751850` player pointer and `module + 0x402DA20`
vtable were revalidated in the live process. The process birth identity was
held fixed across captures.

## Bounded memory and native observations

- At each fixed stamina value, the first `0x4000` player-object bytes were
  identical between samurai and ninja. A samurai → ninja → samurai return
  control at stamina 40 also matched. Between stamina 40 and 80 within one
  style, only bytes `+0x10..+0x12` and `+0x9C` changed in that range. This
  excludes a style-changing cache in the captured range, not elsewhere.
- A 4 GiB exact 48.4 scaled-integer search returned 2,224 candidates; none
  changed to 22.8 after a style switch. A bounded 16 GiB search found five
  exact 22.8 `f32` candidates, all of which stayed 22.8 after switching to
  samurai. It found no exact 48.4 `f32` candidate. A 48.35–48.45 `f32` range
  search returned 4,147 candidates, none of which became approximately 22.8
  after switching to ninja. These searches stopped at their byte limits.
- A bounded CE access observation on `player + 0x9C` recorded 64 events at
  RIP-after-access RVA `0x5561E1`. Live `.text/.pdata` disassembly identified
  the attribute-block read at RVA `0x5561DA`, which copied it into a secondary
  object. A second observation of the copied stamina field found seven access
  RVAs, including `0x818EFD` in derived-stat function
  `0x818B68..0x819400`. No capacity consumer was established.
- Both accepted CE observations have cleanup records showing no active owned
  or global breakpoints and no active timer. An earlier arm-proof failure was
  recovered in the same CE session and recorded separately.

The research used read-only process access and debugger observation. Codex did
not write game memory, edit saves, or invoke state-changing native functions.
The operator changed stamina through normal game controls. A no-change
unrelated-menu negative control was not collected.

## Reproduction and remaining question

The bounded collectors in [`research/load_capacity_v202/`](../../research/load_capacity_v202/)
check process identity and refuse to overwrite captures. Run them through the
project's `tools/run_python_tests.ps1` wrapper and store output outside the
checkout. `research/dump_live_pe_sections.py` now reads bounded PE headers
without the optional `pefile` dependency; it was used to export the verified
live `.text`, `.rdata`, and `.pdata` sections into local ignored research output.

The native formula or table, style selector, internal precision, and display
rounding remain unknown. The four UI rows per style are validation vectors,
not enough to justify interpolation or a fitted formula. The next probe needs
to identify the native capacity output or consumer and then verify its path
against all eight observations before product integration.
