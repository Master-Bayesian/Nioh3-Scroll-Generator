# Planner armor requirements

Portable inputs, current-PR reproduction commands and evidence scope: [maintainer handoff](../research/LOADOUT_PLANNER_HANDOFF.md). Historical capture names below identify the accepted archive evidence.

`apps/workshop/loadout-armor-requirements.json` is a read-only numeric
projection of the already completed PC v2.02 armor remodel research. It holds
806 original armor rows (including internal parameter rows), 502 progression
coefficients, stage caps and hashes. No alternate-row substitution is used for
requirements. Rarity 0-4 selects the low byte; rarity 5 the high byte. At rarity
3 and above each strengthened remodel byte contributes half the progression
coefficient, truncated after summing, only to nonzero requirements.

Reproduction uses `tools/build_loadout_armor_requirements.py` through
`tools/run_python_tests.ps1`, with `--research-root` pointing to the existing
`armor-remodel-batch-pr` checkout and `--output` pointing to the asset. It
imports `tools/armor_remodel_batch_v202.py`, validates its approved public
evidence/hashes and replays the existing 1352 native returns before projection.
Research source: `docs/research/V202_ARMOR_REMODEL_BATCH_20261008.md` and
`research/armor_remodel_v202/` in that checkout. This consumer does not introduce
a second reverse-engineering implementation or attach to the game.

`save.character` exposes raw remodel bytes at equipment offsets 0x31/0x32.
These bytes and audit/inventory metadata survive planner selection but are
deliberately absent from the existing N3E1 wire format. The known six modes
are 0/0, 1/0, 2/0, 1/1, 2/2 and 1/2. Unknown native bytes fail pending. Missing
mode data is bounded over these modes. The stage source is now confirmed:
native 0x1111850 reads [[image+0x47514D0]+8]+0x45. Serializers 0x1050095
and 0x1066C3C associate that same +0x45 byte with hash 0xA6C2D359, the
already parsed current-difficulty field. Its five caps are 5, 15, 30, 45 and
120. The paired save provides the planner stage; missing legacy/code snapshots
still span the supported caps. The owner-authorized remodel selector uses separate
per-slot/per-style simulation overrides; source metadata remains read-only.

Below a minimum requirement is certainly unmet; within a nontrivial range is
pending; at or above its maximum meets this modeled attribute condition.
Unmodified/thickened armor, low rarity, +0 and several other inputs can be
resolved exactly without stage. This check does not remove configured effects
or infer equipment obtainability, defense, attack or combat effectiveness.
The rarity-5 selector has native control-flow/table evidence but no live return
validation. The model corpus validates 52 configurations, not all 806 rows.

Seven profound identities are matched through native effect groups, including
scroll and set contributions. Their required corresponding attribute is 150
by owner confirmation. Skill effects remain outside the aggregate as before.

October 14 source integration: saved equipment selection and full configuration
import establish one shared character remodelStage for both styles. Equipment
source snapshots also retain remodel_stage, which survives local persistence
but remains absent from N3E1. Current stage overrides older item snapshots
when calculating totals, details and requirement warnings. Re-read the paired
save to refresh old drafts that lack stage metadata.

Native bytes, typed field excerpts and full 1,352-return replay are retained
under handoff archive evidence/v088-remodel-stage-20261014/.
Only stage 3 has retained numerical observations; the field mapping and
five-stage cap switch have native control-flow evidence.
