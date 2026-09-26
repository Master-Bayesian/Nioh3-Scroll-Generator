# Nioh 3 Studio 0.8.1

A fix release for 0.8.0: it repairs the regressions players reported after the
move to the Rust backend, makes failures explain themselves, and restores
behavior the migration had lost.

## Direct launch and updates

Use `Nioh3Studio-0.8.1-win-x64.exe` directly. No installation, extraction,
Python, Node.js, Electron, or Cheat Engine setup is required. 0.8.0 offers this
update automatically after its startup checks, or from Settings.

## Fixed

- **Grace and terrain filters work again.** Searches with a selected Grace or
  terrain effect are no longer refused or silently widened.
- **Temporary changes work on PC v2.02.** Enemy, terrain, special-rule and
  challenge-count overrides under "副本内容 · 临时修改" no longer fail with a
  missing-profile error.
- **Live addition is more reliable.** Inventories whose existing scrolls share a
  serial accept additions; a prepared but unfinished batch, stale records from
  an earlier game session, or another program's receipt no longer block later
  additions or closing the app; an expired save snapshot is re-read and the
  addition prepared again.
- **Closing never traps you.** The window closes at once; busy background work
  gets a short grace period and the app then exits either way.
- **A refused operation no longer locks the app** into "another operation is
  still running".
- **The editor's review step reports every outcome** and keeps untouched record
  bytes, and the count editor reads the inventory with the selected game
  version's layout.

## Searching

- **Results appear as they are found** instead of all at once at the end.
- **Impossible effect combinations are flagged while you pick them,** naming the
  effects that clash and what to change, before you start a search.
- **Each effect can be selected once.**
- **Rarity-5 searches with a Grace start about twice as fast** (a cache the
  migration had dropped is restored); results are unchanged.

## Messages and feedback

- Failures explain what happened and what to do next in your language,
  including the save, backup, game-process and search explanations the earlier
  Python backend gave. Technical details stay available under a collapsed
  section.
- **Settings → Report a problem** (and the button under any failure) writes one
  feedback file with diagnostics and recent logs and shows it in Explorer; the
  app no longer copies logs to the clipboard on its own.
- With several saves, the last save you chose is selected again.

## Compatibility and limits

- Supported game build: PC v2.02 (and the earlier approved builds). Unsupported
  builds and unknown resource graphs are refused rather than guessed at.
- Offline tests, synthetic encrypted saves, and packaged startup checks do not
  establish real-game or real-user-save acceptance for every path.

Equipment browsing, editing and generation are planned for a later release.
