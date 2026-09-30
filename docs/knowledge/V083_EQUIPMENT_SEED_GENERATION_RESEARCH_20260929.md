# v0.8.3 equipment generation from a seed (research, 2026-09-29)

Goal: a "legal" new item must be one the game's own generator produces from
some seed, not a per-slot combination of legal effects and values. This note
records the reading of the PC v2.0.2.0 generator. Status: **verified** by
running the game's own code on a read-only memory snapshot and ported to
`crates/nioh3-domain/src/equipment_generation.rs`; see "Verification" below.
The static notes are kept, with the corrections the emulation forced.

Code was read from the decrypted image captured on 2026-09-28
(`tmp/claude-ce/completion-capture/nioh3_image.bin`); the executable on disk is
encrypted (`.bind` section). Helper: `tmp/claude-ce/disfn.py` (capstone over
`.pdata` bounds).

## One generator for scrolls and equipment

`init_generation_context` (`+0x5513C8`) and `generate_effects` (`+0x557F34`)
are the same sites the scroll profile (`pc_v2_02.json`) resolves, so the
scroll port (`effect_sequence.py`, `crates/nioh3-domain/src/sequence.rs`)
already mirrors parts of this function. The port is limited to item mode
`0x12` (scrolls, item row `+0x182`); equipment takes the default branch.

Generation context (`init_generation_context(ctx, id, rarity, seed_word)`):

- `+0x00` u16 item id, `+0x02` u8 rarity, `+0x04` u32 seed word (low u16 flag
  `1`, high u16 seed: 65,536 seeds per item, rarity and context);
- `+0x08..+0x1F` zero, then set by the drop path: `+0x08` u32 flags,
  `+0x0C` u32 chance override, `+0x10` u32 `0x07D0 | area << 16`, `+0x18` u32;
  the reward-grant routine (`+0x2188610`) leaves them zero;
- `+0x20` u8 from `[[+0x4751530..]+8]+0x10` (`+0x551404`), used as the type
  class of the weight formula; `+0x24` 16 bytes of playthrough progress
  (`+0x5592A4`).

The RNG (`+0x9D8AA8`) is a scoped object seeded with the whole seed word and
pushed as the current generator; every draw below uses it.

## Equipment path through generate_effects

1. Fixed slots: the item's set effect (`+0x154`) is placed with entry flags
   `+0xE |= 1`, `+0xD |= 0x40`; otherwise, from rarity 4 and when the item mode
   is not 9, a grace from `+0x5577BC` with `+0xE |= 2`.
2. Innate effects (`+0x158`, `+0x15A`), one loop per innate id. Context
   `+0x14 == 3` takes a weighted draw over the whole effect table (star rows
   excluded) instead of the fixed id.
3. For each random slot (count from the rarity row read at `+0x2FA3B0`, fields
   `+0x34`, `+0x28`, `+0x2C` for equipment): entry `+0xD |= 0x80` (`0x40` for
   scrolls), and entry `+0xE |= 0x40` when `+0x9DEC1C(item)` holds; then
   `rarity-row +0x2C` further slots get `+0xD |= 0x80` with probability
   `rarity-row +0x5C` percent (`+0x35F0A8(100)`): these are the forge-material
   markers. Two runtime predicates (`+0x283928`, `+0x283950`) clear every
   `0x80` marker.
4. `+0x2F9AB4` (not for scrolls) and the star-promotion selector `+0x1114750`
   run next. The selector's chance comes from the playthrough/rarity table and
   the context: flags `+0x08` bit 2 adds param `0xA0D5`, bit 1 multiplies by a
   playthrough table, bit 4 replaces it with `+0x0C`. This is how the drop
   source changes the result: the observed drop contexts `{1|5|0, 10000|2000}`
   are these flags and overrides.
5. Per slot, a weighted lottery (`+0x623660`) over effect rows filtered by
   `+0x558ECC` (context flags), `+0x557E90`/`+0x559210` (group and mask
   conflicts), category capacity and, for slots with `+0xE & 0x40`,
   `+0x5531F0`: the effect group's subject key (`+0x14`) must name a param row
   of kind 2 the player owns (`+0x555EB8`) or one listed by `+0x553264`. Those
   slots use weight column `0x29`, so ninjutsu and onmyo effects depend on the
   player's learned spells.
6. Weight (`+0x558F3C`) = row weight at `+0x58 + 2 * column` × constant ×
   progress gate (`+0x54`) × rarity weight (`+0x28..+0x3C`) × type multiplier
   (`+0x44`, or `+0x48/+0x4C/+0x50` for type classes 3/4/5) × a param
   multiplier (`0x415` or `0xA6D1`, chosen by context `+0x18` against row
   `+0x56`) from type class 5.
7. Each value roll is `+0x983E28(rarity)`, the scroll rarity-roll table.
8. Post-processing: `+0x627ADC` (not for scrolls), then `build_record`.

## Consequences for the product

- A legal new item is fully determined by (item, rarity, level, seed, drop
  context, type class and progress, learned spells for column-0x29 slots).
  Level only scales values; rolls come from the seed.
- The reward route (context zero) is a real game path and enumerates 65,536
  seeds per item, rarity and player state, including empty outcomes. The
  current port uses this route; enemy and region drop contexts, including the
  star-promotion variants of step 4, are not searched. A no-match or all-empty
  result does not establish impossibility through other game routes.

## Verification (2026-09-29)

Method: `tmp/claude-ce/emu_gen.py` runs `init_generation_context`,
`generate_effects` and `build_record` (serial suppressed) in unicorn over pages
read with `PROCESS_VM_READ` from the running game and cached to disk, with the
vector-growth and free helpers replaced. Three traced samples matched the
in-game preview byte for byte first. A run that does not return to the stop
address is an error (an early 5M-instruction cap had silently cut degenerate
items short and looked like a game quirk).

Results:

- Python port (`tmp/claude-ce/equip_port.py`, `build_port.py`) against the
  emulator: 3,600 compact-entry comparisons and 5,000 whole records identical,
  1,500 more across difficulties 1..5 and every progress threshold.
- Rust port against 900 emulated records (all item kinds, rarity 0..5,
  difficulties 1..5): identical (`crates/nioh3-data/tests/equipment_generation.rs`).
- Historical owner-save replay: 1,013 of 1,318 owned items matched selected
  fields from their own seed (effect, roll, category, set/grace/star flags);
  rarity 0..3 about 95%, rarity 4 about half. This was a field projection,
  not whole-record equality. Enemy-drop contexts and blacksmith changes can
  explain nonmatches, but the cause of every nonmatch was not established.
  These counts predate the group-key replay repair and have not been rerun
  against the repaired comparison; they are not current audit-accuracy proof.

Facts the emulation settled:

- Soul cores (mode 9): no grace; innate count rarity `+0x38`, entries
  `+0x40` (1, 2, 3, 4, 5, 5), marked `+0x3C`, no probable markers; promotion
  uses `+0x54`/`+0xD8`; afterwards `+0x2286FAC` draws once (rarity `+0x68` is
  zero, so it never marks).
- A slot whose pool is empty retries once in any category without its star;
  a second failure empties the whole item, set, grace and innate included: the
  compaction at `+0x558AE1` keeps entries whose first word is nonzero, and that
  word is always zero. Such items (for example early armor at rarity 4..5) do
  yield no valid record through the tested reward route; the product reports
  that bounded result rather than ruling out other drop contexts.
- `+0x627ADC` marks entry `pick` itself, not the pick-th eligible entry.
- Values: `resolved_effect_value` at the stored level (capped at 180), plus
  game param `0x98FE` (10) levels at rarity 5; accessories of kind `0x2D32` add
  `+0x550484`. Params `0x415` = 2.0 and `0xA6D1` = 0.5.
- Stored order (`+0x551C54`): exchange sort by key, innate 1 and 2, random 3,
  groups with `+0x08` set (grace) last-but-empty, empty last; a set effect whose
  group has `+0x08` clear stays first.
- Player state: type class = current difficulty (1..5), progress =
  `(A[d], B[d], C[d], max)` of that difficulty; effect gates are 0..5000, so
  only `A[d]` matters today. In the save these are tagged fields (hash
  `0xA6C2D359` byte, arrays `0xD35EEEF8`, `0xCEE5FD21`, `0xB687D380`), each
  unique in every retained save. The grace bias (`player+0x2820` key `0xB11A`)
  and the two runtime predicates are absent for the owner, which is one natural
  state; the restricted-slot flag (`+0x9DEC1C`) held for no item.
