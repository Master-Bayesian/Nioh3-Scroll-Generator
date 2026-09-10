# Crucible possessed-enemy research — seed 86872488

## Product conclusion

The current generator can search for a chosen enemy in a mission whose visible
terrain includes The Crucible. It cannot yet promise that a particular enemy
occurrence is the possessed one. The product must keep those two claims
separate until the runtime assignment has been observed and reproduced.

No possessed-enemy switch was added to the player catalog in v0.7.2. Treating
"The Crucible + Nuppeppo" as "possessed Nuppeppo" would create false positives.

## Reproduced seed structure

`generate_complete_auxiliary(86872488, 3)` produces:

- auxiliary mode `0x8E`, native branch class 2, mode row 5;
- terrain row 0, enum `0xD4`, visible terrain key `0x24` (The Crucible);
- descriptor selector 0 with all three recovered descriptor flags clear;
- wave 1: One-eyed Oni, lookup `0x0003B86E`, role 3;
- wave 2: Nuppeppo, lookup `0x000DCB98`, role 1;
- wave 3: Ippon-Datara plus Spider;
- wave 4: Nuppeppo plus Bakegani;
- ordered special-rule keys 23768, 64956, and 20893.

Both Nuppeppo occurrences use the same native enemy row 368:

`FA1B000098CB0D00FFFFFFFF000040401A82FFFF00001F0000000100`

The recovered enemy row, lookup key, role, descriptor selector, descriptor
flags, and terrain row contain no occurrence-specific distinction between the
two Nuppeppo entries. The terrain row's nonzero `+0x2C` value accounts for the
visible Crucible terrain effect; it is mission terrain state, not proof that a
specific enemy is possessed.

## Static-review boundary

A broad scan for displacement `+0xA88` in the archived PC v2.00.02 text image
produced many unrelated structure accesses. It did not establish a consumer
that maps an enemy occurrence to possessed state. The current accepted live
executor targets PC v2.01, so an old-image address guess is not eligible for a
version-gated product feature.

Third-party seed displays agree on the four enemy groups and Crucible terrain,
but do not identify which occurrence receives possessed state. Community
reports also describe run-to-run variation in how many enemies are possessed.
Those reports are useful experiment guidance, not a native contract.

## Next live experiment

Use seed 86872488 on the accepted PC build and capture the completed mission
descriptor before wave 1. At each wave transition, diff the live enemy spawn
descriptors for the two `0x000DCB98` occurrences and record which one the game
renders as possessed. Repeat the mission several times without changing the
scroll. A product filter requires one of these outcomes:

1. a stable occurrence field derived from seed and wave index, with replay
   parity across repeated runs; or
2. a stable post-generation runtime assignment function that can be evaluated
   before search results are shown.

If the assignment is session-random or occurs only during spawning, search can
offer "The Crucible contains Nuppeppo" but cannot honestly offer "possessed
Nuppeppo" as a deterministic seed filter.
