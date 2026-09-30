# Nioh 3 Studio 0.8.3 — Candidate

This local candidate adds save-file equipment generation from the game's
seed-based generator and improves equipment management.

## Add equipment

- Add weapons, armor, accessories, and soul cores from a save file by choosing
  an item, level, rarity, and seed.
- In Legal mode, the candidate builds the complete item record using the
  game's item-grant route for the selected difficulty and player progress.
  Enemy and region drop variants are outside this route.
- Search seeds for selected effects and preview the matching records before
  preparing a save change. No-match and empty-result messages apply to this
  route and selected state, not every way the game can drop equipment.
- Modded mode remains separate for unrestricted values and effects.
- Hell weapons are available for modded additions only.

## Equipment management

- Remove equipment from a save file; equipped items are removed from their
  equipment sets. Live removal is limited to unequipped items and requires
  confirmation.
- Filter owned equipment by verdict and type, sort by level or rarity, and see
  which items are equipped.
- Natural verdicts now use the item's own seed replay. Rarity-5 soul cores and
  in-game re-rolls receive more accurate naturalness results.
- Repeated effect names include their slot or source so different effects are
  easier to distinguish.
- Effect templates identify the buff, ailment, weapon, skill or item where the
  argument has been resolved from game data.
- Adding and editing are peer entry points on the Equipment & items page.

## Save files and fixes

- If a save changes after a plan is prepared, the app rereads it before
  applying the plan and refuses an unsafe overwrite.
- A stale live-add lock is cleared at startup and can be reset by the player.
- Refusals explain the next action in Chinese.
- A scroll installed into a save starts with a fresh usage counter.
- Enemy-selection help explains the three layouts and impossible combinations.
- Opening the data folder reports success as a status rather than an error.

For save-file changes, return to the title screen before editing a save outside
the game, then load that save in game afterward. The game does not need to be
closed.

## Compatibility and candidate status

The candidate targets PC v2.02 and previously supported builds. Unsupported
versions and unknown resource graphs are refused.

Seed generation passed focused offline parity and synthetic-save E2E checks.
New packaged seed-add UI and real-save write/reload acceptance remain pending.
Prior native insertion and live-removal demonstrations are documented
separately; they do not establish the new seeded save-add flow. This is a local
test candidate, not a public release.
