# Nioh 3 Studio 0.7.3

This maintenance release improves failure recovery, automatic diagnostic copying,
and backup restoration. It includes the reviewed fixes from the withdrawn
v0.7.2 candidate.

## Installation and updates

Download `Nioh3Studio-0.7.3-win-x64-setup.exe` and run it. New users need only
this installer. A complete portable ZIP remains available. Cheat Engine, Python,
Node.js, and Electron are not required.

Existing Tauri installations can use the signed in-app updater. It replaces the
application in place, keeps rollback files until the new frontend and backend
start successfully, and then removes the previous version and download cache.
Installed copies retain their uninstall registration.

## Fixes

- Failed operations automatically replace the clipboard with useful diagnostic
  details, including worker errors, save paths, candidate records, and native
  receipts. The original error remains visible even if diagnostic copying fails.
- Logs use readable timestamps, keep the first actionable error, correctly handle
  Japanese and Chinese text across chunks, and rotate at five 4 MiB files.
- Live addition releases rejected preparation attempts correctly and binds native
  operations to the original process lifetime. Uncertain insertions are never
  replayed; a stopped protected worker can be recovered after its process exits.
- Save transactions verify the original file generation and recover interrupted
  commits without silently replacing later changes. Backup restoration now also
  checks source backups, manifests, sibling files, and rollback checkpoints.
- Generated-scroll addition, permanent editing, deletion, and backup restoration
  are available at the title screen or with the game closed. Every supported write creates and verifies a backup first.
- The result-page **Add to cart** button is easier to see. Grouped special-rule
  families can share one chosen value, including any weapon's Against All Comers
  and any elemental-damage increase.

The interface retains the 25-result default, descending result sorts, favorites,
cart subset selection, editor, backup manager, three languages, and visible
version number.

## Known limitations

A running game may later overwrite externally changed save files. The reported
title-save corruption has not been reproduced or proved fixed. If a change is
lost or a save cannot be loaded, restore a verified backup from **Backups**.
Restoration returns progress to the selected backup's point in time.

The intermittent live-add report has several repaired failure paths, but its
original cause has not been confirmed. If it recurs, include the automatically
copied diagnostic log. Further early-playthrough testing and possessed-enemy
selection research remain deferred.
