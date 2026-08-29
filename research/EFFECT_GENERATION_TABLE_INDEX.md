# Effect-generation table index

Status: verified table/index layer for PC v2.00.02. This is not a complete
offline effect generator.

## Inputs

The runtime capture was converted into the pointer-free resource at:

```text
nioh3_scroll_editor/data/r4_finalizer/pc_v2_00_02/resource_v1/
```

Every resource blob is size- and SHA-256-gated by
`R4FinalizerResourceBundle` before the typed index is built.

## Recovered fields

### Item row (`0x1A0` bytes)

- `+0xB0`: candidate compatibility flags (`uint32`)
- `+0x152`: scroll record type (`uint16`)
- `+0x154`: currently unnamed `uint32`
- `+0x15C`: currently unnamed `uint32`
- `+0x182`: item generation mode (`uint8`)

The five scroll rows are table indices 3308 through 3312 and use record types
`0x1E82`, `0x516D`, `0xE604`, `0xDD82`, and `0xD523`. All five have generation
mode `0x12`.

### Effect row (`0xD8` bytes)

- `+0x00`: effect ID (`uint16`)
- `+0x02`: effect-group key (`uint16`)
- `+0x1C`: native candidate flags (`uint32`)
- `+0x20`: normalization/promotion flags (`uint32`)
- `+0x54`: progress threshold (`uint16`)
- `+0x56`: alternate threshold (`uint16`)
- `+0x58`: 64 lottery weights (`uint16[64]`)

The weight-array interpretation is directly consumed at RVA `0x57896C` and
the progress-threshold bucket selector is at RVA `0x578C18`.

### Optional-multiplier row (`0x20` bytes)

- `+0x14`: lookup key (`uint32`)
- `+0x18`: multiplier (`float32`)

The two keys selected by RVA `0x57896C` are `0x0415` (`1.0`) and `0xA6D1`
(`0.1` as binary32).

### Rarity-generation row (`0xF8` bytes, mode `0x12` path)

- `+0x1C`: minimum effect roll (`uint32`)
- `+0x20`: maximum effect roll (`uint32`)
- `+0x44`: base slot count (`uint32`)
- `+0x4C`: total slot count (`uint32`)
- `+0x58`: promotion-trial count (`uint32`)
- `+0xDC`: promotion probability in percent (`float32`)

RVA `0x110EE26..0x110EFAD` performs the trial(s), shuffles the seven physical
slot indexes using seven LCG draws, rejects slots carrying effect flag `0x01`
or `0x02`, and sets effect flag `0x04` on the accepted slots. Type classes 1
and 2 do not enter the shuffle path.

### Effect-group row (`0x70` bytes)

- `+0x0C`: effect-group key (`uint16`)
- `+0x24`: category key (`uint32`)
- `+0x54`: conflict mask 0 (`uint32`)
- `+0x58`: conflict mask 1 (`uint32`)

RVA `0x5777D0` resolves a stored effect ID to its effect row and returns the
group key at `+0x02`. RVA `0x5778C0` rejects a new candidate when its group key
equals a previously accepted group key or either pair of masks intersects.
RVA `0x578C40` applies the same mask test against the preinserted special/Grace
effect.

### Category row (`0x6C` bytes)

- `+0x08`: category key (`uint16`)
- `+0x18 + rarity*2`: base capacity (`uint16`)
- `+0x5C`: mode-`0x12` capacity (`uint16`)

RVA `0x3DBE9C` selects the mode-`0x12` subvector and RVA `0x91B6E8`
constructs the 32-byte capacity vector. Normal categories use the smaller of
the rarity and mode capacities. Category `0x1A` is the native special case and
uses its rarity capacity directly.

## Implemented semantics

`EffectGenerationTableIndex` provides:

- unique lookups for all five scroll items;
- unique effect-ID and effect-group-key indexes;
- effect-to-group and effect-to-category resolution;
- exact group-equality and conflict-mask compatibility checks;
- complete RVA `0x57896C` weight evaluation for a supplied scroll type,
  rarity, playthrough and destination-slot state;
- the ordinary descriptor-flags-zero slot-promotion path from
  `0x110EE26..0x110EFAD`, including exact RNG consumption;
- the complete mode-`0x12` 32-byte category-capacity vector from RVA
  `0x91B6E8`;
- the table/flag portion of candidate-context eligibility at RVA `0x5788FC`;
- the ordinary and promoted per-slot candidate pools through
  `0x57818D..0x57825B`, including category capacity, requested category,
  normalization flag `0x08`, conflict checks, context flags, and native
  weights;
- the inclusive candidate lottery at `0x57830D..0x57833A` and the exact
  rarity-dependent effect-value roll at RVA `0x980D58`;
- fail-closed behavior for unknown effects or malformed table references.

Tests include six native table vectors, a known conflict-mask pair, and six NG3
rarity-5 weight vectors. The selected effects in the current NG3 rarity-5
native record all resolve through this index. Normal mode-`0x12` slots use the
item row's `+0x15C` selector (`59` for all five scroll types); destination slots
carrying flag `0x40` use selector `0x29`, matching `0x57810D..0x57812C`.

## Remaining work

The index intentionally does not yet claim:

- the alternate live-context predicates called by `0x5788FC` (the ordinary
  batch/title-screen context is implemented, while alternate context remains
  an explicit input);
- destination effect-flag `0x40`: helper RVA `0x572C20` is now bounded and
  disassembled, but its three dependent item/state predicates still need
  semantic recovery before this branch can be enabled;
- consumption/decrement order for the category-capacity vector inside the
  outer effect lottery;
- descriptor flag overrides at `r13+8` in `0x110EC50` (their external context
  multipliers are not yet fully captured);
- non-scroll item modes outside the captured mode-`0x12` path;
- retry/rejection RNG consumption;
- numeric value, metadata, prefix, and tail parity;
- complete R4 finalization or complete `0xE8` record parity.

Those pieces must be added in native control-flow order and compared against
new Seeds that are not present in any replay corpus before the game-closed
generator can be accepted.
