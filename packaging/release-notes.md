# Nioh 3 Studio 0.7.4

This release adds exact Crucible Wraith enemy filtering for supported Hundred
Demon Realms Picture Scrolls, polishes the search and collection interface, and
makes update availability visible at startup.

## Direct launch and updates

Download `Nioh3Studio-0.7.4-win-x64.exe` and double-click it. No installation,
installer wizard, or manual extraction is needed. The runtime is prepared and
verified automatically in a bounded cache. Cheat Engine, Python, Node.js, and
Electron are not required.

The app performs one update check on every packaged launch after startup safety
checks finish. When a newer signed release is available, it opens the update
dialog and asks whether to download it. The same check remains available from
Settings. Applying an update verifies the signed payload and replaces the outer
EXE in place, with rollback data retained until the new version starts.

## New and improved

- Enemy filters can require a specific eligible low-pool enemy to appear as a
  **Crucible Wraith** (Simplified Chinese: `地狱附身`; Japanese: `地獄憑き`).
  Eligibility and preview state come from the recovered native rules rather
  than assigning the state to arbitrary enemies.
- Enemy conditions now fit on one compact row. Preview state markers sit beside
  the enemy name, and the collection view uses the same fixed card geometry as
  search results with a local metadata search.
- Search guidance is integrated into the equipment and mission columns, leaving
  more room for selected conditions. Preview metadata uses consistent type and
  alignment roles, and the ineffective interface-font-size setting was removed.
- The packaged app checks for updates once per launch, prompts once when a newer
  version is found, and retains a manual **Check for updates** action in Settings.

## Preserved safety and recovery

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
- The result-page **Add to cart** button remains prominent. Grouped special-rule
  families can share one chosen value, including any weapon's Against All Comers
  and any elemental-damage increase.
- The full interface remains reachable at constrained window sizes and Windows
  scaling levels. Collection cards keep stable action rows, long enemy lists
  scroll inside their cards, and long localized titles no longer hide metadata.
- Settings use accessible switches for boolean preferences. Favorites and
  editor-selection dialogs can be dismissed from the backdrop without treating
  an interaction that began inside the dialog as an outside click.

The interface retains the 25-result default, descending result sorts, favorites,
cart subset selection, editor, backup manager, three languages, and visible
version number.

## Closed reports and follow-up

The unreproduced intermittent live-add and title-save reports are closed for
this release cycle by the owner's decision. They are not release blockers and
are not described as proven root-cause fixes. Reopen investigation only with a
fresh affected save and diagnostic log. Verified automatic backups and recovery
checks remain enabled. Restore returns to the selected backup's point in time.

A website, usage statistics, and equipment or Soul Core generation are outside
this release's scope.
