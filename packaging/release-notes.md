# Nioh 3 Studio 0.8.3

Download `Nioh3Studio-0.8.3-win-x64.exe` and run it directly, or use the signed
in-app updater. No manual extraction, Python, Node.js, Electron or Cheat Engine
setup is required. Microsoft Edge WebView2 remains a Windows prerequisite.

- Generate equipment from seeds, search selected effects and review a waiting
  list before adding records to a save. Legal mode retains its item-grant route
  and player-progress limits; Modded mode remains available.
- On PC v2.02, preview game-generated equipment and confirm one live addition.
  Save at a shrine afterward to persist it.
- Preview scroll completion, replacement and extra-painting branches for up to
  five rounds. This read-only prediction covers PC v2.02 NG3, type `0xE604`,
  rarity 4; Revelation and automatic slot selection remain unverified.
- Filter and sort equipment, identify worn items, and remove equipment.
  Live removal requires unequipping first; save-file removal also clears the
  item's equipment-set references.
- Improve live-add status checks, stale-lock recovery and newly installed scroll
  usage state. Possible writes retain durable receipt recovery without replay.
- Select the actual game EXE in Settings and inspect its file version and
  operation capabilities. Existing versions retain their evidenced adapters;
  known additions validate their own operation bindings and backup checkpoints.
- Offer an explicit exit from compatibility confirmation. Explain denied
  game-process access with two conditional recovery paths: restart the tool as
  administrator when the game runs elevated, or disable the game's administrator
  setting and restart both before retrying a read.
- Preserve unchanged equipment drafts, recover from proven pre-write plan
  refusals, and keep long-plan confirmations accessible. Bound save, backup and
  restore reads and correct out-of-bounds access in the legacy decrypt helper.
- Correct parameterized effect labels, Sudama Soul Core naming and rerolled
  soul-core naturalness verdicts.
- Remove external VC-runtime dependencies from the shipped native executables
  and prevent inherited developer selectors from changing the packaged worker
  graph. Verify the outer EXE alone in cold isolated caches, including missing
  cache-member repair. Unreadable caches are preserved with explicit IO errors.

## Supported scope and known limitations

The primary target is PC v2.02 (`2.0.2.0`). PC v2.01 (`2.0.1.0`) retains its
existing offline and registered scroll/count paths; seeded native equipment
addition is unsupported. PC v2.00.02 (`2.0.0.2`) retains its existing offline
scope. Older experimental capabilities remain labeled. Unknown executable
versions or missing resource/layout/ABI evidence are refused; this release does
not guarantee every DLC1 build or distribution variant.

Menu recognition in one reported environment remains unresolved. The guarded
reader now records more diagnostic evidence; it is not a confirmed fix for that
report. Account system saves of 235896 bytes exceed the supported 235384-byte
format and are refused, including affected character edits and scroll installs.
Support for that larger format awaits an actual offline sample; no truncation or
unsafe bound relaxation is performed. Terrain/spawn selection remains under
investigation in issue #29.

Local and hosted acceptance cover source checks, synthetic/offline flows and the
actual packaged UI/worker graph. They do not certify every live-game write,
real-save persistence path or unknown game build.

Return to the title screen before editing a save file, then load it in game.
Keep an independent full-account backup. Automatic backups, single-writer locks,
rollback, native validation and protected-operation recovery remain enabled.
