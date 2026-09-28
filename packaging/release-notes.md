# Nioh 3 Studio 0.8.2

The first release that goes beyond scrolls: a new Character & Equipment page
edits Amrita, gold, items, equipment and soul cores, both in the running game
and in a save file. Scroll search now also covers playthroughs 1 and 2.

## Direct launch and updates

Use `Nioh3Studio-0.8.2-win-x64.exe` directly. No installation, extraction,
Python, Node.js, Electron, or Cheat Engine setup is required. 0.8.1 offers this
update automatically after its startup checks, or from Settings.

## New: Character & Equipment

- **Two modes.** "游戏内实时修改" edits the running game; "修改存档文件" edits a
  save file with the same automatic backup and review step as scroll edits.
- **Character opens ready.** Entering the page reads the character at once;
  "重新读取" reads it again. Following the item selected in game is on by
  default.
- **Amrita (精华) and gold** can be set directly.
- **Items:** change the held and stored quantity of consumables and materials.
- **Equipment and soul cores:** browse owned gear by type with the game's own
  item names, then edit level, +value, rarity, familiarity and effects.
  - **Legal mode** (default) only offers values and effects the game can
    generate naturally for that item, with the legal range shown next to each
    value and "全部取理论最高" to max every value at once.
  - **Modded mode** is an explicit opt-in for values outside natural rules.
  - Mutually exclusive effects are filtered out, effects a tool replaced are
    flagged, and edited effects now show the right icon in game.
  - Hell weapons show their real hell martial skill names.

## Scroll search

- **Playthroughs 1 and 2 are searchable offline**, with install and known-seed
  preview, matching the game's own generator byte for byte on 10,000 records
  per rarity. Rarity-5 scrolls of these playthroughs carry six ordinary effects
  and no Grace, and the search refuses combinations they can never have.
- **Rarity-5 searches accept a promoted effect as a secondary.** The game can
  place its single promoted effect in any slot; the earlier rule wrongly
  required it to be the primary.

## Scroll editor

- **Every effect shows its natural value range** (lowest–highest for the
  scroll's rarity and level) next to its value, in the effect list and in the
  preview, so you can judge how good a value is.

## Fixed

- **A save locked by an interrupted write can be unlocked.** If the app was
  closed while writing, the save stayed locked for good once its backup was
  gone. The app now compares the save with the write's before and after
  contents and settles it on its own; when it cannot tell, check the save in
  game and click "我已检查，继续使用". Your confirmation is recorded and only
  that one write is released.

## Compatibility and limits

- Supported game build: PC v2.02 (and the earlier approved builds). Unsupported
  builds and unknown resource graphs are refused rather than guessed at.
- Accessories cannot be re-rolled in game, so an accessory whose effects were
  changed by a tool is reported as not natural.
- Offline tests, synthetic encrypted saves, and packaged startup checks do not
  establish real-game or real-user-save acceptance for every path.
