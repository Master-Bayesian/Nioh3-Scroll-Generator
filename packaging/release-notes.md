# Nioh 3 Studio 0.8.6

Download `Nioh3Studio-0.8.6-win-x64.exe` and run it directly, or use the signed
in-app updater. No manual extraction, Python, Node.js, Electron or Cheat Engine
setup is required. Microsoft Edge WebView2 remains a Windows prerequisite.

- Scrolls can be edited in the running game. The scroll editor has the
  equipment page's two modes, 游戏内实时修改 (default) and 修改存档文件. Basic
  details and effects are written to the live scroll after a review that backs
  the save up first, and the game keeps them when it saves. The sections say
  what is kept: 基础信息, 主副词条, and 副本内容 · 临时，不进存档.
- English and Japanese show the game's own names for equipment, effects,
  soul cores and Crucible Arts, read from the game in each language (#33).
  Item categories are translated, and searches accept English and Japanese.
  Terms follow the game: Japanese 魂代, English Crucible and Crucible Arts.
  Same-name enemies keep their form, such as Hattori Hanzo (Former).
- The dark theme now works in the desktop app; before, it only took effect in
  the browser build.
- Familiarity is shown and entered as in game (999, not 99900).
- 0xCC33 is named 火炎龙计量槽增加量. Seven ninjutsu effects the game itself
  shows as "{}" are listed as 未命名忍术（0x....）. Six unfinished items the
  game names DUMMY are no longer offered for addition.
- Clearer messages: a live edit at the title screen says the character is not
  loaded; a live addition the game never answered says nothing was added; a
  count edit that cannot be confirmed says why; the live editor warns when it
  reads the title screen's placeholder character.
- A second live edit after the game autosaved no longer fails.
- Feedback files now include what the tool expected and what the game built
  for recent live additions.
- Fixes: clicking the current mode emptied the equipment page; a count review
  during another read failed with OPERATION_OBSERVER_DISPOSED; a refused
  review left an empty backup behind; the scroll card was squeezed in the cart
  and history dialogs; the version details showed developer English; the
  backup list named most backup kinds 存档备份.

## Supported scope and known limitations

The primary target is PC v2.02 (`2.0.2.0`). PC v2.01 (`2.0.1.0`) retains its
existing offline and registered scroll/count paths; seeded native equipment
addition is unsupported. PC v2.00.02 (`2.0.0.2`) retains its existing offline
scope. Unknown executable versions or missing resource/layout/ABI evidence are
refused; this release does not guarantee every DLC1 build or distribution
variant.

The dark theme's colors are still being refined. Menu recognition in one
reported environment remains unresolved. Changing terrain with a temporary
scroll edit can leave a mission without enemies (issue #29); when that happens
is not yet known.

Return to the title screen before editing a save file, then load it in game.
Keep an independent full-account backup. Automatic backups, single-writer locks,
rollback, native validation and protected-operation recovery remain enabled.
