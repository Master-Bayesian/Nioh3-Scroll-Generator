# Tauri v0.8.7 publication (2026-10-07)

Published by the owner's authorization ("默认浅色吧。然后可以打包发新版了").

- Product commit `49ce3e9`, tag `v0.8.7`.
- Release preparation run `37676443904` (success). The publish script verified
  37 checks over the six assets, promoted them and re-verified them from the
  public downloads. `releases/latest/download/tauri-update.json` serves 0.8.7.
- Tests run `37676443401` passed every job (rust-crates, rust-packaging,
  rust-python-parity, windows-tests) before publication.
- The first preparation commit `effa440` built (run `37669803891`) but its
  Tests run failed rust-crates: CI's Rust 1.99 clippy rejects `chunks_exact`
  with a constant size, which the local 1.91 toolchain did not flag. Fixed in
  `49ce3e9`; that build was never published.
- Plans and verify reports: `D:/Nioh3_v080_deliverables/deliverables/v0.8.7-publish-49ce3e9/`.

| Asset | Bytes | SHA-256 |
|---|---:|---|
| Nioh3Studio-0.8.7-win-x64.exe | 12254467 | `03496aa5e155c8e70c20db288e64c6b85641ce76ebc1ae163acecc87c0cc155a` |
| Nioh3Studio-0.8.7-win-x64.zip | 11551947 | `2547f9f6c10acf2b8d365c76e5cb4f92d4e274f97e8f75f3bad7f99546515843` |
| Nioh3Studio-0.8.7-win-x64.exe.sha256 | 97 | `71109c7b97a2b73b180389299b3d9abe040d6d68d09f4ec421c19f4aaba4ee1b` |
| Nioh3Studio-0.8.7-win-x64.sha256 | 97 | `a1d8c86cf1966dc807aabb76ab9d85de2c273bbf00ae6230ec9307677ba3afb9` |
| tauri-update.json | 3245 | `55cb5569c32f72a49b793919c910a1d51b1b3b73f198cd5bfaaa31d3be3808f9` |
| test-inventory.json | 88859 | `9551569b5759f5d5baa3977096807c1887886a18ce9b42944b2ae69fce539d6d` |

## Acceptance before publication (PC v2.02, owner present)

On local builds of the same product changes; see the
[engineering record](../product/releases/v0.8.7.md): acquisition keys
65533..65536 added live, saved at a shrine, stored as a full u32 and read back
after a reload; an equipment addition confirmed after a simulated pickup took
the next free slot with the current counters and verified; the in-game count
`1558/2000` matched memory; the packaged reset archived the state directories
and the tool kept working.
