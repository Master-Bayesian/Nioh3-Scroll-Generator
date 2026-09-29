# v0.8.3 equipment generation from a seed (research, 2026-09-29)

Goal: a "legal" new item must be one the game's own generator produces from
some seed, not a per-slot combination of legal effects and values. This note
records the static reading of the PC v2.0.2.0 generator. Status: static only;
field meanings marked below are readings to be confirmed by running the game's
own code on a read-only memory snapshot (`tmp/claude-ce/emu_gen.py`), which
needs the game running with the character loaded.

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
   `+0x34`, `+0x28`, `+0x2C` for equipment): entry `+0xD |= 0x40` (`0x80` for
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
- The reward route (context zero) is a real game path and gives 65,536 results
  per item, rarity and player state; enemy drops add the star-promotion
  variants of step 4.
