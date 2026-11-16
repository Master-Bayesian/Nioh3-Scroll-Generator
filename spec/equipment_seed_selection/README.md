# Equipment seed selection and reviewed preparation

This finite safety model follows the actual equipment seed reducer, the `CandidateBinding` enum, and the existing save preparation transaction. It checks ownership and validation transitions. It does not prove native RNG arithmetic, SHA-256 collision resistance, item legality, game behavior, or a game/save write.

## Explicit state and bounds

`ui.phase` has exactly the reducer's six cases: `idle`, `searching`, `empty`, `results`, `selected`, and `error`. `hostPhase` names the abstract host validation steps: `idle`, `received`, `bound`, `replayed`, `prepared`, and `rejected`. `binding` represents `CandidateBinding::LegacyOrdinary`, `CandidateBinding::Reviewed`, or a rejected request; it is separate from the validation step.

The checked domain contains two request IDs without reuse, two outcomes per successful search, two save identities with distinct progress, three UI modes, one identity change, one queued item, and one host preparation attempt. `condition` has two values and abstracts the generated record inputs, including item, rarity, raw `generation_level`, normalized stored level/plus, and difficulty. `revision` has two values and abstracts other search identity changes, including wanted effects and source/snapshot identity. One identity change may change any combination of these fields. The model bounds a single waiting-list entry; the application's 16-item allocation and interactions between entries are outside this component model.

`Progress` is an opaque token for the actual save-selected difficulty and four progress words. The digest token contains the generation context, progress, and generated record token. It deliberately excludes UI mode, query/snapshot revision, and the independently chosen skill. `Candidate.identity` and `explicitlySelected` are provenance witnesses used by the model, not additional request fields. `frozenQueue` is a comparison witness captured when the entry is queued. `Replay` represents deterministic regeneration against the host's captured save progress; the native algorithm is assumed for this safety abstraction. Ordinary legacy requests therefore regenerate from current save progress and do not claim to preserve an earlier unbound candidate.

The model checks safety only. It has no fairness or eventual-response claim, disables deadlock checking for terminal/bounded states, and assumes a supported item whose modeled generator returns a valid nonempty record. Missing resources, empty native generation, inventory limits, key/serial allocation, commit receipts, and disk rollback are covered elsewhere.

## Transition and guard mapping

| Model transition or invariant | Implementation boundary |
| --- | --- |
| `Start`, `Complete`, `Select`; `StaleResultRejected` | [`equipment-seed-state.ts`](../../apps/workshop/equipment-seed-state.ts) declares the six phases and rejects completion/selection with the wrong identity, request ID, phase, or candidate digest. [`CharacterEditor.tsx`](../../apps/workshop/CharacterEditor.tsx), `seedSearchIdentity`, `currentSeedSearchIdentity`, `seedRequestId`, and `findSeeds`, also reject stale successes and errors before dispatch. The identity includes save ID, source and snapshot digests/ID, UI mode/section/legal mode, hell route, generation inputs, progress, and wanted IDs. |
| `ChangeIdentity` | The current identity masks the previous response immediately; the invalidation effect increments the request token and dispatches `invalidate`. Mode, source, generation conditions, and wanted selections cannot resurrect a prior result. `revision` changes do not alter the model's effect digest. |
| `EffectSearchIndependentOfSkill`, `ChangeSkill` | `findSeeds` sends no martial skill and includes no skill in its identity. The skill selector updates only the draft skill. The native search response lists eligible skills separately. A skill may be pending while effect results are searched and selected. |
| `QueueRequiresExplicitSelection`, `HellSkillRequiredBeforeQueue` | `selectedOutcome` exists only in the reducer's `selected` phase. `addSeeded` requires that outcome, a current legal seed result, and membership of the chosen hell skill in the response's eligible list. The queue action is disabled while selection/skill is missing. |
| `QueueIsFrozen` | `addSeeded` copies every candidate generation field from the accepted response and selected outcome, clones progress and the complete outcome, and copies the chosen skill into a new request. The queue never reads a subsequent draft to prepare its entry. `planQueue` passes the stored request. [`SaveSession.prepareCharacterEdit`](../../apps/desktop/src/save-session.ts) clones it again before preparation. |
| `SubmitQueued`, `Bind`; `CompleteReviewedBinding`, `ReviewedProgressMatchesSnapshot`, `ReviewedContextMatchesRoute` | [`equipment_seeds.rs`](../../crates/nioh3-protected/src/equipment_seeds.rs), `candidate_binding`, treats any supplied binding field as a reviewed request: digest, context, progress and raw generation level must all be valid. Context must match the hell route, and progress must match `state_from_save`. Partial bindings reject. |
| `SubmitLegacy`; `HellRequiresReviewedBinding`, `LegacyOrdinaryIsAccepted` | `candidate_binding` explicitly accepts a request with no binding fields only as `CandidateBinding::LegacyOrdinary`. An unbound hell request rejects. This compatibility path does not bypass the reviewed branch's validation. |
| `ReplayRecord`; `ReviewedDigestMatchesReplay`, `HostHellSkillIsEligible` | `generated_record` rebuilds the ordinary/hell effect candidate using the actual save context, raw generation level, seed, and plus. The reviewed branch checks stored level, stored plus, and the regenerated digest. It then validates the independent hell skill against the item's final stored level and builds the hell record with that exact skill. |
| `Plan`; `ReviewedPlanPreservesCandidate` | [`save_app.rs`](../../crates/nioh3-protected/src/save_app.rs), `prepare_character_edit`, reads generation progress from the captured save, calls `state_from_save` and `generated_record`, and passes the generated record to `build_generated_equipment_record`. Its existing transaction allocates inventory identity and calls `plan_character_edit` with the source hash. The prepared output preserves the verified candidate and chosen skill. No commit action exists in this component model. |

The source reload guard was audited separately: `automaticCharacterLoad` stores the last automatic-read identity for per-source deduplication; `currentSeedSearchIdentity` and `seedRequestId` are ownership tokens. None stores a hidden `busy`, `ready`, or terminal phase. The pre-existing shared `run` busy lifecycle remains outside this candidate reducer. The source load checks the current selected save, snapshot ID and source digest before adopting a response; changing the source triggers a fresh character read. This audit found no additional async phase introduced by these identity refs. The model does not claim to model the whole character loading workflow.

## Deliberate faults

Every fault is normally `FALSE`. The adjacent `.negative.json` flips exactly one named constant and declares the exact required invariant. `tools/check_models.ps1` requires TLC exit code **12** and that exact invariant error; parse/runtime failures, an initial-state-only message, or a different violated invariant do not pass a control.

| Control | Deliberate change | Required invariant |
| --- | --- | --- |
| `accept-stale-completion` | Install a pending response without the identity/request/phase guard. | `StaleResultRejected` |
| `queue-without-selection` | Queue an outcome directly from `results`. | `QueueRequiresExplicitSelection` |
| `queue-ineligible-hell-skill` | Queue a hell candidate without an eligible explicit skill. | `HellSkillRequiredBeforeQueue` |
| `mutate-frozen-queue` | Update a queued candidate or skill after a later draft change. | `QueueIsFrozen` |
| `skip-progress-binding` | Accept reviewed progress that differs from the captured save. | `ReviewedProgressMatchesSnapshot` |
| `skip-context-binding` | Accept a context belonging to the other generation route. | `ReviewedContextMatchesRoute` |
| `skip-replay-digest` | Accept a digest that differs from the regenerated record token. | `ReviewedDigestMatchesReplay` |
| `allow-unbound-hell` | Treat an unbound hell request as legacy ordinary. | `HellRequiresReviewedBinding` |
| `reject-legacy-ordinary` | Reject the supported unbound ordinary compatibility branch. | `LegacyOrdinaryIsAccepted` |
| `accept-partial-binding` | Accept a partial binding as reviewed. | `CompleteReviewedBinding` |
| `require-skill-for-effect-search` | Require an eligible skill for a subsequent hell effect search. The first search stays available so TLC emits a transition counterexample rather than an initial-state-only message. | `EffectSearchIndependentOfSkill` |
| `skip-host-skill-eligibility` | Permit a tampered ineligible skill through host replay. | `HostHellSkillIsEligible` |
| `change-skill-in-plan` | Replace the verified submitted skill while constructing the plan. | `ReviewedPlanPreservesCandidate` |

## Repeatable checks and evidence

Run only this component and all its declared controls with PowerShell 7:

```powershell
pwsh -NoProfile -File tools/check_models.ps1 `
  -Model EquipmentSeedSelection `
  -Java 'D:/Nioh3_v080_deliverables/tools/java/jdk-21.0.13+9-jre/bin/java.exe' `
  -Evidence 'D:/Nioh3_v080_deliverables/deliverables/hell-weapon-seed-search-20261116/models-component'
```

The runner retains the exact correct/faulty configurations, TLC logs, and `model-controls.json` under that evidence directory. The checked component run passed the normal model with **94,184 distinct states**, depth 15, and zero states left on the queue. All **13 deliberate faults** passed their controls with exit code 12 and the exact declared invariant. The evidence directory also retains a `source/` snapshot of these four model files and a SHA-256 artifact manifest. The repository's normal full-model command discovers this module automatically.

The real production-bundle browser E2E is [`verify-hell-equipment-seeds.mjs`](../../tools/ui-audit/verify-hell-equipment-seeds.mjs). Its accepted artifact is `D:/Nioh3_v080_deliverables/deliverables/hell-weapon-seed-search-20261116/browser/acceptance/verify-hell-equipment-seeds.json`, SHA-256 `414b619c2172415e47758621b0ebd8811b1bb750da94e844554be939b9f5e218`. It records **129 passing checks across zh-CN, en-US and ja-JP**, nine hashed screenshots, raw search parameters/outcomes, frozen queue entries, and the exact prepared request and preview. Named checks exercise explicit candidate selection, effect-only search before skill selection, independent skill changes, immutable queue entries after all draft changes, late success/error rejection, source/snapshot and mode invalidation, and a newly selected save with distinct progress. Its preparation bridge preserves all payload fields and forbids game/save write dispatch. It is actual browser execution against the production UI with a synthetic bridge, not native generator or game-write acceptance.

The native generator and host transaction E2E remain separate evidence owned by the root task. This model does not substitute for them or extend the distinct live equipment-add workflow.
