# Nioh 3 Studio 0.8.0

This release candidate moves the packaged worker graph to Rust while preserving
the v0.7.5 search, preview, collection, editor, backup, and update workflows. It
also adds explicit resource selection for the current PC v2.02 game build.

## Direct launch and updates

Use `Nioh3Studio-0.8.0-win-x64.exe` directly. No installation, extraction,
Python, Node.js, Electron, or Cheat Engine setup is required.

After startup safety checks finish, the packaged app checks once for a newer
signed release and asks before downloading it. The same check remains available
from Settings. Applying an update verifies the signed payload, replaces the
outer EXE in place, supports rollback, and cleans bounded caches after the new
version starts successfully.

## New and improved

- **The packaged backend now uses the staged Rust worker graph.** Search,
  preview, protected save, and runtime roles are resolved from a hash-checked
  package manifest instead of shipping the legacy Python worker.
- **PC v2.02 resources are selected from the installed game version.** The host
  reads the exact `Nioh3.exe` file version, binds it into generation identity,
  and fails closed when the version or packaged resource graph is unknown.
- **The PC v2.02 recommendation cap is 356.** This matches the game's current
  native cap and avoids producing the former 700-level presentation that the
  updated game normalizes down to 356.
- **Search continuation and ordering preserve the v0.7.5 fix.** Demanding
  searches continue until the requested count, actual exhaustion, or explicit
  cancellation; Next batch resumes from the returned cursor.
- **The high-resolution application icon remains in use.** Windows no longer
  enlarges the 16x16 bitmap for the window and taskbar.
- **Save and runtime ownership are stricter.** Protected operations retain
  verified backups, exact readback, no-replay recovery, process-lifetime
  binding, and fail-closed behavior for ambiguous or unsupported writes.
- The visible product version and generation identity report 0.8.0.

## Current RC boundary

- Search and preview use the PC v2.02 resource bundle.
- Protected writes against PC v2.02 remain disabled until version-matched live
  game and real-save acceptance is completed. The application refuses these
  operations instead of guessing compatibility.
- Offline tests, synthetic encrypted saves, and packaged startup checks do not
  establish real-game or real-user-save acceptance.

## Preserved product behavior

The interface retains the 25-result default, descending result sorts, exact
preview-to-operation candidate identity, favorites, cart subset selection,
editor, backup manager, automatic update prompt, manual update check, three
languages, and the install-free single-EXE delivery contract.

Equipment generation, Soul Core generation, noncanonical affix editing, and the
new Divine-scroll `添画` mechanism are outside this release candidate.
