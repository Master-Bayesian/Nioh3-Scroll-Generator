# Nioh 3 Studio 0.7.2

This maintenance release makes failed operations diagnosable, hardens both
addition paths, and adds a one-file installer for new users.

## Installation

Download and run `Nioh3Studio-0.7.2-win-x64-setup.exe`. The portable ZIP is
also available for users who prefer to extract the complete application folder.
No Cheat Engine, Python, Node.js, or Electron installation is required.

Existing Tauri installations can use the signed in-app updater. It downloads
the complete verified portable package, replaces the application in place,
rolls back a failed startup, and removes the previous version and update cache
after the new frontend and backend complete their startup handshake. Updates
from the installer preserve its verified uninstall entry.

## Changes

- Automatically copy a bounded diagnostic log to the clipboard when an
  operation fails. Logs now retain save paths, candidate records, native
  receipts, and worker errors needed to reproduce a failure, use readable UTC
  timestamps, and rotate at five 4 MiB files.
- Retry only the read-only live-add preview when the game reports a fully
  released idle-window miss. Actual insertion is never replayed.
- Refuse every save-file plan and commit while Nioh 3 is still running. This
  prevents a title-screen add followed by an immediate game exit from
  overwriting the modified save with stale in-memory state.
- Make the result-page **Add to cart** control prominent.
- Allow grouped special-rule families such as Any Against All Comers or any
  elemental-damage increase to share an exact selected value.
- Keep the 25-result default, descending result sorts, favorites, cart batch
  selection, editor, backup manager, trilingual interface, and visible version.

Every live addition still creates and verifies a save backup before dispatch.
The live executor remains version-gated to the accepted PC build. Independent
selection of a possessed Crucible enemy is not exposed because the native
per-occurrence state has not yet been proved.
