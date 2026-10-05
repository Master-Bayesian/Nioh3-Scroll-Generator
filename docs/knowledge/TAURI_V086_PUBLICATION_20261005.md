# Tauri v0.8.6 publication (2026-10-05)

Published by the owner's authorization ("你觉得没问题就发").

- Product commit `ddfbe27c482aeed0974b65911666f274f7378c23`, tag `v0.8.6`.
- Release preparation run `37263303720` (success). The publish script verified
  37 checks over the six assets, promoted them and re-verified them from the
  public downloads. `releases/latest/download/tauri-update.json` serves 0.8.6.
- Two earlier preparation runs failed before signing and published nothing:
  `37256356468` (a new message lacked its en/ja translation) and `37256670483`
  (the release UI check still looked for the old section name and did not
  switch the scroll editor to save-file mode). The Tests run on that commit
  also failed clippy `type_complexity` and rustfmt; all fixed in `ddfbe27`.
- Frontend V2 foundation passed on `ddfbe27`; Tests' rust-crates,
  rust-packaging and windows-tests jobs passed before publication, and the
  Rust/Python parity job was still running.
- Plans and verify reports: `D:/Nioh3_v080_deliverables/deliverables/v0.8.6-publish/`.

| Asset | Bytes | SHA-256 |
|---|---:|---|
| Nioh3Studio-0.8.6-win-x64.exe | 12231183 | `76ba087d5903824bcaa4796327e550dc0d78196bfc8793f072413be9c5226fd9` |
| Nioh3Studio-0.8.6-win-x64.zip | 11530199 | `f29d183dd9dcd00337cdee9cf858432bc26cf63324dc1e29ef71457b64f1f8ab` |
| Nioh3Studio-0.8.6-win-x64.exe.sha256 | 97 | `36bd0416be980bc5cde75295f7b35a7457106169f9bb6359d6fab99d367d1f49` |
| Nioh3Studio-0.8.6-win-x64.sha256 | 97 | `39c6fa82dceab744831ee6308b43b72838adf72d53859ff24d38956dfdced8a6` |
| tauri-update.json | 3728 | `915bce7f694de3c2862f7764d7614a4b1a019ba80bca816b44ac241621db73c9` |
| test-inventory.json | 88859 | `39904a9132af41982d6283687ab1b67b013712ea0b67c539a88ba39fc409a859` |

## Acceptance before publication (PC v2.02, owner present)

Live acceptance was on local builds of the same product changes; see the
[engineering record](../product/releases/v0.8.6.md): live scroll edits (persisted
after a shrine save, two edits before a save, a revealed scroll seen changing in
the game menu), live NG1/NG3 scroll additions with no crash, and the earlier
gold, item, equipment and count edits.
