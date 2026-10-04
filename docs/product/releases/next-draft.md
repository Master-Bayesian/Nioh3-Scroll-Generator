# Next release (draft, not published)

Collected on 2026-10-04 after 0.8.5 (`e29688f`). The version number and the
release are the owner's decision; this file only gathers what changed.

## Player-facing changes

- **Scrolls can be edited in the running game.** The scroll editor has the
  equipment page's two modes, `游戏内实时修改` (default) and `修改存档文件`.
  Basic details and effects are written to the live scroll after a review that
  backs the save up first; the game keeps them when it saves. Sections now say
  what is kept: 基础信息, 主副词条, 副本内容 · 临时，不进存档.
- **English and Japanese show the game's own names** for equipment, effects
  (with their buff, ailment or ninjutsu filled in), soul cores and hell
  skills, captured from the game in each language (#33). Item categories are
  translated, and the editor's searches accept English and Japanese.
- Interface terms follow the game: Japanese 魂代 (not 魂核), English
  Crucible (not Hell) for 地狱.
- Same-name enemies keep their form or generation in English and Japanese
  (Hattori Hanzo (Former), Takeda Shingen (Yokai form), ...).
- 0xCC33 is named 火炎龙计量槽增加量. Seven ninjutsu effects that the game
  itself shows as "{}" are named 未命名忍术（0x....） and no longer merge in
  the effect picker.
- Familiarity is shown and entered in the game's unit (999, not 99900).
- The dark theme works in the desktop app (it only worked in the browser
  build), and the scroll card stays dark in it.
- Six unfinished items that the game names DUMMY are no longer offered for
  addition.
- Clearer messages: a live edit at the title screen says the character is
  not loaded; a live addition the game never answered says nothing was added;
  a count edit whose confirm button is disabled says why; the live editor
  warns when it reads the title screen's placeholder character.
- Feedback files carry the record the tool expected and the record the game
  built for recent live additions, so a failed addition can be diagnosed.
- Fixes: clicking the current mode emptied the equipment page; a count review
  started during another read failed with OPERATION_OBSERVER_DISPOSED; a
  refused count or scroll review left an empty backup behind; the cart and
  history dialogs squeezed the scroll card's header; the version details
  showed developer English; the backup list named most kinds 存档备份.

## Verified

- Live (owner's PC v2.02 game, through the interface): gold, item quantity,
  equipment effect, remaining count, live equipment add/remove.
- Save mode with the game at the title screen: equipment familiarity and a
  scroll effect written and restored, backups taken.
- Gates: frontend tests (95/108; the 12 local IPC failures predate this
  work), the four CI UI checks, Python CI subsets, nioh3-runtime (217),
  nioh3-protected, desktop host (50 with NIOH3_PYTHON set), locale audit.

## Not yet verified (needs the game loaded)

- A live scroll edit in game, its persistence after a save and reload, and
  a second edit before the game saves.
- Whether a live scroll addition caused the 2026-10-04 crash (see the failure
  ledger); a definitive "character loaded" signal to refuse live additions at
  the title screen before any native call.
- The English name of 地狱武技.
