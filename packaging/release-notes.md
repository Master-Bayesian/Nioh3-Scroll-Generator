# Nioh 3 Studio 0.7.5

This emergency update fixes two shipped problems: a search could stop early and
report no results while the range was still unfinished, and the window and
taskbar icon was blurry because Windows scaled up the smallest stored image.

## Direct launch and updates

Download `Nioh3Studio-0.7.5-win-x64.exe` and double-click it. No installation,
installer wizard, or manual extraction is needed. The runtime is prepared and
verified automatically in a bounded cache. Cheat Engine, Python, Node.js, and
Electron are not required.

The app performs one update check on every packaged launch after startup safety
checks finish. When a newer signed release is available, it opens the update
dialog and asks whether to download it. The same check remains available from
Settings. Applying an update verifies the signed payload and replaces the outer
EXE in place, with rollback data retained until the new version starts.

## New and improved

- **Searches no longer stop short and look empty.** A demanding condition set
  could end early while the searchable range was still unfinished, and that
  ending read the same as a genuinely empty result, so a working combination
  appeared impossible. Searching now keeps going until it has the number of
  scrolls you asked for, until the range is exhausted, or until you cancel.
  A three-rule combination that returns nothing on 0.7.4 now returns its
  matching scroll directly.
- **The result line says which of those happened**, and only reports the range as
  exhausted when it really was.
- **Cancel and Next batch still work together.** Cancelling stays responsive, and
  **Next batch** resumes from where the search stopped instead of starting over.
- **Some long searches are faster.** One internal pass now covers more ground
  before returning, which removes repeated per-pass overhead for searches that
  scan a long way. Matches and ordering are unchanged.
- **The window and taskbar icons use the high-resolution artwork.** The packaged
  icon still carries every original image, but it previously led with the 16x16
  bitmap and Windows stretched that bitmap for the window and taskbar. It now
  leads with the 256x256 image and Windows scales that artwork down, so the icons
  are no longer blurred by a 16x16 upscale.
- The visible version number reports 0.7.5.

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
  commits without silently replacing later changes. Backup restoration also
  checks source backups, manifests, sibling files, and rollback checkpoints.
- Generated-scroll addition, permanent editing, deletion, and backup restoration
  are available at the title screen or with the game closed. Every supported
  write creates and verifies a backup first.
- The full interface remains reachable at constrained window sizes and Windows
  scaling levels, cards keep stable action rows, and Settings use accessible
  switches for boolean preferences.

The interface retains the 25-result default, descending result sorts, favorites,
cart subset selection, editor, backup manager, three languages, and visible
version number.

## Closed reports and follow-up

The unreproduced intermittent live-add and title-save reports remain closed for
the active release cycle by the owner's decision. They are not described as
proven root-cause fixes. Reopen investigation only with a fresh affected save
and diagnostic log.

A website, usage statistics, and equipment or Soul Core generation are outside
this release's scope.
