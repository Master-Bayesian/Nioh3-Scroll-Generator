# Nioh 3 Studio 0.8.7

Download `Nioh3Studio-0.8.7-win-x64.exe` and run it directly, or use the signed
in-app updater. No manual extraction, Python, Node.js, Electron or Cheat Engine
setup is required. Microsoft Edge WebView2 remains a Windows prerequisite.

- Live equipment addition no longer gets stuck. Selling the added item or
  restarting the game before the check left 读取添加状态 and 恢复核对 doing
  nothing, and blocked every later addition. The check now concludes when the
  game carried the addition out ("check your inventory, do not add it again"),
  an unresolved addition no longer blocks a restarted game, and an unconfirmed
  record can be dropped.
- Settings → 重置工具: with the game closed, it moves leftover operation
  records aside and reopens the interface. Backups, favorites and settings are
  kept. Use it when a button does nothing or the tool keeps asking about a
  previous operation.
- Ordinary play no longer refuses a live edit: picking up, selling or
  auto-dismantling items and game autosaves between review and confirmation
  are accepted. Equipment addition no longer stops at 65535 acquired items
  (players using auto-dismantle reached it).
- Adding equipment when the held inventory is full (2000 items) now says so,
  in both modes. Before, the item was placed past the limit and the game
  discarded it.
- A scroll batch keeps going when the inventory changes between items, and
  the cart and the count editor offer a way out when a check cannot settle.
- If the app closes during startup (for example blocked by antivirus), the
  EXE now explains why and where the logs are, instead of showing nothing.
- The light theme is the default again; 深色 and 跟随系统 remain in Settings.

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
