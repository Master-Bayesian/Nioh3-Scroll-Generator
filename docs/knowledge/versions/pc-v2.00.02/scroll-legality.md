# Scroll legality rules — PC v2.00.02

## Verdicts used by the product

The application must keep three different conclusions separate:

1. **Structurally impossible**: native table and control-flow rules prove that
   no Seed can produce the request. Search must stop immediately and explain
   the conflicting selections.
2. **Structurally possible**: at least one legal native path exists. This is
   permission to search, not proof that the complete conjunction has a Seed.
3. **Exact Seed exists**: complete offline generation of one Seed matches every
   requested effect, value variant, terrain, enemy, and rule. Only this verdict
   may be shown as a result.

Pairwise compatibility is not enough for a request spanning several effects
or subsystems. Every check below is evaluated in the selected record type,
rarity, playthrough, and PC v2.00.02 table set.

## Effect-layout rules

The primary effect consumes one ordinary effect slot. The maximum number of
additional required ordinary effects is:

| Rarity | Required ordinary secondaries | Special constraint |
| ---: | ---: | --- |
| 3 | 3 | The native growth token is not an ordinary secondary. |
| 4 | 4 without a required final Grace; 3 with one | Stage-one slot 5 is finalized before the final effect set is known. |
| 5 | 4 | The single promoted/deep effect slot becomes the primary effect. |

An unconstrained primary means that any selected ordinary effect may occupy
the primary slot. It does not force every selected effect to be a secondary.
Primary alternatives and each user-created “any-of” group are OR constraints;
separate groups and required selections are AND constraints.

Every selected ordinary effect must also pass all native gates:

- its effect row exists;
- the row is enabled for the selected record type;
- its native lottery weight is nonzero for the selected rarity and
  playthrough;
- no two selected effects use the same effect group or intersect either native
  conflict mask;
- the number of selected effects in every native category does not exceed that
  category's capacity for the selected record type and rarity;
- rarity 5 does not place a promoted-only/deep effect in a secondary slot.

The application reports localized effect names and IDs for conflict and
category-capacity failures. A raw category such as `0x03` is retained only as
diagnostic context; it is never the sole player-facing explanation.

## Rarity-4 finalization and Grace

Rarity 4 first produces a stage-one record. The slot-5 Grace draw is only a
candidate: the finalizer may retain it, replace it, or normalize other fields.
Consequently:

- a stage-one Grace interval can be used as a mathematical Seed prefilter;
- a stage-one token must never be displayed or installed as a final effect;
- final Grace and final ordinary-effect requirements must be checked against
  the completed `0xE8` record;
- `0xBABD` is Tsukuyomi's Grace only in final-record context. A stage-one
  `0xBABD` does not guarantee that final result.

## Enemy-combination rules

The normal scroll generator has exactly three recovered branch classes:

- Class 0 draws roles 4 and 5. It can contain two distinct role-5 bosses but
  cannot draw ordinary roles 0–3.
- Class 1 draws ordinary roles and permits at most one role-5 selection from
  its highest group.
- Class 2 draws ordinary roles 0–3 and excludes roles 4 and 5.

Enemy role belongs to a raw candidate row, not to a localized display name.
Some names have separate rows in more than one role. The product therefore
checks every raw-row assignment and every class rather than assigning one role
to the visible name.

If no class can satisfy all required enemies, the combination is proved
impossible. If a class survives, exact replay is still required because
terrain gates, budgets, linked groups, and changing candidate pools are
path-dependent.

## Special-rule rules

At most three ordered special-rule rows can be emitted. Structural preflight
enumerates every allowed order and each initial integer budget in `1..5`, while
enforcing:

- enabled row and nonzero playthrough weight;
- unique rule keys and both native conflict-group references;
- the second-slot opposite-sign rule;
- the third-slot remaining-budget sign and magnitude rule;
- the third-slot maximum-absolute-cost replacement rule;
- the three-output limit.

A requested rule family with “any variant” is satisfied by any legal native
row in that family. Several required rule chips are AND constraints. A pass
means a legal ordering exists; exact Seed replay decides whether that ordering
coexists with all effect, terrain, and enemy requirements.

## Terrain and cross-subsystem intersections

Terrain rows are filtered by record/playthrough context and then reproduced by
the complete auxiliary generator. Terrain, enemies, and rules derive correlated
scoped states from the displayed Seed, so their independently nonempty sets
cannot be multiplied or assumed independent.

A known PC v2.00.02 example passes both enemy and rule structural checks but has
an exact empty full-space intersection. After filtering 6,177,768 primary
survivors, the final requested rule reduces the survivor count from 7 to 0.
This is why the product must distinguish “legal structure” from “matching Seed
exists” and must not add one-off blacklists for reported combinations.

## Required search order

The player-facing search follows this order:

1. Validate record type, rarity, effect rows, slot counts, conflicts, category
   capacities, enemy classes/roles, and special-rule order/budget feasibility.
2. If any proof fails, stop before scanning and show the selected names, IDs,
   capacity or class rule, and the minimum action needed to repair the request.
3. Build a mathematical Seed family only from a parity-backed fixed-draw
   constraint.
4. Stream candidates through complete effect and auxiliary replay; do not wait
   for the whole search to finish before showing accepted results.
5. If the complete finite family is exhausted, report an exact empty
   intersection. If only a bounded cursor range was checked, report progress
   and resume state instead of claiming nonexistence.

## Evidence boundary

These rules apply only to PC v2.00.02 and the captured table hashes. A game
update requires new table captures and parity vectors. Structural preflight is
implemented in `effect_seed_solver.py`, `auxiliary_feasibility.py`, and the
special-rule feasibility functions in `auxiliary_generation.py`. Exact
acceptance uses the full effect and auxiliary generators.
