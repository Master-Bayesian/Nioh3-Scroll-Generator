# Missing v0.8.3 feature acceptance

Owner scope: expose real-time equipment insertion, ordinary scroll completion
replacement prediction, and extra-painting prediction. The root implements this
work personally. A package containing only one slice is not the new-version
delivery. No new live write or publication is performed by these offline checks.

## Failure scenarios recorded before implementation

- Prediction receives an unsupported version/type/rarity, malformed bytes,
  unknown effect, inconsistent group, exhausted attempts, or overflowing counter.
  Reject rather than silently guessing.
- Weighted lottery order, category draw, warm-up, signed seed arithmetic,
  prior-candidate exclusions, roll/value rounding, or painting selector differs
  from the game. Compare retained native records and choices through the host.
- Declining fails to advance the counter/attempts, or painting uses the record
  before the decision. Check accepted and declined branches and subsequent rounds.
- Pity thresholds are rounded as decimal arithmetic; successful rolls increment
  pity; grace gets offered/replaced; painting occurs twice. Cover these guards.
- A fresh scroll needs revelation. Ordinary-completion results must explicitly
  remain conditional; unknown revelation slot selection is not invented.
- Prediction mutates a file/game, or a simulated record leaks into an install
  command. The route is read-only and simulation only.
- A response arrives after changing a scroll/snapshot/draft/round. The UI ignores
  it and clears stale results; unsaved edits do not become persisted input.
- Live equipment uses scroll descriptor offsets/stride/builder/container,
  overwrites cave code with data, allocates serial twice, or inserts on a foreign
  thread. Profile and helper-process E2E must cover the equipment ABI.
- An uncertain native dispatch is replayed; a second worker takes ownership;
  cleanup/cancellation loses a receipt. Retain persistent no-replay recovery and
  the shared admission lock across equipment and scroll additions.
- Live equipment preview and final inventory disagree; player state changes
  between seed search and preparation/execution; inventory is full; game layout
  is unloaded/foreign. Reject before insertion, or retain uncertainty after it.
- UI hides any of the three features, falsely claims live/save persistence, or
  loses a prepared operation on navigation. Verify actual entry points,
  confirmation, stale state, all locales and the final packaged outer EXE.

## Completion criterion

The equipment mismatch control changes the helper's builder template after
preview. The shim must compare the full rebuilt record before insertion;
An allocated serial with no insertion can close as rejected before insertion
only with acknowledged guard, unchanged full container and verified cleanup.
Missing any proof retains uncertainty; neither state can be replayed.
The insertion helper must preserve destination +0x24..0x27 and +0xE4..0xEF,
as the retained PC202 copy routine 0x552DD0 does, and assign the old acquisition
counter to +0x1C while advancing that counter exactly once (0x54C380).
Preview still compares the whole 0xF0 scratch record; destination validation
compares copied fields plus these independently checked inventory-owned fields.
An unregistered request can be rejected without opening a game process, so an
unloaded game or transport rejection does not permanently trap the UI.

All three have reachable product UI, protected requests, bounded repeatable E2E
artifacts and a matching reviewed standalone EXE. New equipment game insertion
and save/reload acceptance remain a separate owner-present step; returning to
title is sufficient when a file workflow requires unloading a character.
