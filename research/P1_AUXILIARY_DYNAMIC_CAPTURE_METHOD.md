# P1 Auxiliary Scroll Dynamic Capture Method

## Scope

This capture identifies the compact runtime descriptor consumed by the scroll-detail UI for:

- ordered enemy composition;
- ordered special-rule keys and row data;
- terrain enum, terrain lookup key, and terrain row data.

It is observational. Cheat Engine temporarily installs software execution breakpoints in the running process, but the script does not write inventory fields, generated records, or save data. It intentionally does not claim that a UI consumer is the corresponding generator.

## Static basis

The receive-side canonical reconstruction path is:

```text
0x21F0280
  -> 0x21EF598
  -> 0x577964  effect generation
  -> 0x2277FE8  canonical 0xE8 record assembly
```

Static disassembly shows that this path reconstructs effect and record fields but does not materialize the enemy/rule/terrain UI outputs. Those outputs are consumed later through a separate descriptor path.

The scroll-detail UI builder at `0x1F0B3D4` provides a verified consumer boundary:

```text
0x1F0B62B  calls 0x3C1B84 to resolve/copy descriptor ID 0x1B2
0x1F0B630  compact descriptor is ready at RBP+0x270

0x1F0D475  reads a u16 rule key from RBP+0x288+index*2
0x1F0D47D  resolves it through parameter manager +0xAA0

0x1F0DFD1  reads the terrain enum from RBP+0x28F
0x1F0DFE2  resolves terrain context through parameter manager +0xA88
0x1F0E0A2 and sibling branches resolve display rows through manager +0x7A0
```

These are consumer facts only. The persistent source object and its writer remain to be identified dynamically.

## Capture procedure

1. Start Nioh 3 v2.00.02 offline and load the three-playthrough save.
2. Return to the title screen before attaching Cheat Engine 7.7.
3. Attach Cheat Engine to `Nioh3.exe`.
4. Open `capture_scroll_auxiliary_consumer_v2.00.02.CT` and accept its Lua script.
5. Enter the game and open the detail page for one already-revealed scroll.
6. Return to the list, then open a second revealed scroll with a different Seed and preferably different enemies/rules/terrain.
7. After two distinct Seeds have been deep-copied, the script removes all breakpoints and calls `detachIfPossible()` automatically. If capture is stopped earlier, execute:

```lua
nioh3P1StopAuxiliaryCapture()
```

8. Return to the title screen before further analysis.

The capture is written under:

```text
Desktop/Nioh3_P1_auxiliary_consumer_capture/run_YYYYMMDD_HHMMSS/
```

## Output interpretation

Each event includes a sanitized `0xE8` record and, where applicable:

- `descriptor_40.bin`: compact descriptor copied for the current scroll;
- `deep_seed_<seed>/outer_vector.bin`: the outer enemy-composition container copied while the breakpoint is active;
- `deep_seed_<seed>/inner_*.bin`: each ordered `0x14` enemy entry vector copied before its source allocation can be reused;
- `source_80.bin`: source container/object observed at the descriptor copy boundary;
- `iterator_40.bin`: selected lookup/iterator state;
- `row.bin`: resolved `0x38` special-rule row, `0x40` terrain context, or `0x1C` terrain row;
- `manifest.tsv`: Seed, type, event RVA, index, key/enum, row pointer, and runtime context.

Record origin fields are zeroed before writing. No account ID is intentionally emitted.

## Acceptance gate for the next phase

The capture is useful only if at least two distinct Seeds produce:

- different `descriptor_40.bin` values;
- rule lookup events whose keys agree with the displayed rules;
- a terrain enum/key/row that agrees with the displayed terrain.

After that correlation is established, the next script will set a write breakpoint on the persistent source field or producer output—not on the stack copy—and capture the real generator call stack and RNG state.

The current CE 7.7 setup must use the Windows debugger backend. VEH debugger initialization exits CE before the first breakpoint is armed. The deep snapshot is intentionally taken during the first rule lookup for each Seed; reading descriptor-owned pointers after leaving the detail page is not valid because those allocations may already have been freed or reused. Do not force-terminate CE while the Windows debugger is attached because Windows will terminate the debuggee as well; wait for `DETACHED.txt` before closing CE.
