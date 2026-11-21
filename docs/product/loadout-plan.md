# Loadout optimizer working plan

Start here when resuming optimizer work. The [product contract and implementation map](required-set-loadout.md) defines the score. This page records the current decision, the evidence we can reuse, and the order of the remaining work.

**Outcome.** A player selects one attack and a weapon type. The App ranks legal outfits from the current character's inventory by final damage under the displayed opponent, hit and buff assumptions. The winner includes the actual slot assignments. The beta permits a required set tier. A weapon type can include several owned weapons.

## Resume checkpoint — 2026-11-21, PC202

Implementation proceeds on `codex/required-set-fastpath`. The last committed numerical baseline is `6fa829c`. The separate integration checkout carries its own app changes. Its November 18 standalone handoff documents an earlier stage. Reconcile the two trees at integration.

| Layer | Verified scope | Open product work |
| --- | --- | --- |
| Character and gear | One Samurai role projected into three weapon types. Six cross-type searches agreed on 6,384 candidate evaluations. Ninja source maps and stats matched three held-out outfits. | Reuse the shared form rules. Revisit a producer when a new candidate or state contradicts an existing witness. |
| Damage arithmetic | The representative single-contact domains answered 6,832 Samurai and 4,520 Ninja candidates across eight weapon types per form. The later Ninja host receipt checks every candidate against original MAIN, CPU and CUDA. | These fixtures exercise numerical paths. Each player-facing named move still needs the correct event sequence. |
| Player selections | The recorded candidate catalog contains eight bound choices across four form/weapon pairs. A catalog identity gives the parameter and damage rows. | Bind verified moves in batches by event mechanism. The in-progress Spiral Thrust compiler has a four-hit witness; current-role ranking and normal App acceptance are pending. |
| Buffs | The 406-row source table and REA results cover source identities and several native contribution rules. Class 0 has same-key strength competition. Deflect supplies a regression control. | Establish which sources each outfit can actually activate together and which contribution survives at each scored hit. The seven Deflect-only experiment bundles were withdrawn from the product path. |
| Delivery | Windows WebView, protected worker and real CUDA ran the recorded searches. The latest Ninja host report records that the full installer was not rebuilt. | Integrate with current app changes, build a fresh local candidate and submit it to final independent acceptance. |

## Work order

Starting finding for item 1. In `crates/nioh3-protected/src/loadout_context.rs`,
the ordinary first-read path currently returns a partial result for a nonempty
status tree and admits the current or untriggered scene. Downstream,
`crates/nioh3-loadout/src/event_program.rs` already distinguishes an active
single-use effect, removal pending a rebuild, and inactivity. Trace the
missing producer into this existing path before adding an activation mode.

1. **Active — trace a candidate's buffs through one search.** Start with the existing source report, `loadout_first_use`, `source_rebuild`, the event program and `Engine::prepare_frame`. For two real candidate outfits and the same selected move, identify what comes from the character, what is rebuilt on equip, what the hypothesis activates, what competes or expires, and what MAIN consumes. Start from a verified native group; include a competing source or an ineligible candidate. Record the first missing producer or the point where production already implements it. That result determines the code change. A difference in score is useful even if the winning outfit stays the same.
2. **Complete the two inputs to the score.** Extend shared candidate-effect rules where that trace shows a gap. In parallel, finish named-move bindings for supported event mechanisms using the existing action registry and cast witnesses. For a new move, identify the variant, owned contacts, physical and elemental channels, and any charge, resource or stop condition that changes its score. Reuse Spiral Thrust as a multi-contact control. List its HP sequence and current-role ranking separately. The trigger button has no role in this work.
3. **Close the player workflow.** Run the normal picker, current-character read, constrained search and ranked gear display. Match native contact results and CPU/CUDA rankings, coverage, ties and slots. Show the assumptions behind a theoretical score. A maximum over a subset of effects must identify the subset. Verify a held-out gear or status change independently of the scorer's prepared inputs.
4. **Integrate and release.** Reconcile the optimizer branch with the current app, rerun affected checks, verify the packaged workflow and measure background search under the normal game-coexistence policy. Use lint/Clippy, explicit state transitions and relevant TLA+ counterexample controls for lifecycle changes. Preserve accepted evidence for unaffected paths.

Combat-style statistics, automatic inventory re-search, multi-move weighting and optimization over unowned generated equipment are deferred. Resume one of these when the product scope changes.

## How the lead keeps the work on track

- On resumption or context compaction, read this checkpoint, inspect the branch and pending diff, then open the evidence for the active item. Files named `accepted` or an isolated `passed=true` need their recorded scope.
- Before pursuing a detail, identify the player selection, candidate input, score, ranking or user-visible operation it could change. Record a counterexample or an unresolved question. Park details without a current dependency.
- Give independent questions to subagents. The lead owns the shared assumptions and the end-to-end result. For reverse engineering, prefer REA and retain the PC build, function or evidence ID, and what its behavior actually establishes.
- When a finding changes the next action, a slice passes, or work is handed over, update the checkpoint and evidence pointer here. Keep historical receipts intact. Consolidate genuinely missing runtime facts only after the offline work and collector rehearsal.

## Local evidence index

The full receipts remain under the host's `F:/Nioh3_r16_build/deliverables/` directory. Paths below are relative to that directory.

| Evidence | Where to resume |
| --- | --- |
| Shared source predictions and search parity | `optimizer-live-samurai-20261120/SHARED_OPTIMIZER_ACCEPTANCE.json` |
| Catalog coverage and original single-contact domains | `optimizer-live-samurai-20261120/ALL_WEAPON_COVERAGE.json` and `ALL_TYPE_NUMERIC_MATRIX.json` in the same directory |
| Later Ninja observation and full numerical host acceptance | `optimizer-live-samurai-20261120/ninja-reference-game-read-cddd918b04054ddd8f2375d674c66479/all-ninja/HOST_ACCEPTANCE.json` |
| Source definitions, native competition and evidence limits | `optimizer-action-buff-20261121/rea-source-rules/README.md` and `RECEIPT.json`; REA batches 119–121 under `martial-event-catalog-20261111/` |
| Isolated buff fault controls | `optimizer-action-buff-20261121/buff-controls/RESULT.json` |
| Four-hit cast work in progress | `optimizer-action-buff-20261121/whole-cast/control-preparation/CAST_INPUTS.json` and `whole-cast/replay_witness.py` under the same task directory |
