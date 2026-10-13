# Nioh 3 Studio 0.8.9

Download `Nioh3Studio-0.8.9-win-x64.exe` and run it directly, or use the signed
in-app updater. No manual extraction, Python, Node.js, Electron or Cheat Engine
setup is required. Microsoft Edge WebView2 remains a Windows prerequisite.

- A new interface: navigation moves to a bar at the top, with light and dark
  themes, a three-column scroll search and cards that grow with their text.
  Long names, rules and translations wrap instead of being cut off, down to the
  smallest window.
- The game's own marks: the star on a scroll or a piece of equipment is the
  game's Favorite mark, and the lock is its Protect mark. Both can be set or
  cleared in a save file or in the running game; favorites sort to the top.
  Deleting a protected scroll asks again.
- Find the scroll under the game's inventory cursor in the scroll list.
- The scroll editor uses the same effect table as the equipment page, with a
  choice of which effects to offer, and its preview shows enemies, rules,
  terrain and the attempt limit.
- The save picker shows what the game's load menu shows: playthrough, level,
  play time and when it was saved.
- Restoring a backup made by an in-game edit works again. These backups hold
  only the main save; restoring one now puts back just that file.
- If the program cannot start because antivirus software locked or quarantined
  its files, the message says so first, can open the log folder, and offers
  the portable ZIP from GitHub.

## Supported scope and known limitations

The primary target is PC v2.02 (`2.0.2.0`). PC v2.01 (`2.0.1.0`) retains its
existing offline and registered scroll/count paths; seeded native equipment
addition is unsupported. PC v2.00.02 (`2.0.0.2`) retains its existing offline
scope. Unknown executable versions or missing resource/layout/ABI evidence are
refused; this release does not guarantee every DLC1 build or distribution
variant.

Item marks are known only for scrolls and equipment; stars on consumable items
stay in this program. Menu recognition in one reported environment remains
unresolved. Changing terrain with a temporary scroll edit can leave a mission
without enemies (issue #29); when that happens is not yet known.

Return to the title screen before editing a save file, then load it in game.
Keep an independent full-account backup. Automatic backups, single-writer locks,
rollback, native validation and protected-operation recovery remain enabled.