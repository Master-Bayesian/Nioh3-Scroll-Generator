# Nioh 3 Studio 0.7.3

This unpublished candidate improves failure recovery, automatic diagnostic
copying, backup restoration, and direct launch from one install-free EXE. It
includes the reviewed fixes from the withdrawn v0.7.2 candidate. Publication
requires the owner's review of the final local package.

## Direct launch and updates

Download `Nioh3Studio-0.7.3-win-x64.exe` and double-click it. No installation,
installer wizard, or manual extraction is needed. The runtime is prepared and
verified automatically in a bounded cache. Cheat Engine, Python, Node.js, and
Electron are not required.

The in-app updater verifies the signed payload and replaces the outer EXE in
place. It keeps a rollback copy until the new frontend and backend start
successfully, then removes the previous executable and download cache. The
internal signed ZIP format is retained for compatibility with earlier Tauri
updaters. The old installer is retained locally as a historical artifact and is
not the default download.

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

## Closed reports and follow-up

The unreproduced intermittent live-add and title-save reports are closed for
this release cycle by the owner's decision. They are not release blockers and
are not described as proven root-cause fixes. Reopen investigation only with a
fresh affected save and diagnostic log. Verified automatic backups and recovery
checks remain enabled. Restore returns to the selected backup's point in time.

A website, usage statistics, and possessed-enemy research are outside this
release's scope.
