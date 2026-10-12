# Arc UI preview: handoff (2026-10-11)

Branch `claude/arc-ui-preview`. Every push builds an unsigned preview EXE through
`.github/workflows/ui-preview.yml` (artifact `nioh3-ui-preview-exe`; never signed or published).
This note hands the work from a cloud session to a local session that can reach the game.

## State of the branch

- Arc restyle (`apps/workshop/arc.css`, loaded after `style.css`), three-column search page, top bar,
  dark mode, owner's flat Ippon-datara mark (`apps/workshop/brand-mark.ts`, `assets/*.ico|png`).
- `.ico` entries are ordered largest first: Tauri (`tauri-codegen` `image.rs`) builds the window and
  taskbar icon from the first ICO entry; it used to be 16x16, which made the taskbar icon blurry.
- UI-001 to UI-008 addressed (owner rejected UI-009 and an About intro; both removed).
- Last commit adds `apps/workshop/EffectPicker.tsx`: the searchable effect dropdown extracted from
  `CharacterEditor.tsx` unchanged, so the scroll editor can reuse it. Typecheck passes; tests 108 pass,
  2 fail only for want of the Windows crypto tool (same on main).

## Owner decisions on UI-010 to UI-022 and new requests

Search page
- Primary-effect drag: the whole primary group must accept a drop (today only the first row does,
  because `.primary-group` in `main.tsx` lacks `data-condition-group`; see `condition-drag.ts`
  `move`/`finish`). Dragging a primary effect out of the group must remove it from the primary set
  (decrease `primaryCount`); not implemented today.
- UI-016: mark the conflicting effects in the selection. The worker's feasibility `reason` carries the
  `0xNNNN` ids (`crates/nioh3-worker/src/feasibility.rs`); `main.tsx` keeps only the localized text,
  so keep the raw reason and match ids to rows (`data-effect-id`).
- UI-018: selected entries sort to the top of catalog lists; clicking a selected entry ("已选") again
  deselects it (effects and enemies are `disabled` today; graces already toggle).
- UI-019 to UI-022: names are squeezed in catalog tabs (count badge pushes the label out; label is a
  bare text node), rule pickers (fixed 116px select plus button), selected rule/enemy chips and the
  enemy list. Fix the layout. Do not abbreviate "全部接受"/"任意数值" to "[ALL]" (owner rejected).
- UI-011: settings icon becomes a gear.

Scroll editor (`apps/workshop/Editor.tsx`)
- UI-017/UI-010: rebuild 主副词条 like the equipment editor table: one row per slot, the effect name
  is the shared `EffectPicker`, value input and native range on the same row, raw fields
  (prefix/metadata/tail) behind an "advanced" toggle. Removes the expand-under-row editor.
- New: effect scope selector, default "effects and graces possible on a scroll of this playthrough and
  rarity" (`data.contexts["<ng>-<rarity>"]`), switchable to "possible on any scroll" (union of contexts,
  about 72 ids) and "all effects" (`data.editorEffects`, 3609).
- UI-012: keep the primary effect as the name; add a rarity-colored dot after "R3/R4".
- UI-012 favorites: star existing save scrolls; starred scrolls always list first; provide a
  "favorites only" filter. Same marker and filter for equipment and items on the equipment page
  (equipment already has `equipment-favorites.ts`; items do not). The 收藏夹 page itself is unchanged.
- UI-012 delete confirmation: deleting a protected (locked) scroll needs a second confirmation
  (depends on reading protection state, below).
- UI-013: the preview must show special rules, enemies and terrain like the search result card
  (`ScrollCard.tsx`); `Sample` carries `rules`, `enemies`, `capacity`, `terrainKeys`.
- Remove the "备份与恢复" section from the editor; backups live only on 存档管理.

Save management (`apps/workshop/BackupManager.tsx`)
- UI-015: bound the backup list height (scroll inside), keep actions visible; error "技术详情" must be
  fully readable and scrollable (today clipped by `.editor-work{overflow:clip}` in `style.css`).
- UI-014: label saves by character level and 周目 instead of "存档 1（SAVEDATA00）· 账户 …"; the
  account id is not useful. In-game save order changes (last loaded first), so slot numbers do not help.

## Backup restore bug (real defect)

Restoring any backup made before a live (in-game) write fails with "备份清单格式无效".
- `crates/nioh3-save/src/backup.rs` `authenticate_restore_source` requires the manifest
  `operation_id` to be 32 lowercase hex (`transaction.rs` `is_operation_id`) and exactly three roles
  (main save, game backup, system).
- Runtime checkpoints (`crates/nioh3-protected/src/runtime_backup.rs` `SaveBackupAdapter::checkpoint`)
  write the same manifest schema with ids such as `scroll-<millis>`, `count-<millis>` or dashed UUIDs
  (`runtime_app.rs`, `live_add.rs`, `count.rs`) and only the main save file.
- `save.backups` (`save_app.rs`) lists them anyway, so 存档管理 and the editor offer restores that
  always fail.
- Owner position: restoring a backup is replacing files; it must work. Fix the checkpoint writer so new
  checkpoints are restorable, and make existing single-file checkpoints restorable for the files they
  hold, without weakening authentication of what is restored (keep backups, rollback, locks and
  protected-operation recovery intact; add regression tests).

## Needs the game (local session)

None of these has evidence yet; per repository rules they stay unsupported until captured.
- Protection (lock) state of scrolls and equipment, and a way to set it ("一键保护" via the game's own
  path). Only lead: equipment record `+0x18` bit `0x01` "probably a lock/favourite flag"
  (`docs/knowledge/V082_LIVE_CHARACTER_EQUIPMENT_RESEARCH_20260926.md`). Suggested capture: lock and
  unlock one scroll and one equipment, saving after each; diff the records; then find the live write.
- Locate the scroll selected in the game menu, like equipment "跟随游戏内选中"
  (`crates/nioh3-runtime/src/character.rs` `read_menu_selection`). Owner says scrolls share the same
  inventory menu; check whether the existing reader classifies a selected scroll and how it maps to a
  save scroll slot.
- Character level and 周目 for the save picker: not parsed today. `save.character` exposes
  `generation.difficulty` (1-5) used by equipment generation; whether it equals the in-game 周目 is
  unverified.

## Local session status (2026-10-11)

Done on this branch (details in the commits and the patch note):
- Backup restore: live checkpoints (main save only, `scroll-`/`count-`/UUID ids)
  restore by replacing the files they hold; a bundle without the main save is
  still refused. Regression tests in `crates/nioh3-save/tests/restore_live_checkpoint.rs`.
- Scroll editor: table of slots with `EffectPicker`, three-way effect scope, raw
  fields behind "advanced", rarity dot, preview with enemies/rules/terrain/capacity,
  backup section removed, open section shows all content.
- Search page: primary box drop in/out, conflict marks, chosen-first catalog rows
  with click-to-deselect, chip/tab/rule-picker layout, gear icon.
- Save management: bounded backup list, readable technical details.
- Game research (evidence: `docs/knowledge/ITEM_MARKS_MENU_SCROLL_SAVE_SUMMARY_RESEARCH_20261011.md`):
  protection bit `0x04` and favourite bit `0x20000` at record `+0x18`; the
  selected scroll via the inventory cursor; the save summary (level, playthrough,
  play time, save time).
- Wired: save picker labels by level/playthrough; lock mark and protected-delete
  confirmation; stars are the game's 喜好项目 for scrolls and equipment (local
  stars only for items and the browser preview); protect/favourite edits through
  the existing reviewed save and live write paths; "locate the scroll selected in game".

Still open:
- Live acceptance of the protect/favourite writes in game (save-file and live).
- Item (道具) marks were not captured, so item stars stay local.

## Deliverable at the end

A full patch note of every change on this branch, for the owner.

## Appendix: owner's words (verbatim, Simplified Chinese)

Tester feedback UI-010 to UI-022 (raw text and screenshots) is the owner's package
`Nioh3Studio_UI010-UI022_用户原话及截图.zip`. The owner's review of it, in order:

> 主词条那个框，直接拖进去可以增加主词条备选量的设计可以，但是拖到第一个词条上而不是框内，才能触发，有点苛刻，反正就是很难直接拖进去。然后是拖进去应该还能拖出来，目前出不来。
> 016是合理的，直接把互相冲突的标记出来。018也合理，而且现在已选点了不能取消掉，只能在右边去掉，应该再点一下已选，就取消选择。019的问题也存在。020和021如果有提供截图的话你应该更好理解，确实有挤压不显示完整的问题。022同理。
>
> 010存在，而且还有个问题，就是点开这个替换词条的表之后下面离很远，用户很容易错过下面还能输入数值。012的第一条我不认可，名称就用主词条。认为这是一个问题主要是难以定位绘卷，我们可以直接把装备和道具里定位装备的功能拿过来定位游戏菜单选中的绘卷。这里应该也允许直接点击收藏，然后收藏的绘卷总是在最上方显示。也就是说收藏夹除了预备绘卷，也应该允许已有绘卷。如果可以，再加入一键保护功能，用游戏里的保护函数，让绘卷（或者装备）被上保护状态，不能直接丢弃或者卖掉。012的第二条不用那么夸张，R3R4这个标记后面加一个有颜色的圆就可以了。012的第三条被我说的包括了，我们直接编辑保护状态。第四条合理，照做。
> 013合理，照做。017的本质还是我前面说的那个，这里的UI交互很糟糕，很难用。不用和绘卷搜索一样，因为功能都对不上，只是体验上要相似，要直观易操作。
> 014合理，账户都是16位数字，对用户几乎无意义，用ID更好，等级也有用。周目也有用。存档1（SAVEDATA00）这些对于用户分辨存档来说几乎看不明白。 015如图一，不仅备份多了挤下去了，而且不小心点到了不是查看备份是什么样，而是直接试图替换。而且替换还报错了，而且报错之后点开技术详情还显示不全，而且还不能往下滚。我们专门有一个section做存档管理，这里为什么还有备份存档的内容？
> 011 也确实如此，看起来是太阳或者灯的标志，我还以为是专门切换亮度或者深色模式的，结果是设置。改成齿轮好一点。
> 还有一个额外的，不是UI修改的小需求：绘卷修改那里的词条不只是绘卷内可能出现的词条，而是全词条，应该加一个开关，默认只有绘卷词条可以改进去，解除限制之后才能改进去所有词条，避免用户搞不明白哪些词条是绘卷上的。
> 做完之后要给出所有的改动patch note。

The owner's answers to follow-up questions:

> （保护字段）这个我开游戏，你读一下。
> （绘卷菜单对象）按理说都在背包里，其实都是混着的
> （人物 ID）应该就是steam的ID，联机的时候名字就是对应的这个，但这个也没有辨识度（自己的存档都是一个名字），所以也不显示吧，没啥用。
> （存档位命名）游戏里的读档里存档顺序是会变的，最后读的在最前面，和存档文件里的编号不一致。最好还是用等级和周目来标记。
> 角色等级抓一下。
> （词条范围）那就做两个开关，或者一个多选一的框，默认当前绘卷周目下能出现的词条和恩宠，然后可以切换成剩下两种。
> （主副词条表格方案）合理，做出来看看效果
> （收藏已有绘卷）没错，改，还应该支持装备和道具，虽然不确定有什么用但肯定有人会想这么干，比如收藏某个道具，经常去添加数量。
> （收藏夹页面）这就是收藏夹section的功能了，说实话暂时不明白，暂时提供筛选效果吧，方便定位，因为绘卷动不动几百张，只靠保护很难说定位到常用的个别几个。
> （备份）删掉绘卷编辑页的存档相关内容合理，但是不可恢复那不就扯蛋了吗？说白了就是备份没做明白呗，替换存档还能失败了？就是literally替换文件。
> （UI-021）塞不下的问题你自己想办法解决，我不认为应该改成ALL，你这地方前端代码没写明白不应该用硬编码的思路绕过去。

Earlier rounds, still binding: the owner rejected UI-009 (no separate small-size icon) and the About
intro text; the mark is the owner's flat illustration with the restored toenail band. Talk to the owner
in Simplified Chinese; do not push, tag or publish a release without the owner's explicit authorization
(pushing this preview branch for preview EXEs is authorized).
