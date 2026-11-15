# Enemy roster presentation

`base` is the known auxiliary roster and `detailed` is the per-occurrence
enemy-state preview. `resolved` means the worker explicitly resolved the roster
context; it does not mean that Possessed or Curse state is certified.

`ReceivePreview` maps to `candidateSample` in `desktop-bridge.ts` and
`enemyDisplayState` in `enemy-preview.ts`. `ScrollCard` renders the exhaustive
detailed, base-only, empty and unknown states. The local sample filter uses the
same state and cannot infer a special-state match from a base roster.

For scroll 141974638 (NG3/R4/level 180), the published worker returns known base
enemies but no detailed occurrences because that state branch is unverified.
The adapter must preserve those base enemies and keep special states unknown.
An empty detailed array is not evidence that the scroll has no enemies.

The three negative controls must violate the exact named invariants. The
production UI E2E uses the reported ID and an actual read-only worker. Explicit
unknown/empty response controls test presentation boundaries only. This model
does not certify native enemy-generation arithmetic, possession, expedition
extras or a game/save write.
