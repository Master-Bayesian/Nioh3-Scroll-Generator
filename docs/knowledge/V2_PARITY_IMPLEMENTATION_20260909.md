# V2 parity implementation and acceptance boundary — September 9, 2026

This report supersedes the open implementation-gap table in
`LEGACY_UI_PARITY_20260909.md`. No release, commit, or publication was performed.
The verified numerical generation and finalization paths remain in place.

## Implemented

| Area | Current implementation |
| --- | --- |
| Editor layout | The editor host spans both workspace columns. Inventory, editor and preview resize together; save labels/selects shrink within their panel. |
| Window chrome | Custom minimize, maximize/restore and close controls, draggable title area, original application PNG icon. Closing still uses protected-worker cleanup. |
| Selected conditions | Collapsed and expanded content have identical padding, borders and scrollbar geometry; hovering does not move chips. |
| History | Latest three previous search batches, with retained candidate references. History candidates can enter the cart after their worker page is replaced. History is session-local. |
| Preview management | Remove current, remove selected, and clear previews. These actions do not delete inventory records. |
| Inventory deletion | Multiple occupied slots can be selected, reviewed and deleted together. Slot indices, not seeds, identify inventory entries. |
| Backup administration | Open save folder, open/change/reset data directory. Root changes take effect after restart; existing operations keep their original root and old data is retained. |
| NG1/NG2 acceleration | Reuse existing measured draw/primary maps and inverse traversal; scope caches to save/context. Continuation carries the native trial cursor, including bounded no-match pages. |
| Temporary editing | Auxiliary preview for draft seed and NG1–NG5; native enemy-slot restrictions follow the previewed seed. Independent enemy/terrain/rule switches, enemy group count within original capacity, undo/redo, apply/stop, state and hit inspection. |
| Research caches | NG4/NG5 R5 capture and existing-cache binding are exposed. Existing research/install restrictions are retained. |
| Updates | Stable/beta signed V2 ZIP manifest, hash/size checks, bounded extraction, whole-package verification and safe-exit replacement with previous-directory preservation and immediate failure rollback. Legacy EXE assets are rejected. |
| Diagnostics | Five rotating 4 MiB JSONL logs, bounded entries and copied tail; search and protected-worker stderr are retained. Request payloads are not recorded. |

Notable integration bugs fixed during acceptance: uppercase template SHA-256
was rejected by the IPC contract; completed private exports could invalidate an
unconditionally recovered old job; history preparation briefly exposed stale
search completion state. These paths now have runtime or connected-UI evidence.

## Evidence

Evidence files are under `deliverables/frontend-v2/search-ui-demo-v2/`.

- 45 Python tests: review integration, cart batch, protected operations and search
  contracts, including data-root restart boundaries and auxiliary previews.
- All 31 desktop tests passed together, including real Python IPC, protected
  ownership, rolling logs, signed update validation and package integrity.
- `connected-verification.json`: 17 checks against an isolated encrypted
  synthetic save, including exact selected additions/deletions and restore.
- `history-verification.json`: five distinct real search batches, exactly three
  retained previous batches, expected eviction, and cart retrieval.
- `window-editor-verification.json`: editor widths 1366/1600/2560, stable hover
  geometry, custom controls and temporary-editor interaction checks.
- `packaged-review-verification.json`: independent portable startup defaults to
  the connected UI, custom controls, 25 actual results, NG2 auxiliary preview.
  Final artifact: `deliverables/frontend-v2/portable-v2-ui-parity-verified/`;
  complete integrity verification covers 152 files. This is an unsigned local
  acceptance build, not a published release.
- `../packaged-parity.json`: source/standalone R3/R4/R5 DTO and identity parity,
  without enabling CPU fallback.
- `update-helper-verification.json`: isolated whole-directory replacement and
  traversal rejection; the helper used a locally compiled synthetic executable.
- `native-ui-verification.json`: actual game v2.01 at title, NG1/NG2 known-seed
  generation, NG2 search and continuation, no remaining runtime ownership.
- `native-cache-verification.json`: actual NG1 primary-map capture (~22 seconds
  initially), second-call cache reuse, distinct continuation and safe cleanup.
- `native-title-verification.json`: NG1 R3, NG2 R3 and NG2 R4 generation;
  source save hash unchanged before/after. Existing experimental inventory is
  not evidence of natural early-playthrough R4 drops.

No real inventory insertion, save mutation, deletion or restoration occurred in
this iteration. Native title-screen generation does execute the existing native
oracle; it is not a claim of purely passive process-memory observation.

## Remaining acceptance and existing capability limits

Implementation parity does not imply release acceptance. Real-game temporary
override activation/stop still requires a selected scroll detail and a menu
refresh. The user was asked to provide the selected scroll ID; no challenge is
needed. Do not leave an armed override behind after a test.

The new UI still needs real selected-cart addition and edit/restore/reload tests
on an explicitly chosen test scroll and save. The live-add executor remains the
existing configured CE adapter; this work did not make it CE-free.

Early R3 native candidates without final-record certification remain blocked
from installation; raw values are not percentile scores. NG4/NG5 remain research
contexts. Challenge capacity is seed-derived and has no supported temporary
override. Full English/Japanese UI activation remains separate work; old Tk did
not provide a fully translated equivalent screen.

The V2 signing and update-feed tools exist, but no signed V2 manifest was
published. Public download-to-install acceptance remains pending. Do not describe
the isolated helper smoke as successful delivery of a public update.

## Preliminary user acceptance order

1. Close the old app normally; launch the complete verified portable directory.
   Check normal/maximized editor, save selector, navigation and hover expansion.
2. With the game closed, exercise NG3 R3/R4/R5, 1 and 25 results, next batch,
   grouping/value filters, history eviction, cart selection and preview removal.
3. Confirm the detected account and slot. Read inventory and backup lists first.
   At title, add one selected cart scroll, then two of three selected cart items;
   verify exact count/seed changes after loading and after another normal save.
4. On one disposable test scroll, edit a legal effect value, undo/redo, review
   and commit at title. Reload to verify persistence. Return to title and restore
   the pre-test backup; verify the original inventory. Then test multi-delete
   only on the disposable records, with another backup/readback check.
5. Test NG1/NG2 native search at title, with an authentic matching template:
   simple filter first, cache reuse, cancel and next batch. Observe installation
   capability restrictions rather than bypassing them.
6. In a safe area, test live add with one selected item and then a selected subset;
   normal shrine save and title reload must preserve exactly those seeds.
7. Select a test scroll detail. Apply one temporary change, switch to another
   scroll and back, inspect a positive hit count, stop, and refresh the detail
   again to confirm restoration. No dungeon clear is required.
8. Before publication, repeat critical checks on the final signed/distributed
   package, including unavailable game/GPU handling and a staged signed V2 update.
   Stop after uncertain writes and inspect receipts before retrying.
