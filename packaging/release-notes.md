# Nioh 3 Studio 0.8.5

Download `Nioh3Studio-0.8.5-win-x64.exe` and run it directly, or use the signed
in-app updater. No manual extraction, Python, Node.js, Electron or Cheat Engine
setup is required. Microsoft Edge WebView2 remains a Windows prerequisite.

- Adding or editing equipment in a save file no longer keeps failing with
  "the save changed" after the game saved once (for example after a live
  addition). Reload on the equipment page now reads the save again; there is
  no need to restart the tool.
- Live remaining-count edits work. In earlier 2.x releases they always
  failed before writing anything. The save is backed up first, as with live
  additions.
- Soul cores carry the name the game shows, such as 一目连的魂核. Six that
  had no name are now listed and can be added: 一目连, 铁鼠, 垢尝, 雪入道,
  蛤蟆附身 and 大蛤蟆. Item 0x3336 is 狱卒鬼（业风）的魂核.
- The earlier Hattori Hanzo is shown as 服部半藏（前代）, as in game.
- If the single EXE cannot start, the message is now in Chinese and says
  what to do.

## Supported scope and known limitations

The primary target is PC v2.02 (`2.0.2.0`). PC v2.01 (`2.0.1.0`) retains its
existing offline and registered scroll/count paths; seeded native equipment
addition is unsupported. PC v2.00.02 (`2.0.0.2`) retains its existing offline
scope. Unknown executable versions or missing resource/layout/ABI evidence are
refused; this release does not guarantee every DLC1 build or distribution
variant.

Menu recognition in one reported environment remains unresolved. Changing
terrain with a temporary scroll edit can leave a mission without enemies
(issue #29); when that happens is not yet known.

Return to the title screen before editing a save file, then load it in game.
Keep an independent full-account backup. Automatic backups, single-writer locks,
rollback, native validation and protected-operation recovery remain enabled.
