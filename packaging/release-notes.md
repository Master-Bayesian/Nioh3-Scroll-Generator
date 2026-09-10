# Nioh 3 Scroll Editor 0.7.0

The new desktop interface combines scroll search, editing, favorites, a selected-item
cart, and backup management. Chinese, English and Japanese can be selected in the app.

- Search by primary alternatives, secondary effects, Grace, enemies, special rules,
  terrain and maximum attempts. Each batch returns up to 25 scrolls; continue with
  the next batch, keep the latest three searches, or load a known scroll ID.
- Keep up to 50 favorites and 50 cart items independently. Add only the selected
  cart items, either directly in the running game or to a save at the title screen.
  Native live addition includes a verified automatic backup and durable receipts;
  Cheat Engine is optional and is not needed for the default executor.
- Edit existing scroll effects and header values with undo/redo and reviewed save
  writes. Choose seeds from the cart. Native value ranges are shown when available.
- Temporarily change enemies, terrain, rules and maximum attempts. Change current
  remaining attempts separately for the selected scroll instance, with backup and
  readback. Temporary capacity overrides do not propagate with a shared scroll.
- Manage automatic backups, restore at the title screen, copy bounded diagnostic
  logs, adjust font size, show catalog IDs and check signed whole-package updates.
- Preserve the verified generation, RNG, R4 reveal-finalizer and native ABI behavior.
  The legacy Tk source entry remains runnable during migration.

## Install or migrate from v0.6

Download `Nioh3ScrollEditorV2-0.7.0-win-x64.zip`, extract the entire archive into a
writable folder and launch `Nioh3ScrollEditorV2.exe`. Python and Cheat Engine are
not required. The legacy single-EXE updater cannot install V2; this first migration
requires the ZIP download. Future V2 updates replace the complete portable directory.

## Compatibility

Native live memory operations are gated to the verified PC v2.01 build. Early
playthrough R4/R5 configurations are constructible custom records, not a claim of
natural drops. Free effect edits are local; another player regenerates them from
the seed. Save/reload, offline parity and local readback do not prove multiplayer
propagation or every GPU configuration. The accepted R5 load-time normalization
and icon discrepancy remain game behavior rather than an application workaround.

The V2 update manifest is signed using the existing official Ed25519 key. Windows
executables do not carry an Authenticode certificate.
