# Tauri v0.8.4 publication (2026-10-04)

Emergency fix release for v0.8.3. Published by the owner's authorization.

- Product commit `5bce8e9de0ed5dc0f888885b245a8ab644f0748d`, annotated tag `v0.8.4`.
- Release preparation run `37183134882` (success); the publish script verified
  37 checks over the six assets, promoted them and re-verified them from the
  public downloads. `releases/latest/download/tauri-update.json` serves 0.8.4.
- Plans and verify reports: `D:/Nioh3_v080_deliverables/deliverables/claude-v084-publish/`.

| Asset | Bytes | SHA-256 |
|---|---:|---|
| Nioh3Studio-0.8.4-win-x64.exe | 12144553 | `f7bdd206a2244f7c0106ce5d955de2a426cd66917a7b707d1ba56befa2a7a805` |
| Nioh3Studio-0.8.4-win-x64.zip | 11443569 | `c7a21bbe546856468afd7614970439057646eb4fc7048c8d5e1ad90d8e9f22ce` |
| Nioh3Studio-0.8.4-win-x64.exe.sha256 | 97 | `8b573f45f59094106f739f2f788c9537ae874604f932d2ce649de09e8c2a5290` |
| Nioh3Studio-0.8.4-win-x64.sha256 | 97 | `b2794bc1ea2bfb01dffdec5f2f666804fd9f1455828eaf79083c4d72c8166fa7` |
| tauri-update.json | 3829 | `ed27c9b5d482c829438e53eafafbcbaf6121180c8e943ff37942b47a201c3232` |
| test-inventory.json | 88859 | `09fb911883cbf9eaf4823bd09a9a4a6ae7502a9f1e342c0d9737a50fd95f7bfd` |

## Live acceptance (PC v2.02, owner present, test saves)

All of the following ran on the published candidate's exact bytes, driven
through the packaged UI's `window.operations` bridge:

- `runtime.menu_selection`: open and closed menu replies accepted (the v0.8.3
  INVALID_RESULT regression).
- `runtime.character_edit`: gold +1 and equipment familiarity -1 were written,
  re-read and reverted.
- `runtime.equipment_add_*`: item 14145 seed 8774 previewed with exactly the
  seed-search effects and was verified at slot 1551. An earlier build with the
  same fix inserted seed 48129 at slot 1549; the owner saw it in game and saved,
  and the decrypted `SAVEDATA00` holds that record (serial `0x26C4C8`).

Known CI gap: the "Frontend V2 foundation" workflow fails on
`verify-scroll-completion.mjs`, which reads a host-local Codex evidence file
(`deliverables/codex-v083-missing-features-20260930/...`) that does not exist on
the runner. This predates v0.8.4 and does not cover any v0.8.4 change.
