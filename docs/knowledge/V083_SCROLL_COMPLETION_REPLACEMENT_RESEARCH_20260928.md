# Scroll challenge-completion replacement — PC v2.02 (2026-09-28)

Owner goal: offline search should offer effect combinations that no natural
scroll carries but that a few challenge-completion replacements can reach
(bounded to about five). This records how the game builds the replacement
candidates. Status: the offline model reproduces the recorded live replacement
vectors, a further predicted round was confirmed by the owner, and the
extra-painting selector was resolved with a predicted playthrough and saved
record comparison (see Evidence and Extra painting below). The reveal trigger
and slot choice remain open. No prediction UI or search is shipped.

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
- The second live round (counter 2) matched the model again, id and value.
  The owner declined it: `+0x0C` still advanced to 3 and `+0x33` still
  decremented, so declining is a branch of the same step, not a skip.

## Extra painting (添画)

A PC v2.02 addition: after a challenge completion an R4 scroll may gain one
more effect. It can happen at any completion, not only the last one.

- `0x102ACD0(record)` runs it: if `0x1111A30` accepts the record it calls
  `0x110E710(out, record)` and copies the result back. When the result carries
  flag `0x20000000` at `+0x18` it succeeded; otherwise `+0x32` is incremented
  (saturating at `0xFF`). So `+0x32` counts failed extra-painting rolls (a pity
  counter), not completed challenges or accepted replacements; the save
  installer's reset of `+0x32` to 0 matches a fresh scroll.
- Eligibility (`0x1111A30`): rarity `+0x30` is 4 and fewer than six of the
  seven entry group words (`+0x34 + 0x18 * i`) are non-zero. A success fills
  entry 5, so it happens at most once per scroll.
- Trigger (`0x110E710`): parameters `0xD56F` = 30, `0xAA65` = 15 and
  `0x3472` = 100 (the three `optional_multiplier` rows new in v2.02, int at
  `+0x10`, float `+0x18` = 1.0), each times 0.01. The threshold is
  `int(min(1.00, 0.30 + 0.15 * byte(+0x32)) * 10000)` in binary32 (2999, 4500,
  5999, 7500, 8999, 10000). A scoped LCG seeded with `~displayed_seed(+0x20)`
  draws `byte(+0x32) + 1` values `min(int(float01 * 10000), 9999)` and keeps
  the last; the roll succeeds when it is below the threshold and entry 5's
  group word (`+0xAC`) is zero. The roll does not depend on the counter or on
  the replacement choice.
- Content: `0x110ED60(out, record, 5, 0, no exclusions)` on the record after
  this completion's replacement decision (counter already advanced), with no
  category draw and `(5 + counter) & 0x1F` warm-up draws (the replacement path
  has one more), and selector `0x3D`. The new effect is then sorted in front
  of the grace.
- Evidence: seed 121723131 had `+0x32` = 1 before the accepted first round
  (draw 5659 against 4500: no extra painting, `+0x32` became 2) and 2 before
  the declined second round (draw 3388 against 5999: extra painting, `+0x32`
  stayed 2). The added effect, 道具掉落率 `0xB393` value 66 roll 90, matches
  the content model byte for byte.

- Predicted before play, owner's NG3 R4 scroll seed 47878870 (save slot 46):
  the roll draws 416 against 2999, so its first ordinary completion must add
  an effect. Declining, the model gave 近距离攻击伤害 `0xDB20` value 57 roll 93
  with selector `0x3D` and 近距离攻击打倒敌人时恢复体力 `0xA0A7` with `0x3B`; the
  game added 近距离攻击伤害 value 57 roll 93 in front of the grace, which settles
  the selector. The saved record also shows the success path: `+0x0C` 0 -> 1,
  `+0x32` stayed 0, `+0x33` 6 -> 5 and `+0x18` gained `0x20000000`.

## Reveal (揭秘) completions

The first challenge of that never-challenged scroll was a reveal, not an
ordinary completion: `+0x0C` and `+0x32` stayed 0 (no extra-painting roll),
`+0x33` went 7 -> 6, `+0x18` gained bits `0x09000000` (`0x06800084` ->
`0x0F800084`), and entry 3 (防御时的属性攻击伤害降低) became 精髓并存（武士）
`0x512D` roll 88 -- exactly the modelled replacement candidate for entry 3 at
counter 0 with the earlier eligible slots as exclusions. A reveal therefore
uses the same generator but no choice, no counter advance and no extra
painting. Which records reveal, and which entry, is not yet traced (the
`+0x18` bits are the lead).

## Open

- The reveal trigger and its slot choice (see above).
- Selector and type class for other record types and rarities (both samples
  are NG3 `0xE604`, rarity 4).
- Research note: debugger watches on the live game crashed it once
  (`0xC0000005` outside the image) after an MCP execute-watch call failed;
  the static analysis above used a dumped image instead.
