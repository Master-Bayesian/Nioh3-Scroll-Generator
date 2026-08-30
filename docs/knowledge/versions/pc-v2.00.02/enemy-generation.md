# Enemy roles and branch classes

Status: `native-table` + `native-control-flow`, normal UI path only
(`caller_option = 0`).

## Authoritative tables

The role table is not a handwritten enemy list. It is the native
`auxiliary_enemy_candidate` table captured from parameter-manager offset
`+0xA80`:

- product resource: `nioh3_scroll_editor/data/auxiliary_generation/pc_v2_00_02/resource_v3/tables/auxiliary_enemy_candidate.bin`
- shape: 487 rows × `0x1C` bytes
- SHA-256: `EFB0672B86A5F87D419D752E3B2FD75A51F5182CC1F809705C1627B0D4E9785D`
- independent captures with the same row-blob hash:
  `audit/p1_dynamic/auxiliary_generation_tables_20260828_145055/` and
  `audit/p1_dynamic/enemy_parameter_gate_20260828_231828/`

The mode-to-class table is `special_context.bin`, seven rows × `0x30` bytes,
SHA-256 `E5588CEBA9C7AFB5F251EDDD018FF7DC3CF735C78323E174249AF0432FAEB94C`.

## Candidate-row layout

| Offset | Type | Recovered meaning |
| ---: | --- | --- |
| `+0x04` | `uint32` | Enemy lookup key. |
| `+0x0C` | `float32` | Budget cost. |
| `+0x12` | `uint16` | Scratch special-rule key. |
| `+0x14` | `uint16` | Terrain exclusion mask. |
| `+0x16` | `uint8` | Playthrough bit mask. |
| `+0x18` | `uint8` | Linked-group identifier. |
| `+0x19` | `uint8` | Descriptor selector. |
| `+0x1A` | `uint8` | Native role. |
| `+0x1B` | `uint8` | Terrain discriminator. |

The complete raw row is retained even where a field is not yet named.

## Class selection

RVA `0x10291F0` derives one branch class per scroll. The first `0..9999`
lottery uses optional-multiplier key `0x1E7D`, threshold 2000. Values below the
threshold select class 2; otherwise a second binary draw selects class 1 or
class 0. The natural distribution is therefore approximately class 2 = 20%,
class 1 = 40%, class 0 = 40%, subject to the exact discrete 16-bit RNG stream.

| Mode | Class | Active group budgets |
| ---: | ---: | --- |
| `0x57` | 0 | 4, 4 |
| `0x6F` | 0 | 4, 4, 4 |
| `0x4C` | 1 | 3, 3, 4 |
| `0x7D` | 1 | 3, 3, 4, 4 |
| `0x48` | 2 | 3, 3, 4 |
| `0x8E` | 2 | 3, 3, 4, 4 |
| `0x62` | 2 | 3, 3, 4, 4, 5 |

The complete dispatcher calls exactly one recovered generator:

- class 0: RVA `0x102AB7A`
- class 1: RVA `0x102A259`
- class 2: RVA `0x1029A70`
- shared budget helper: RVA `0x1026FD0`

## Structural role constraints

- Class 0 builds only role-4 and role-5 pools. Its highest active group uses
  role 5 whenever available; lower groups use key `0xCEFC` (threshold 2000) to
  choose role 4 or role 5. Every role-4/role-5 candidate costs 4, so its
  budget-4 profiles contain exactly two or three entries. At most two entries
  can be role-4-only because the highest entry is reserved for role 5.
- Class 1 excludes roles 4 and 5 from ordinary groups. Its highest group uses
  the role-5 pool, and the helper returns after its first accepted entry. A
  class-1 scroll can therefore contain at most one role-5 requirement.
- Class 2 excludes roles 4 and 5 completely and draws ordinary roles 0–3.

Role is a per-candidate-row selection field, not a property of the localized
display name. Twenty-five displayed Chinese names span multiple roles. For
example, different raw keys named `金井半兵卫` occur in roles 4 and 5.

For the normal UI path, the dispatcher contains exactly three recovered branch
classes: 0, 1, and 2. The seven `special_context` mode rows are budget/group
profiles inside those classes, not additional classes. Nonzero
`caller_option` paths remain uncertified.

The [player enemy-combination guide](catalogs/enemy-combinations.md) translates
these raw roles into four documentation families:

- O: roles 0-3;
- A: role 4;
- B: role 5;
- A/B: a display name with distinct role-4 and role-5 candidate rows.

It includes a quick compatibility matrix and all 142 trilingual display-name
entries. Its result is a structural preflight, not a promise that a matching
Seed exists.

## Proved impossible example

The native candidate rows for the requested enemies are:

| Enemy | Lookup key | Candidate row | Role | Cost | Playthrough mask |
| --- | ---: | ---: | ---: | ---: | ---: |
| 一目连 | `0x0006DE91` | 292 | 1 | 3 | `0x1F` |
| 德川国松 | `0x000F1A7F` | 394 | 5 | 4 | `0x1F` |
| 德川庆喜 | `0x00041A50` | 451 | 5 | 4 | `0x1F` |

Therefore `一目连 + 德川国松 + 德川庆喜` is structurally impossible in the
normal generator:

1. class 0 can draw both role-5 Tokugawas but cannot draw role-1 Ichimokuren;
2. class 1 can draw role 1 but permits at most one role-5 enemy;
3. class 2 can draw role 1 but cannot draw role 5.

This is a proof of no solution, not a statement that a finite scan failed to
find one. The executable preflight is in
`nioh3_scroll_editor/auxiliary_feasibility.py`; the regression is in
`test_effect_seed_solver.py`.

## Seed-solving consequence

Enemy generation uses the scoped state `displayed_seed & 0x0FFFFFFF`, but it is
path-dependent: parameter gates, terrain masks, per-group budgets, modulo pool
sizes, linked-group removal, and local RNG states change the number and meaning
of later draws. Fixed-draw modular inversion is not a complete enemy solver.
The safe strategy is:

1. reject structurally impossible role combinations;
2. use cheap mode/terrain constraints as prefilters where valid;
3. exactly replay the recovered branch and accept only the requested ordered
   or unordered enemy semantics.

## Catalog artifacts

- [Player combination guide](catalogs/enemy-combinations.md)
- [Player combination JSON](catalogs/enemy-combinations.json)
- [Player combination CSV](catalogs/enemy-combinations.csv)
- [Human-readable grouped table](catalogs/enemy-roles.md)
- [Complete machine-readable table](catalogs/enemy-roles.json)
- [Complete CSV table](catalogs/enemy-roles.csv)
- deterministic exporter: `tools/export_enemy_role_catalog.py`

The JSON preserves all 487 raw rows, native keys, role, known row fields,
enabled playthroughs, class paths, and native Simplified Chinese, Japanese, and
English names.

## Verification boundary

The class implementations match captured native vectors for all three classes;
the strongest corpus is third-playthrough data. Nonzero `caller_option` paths
remain fail-closed. A future game update requires a new table capture and new
version directory even if an AOB still matches.
