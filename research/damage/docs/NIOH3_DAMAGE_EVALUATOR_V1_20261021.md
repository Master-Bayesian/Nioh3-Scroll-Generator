# Nioh 3 damage evaluator v1 — native equipment-to-damage chain (PC 2.0.2.0, 2026-10-21)

## Result

For one captured attack, the evaluator predicts the exact in-game damage of an arbitrary set of real equipment.
The set covers weapon, armor, accessories and the equipped scroll. The game's own code computes every number;
no coefficient is fitted. Four predictions were made before the hit and matched the game exactly: 1018, 1120,
1018 and 1114. The package, frozen inputs and regression live in
`F:/Nioh3_r16_build/deliverables/r16-damage-evaluator-v1-20261021/`, in local git commit `5dbe65c`.
`regress.py` reproduces all 8 accepted observations offline.

## Native chain (rebuild order)

| Stage | RVA | What it contributes |
| --- | --- | --- |
| Prepared weapon stage | `818248` | STAT records at `+0x24`, 64 bytes each. Primary and secondary attack come from `ordinary_weapon` plus core bonus. Weapon effects are collected. |
| Body stage, slots 4..12 | `2FB2BC` | Per piece: defense (`+0x1C8`, `+0x1F4+4*(slot-4)`), toughness `+0x218`, weight `+0x7B8`, `+0x21C`, twelve body floats. Effects pass through collectors `55764C`/`5557DC` (aggregation `557BA0`), and set pieces are counted in side banks. |
| Slot-15 stage, the equipped scroll | `81744C` | Adds a flat attack to `+0/+4` of every weapon record, adds defense, and collects the scroll's effects. |
| Set expansion | `3FAA98` | Set rewards once a side-bank count reaches its threshold. |
| Load factor | `819204` | Sets `STAT+0x7A0` from weight and capacity. |
| Materializer | `81920E..819251 -> 819400` | Turns the attribute-source entries into D and STAT fields. |
| Attack packet | builder `2B30A0` / COPY `78A194` | MAIN reads weapon record 0 through its packet copy: `MAIN+8 = rec0+0 + rec0+8`, `+C = rec0+C`, `+14 = rec0+18`. This holds in all 10 captured hits. |
| MAIN | `78CE8C` | Damage at `MAIN+0x218`. Class0 statuses act through owner `+0x2DC0+0x7C` (`+0x2E3C`); MAIN folds that into its boost/cut pair. |

The model replays these stages on the captured baseline twice, once with the baseline banks and once with the
candidate banks. The prediction is the captured state plus the per-word model delta. Stages the lift does not
replay therefore cancel instead of being guessed, and baseline contributions are not double-counted. Integer
words are carried exactly. Float words are exact when the model reproduces the baseline word; otherwise they
are carried by a delta.

## Findings worth keeping

- **Weight drives this attack.** `STAT+0x7B8` is equip weight: item-table u16 `+0xA0`, using the rarity-redirect
  row `+0x168` when rarity is at least 4. It feeds the 8E2D curve, and that curve raises this hit's damage.
- **The equipped scroll sits in slot 15.** Its flat attack, +82 in the observed case, passes through the packet.
- **The attack packet is a copy.** Changing STAT weapon records alone does not change MAIN damage; the packet
  fields above must follow.
- **The 2FA678 lookup table on the heap is static data.** It is `resource01119` (662 rows of 84 bytes) in native
  row order, plus a sorted pointer array. It can be rebuilt offline: 794 retained bytes, 0 mismatches.
- **Samurai and ninja equipment are separate.** The class comes from the item catalog. Item row byte `+0x183`
  groups 刀 with 忍刀, so the weapon type must also be checked before reusing an action capture.
- **Training grounds can flood the observer.** The player's stats are rebuilt and HP refilled every frame for
  a while. The observer's callback budget must not count those events.

## Limits

- One action and one target per capture.
- Baseline actor state from 10-16 (identical on 10-21).
- Assumed config byte `CF8 = 0`.
- Possible ±1 on weapon swaps.
- Conditional effects need their runtime state at the hit.
- Search results are best-known, not certified optimal.

Next steps:

- capture more actions, weapon types and enemies;
- read CF8;
- read the baseline from a live demand fill instead of the 10-16 capture.
