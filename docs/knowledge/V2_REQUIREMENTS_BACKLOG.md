# V2 requirements and later research

Updated: 2026-09-07. The user's shared tracking screenshot is reference material;
its priority labels are explicitly non-authoritative.

## Later runtime-add research

Investigate adding a scroll directly to the running game's inventory without
returning to the title screen and without editing the save file externally.
This is a separate RuntimeHost operation, not permission to relax the existing
save/native-generation title-screen gates. Required evidence includes the
actual inventory-add function, ownership/allocation rules, thread/context
requirements, serial allocation, capacity handling, persistence, UI refresh,
reloading and propagation. Do not model this as a save edit with a different
button label or claim that a generated preview proves live insertion.

The current migration keeps runtime ownership separate so this capability can
be added after evidence exists. No speculative addresses or live-add API are
introduced during infrastructure work.

## Screenshot requirements interpreted against current evidence

| Item | Engineering treatment |
| --- | --- |
| Two-level special-rule filtering | Preserve rule family and exact variant/value data in catalogs, allowing future grouped controls for traversal/part/probability, automatic activation/item, companion/NPC, drop category and damage type. Final presentation follows Figma. |
| Possessed Underworld enemies in temporary overrides | Screenshot marks this abandoned. Do not reopen it as a migration prerequisite or invent identities. |
| R3 unfinished-masterpiece search and Grace prediction | Research only until the same-seed R4 relationship and actual R3 reveal are verified. Preserve the distinction between raw growth-result prediction and accepted final Grace prediction. |
| Seed 43723117 reveal replacement | Already repaired; preserve the final-record matching regression. |
| Seed 36526331 intermediate-state rejection | Already repaired; preserve valid unchanged finalization and paired installation semantics. |
| Challenge-count upper limit | PC v2.01 runtime parameter and setter review confirm a global cap of 7; seed-derived capacity is 4–7 and the stored remaining byte is separate. Offline search now accepts `initial_challenge_counts`, including `[7]` for full-capacity candidates. Inventory exposes both capacity and remaining attempts as read-only metadata. Direct remaining-count editing is not enabled. |
| Early-playthrough rarity 3/5 investigation | Research/document freshness; not a demand to add unverified product behavior. |
| Same-name effects with different values appearing together | Local free editing already accepts all seven slots. Search multiplicity needs native evidence and an occurrence-aware query model; do not silently deduplicate such a future requirement into sets. |
| Clean-scene selection | Add only once player-visible clean/polluted semantics are mapped to exact captured terrain results; no guessed negative filter. |
| Default transfer count -1 | Stored field is uint32; requested representation is 4294967295, with observed recipient increment/wrap to zero still to verify. Keep explicit review of the raw value; do not encode a negative uint32. |
| Teaching hints on every section | Supply semantics and metadata independently from layout; use Figma to choose disclosure and grouping. |
| Recommended level default 350 | Read-only displayed-level resolution is implemented: predicted 350 maps to raw 585/586, selecting 585 explicitly. Actual game display acceptance remains pending; defaults are unchanged. See V2_RECOMMENDED_LEVEL_SELECTION.md. |

These requests do not change the evidence boundary of the frozen backend or
authorize writes to a player's live save during engineering tests.

## 2026-09-07 research progress

The user subsequently authorized title-screen experiments. Three isolated native
matrices totaling 172,032 records distinguish fixed-growth R3 source metadata
from unforced construction and separate the R5 header cap from generated effect
count. CE MCP verified an insertion-related candidate call chain read-only;
no live inventory insertion was attempted. See
`deliverables/frontend-v2/EXPERIMENTS_20260907.md` for evidence and remaining
gameplay acceptance. These results do not establish natural-drop or reveal rules.

English/Japanese presentation and locale persistence are implemented independently
of Figma. All 32 Japanese talisman qualifiers were captured from PC v2.01;
five English dummy effects still share one missing placeholder text.
`tools/audit_v2_localization.py` tracks those gaps without
mistaking fallback text for verified translations.

## Loaded-game evidence and provenance follow-up

The user subsequently authorized continued CE-assisted experimentation after
entering the save. Three actual equipment/consumable pickup calls were captured;
no scroll was acquired. The 28 pre-existing scrolls may include prior experiments
and must remain provenance unknown. They establish container/readback identity,
not natural generation or rarity/early-playthrough rules.

See `deliverables/frontend-v2/live-acceptance/20260907T225708Z/SESSION.md` and its
contract review. The candidate insertion routine consumes a preallocated serial,
can overwrite an existing serial lookup on collision, updates acquisition order,
and returns unaccepted remainder. Observed calls run on different game threads.
These findings refine the required experiment; no production live-add endpoint
or arbitrary remote-thread insertion has been enabled.
