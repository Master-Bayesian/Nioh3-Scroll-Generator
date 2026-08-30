# PC v2.00.02 knowledge index

Target executable: Nioh 3 PC v2.00.02. The strongest verified product context
is the third-playthrough record type `0xE604`, rarities 3 and 4, and the normal
scroll UI path (`caller_option = 0`).

## Capability matrix

| Subsystem | Current status | Evidence boundary |
| --- | --- | --- |
| R3 final effects | `native-byte-parity` (stable bytes) | 10,000 deterministic third-playthrough Seeds; aggregate report, runtime header byte `+0x1B` excluded. |
| R4 stage one and finalizer | `native-byte-parity` | 10,000-Seed aggregate live report plus 10 native stage/final pairs (9 unique Seeds). |
| R5 effects | `native-byte-parity`; product hidden | Technically verified by a 10,000-Seed aggregate live report, but intentionally absent from the current UI. |
| Mode, terrain, enemies, rules | `native-control-flow` | All three branch classes, 22 native vectors; strongest on third playthrough and `caller_option = 0`. |
| Enemy role table | `native-table` + `native-control-flow` | 487 native candidate rows; two independent process captures share the same table hash. |
| Recommended-level curve | `native-table` + `native-control-flow` | Exact internal-to-display conversion. The final link to combatant AI level is still unproven. |
| Scroll item level | `native-control-flow` | Drives effect value normalization and outbound effective level; no proven challenge-level consumer. |
| Exchange payload | `native-control-flow` | Compact canonical tuple recovered; effect slots are absent from the message. |
| Save transactions | executable specification | v2.00.02 `RNNUSR` layout, checksum/encryption roundtrip, source-hash gate. |
| Playthroughs 4 and 5 | latent research only | Record types exist, but the content is not released and eventual DLC behavior is unknown. |

## Documents

- [中文版玩家敌人组合指南](catalogs/enemy-combinations.zh-CN.md)
- [Player enemy-combination guide](catalogs/enemy-combinations.md)
- [Enemy roles and class constraints](enemy-generation.md)
- [Scroll level and recommended level](levels.md)
- [词条、恩宠与 Seed 求解（中文版）](effect-and-seed-solving.zh-CN.md)
- [Effects, Grace, and Seed solving](effect-and-seed-solving.md)
- [Save record and propagation protocol](save-and-propagation.md)
- [Versioned catalogs](catalogs/README.md)
- [Evidence and supersession register](evidence-register.md)

## Current record types

| Playthrough | Record type | Product status |
| ---: | ---: | --- |
| 1 | `0x1E82` | Existing native type; not covered by the strongest third-playthrough parity corpus. |
| 2 | `0x516D` | Existing native type; not covered by the strongest third-playthrough parity corpus. |
| 3 | `0xE604` | Certified product context. |
| 4 | `0xDD82` | Latent research context; not released in v2.00.02. |
| 5 | `0xD523` | Latent research context; not released in v2.00.02. |
