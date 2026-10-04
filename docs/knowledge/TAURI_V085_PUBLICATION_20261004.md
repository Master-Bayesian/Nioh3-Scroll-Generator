# Tauri v0.8.5 publication (2026-10-04)

Published by the owner's authorization ("test first, then publish").

- Product commit `9e81a2971e546dae61b3fffc0b8c425cf3488dab`, annotated tag `v0.8.5`.
- Release preparation run `37185958576` (success). The publish script verified
  37 checks over the six assets, promoted them and re-verified them from the
  public downloads. `releases/latest/download/tauri-update.json` serves 0.8.5.
- The Frontend V2 foundation workflow passed on this commit for the first time
  since v0.8.3.
- Plans and verify reports: `D:/Nioh3_v080_deliverables/deliverables/claude-v085-publish/`.

| Asset | Bytes | SHA-256 |
|---|---:|---|
| Nioh3Studio-0.8.5-win-x64.exe | 12145942 | `31890bca0a3d2f07a51667748ed57c152d6855918c649dc66cbd1963a2701207` |
| Nioh3Studio-0.8.5-win-x64.zip | 11444958 | `909c4cb5c9e00e8c22f2bdf269df0f0e704524a06b4fa70a4132695b23f5ed13` |
| Nioh3Studio-0.8.5-win-x64.exe.sha256 | 97 | `0cf3285460bf7d49cb1fd297c9ad11aafe289b3c79868f33b09a52ce8b779fb6` |
| Nioh3Studio-0.8.5-win-x64.sha256 | 97 | `4eddc890bc2f16d76ea1a98eadfd155e975910fbc768d2597a6af8c6b807e73f` |
| tauri-update.json | 2516 | `3c4b44405da2eadb2ab2285ec9c48bbca474e1653d97bc94a42c2b51f98f885f` |
| test-inventory.json | 88859 | `a687ddcd625224b6295a07513a64359638375f565a84f0fa7e83c9ceec20dc75` |

## Acceptance on the published candidate's bytes (PC v2.02, owner present)

- Live remaining count: scroll 170827512 6 -> 5 -> 6. Both runs were verified
  with an automatic backup, and the second plan read 5 back from the game.
- Live equipment addition: item 14145 seed 21705 previewed with the predicted
  effects and was verified at slot 1551.
- Live gold +1 and revert; the closed-menu `runtime.menu_selection` reply was
  accepted.
- Save-file edit after an external save write: the save was changed by a
  separate worker (gold +1), then Reload and a gold edit in the UI planned
  and committed without "Save changed since it was read".
- New soul-core names are listed and searchable (大蛤蟆的魂核, 蛤蟆附身的魂核).

On v0.8.4 bytes in the same session, live scroll addition (single and cart)
and the temporary override (#1) were accepted in game.
