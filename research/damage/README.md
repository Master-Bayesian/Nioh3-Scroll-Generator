# Damage research (Nioh 3 PC 2.0.2.0)

Research branch, not product code. It keeps the damage-formula and evaluator work safe and shareable.

- `docs/`: research notes, October 2026. Start with `NIOH3_DAMAGE_EVALUATOR_V1_20261021.md` (accepted evaluator v1:
  blind in-game predictions 1018, 1120, 1018 and 1114 matched exactly) and `NIOH3_NATIVE_BUFF_MECHANISMS_20261019.md`
  (status buffs, stacking and dilution).
- `evaluator/`: evaluator v1 source, regressions and the file manifest. Its frozen inputs (game-memory captures and
  inventory records, about 11 MB) and in-game evidence stay outside the repository; `MANIFEST.json` pins their hashes.
