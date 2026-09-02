# PC v2.01 evidence register

Raw runtime sections and process-specific captures are private audit artifacts
and are not distributable release assets.

| Evidence | Result | Boundary |
| --- | --- | --- |
| `audit/runtime_sections/v2.0.1.0_20260902_title/manifest.json` | Captured | Read-only live `.text`, `.rdata`, and `.pdata`; exact executable identity. |
| `outputs/version-migrations/pc-v2.01-20260902/relocation-report-v5.json` | All sites resolved | Anchor relocation only; one LCG signature contains a relocated RIP displacement. |
| `outputs/version-migrations/pc-v2.01-20260902/xrefs-one-over-1000-new.json` | 1086 references at the selected target | Disambiguates the three identical `0.001` constants. |
| `outputs/version-migrations/pc-v2.01-20260902/xrefs-float-new.json` | Reference counts align with baseline | Disambiguates repeated float constants. |
| `outputs/version-migrations/pc-v2.01-20260902/disasm-lcg-new.json` | Normalized structure equal | Static control-flow evidence, not native output parity. |
| `outputs/version-migrations/pc-v2.01-20260902/runtime-tables-ng3-v1/manifest.json` | Complete | Read-only runtime capture; all profiled live signatures passed. |
| `outputs/version-migrations/pc-v2.01-20260902/resource-comparison-ng3-v2.json` | Static resources and active NG3 context equal | Does not prove native control flow or save compatibility. |
| `outputs/version-migrations/pc-v2.01-20260902/native-parity-r3-10000.json` | 10,000 / 10,000 full records equal | Isolated remote buffers; no save access. |
| `outputs/version-migrations/pc-v2.01-20260902/native-parity-r4-10000.json` | 10,000 / 10,000 stage and final records equal | Includes exact accepted finalizer slot. |
| `outputs/version-migrations/pc-v2.01-20260902/native-parity-r5-10000.json` | 10,000 / 10,000 effect slots equal; only the documented rarity-header cap differs | Feature flag 9 was unavailable in this process. |
| `outputs/version-migrations/pc-v2.01-20260902/native-auxiliary-parity.json` | Repeats stable; shared control Seeds exactly equal to v2.00.02 | Temporary private remote buffers only. |
| Current encrypted `SAVEDATA00` read-only parse | 9,437,616 bytes; 25 visible records; 375 zero slots; next physical slot 47 | Decrypted only inside a temporary directory; no save write. |

## Safety

- Section and table capture used process read access only.
- No save was read or written during runtime table capture.
- Native parity used private process allocations and restored all handles; it
  did not install inventory records or write the save.
