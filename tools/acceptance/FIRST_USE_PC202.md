# First-use outfit search on PC 2.02

The cross-weapon candidate also registers katana mid quick/strong/Sword Ki,
odachi mid quick/strong and dual swords mid quick. The ordinary reader uses
a role-scoped read map and the selected action's own parameter/row list.
REA/Ghidra and the original instructions at 2E98F0 and 2E9948 confirm the
row-list address, start and count. Test the same role with multiple choices
while the player remains idle. The host export accepts `--scene ID`.

Build the three canonical scorer groups, then stage the six Samurai choices
with `tools/materialize_loadout_scenes.py` beside the sealed kusarigama
bundle. Pass the resulting scene directory to `tools/build_tauri.ps1
-LoadoutRuntime`. The final host check selects each requested scene through
the normal picker. A partial player receipt verifies only its named scene.
Run `tools/check_models.ps1 -Model FixedContactRanking` to check candidate
ownership, contact order and cancellation in the batched rank merge.

For Water Tsubame Gaeshi, verify the action-only score over three contacts.
With the recorded neutral katana the direct hit gives 241 physical and 425
water. One spawned contact gives zero; the other gives 847 water. Verify the
whole move and each channel when changing an imbued candidate to an unimbued
one. The independent constructor control is the original PC202 function at
2B4430 with its caller's eight-byte argument offset.

The first supported fresh profile is a Ninja holding a kusarigama in slot one, using the first direct strong hit against the fixed training warrior. The app reads the current role, verifies its worn records against the selected save and builds an in-memory context. It requires no imported character directory, source epoch file, debugger or live attack. Existing verified epochs remain a separate acquisition route.

The ordinary reader reconstructs elixir and regional ranks, clan/title effects, three area layers, configured passive skills and slotted skills in native order. It reads the two guardian definitions, eight core records, unlock curve and the native post-equipment selectors. The active core and selected SOURCE maps are compared in order, including zero values and duplicate contributions. Scoring uses the declared progress-rank branch. A transient skill descriptor from a different native rebuild is not observed by an ordinary read. The UI reports the conditional model scope.

The selected live STAT and D buffers supply the prestate for the constant stores of 37574C/8731E4. For normal owner kind zero the preceding selected reset also clears STAT+7D0. Untouched bytes remain current. Row 90 supplies the independent health seed; the PC202 parameter AEAC supplies the plus cap and the two DATA unarmed fields at +2461B4 supply the table-70 unarmed input. The direct packet uses the prepared weapon record, including its +8 attack addition. Before publishing a handle, the scoring engine must reproduce both active SOURCE maps, primary/secondary attack, health and defense.

## Reference encounter

`combat_context/kusarigama_strong_pc202.json` is a compiled-in description of virtual operands. Live actual addresses are resolved from the verified game's module and the snapshot's actor, owner, SOURCE, STAT and D roots. Heap paths have no user-provided base address. Virtual pointers preserve object binding and module-relative function identity. The published fields contain no source character scalar payload, complete process pages, save or entry-register dump. Fixed scalar payloads belong to the reference NPC and action. The virtual stack starts with RCX/RSP and the standard MXCSR; the initial thread epoch is obtained from the initialized process-global guard read at 624535. The CRT epoch is used only for the reference thread's already-initialized branch.

The curve vectors (manager+718/+720) and scalar hash (+230) are materialized from complete current tables. Pointer indices are rebased explicitly. Every initialized image byte has a known bit; inaccessible inputs and unsupported execution paths cannot become default zero damage. This recipe covers a declared one-hit scenario, not arbitrary target conditions, every skill, other builds or active temporary status objects. With no scoreable encounter the worker returns `sources_read` with the actual source checks and no searchable handle.

## Controls

Run `tools/check_models.ps1 -Model FirstUseLoadout` for current-request ownership and the separation between source readiness and damage readiness. The Chromium product control also refreshes a ready view to a sources-only view and checks that its ranking and handle retire.

On the originating host, set `NIOH3_FIRST_USE_CASE` to a prepared case directory and run the protected library test `first_use_sources_from_ordinary_memory -- --ignored`. A case specifies the installed data root, the ordinary-memory replay, the independent ordered compiler records and the original numeric reset hashes. The positive retained control explicitly supplies two synthetic zero unarmed inputs that older recordings omitted. A separate case leaves those bytes missing and checks that no damage handle is issued. The normal application always reads the unarmed fields from the current game.

The headless local verifier is built with `cargo build --release --locked --manifest-path crates/nioh3-protected/Cargo.toml --features first-use-acceptance --example first_use_export`. It takes DATA_ROOT, an encrypted save copy, and a new private output directory. With `--fixture MEMORY` it records a controlled replay origin. Without that switch it reads only the verified PC202 game. Its private, restorable input evidence stays on the host. The production worker does not include this exporter.

The originating Windows evidence directory is `F:/Nioh3_r16_build/deliverables/optimizer-first-use-20261119`. The original-input check used observed REA/Ghidra reset and packet routines, original-instruction replay, a second process's observed attack, and the current complete equipment domains. Keep public package, fresh player search, retained-host numerical proof and live gameplay observations scoped to their own artifact identities.
