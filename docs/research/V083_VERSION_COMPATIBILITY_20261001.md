# Version compatibility and executable variants

## Owner decision and completion contract

On 2026-10-01 the owner authorized replacing whole-executable equality refusals
with an explicit compatibility warning and backup confirmation. The target old
game versions are PC 2.00.02 (2.0.0.2) and PC 2.01 (2.0.1.0), alongside PC
2.02 (2.0.2.0). Repackaged/modified executables must not be rejected solely
because their whole-file SHA differs. Preserve actual process identity, bounded
memory reads, record ownership, compare-and-swap/readback, validated native code
sites, durable receipts and no-replay recovery. No instruction requires closing
the game; save-file operations use the title-screen boundary.

Completion requires a visible per-feature compatibility result, truthful backup
outcomes, deliberate opt-in for an unverified variant, preserved old-version
resource selection and a matching standalone candidate. Unknown version/layout
semantics remain explicitly unsupported instead of silently borrowing current
generation tables. Offline/helper/package evidence is separate from acceptance
on the community's actual executable.

## Failure cases recorded before implementation

- A same-version modified executable is rejected only for its whole-file hash.
- Reading silently changes user memory or claims a write happened.
- An unverified write/native preview proceeds before explicit risk/backup
  confirmation, or a confirmation from another PID/birth/image is reused.
- A failed/missing/ambiguous automatic backup is described as successful.
- A backup copy differs from its source or its location is not shown.
- Code/layout/ownership/record checks disappear along with the whole-file gate.
- A process restart, code-site change, altered container or uncertain receipt
  causes replay instead of recovery/refusal.
- Old 2.00.02/2.01 requests use 2.02 tables or current-version RVAs silently.
- The UI advertises an unsupported old-version feature, or disables already
  supported old-version search/save/scroll functions.
- Startup ignores the sole running Nioh3.exe outside Steam; multiple running
  games or an explicit invalid selected path are guessed rather than reported.
- GPU unavailability is attributed to installation edition without probe data.
- A new warning/confirmation is hidden, untranslated or lost across navigation.
- Acceptance/build leaves disposable profiles or development caches consuming
  the delivery drive after completion.

Use E2E/native helper workflows and retained artifacts; add no postimplementation
unit tests. Preserve all existing candidate EXEs and evidence. Work in the
verified D: source checkout and shared release target, with no push/publication.

## Implemented scope and bounded evidence

Whole-file hash mismatch is now advisory for character reads. The native host
owns explicit compatibility consent for writes/previews, bound to PID, process
birth, image path, file version and actual hash. Missing/failed backup attempts
are reported truthfully and require the owner's manual-backup confirmation;
verified copies include paths and byte-identical readback. Code-site, container,
record, compare-and-swap and no-replay checks remain. Library executors retain
their strict default and expose a separate host-consent opt-in.

Bootstrap can resolve the sole running Nioh3.exe before bounded Steam discovery;
explicit selected paths remain authoritative. Feedback includes the worker's
real accelerator capability flags. No edition-based GPU restriction was added.

The old character globals are version-selected: CT 2.00.02 uses 0x4749820 /
0x4749500; 2.01 uses the accepted inventory manager 0x474D4E0 plus the shared
0x320 player-global displacement. Old player vtables/functions must lie inside
the actual PE image, and the independently resolved equipment owner must still
match the player-relative container. This is an experimental old-layout path,
not real-game acceptance on either old executable. Current 2.02 retains its
exact loaded-player vtable. Native runtime preview resources now select the
actual GenerationContext version rather than CURRENT_RESOURCE_VERSION.

Existing offline search/save features cover 2.00.02 and 2.01. Native scroll
addition retains its existing 2.01/2.02 bindings. The new native seeded equipment
addition is still 2.02-only: old equipment-builder code/semantics are not yet
captured. The warning reports that limitation instead of advertising support
or calling a 2.02 builder in an old game.

The real framed host/schema E2E passed six version/backup combinations, including
process-birth invalidation and version-selected character reads over an owned
fixture memory map. The component/browser E2E passed 42 checks over Chinese,
English and Japanese, successful/missing automatic backup and constrained
window geometry. Screenshot review caught a harness-only JSX localization
configuration omission; it was corrected to match the production bundler.
The added character-read frame was also moved before shutdown in the verifier.
No unit tests were added after implementation. Source type/locale and focused
release Clippy checks pass. Matching outer-EXE/helper acceptance remains pending.
