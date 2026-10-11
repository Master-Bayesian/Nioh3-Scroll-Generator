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

## Deliverable at the end

A full patch note of every change on this branch, for the owner.
