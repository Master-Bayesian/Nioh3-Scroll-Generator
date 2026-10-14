# Loadout weight and state calculation

Portable inputs, current-PR reproduction commands and evidence scope: [maintainer handoff](../research/LOADOUT_PLANNER_HANDOFF.md). Historical capture names below identify the accepted archive evidence.

The October 13, 2026 local v0.8.8 planner replaces sample weight, capacity,
agility and toughness with current configuration inputs. It preserves the
owner-approved skill exclusion from the ordinary effect summary. Skill
mechanisms are evaluated individually only when they explicitly alter state
values. Unbound mechanisms do not alter totals; the blanket selected-skill
disclaimer was removed at the owner's request on October 14.

Life/Ki are now integrated through the [resource calculation contract](LOADOUT_RESOURCES.md).

## Calculation contract

- Five active armor pieces and two active accessories contribute base weight
  and toughness. Accessories have native weight (typically 0.1), not zero.
  Weapons and soul cores contribute relevant effects, not invented base weight.
- Rarity selects the original/alternate item row. Thickening uses the reviewed
  remodel coefficient and getter-stage cap. Save projection supplies remodel
  bytes and record flags at +0x18 without changing the save or equipment code.
- The original-row cold index and flags 0x40000/0x200000 select the native
  exceptional toughness branch. Missing metadata produces bounded values;
  unsupported items/modes produce pending results. The paired save difficulty
  now supplies the verified remodel stage; missing older snapshots remain bounded.
- Capacity reads the captured native attribute table using current Stamina
  (the third attribute, 刚) and style, within the planner's 5..150 range.
  Native group 0xB3FE scales base capacity through float32 multiplication and
  truncation; group 0xECC1 adds flat toughness. Relevant equipment, scroll,
  clan, guardian and set inputs share the existing verified group accumulator.
- Agility uses integer floor(weight * 1000 / capacity), with 20/30/70% gates
  and a direct overload check. Toughness uses 130/260/320 gates. AA requires
  active Ultimate Stamina at Stamina 150. Red Demon promotion requires both
  clan group 0xE63F (如风赤备) and all five Red Demon armor pieces; a set marker
  alone does not grant it. No AAA/AAA+ state rank exists in the recovered formatters.
- Different endpoint grades remain pending rather than selecting one endpoint.
  Unknown guardian activation is bounded for stat-related effects.

## Provenance and reproduction

PC v2.02 executable SHA256:
`E22C4A635E4EC1E27A177B76E27D7F6A637F426C0ED3928B60F5693BC52AE130`.
Existing armor model: `tools/armor_remodel_batch_v202.py` in the retained
`armor-remodel-batch-pr` worktree. Its reviewed corpus contains 1352 getter
returns. Existing capacity exporter: `scripts/export_capacity_table.py` in
the original checkout's retained `deliverables/load-capacity-research`.
The later October 8 captures supersede that research branch's interim note
that the attribute-table root was unresolved. The exporter verifies table
bytes/hash, recovery, 48 runtime events, 23 base/final pairs and eight UI vectors.

Use `tools/run_python_tests.ps1` to run `tools/build_loadout_stats.py` with
`--armor-root <reviewed worktree>`, `--capacity-root <retained research>`,
`--validation-output <new D-build-root receipt>` and
`--output apps/workshop/loadout-stats.json`. The validation output must not exist.
The projection retains source hashes and validation counts in the asset.
It contains 806 armor rows, 99 accessory rows, 150 capacities per style,
502 weight coefficients and three cold rows.

Static native consumer evidence is retained under
`handoff archive evidence/v088-loadout-stats-inspection-20261013/research/`:
base lookup 0x818AEC and derived selector 0x818B68; equipment aggregation
0x2FB2BC (slots 4..12); group dispatcher 0x819400, capacity handler 0x81FA40
and flat toughness handler 0x821BC5. The planner's extra native accessory
slots remain unrepresented and excluded, as in the existing import contract.

Source evidence: 93 focused loadout tests, typecheck/locale audit, a schema-valid
read-only synthetic save-host test, 21 browser interaction/geometry checks,
and independent TypeScript replay of 298 weight plus 310 toughness returns.
Exact candidate/native/standalone receipts are retained in the same delivery
directory after packaging. This is offline/captured evidence, not a new
game-menu boundary test. Rarity-5 full return validation, current-game menu
comparison remain open. Evaluate explicit skill state modifiers individually. No game process
was attached, no native calls were made, and no game/save writes occurred.

## Interaction changes

Click an equipped row to inspect its read-only detail card. Its separate
Replace action opens the existing source picker. Empty slots still open the
picker directly. Cancellation preserves the selected equipment. Scrolls follow
the same interaction. Soul-core picker behavior is preserved. Skill capacity
left unset produces no warning; explicitly configured overflow remains a conflict.

## Remodel eligibility and feedback (October 14)

The owner confirms that +0 armor cannot be remodeled in game. Planner choices
are disabled at +0 and old simulated overrides are ignored without deleting
source records. Positive-plus choices use the full native coefficient tables
and per-stage caps; captured combinations validate the model rather than
limiting selectable combinations. Toughness does not depend on remodel mode
in the reviewed getter. Weight and requirement increments can still truncate
to zero at small positive plus values. Current browser checks replay all six
+20 modes against the existing native anchors and verify disabled +0 controls.
