# Reviewed optimizer plan retirement

The existing context model checks the binding again at commit. This model also
owns the period between preparation and commit: a changed search input used to
leave a visible, unusable confirmation and block the next search. The renderer
now hides the stale confirmation immediately and retains its exact service
plan ID through asynchronous discard or retry.

| Model action / invariant | Production boundary |
| --- | --- |
| `StartPrepare`, `FinishPrepare` | `LoadoutOptimizer.planWear` checks the result binding before and after `SaveSession.prepareCharacterEdit`; a late reply enters `retiring` with the exact returned ID. The same state handles a no-change plan, so a lost discard acknowledgement retains its retry ownership. |
| `ChangeInputs`, `ReplaceServicePlan`, `VisibleConfirmationOwned` | `pendingIsCurrent` requires the exact service `plan_id` and `canWearLoadout` scope/context/request/input binding before rendering `.character-save-plan`. A failed check transitions `planned` to `retiring`. |
| `CancelPlan`, `RetirePlan`, `DiscardOnlyReviewedPlan` | `discard()` transitions only its local plan to `retiring`. `SaveSession.discardReviewedPlan` checks the exact ID **inside** its serialized operation. A replacement service plan survives a stale discard. |
| `RetireFailed`, `RetryRetire`, `RetirementKeepsIdentity` | Failed discard becomes `retirement_failed` with the same ID. Retry lets the actual operation observer recover its transport before repeating the idempotent discard. No search starts while retirement is unresolved. |
| `CommitPlan`, `CommitOnlyCurrentPlan` | `commit()` accepts only the current planned binding; `SaveSession.commit(planned.planId)` independently requires the exact reviewed service plan and source bytes. |

The finite model explores two preparations, three input generations and one
foreign service plan. It checks safety, including late completion, input
changes, plan replacement and failed retirement/retry. It does not prove UI
liveness, network reliability, damage arithmetic or any save-write result.

The three named controls must exit TLC with code 12 and their exact invariant
failure; parse errors are failures of verification. The model is discovered by
`tools/check_models.ps1` on the host. The declared controls use the digest-pinned TLC jar through that runner.
The production React/SaveSession/OperationController flow control is
`apps/workshop/verify-loadout-product.mjs`. It exercises a late plan after
input changes, a failed discard and retry, and an invalid preview after the
service has already claimed a plan. It also covers read/refresh, search cancel,
double clicks, contradictory scores, unmount, and physical-item undo identity.
Its API is a strict in-memory fixture. Its simulated commit changes fixture
objects only: no worker, game, or save file is opened. `NIOH3_DOM_MODULE` can
select a jsdom DOM-only execution when Chromium cannot run; that is not browser
or WebView2 acceptance. The separate packaged Windows gate must verify those
boundaries. Model success is not a refinement proof of the React implementation.
The old named portable runner and verify-loadout-prepared-wear.mjs were absent
from the fbbac77 archive and must not be cited as supplied executable evidence.
