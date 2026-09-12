# Independent live-add executor — September 9, 2026

## Final acceptance checkpoint

All nine NG1-NG3 scroll-payload x requested R3/R4/R5 cells were added through
the independent Windows executor. Here NG1-NG3 describes each generated
scroll's category, not the progression currently selected in the running game.
The runtime progression was not varied or recorded as a test dimension. The
user confirmed normal save/reload and all added scrolls.
Read-only save-copy verification matched all 39 records to their pre-reload
inventory bytes. Loaded inventory kept all 39 serials, generated fields and
native index entries. Only the new-item marker cleared, and the three requested
R5 records changed 5/5 headers to 4/4 when loaded. The saved file still contains
5/5. The user reports the R5 icon remains and explicitly asks not to investigate
or work around this version behavior further. Do not claim persistent orange R5.
Evidence: `deliverables/frontend-v2/native-live-add-matrix-20260909-r5/reload/verification.json`.

Latest package: `deliverables/frontend-v2/portable-v2-native-matrix-20260909/`.
Both workers and TypeScript built; 152-file integrity manifest SHA256
`f880907f7e8a174bfa772e36b8eb47e7b4fc3d3f0f6fd5a25abd4afc7dbc6645`.
Four protected-worker IPC tests passed against these exact packaged binaries.
No release or publication. Full React click-through live insertion is still
distinct from the accepted product application/native execution path.
The dated progress/next-step notes below are retained as execution history;
this checkpoint supersedes their pending matrix persistence statements.

## Current implementation and acceptance

The Windows executor is implemented without loading or starting Cheat Engine.
It is now the default in the protected host after one verified real insertion.
`NIOH3_LIVE_ADD_EXECUTOR=ce` retains the explicit research fallback.
Normal-save/reload and packaged frontend acceptance remain separate gates.

The application now separates `LiveAddAdapter` inspection/evidence from its CE
and native transports. The native executor follows the same accepted mission
dispatch boundary using owned DR0/DR1 execution breakpoints and Windows debug
events. It does not patch the original entry instructions or create an arbitrary
remote thread to invoke inventory functions.

`windows_debug_session.py` models Windows SDK x64 CONTEXT and DEBUG_EVENT layouts.
Contexts are explicitly 16-byte aligned. Active hardware breakpoints cause a
conflict rejection. Original debug registers are restored at a stopped event.
`DebugSetProcessKillOnExit(False)` prevents debugger exit from intentionally
terminating the game. Fatal/unacknowledged ownership must retain its debugger
thread and remote allocation until cleanup can be established; no blind retry.

`live_add_dispatch_code.py` emits the accepted builder/insertion shim. The full
357-byte insertion fixture matches the historical CE emitter byte for byte.
`live_add_native_transport.py` persists an operation claim before redirecting
RIP, preserves the original native prologue, verifies acknowledgement and
canaries, and retains a per-operation receipt. It exposes only fixed internal
transport methods, never raw addresses or arbitrary code to the renderer.

## Evidence

- Synthetic executable: `research/native_dispatch_fixture.cpp`, compiled locally
  with the Windows SDK and MSVC. It contains no game code. Real attach/redirect,
  rejection for a wrong caller, cleanup, and another successful attach passed.
- Game PC v2.01, PID 26572: independent no-call dispatch on thread 9264 passed.
  `deliverables/frontend-v2/native-live-noop-20260909/verification.json` records
  exact context/prologue verification, one redirect, allocation release, removed
  breakpoints, and an unchanged entire inventory and serial counter.
- Initial insertion preflight was correctly blocked by changed native code at
  RVA `0x2FA624` (the item quantity getter). The running FLiNG trainer must release
  that modification. The user closed it; code identity then passed without bypass.
- Independent native insertion succeeded for R3 seed 10030566, serial 2418728,
  slot 16, operation 00854624-25b4-4665-a19e-f40c42e257e1. Inventory increased
  from 30 to 31; all previous records and the full native index were verified.
  Cleanup passed and the source save was unchanged. Evidence:
  `deliverables/frontend-v2/native-live-add-r3-20260909-r5/verification.json`.
  This operation must never be replayed. Normal save/reload is pending.
- Existing insertion/backup/evidence tests plus new native ABI/emitter tests pass.
- Current focused regression: 52 Python tests, two frontend live-add recovery
  tests and four packaged protected-worker IPC/cleanup tests passed.
- Fresh portable build: `deliverables/frontend-v2/portable-v2-native-live-add-20260909/`.
  TypeScript and both standalone workers built successfully; the manifest covers
  152 files (SHA256 bc1bd5741d263beb744fe005630cc8424172c257029b82952e2f3494a5ba222d).
  PyInstaller analysis includes the native adapter, transport and debug session.
  This is an unsigned local test package, not a published release or full UI
  live-write acceptance.

## Automatic backup invariant

Every actual addition, including each batch child, requires a newly created
backup through `LiveAddApplication.prepare`. The backup is flushed/fsynced and
read back for exact equality, then decrypted and matched to the live inventory.
Immediately before the durable insertion claim, the saved backup is rechecked
against the source-save hash. Missing/corrupt backups prevent dispatch. Live
addition never substitutes an offline save write for normal in-game saving.

## Next acceptance steps

1. Verify user UI, normal shrine save and reload of seed 10030566.
2. Expand to R4/R5 and selected batch subsets.
3. Package and test
   the complete React-to-protected-host route, including unavailable/conflicting
   debugger handling. Keep the optional CE adapter as a fallback/research tool.

## Full NG1-NG3 scroll-payload / R3-R5 installation matrix

The user confirmed the first R3 appeared, normally saved and closed the shrine
menus, then requested all remaining rarity/early-playthrough combinations.
All eight additional cells passed real native insertion, independently verified
full-container/index changes and cleanup. Inventory went from 31 to 39.
Evidence: `deliverables/frontend-v2/native-live-add-matrix-20260909-r5/verification.json`.
Each operation has its own flushed, decrypted and verified automatic backup.

| Playthrough | Rarity | Seed | Instance serial |
|---|---|---|---|
| 1 | 3 | 10031013 | 2419496 |
| 1 | 4 | 10031014 | 2419497 |
| 1 | 5 | 10031015 | 2419498 |
| 2 | 3 | 10031023 | 2419499 |
| 2 | 4 | 10031024 | 2419500 |
| 2 | 5 | 10031025 | 2419501 |
| 3 | 4 | 10031034 | 2419502 |
| 3 | 5 | 10031035 | 2419503 |

Together with seed 10030566, all nine payload cells have insertion evidence. It
does not establish that live insertion works while the running character is
actually playing NG1 or NG2; that separate runtime-context matrix is pending.
Normal
save/reload of the full matrix is pending. This is explicit requested-rarity
installation, not evidence of natural early-playthrough R4/R5 drops or reveal.

Three integration gaps were found and fixed rather than weakening readiness:

1. Early R3 native candidates were still stage-one and blocked by installation.
   The existing native completion loop now runs before search filtering for
   NG1/NG2 R3, as it does for R4. Final preview and original installation source
   stay separate. A regression verifies filtering against completed effects.
2. Explicit-playthrough search construction allocated a global instance serial
   for a scratch result. Set compact descriptor +0x21 to one before assembly.
   Before the fix, one bounded NG1 R3 probe consumed one serial without changing
   any inventory byte/acquisition counter. Never roll back a global counter.
   After the fix, all eight isolated generations left records and both counters
   unchanged; per-cell snapshots are retained in the matrix evidence.
3. The assembly allowlist now includes authentic NG1 0x1E82 and NG2 0x516D
   types alongside NG3 0xE604. Every output still receives exact preview checking.
   PC v2.01 assembly caps R5 headers to 4/4. The native shim now applies the same
   narrow 4/4-to-5/5 preservation as NativeBatchOracle, in its owned scratch
   output before insertion, after checking the returned pointer. Unexpected
   headers are rejected. This changes no effect-generation or RNG semantics.

Two successful matrix insertions preceded the R5 preflight rejection. The
resume driver skipped only their durable verified receipts; no insertion was
replayed. Remaining six insertions then passed. All preview rejection receipts
were released. The latest focused regression passed 72 tests, and the broader
native/editor regression passed 136 tests before the final matrix changes.

## Builder metadata boundary and transient scheduling

An offline installation template carries existing inventory flags/acquisition
order. These are not assembly descriptor inputs. The real v2.01 builder returns
flags 0x02800002 and acquisition order zero; insertion applies destination flags
and the current acquisition counter. `new_assembly_record` normalizes only those
two words before preview. Every generation byte remains strictly compared,
including seed, effects, level, rarity and transfers. Do not broadly ignore
metadata mismatches or modify the generation/RNG/finalizer to address this.
Both preview and insertion acknowledgement verify the source against the
reviewed record, including the expected unallocated/allocated serial.

An idle character does not imply scheduler flags stay constant between frames.
Read-only sampling observed transient pending flags while the character stood
by the shrine. The research driver retries only the exact preflight idle error,
before any operation is prepared. Actual dispatch still requires the accepted
idle flags at the stopped entry event. Never replay an uncertain insertion.

## Temporary override acceptance completed separately

Seed 10030565 R3 had two inventory instances (slots 14/15). The existing
RuntimeApplication temporarily replaced its three rules with none and recorded
five hits. After stop and menu refresh, Computer Use observed the original
foot-armor adversity 65%, Kuzuryu grace preference 30%, and automatic Rejuvenation
Talisman 60 seconds. The entire 30-record inventory was byte-identical and
runtime cleanup was confirmed. Evidence is in
`deliverables/frontend-v2/temporary-override-acceptance-20260909/`.
This validates the application/runtime path and game display, not a new end-to-end
React editor trial or natural generation of the user's historical samples.
