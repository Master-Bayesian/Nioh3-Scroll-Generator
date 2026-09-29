# Scroll challenge-completion replacement — PC v2.02 (2026-09-28)

Owner goal: offline search should offer effect combinations that no natural
scroll carries but that a few challenge-completion replacements can reach
(bounded to about five). This records how the game builds the replacement
candidates. Status: offline model reproduces two live vectors exactly (eight
candidates, effect id and value); one further in-game prediction check is
pending. Nothing is shipped.

## Where it lives

RVAs are PC v2.02 (`Nioh3.exe` 2.0.2.0, image dumped from the live process).

- `0x20E4B68` applies the chosen replacement: `0x2281460(out, record, slot,
  flag)` builds the new record, `0x552DD0` copies it into the record, then
  `+0x0C` is incremented. A write watch on the chosen slot's id hit only this
  copy (source on the stack).
- `0x2281460` does not read the candidates shown on screen; it regenerates
  them. For every earlier eligible slot it calls `0x110ED60` and collects the
  results as an exclusion list, then calls `0x110ED60` for the chosen slot.
  Eligible slots: group word non-zero, entry `+0x0D` bit `0x40` clear (the
  primary slot is never offered), entry `+0x0E` bit `0x02` clear, id not 1.
- `0x110ED60(out, record, slot, flag, exclusions)` generates one candidate.

## The candidate generator (`0x110ED60`)

1. Seed (64-bit sum, low 32 bits kept), on the unmodified record:

   ```text
   seed = displayed_seed(+0x20) + counter(+0x0C)
        + int32(rarity(+0x30) * counter) * (displayed_seed >> 16)
        + int32(slot << 16) * 7
        + sum over the 7 entries of int32(id) * min(roll(entry +0x0C), 100)
   ```

   `0x9D8AA8` pushes a scoped LCG with that state; while it is active the
   game's "global" random helpers (`0x35F0D4` and friends) draw from it, so the
   candidates are deterministic, not global randomness.
2. `(slot + counter) & 0x1F` warm-up draws.
3. The chosen slot is cleared; `0x2F9AB4` then assigns a category to it with
   the ordinary mode-0x12 category lottery (one `random_inclusive(0, total)`
   draw, same as `R4FinalizerEngine`'s category assignment). The drawn
   category does not restrict the pool.
4. Pool, in effect-row order from row 1: skip the chosen slot's current effect
   id; category capacity (`0x921120` capacities minus the other entries whose
   `+0x0E & 3` is clear) must be non-zero; context bit `0x40`
   (`0x80` in the alternate runtime context), item flags `0x800`/`0x1000` for
   effect flags `4`/`8`; no group equal to, or conflicting (`+0x54`/`+0x58`
   masks) with, the other entries or the earlier candidates. Weight is
   `0x558F3C` (= `effect_weight`) with type class 3 for these NG3 scrolls and
   lottery selector `0x3E`. The selector is `0x3C` with the flag set, or the
   item's `+0x15C`, overridden to `0x3D + flag` when a challenge-context row
   has `+0x2F` bit 1; both live vectors used `0x3E`, where mystic ("…之深奥")
   rows weigh 180 against 200, which is why they show up so often.
5. One inclusive weighted ticket (`r <= w`), then the rarity roll (`0x980D58`
   equivalent) and the value from the effect's curve.

## Evidence

- Live, PC v2.02, owner's NG3 R4 scroll seed 121723131, counter 1: shown
  技之深奥 / 不消耗使役符 +1.3% / 远距离伤害 +7.5% / 道具掉落率 +6.6%; the
  model gives `0xAE5A` 150, `0x6CE3` 13, `0x2EFC` 75, `0xB393` 66. The applied
  slot wrote `0xAE5A`, value 150, roll 85, as modelled.
- Archived PC v2.00.02 vector (`captures/reroll_live/seed_203900415`, counter
  1): 体之深奥 / 武之深奥 / 不消耗使役符 +1.4% / 精华槽增加量 +9.3%; the model
  gives `0xDAC2` 150, `0xDFF0` 150, `0x6CE3` 14, `0xBC51` 93 (the slot-5 id
  the capture could not confirm).
- A completion also increments `+0x32` (completed challenges) and decrements
  `+0x33` (remaining attempts).

## Open

- One more in-game check: the owner's save reverted to counter 1 after a
  crash, so the same scroll must show the same four candidates again.
- Whether declining a replacement still advances `+0x0C`.
- Selector and type class for other record types and rarities (both samples
  are NG3 `0xE604`, rarity 4).
- Research note: debugger watches on the live game crashed it once
  (`0xC0000005` outside the image) after an MCP execute-watch call failed;
  the static analysis above used a dumped image instead.
