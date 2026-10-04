# Nioh 3 Studio 0.8.4

Download `Nioh3Studio-0.8.4-win-x64.exe` and run it directly, or use the signed
in-app updater. No manual extraction, Python, Node.js, Electron or Cheat Engine
setup is required. Microsoft Edge WebView2 remains a Windows prerequisite.

- Save-file edits and scroll installs work again after the September game
  update, which grew the account system save to 235896 bytes. The system save
  is still only fingerprinted, backed up and restored, never modified.
- Live editing works again: following the in-game inventory selection failed
  with an "operation output does not match the protected contract" error in
  0.8.3 whenever the inventory menu was closed.
- Compatibility mode keeps only the three newest save copies instead of adding
  a full copy every time it is confirmed.
- Loadout codes: copy one piece, everything you have equipped, or all saved
  equipment as a short `N3E1-` code. Paste a code under Add new equipment ->
  Favorites to save its pieces, then queue them together as modded additions.
- Equipment favorites: star an owned piece to add it to any character later.
- Equipment sets: a Sets filter groups set pieces across weapons, armor and
  accessories by set effect. Lists filter by class, then weapon type, armor
  slot or accessory kind, with a samurai/ninja choice for armor, and scroll
  instead of paging.
- Saves page (formerly Backups) with save re-sign: import a character save
  from another account into the selected slot, re-signed to this account.
  The slot is backed up first. Re-signed saves have not yet been accepted
  in game; try it on an unimportant slot.
- Dark mode: Settings -> Appearance follows Windows or forces light or dark.
- Settings is a small menu beside its button, with game version details and
  a new About & safety page (free, official download sources, what the tool
  reads, writes and connects to).
- Favorites is a sidebar page.
- The compatibility notice can be hidden for the current game version; it
  returns after a game update or if a backup fails.
- Live equipment addition backs up the save chosen in the save picker.
- The equipment editor marks a row with unapplied changes; backups show
  readable times, newest first.

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
