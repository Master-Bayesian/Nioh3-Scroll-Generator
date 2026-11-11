# Planner Life and Ki

Portable inputs, current-PR reproduction commands and evidence scope: [maintainer handoff](../research/LOADOUT_PLANNER_HANDOFF.md). Historical capture names below identify the accepted archive evidence.

The October 14, 2026 planner consumes the owner-provided PC v2.02 resource
research through a verified projection. Values describe an ordinary
unsynchronized character with context/actor factors equal to one and no
unrepresented player-owned fixed reward. They are not current combat-bar
amounts, erosion caps or a prediction of every encounter correction.

## Calculation

- Character level remains the sum of the seven allocated attributes minus 35.
- Base Life is the level column plus Constitution (体) and Stamina (刚)
  columns. Base Ki is the Heart (心) column. Strength (武) affects derived
  level through point allocation, not the Stamina contribution column.
- Group B1B6 adds flat Life; 7F69 adds flat Ki.
- Group 5D3A uses FA3E at point 1 and the effective merged grade. Its Life
  addition is trunc(f32(f32(percent * f32(0.01)) * StaminaContribution)).
  It does not multiply total Life or use a second parenthetical grade.
- Group B95E activates CBE1 only for a nonnegative signed selector. The gauge
  ratio becomes a float32 point in 0..1000; native linear interpolation and
  truncation produce a flat Ki addition after the ordinary final multiplier
  stage. The projection has only the verified knots 0 and 1000.
- Grade inputs use the existing native max-base plus duplicate increment
  merger, followed by native sample saturation. Flat additions retain the
  existing sum/cap rules.

The consumer reuses current-style effect sources: represented equipment, scroll,
two soul cores, clan choice/rank, selected guardian level/activation, active
set/grace tiers and the existing secondary-guardian mechanism. Inventory,
inactive styles and internal extra records do not become active sources.
Source slot metadata permits Life/Ki armor effects to respect the reviewed
attribute requirements. Unknown gates are bounded; missing numerical inputs
remain pending rather than silently contributing zero.

amritaGaugePercent is optional planner-only state in the existing versioned
draft schema. It accepts 0..100 and is preserved across save import, which has
no verified gauge field. Its input appears only when a relevant effect can
contribute. Blank input shows the empty-to-full Ki interval; it never assumes
a full gauge. Removing the effect hides the input without deleting its value.

## Reproduction and evidence

Run tools/build_loadout_resources.py through tools/run_python_tests.ps1 with
--research-root pointing at the original research checkout, --output pointing
at apps/workshop/loadout-resources.json and --validation-output pointing at a
fresh receipt under the project D: build root.

The exporter reuses reference.load_table(), the qualified Life Reflection
analyzer/live replay and research.native_bonus_curve.NativeBonusCurve. It
verifies the raw attribute table, semantic columns, full native curve/resource
parity and modifier-report knots; it adds no live research capability. Product
data retain executable/table/evidence hashes and only the required columns.

Inputs are retained under the original checkout's
deliverables/player-hp-ki-research-20261014/,
deliverables/life-reflection-stamina-research-20261014/ and
deliverables/player-resource-modifiers-research-20261014/.
Use the corrected Stamina semantics and later clan association; earlier
Strength/Kato wording is superseded by those reports.

Validation: 128 focused TypeScript tests, 24 existing Python reference/evidence
tests, typecheck/locale audit, 4,968 independently generated reference vectors
(450 base, 2,628 Life Reflection, 1,890 gauge), and 40 synthetic browser
interaction/geometry checks across three locales. The retained level-one
negative/positive controls reproduce Life 880/1015 and Ki 960. No new game
control, save write or native/package acceptance is claimed. Per owner scope,
no EXE was built. Browser evidence:
handoff archive evidence/v088-loadout-resources-20261014/.
