# Effect Seed Solver

`solve_effect_seed.py` finds Nioh 3 scroll Seeds from effect constraints without
sequentially scanning the full 32-bit Seed space.

## Current execution model

- NG3 rarity-5 `--fixed-only` is game-closed: it does not require a running
  game, a save file, or a cached primary map.
- Latent NG4/NG5 rarity-5 has the same game-closed effect and auxiliary replay after
  one complete 65,536-bucket Grace map has been captured for the exact save
  context. The GUI persists that map under
  `%LOCALAPPDATA%\Nioh3ScrollGenerator\grace-output-maps` and reuses it without
  opening a game process. The selected save is still read to enforce the cache
  fingerprint gate.
- NG4/NG5 are not accessible in v2.00.02 and are expected DLC2 content. Forced
  `0xDD82`/`0xD523` output is research evidence for latent contexts only; it is
  not a compatibility claim for the eventual DLC release.
- Contexts without a complete validated map still use the live native
  generator. For those paths the game must be running offline at the title
  screen. Temporary decrypted data is read only to obtain a canonical record
  template; the solver never modifies or installs anything into the save.
- Grace is inverted through its complete first-draw 65,536-bucket map.
- The historical draw-2 primary map contains one representative low-16 state
  per bucket. Primary output is not invariant inside a bucket, so this map is
  only an incomplete candidate prefilter and never final proof.
- NG3-NG5 rarity-5 candidates with a complete Grace map are replayed by the
  game-closed effect-sequence generator before primary or secondary output is
  accepted.
- Terrain, ordered enemy output, and ordered special rules are generated and
  filtered offline before a candidate is returned.
- Mathematical pivot construction and exact NG3-NG5 primary-effect batches use the
  bundled CUDA accelerator when an NVIDIA CUDA device is available, with a
  bounded native CPU fallback. Pivot calls contain at most 1,000,000 trials;
  primary calls contain at most 16,384 surviving Seeds. The accelerated output
  is sorted by the canonical cursor and then replayed by the exact offline
  generators.
- Legacy/live primary maps are cached under
  `%LOCALAPPDATA%\Nioh3ScrollGenerator\primary-effect-maps`; the NG3 rarity-5
  game-closed path neither needs nor trusts one as a complete inverse.

For NG3 rarity-5, `--fixed-only` searches the complete exact Grace inverse
family and verifies each complete Seed through the recovered effect path. It is
not restricted to the historical representative primary map. Exhausting this
family is therefore an exact no-solution result for the supplied Grace plus
effect and auxiliary constraints, subject to the currently supported native
context and recovered path.

Use `--max-results N` to return a candidate list instead of stopping at the
first match. In live-assisted mode every candidate contains the complete native
effect record. In NG3 rarity-5 `--fixed-only` mode each candidate contains the
exact ordered effect IDs, percentile rolls, resolved values, canonical effect
slot bytes, terrain, enemies, and special rules. The solver payload does not
automatically bind those slots to a user's save template. The GUI performs that
binding only inside the guarded install action.

## Examples

Use exact native Chinese, English, or Japanese names, or raw IDs such as
`0xAE5A`.

NG3 rarity-5 Grace plus primary and two required secondary effects:

```powershell
python .\research\solve_effect_seed.py `
  --fixed-only `
  --playthrough 3 `
  --rarity 5 `
  --grace "月读的恩宠" `
  --primary "技之深奥" `
  --secondary "体力" `
  --secondary "伤害反映（忍术威力）" `
  --terrain "地狱" `
  --rule "一难横行（足部防具）" `
  --enemy "一目连" `
  --max-results 20 `
  --output .\research\solver-result.json
```

Allow either of two primary effects by repeating `--primary`:

```powershell
python .\research\solve_effect_seed.py `
  --playthrough 3 `
  --rarity 5 `
  --grace 0xBABD `
  --primary 0xAE5A `
  --primary 0xDFF0
```

NG1/NG2 have no Grace slot and require at least one primary constraint:

```powershell
python .\research\solve_effect_seed.py `
  --playthrough 2 `
  --rarity 3 `
  --primary "Ultimate Skill"
```

If more than one save is discovered, pass an explicit file:

```powershell
python .\research\solve_effect_seed.py `
  --save "D:\path\to\SAVEDATA.BIN" `
  --playthrough 3 `
  --rarity 5 `
  --grace "月读的恩宠"
```

## Output

On success, the command prints JSON containing:

- the decimal and hexadecimal Seed;
- rarity and generation stage;
- every generated effect ID and localized name;
- for NG3 rarity-5 game-closed results, the exact percentile roll, native
  category/effect flags, and recovered base resolved value for each effect;
- for live-assisted results, the complete native value and slot metadata;
- terrain, ordered enemies, ordered special rules, and rule values;
- the canonical seven-slot effect region as `effect_slots_hex` for NG3 rarity-5
  game-closed results;
- the complete `0xE8` record as hexadecimal data only for live-assisted native
  records;
- the joint-search trial cursor.
- `intersection`, containing the exact cumulative survivor count after each
  selected Grace, primary, secondary, terrain, rule, and enemy constraint. The
  scope is `inspected_range_exact_count` until the complete inverse family has
  been exhausted; only `global_exact_total` is a full-space cardinality.

When `--max-results` is greater than one, the top-level JSON uses the batch
schema and contains `results`, `result_count`, and `next_resume_trial`. Use
`--resume-trial` with `next_resume_trial` to request the next page.

Exit code `0` means a candidate was found, `2` means the requested candidate
budget was exhausted, and `1` means input or runtime failure.

Use `--resume-trial N` to continue a Grace-plus-primary joint search after the
reported trial cursor. For NG3 rarity-5 game-closed searches it is the exact
cursor in the Grace inverse family, with or without a primary filter.

## Acceptance boundary

Live-assisted output is exact native-generator record output for the current
game version and loaded save context. NG3 rarity-5 game-closed output is exact
for the recovered effect sequence, value/slot serialization, auxiliary
subsystems, and complete record assembly. The materializer passed 10,000
deterministic live-native full-record vectors without a byte mismatch. The GUI
binds lineage and a fresh serial from the current save only at install time.
The command never installs a result and does not by itself prove online
propagation; recipient-side receipt remains the propagation acceptance test.
Latent NG4/NG5 game-closed support currently has two historical native
effect-region vectors plus single/batch/full-path consistency tests. Because
these playthroughs are not released, the path is permanently read-only for the
v2.00.02 build. Future DLC binaries and tables must be recaptured and requalified
before any installation or propagation claim.
