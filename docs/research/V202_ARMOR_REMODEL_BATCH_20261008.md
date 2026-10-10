# PC v2.02 armor remodel calculation and validation

## Result and scope

The offline calculator exports weight, seven attribute requirements, and
toughness for six remodel configurations. It reads the existing PC v2.02 item
resource and the reviewed, pointer-free evidence in
[`research/armor_remodel_v202`](../../research/armor_remodel_v202/README.md).
It does not attach to a game, modify a save, or change product behavior.

The initial inventory-candidate export covers **636 parameter IDs and 3,816
configurations** at rarity field 4, level 180, +20, stage 3, and flags 0.
The raw five armor groups contain 806 rows; 170 unique alternate targets are
excluded by default. Catalog membership does not establish obtainability.
There are 67 unnamed candidate IDs; their IDs and calculations are retained.
Four source edges cross armor slots; selection follows the native ID lookup
rather than enforcing an inferred same-slot rule.

Names retain the shipped PC v2.01 localization provenance. Parameters and the
executable identity are PC v2.02:

- Executable SHA-256: `E22C4A635E4EC1E27A177B76E27D7F6A637F426C0ED3928B60F5693BC52AE130`.
- Item table SHA-256: `F680966B0A21AB64EADF31A26E80F83E2EEEAAC1446E305BB87B9E64617B53CE`.
- Item table: 3,362 rows, 416 bytes per row, 8-byte header.

## Recovered numerical paths

The six record-byte pairs are `0/0` (unmodified), `1/0` (strengthened),
`2/0` (thickened), `1/1` (extreme strengthened), `2/2` (extreme
thickened), and `1/2` (strengthened plus thickened).

For rarity >= 3, each byte equal to 1 contributes half the requirement
coefficient; each byte equal to 2 contributes half the weight coefficient.
Rarity below 3 has no contribution. Contributions are truncated before
addition. A zero base requirement stays zero.

Effective + value is clamped by the native stage caps:

| Stage field | 1 | 2 | 3 | 4 | 5 |
| --- | --- | --- | --- | --- | --- |
| Maximum effective + | 5 | 15 | 30 | 45 | 120 |

The native fallback for other stage values is 5. The calculator accepts only
the explicit 1..5 domain. The progression table has 502 rows of 148 bytes:
requirement coefficient at +0x62, weight coefficient at +0x6A.
For +20 at stage 3 these coefficients are 15 and 9.

| Quantity | Getter RVA | Base and calculation |
| --- | --- | --- |
| Weight | 0x818018 | Selected item row +0x98, plus `floor(weight_coefficient * thick_count / 2)`; raw units are tenths. |
| Requirement index 0..6 | 0x2F9E30 | Original row +0x18C + 2*i, plus `floor(requirement_coefficient * strong_count / 2)` if base is nonzero. |
| Toughness, ordinary path | 0x817FD4 | Selected item row +0xFC; no direct remodel-byte contribution. |

Selection at 0x8180AC uses one alternate ID at original row +0x160 when
rarity >= 4 and that ID is nonzero. It is independent of the remodel bytes.
Weight and toughness use the selected row; requirements use the original row.
Rarity >= 5 selects the high requirement byte, except type 0x2D32. This
selector is supported by native control flow but has no live return validation
in this corpus.

Toughness additionally has a branch for record flags & 0x40000. It looks up
the original row's signed byte at +0x183 in a captured map/table. A missing
lookup retains the selected base. A successful lookup computes, in native
float32 operation order:

```text
factor = 0 if flags & 0x200000 else min(level, 180)
candidate = trunc(float32(float32(factor * float32(slope * float32(0.01))) + offset))
toughness = min(selected_base, candidate)
```

The lookup and three numeric rows were captured. The gameplay meaning of
these flags, whether remodeling changes them upstream, and a live return
from this exceptional path remain unresolved. The calculator labels this
branch as unverified; no cold/status-effect semantic is inferred.

## Native and owner validation

The public corpus retains 1,352 individual numerical returns:

- **1,190 paired returns / 2,380 callbacks** from 14 bounded natural UI
  observation phases, covering ninja armor in all five slots and the Aiji
  helmet. All 34 item/mode configurations have all nine fields represented.
- **162 direct queries** against the running game's getters with temporary
  records: three chestplates, six modes, nine fields. This adds 18
  configurations, for **52 configurations / 468 distinct field checks**.
- All 1,352 returns match the calculator. The 15 runs have reviewed cleanup
  evidence; the query allocation was independently observed as MEM_FREE.
  Function bytes and the item table remained unchanged.
- The owner subsequently remodeled the three named chestplates manually in
  the game and reported that displayed weight, requirements, and toughness
  agreed. This is owner-reported visual acceptance, not a screenshot or a
  separately transcribed numeric corpus.

| Chestplate | Base / thickened / extreme thickened weight | Toughness in all six modes |
| --- | --- | --- |
| Thief's Light Armor, 0xC18E | 1.2 / 1.6 / 2.1 | 36 |
| Footsoldier's Medium Armor, 0x35BC | 7.0 / 7.4 / 7.9 | 62 |
| Tatenashi, 0xC288 | 10.4 / 10.8 / 11.3 | 100 |

In all three groups strengthened nonzero requirements increase by 7 and
extreme strengthened requirements increase by 15. Zero requirements remain
zero. Mixed strengthening/thickening combines the +7 requirement and +0.4
weight contributions.

The owner message did not independently restate rarity, level, + value,
stage, or flags. The numeric conditions above identify the linked native
comparison. They must not be treated as newly transcribed owner measurements.

The export's `verification` and `verification_method` columns distinguish
computed-only rows, natural UI getter observation, and native queries on
temporary records. The other **3,764 configurations** are calculated from
recovered functions and tables, not individually observed in-game.

## Deliverables and remaining work

- [Public CSV](../../deliverables/armor-remodel-v202/armor-remodel.csv),
  [validation summary](../../deliverables/armor-remodel-v202/validation.json),
  and [three-series native CSV](../../deliverables/armor-remodel-v202/cross-series-native-values.csv).
- [Offline calculator](../../tools/armor_remodel_batch_v202.py), bounded field
  xref scanner, three passive CE observers, a temporary-record query adapter,
  and a Rust read-only record snapshot example.
- Portable regression tests exercise stage/rarity boundaries, original versus
  selected rows, zero requirements, float32 rounding, corpus corruption,
  ownership, cleanup, and incomplete native calls.

Different + values and rarity 5 need further controlled game validation.
The exceptional toughness path needs a genuine triggering state and return
observation. This research does not approve product integration, inventory
insertion, all-item legality, save persistence, or network behavior.
