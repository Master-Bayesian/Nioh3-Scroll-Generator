# Loadout discovery recovery

This finite model maps to `apps/workshop/loadout-discovery-state.ts` and the
navigation/retry selectors used by the native workshop UI. The observed startup
failure was an unsuccessful bundle discovery that hid the optimizer navigation
indefinitely. That failure must remain visible with a retry path. It is distinct
from a successful discovery of an empty bundle list.

| Model action or state | Reducer / UI contract |
| --- | --- |
| `Init`, `Idle` | No request or payload; optimizer navigation is hidden. |
| `Requested(id)` | Only `id > current.requestId` starts `Loading(id)`; it clears previous data and error. Old and duplicate requests return the existing state. |
| `Resolved(id, TRUE)` | Only current `Loading` and matching id accepts a nonempty bundle payload and returns `Ready`; data belongs to that id. |
| `Resolved(id, FALSE)` | The same guard returns `Absent` for a successful empty bundle list, with no error. |
| `Failed(id)` | The same guard returns `Failed`, owning the error under that request id. An error never becomes `Absent`. |
| `issued` | A bounded async environment: retired and completed requests can still reply. It permits stale, duplicate, success, empty and error responses in either order. |
| `NavigationVisible` | Visible in `Loading`, `Ready` and `Failed`; hidden in `Idle` and `Absent`. |
| `RetryVisible` | A failed discovery exposes retry. Retrying is `Requested(new id)` and retires the error. |
| `TerminalStutter` | A generation may finish in Ready/Absent/Failed without more work. Newer requests can still recover. No artificial deadlock is used as a fault. |

The two-request configuration explores a newer request overtaking an older one,
responses after completion, duplicate requests and retries after failure. The
model abstracts bundle payload content into `dataPresent` plus `dataRequestId`,
and error payload ownership into `errorPresent` plus `errorRequestId`. It does not infer
installed bundle availability from a pending or failed request.

`ResultBelongsToCurrentRequest` binds accepted success/empty/error outcomes and
their payload to the latest request. `CurrentRequestIsNewest` preserves monotonic
request ownership. `LoadingClearsPreviousData`, `ReadyOwnsData`,
`AbsentMeansSuccessfulEmpty`, `FailureOwnsError` and
`NoPayloadOutsideTerminalOutcome` distinguish lifecycle payload rules.
`DiscoveryFailureRemainsVisible` requires both navigation and retry for failure.
Separate invariants cover Loading/Ready visibility and Idle/Absent hiding.

## Required exact faults

- `FaultAcceptStale = TRUE` removes request-id equality while retaining the Loading admission
  guard. TLC must exit12 with
  `Invariant ResultBelongsToCurrentRequest is violated`.
- `FaultHideFailure = TRUE` removes Failed from the visible navigation states.
  TLC must exit12 with
  `Invariant DiscoveryFailureRemainsVisible is violated`.

The normal configuration must exit0 and exhaust the finite state graph. A syntax
error, semantic error, unrelated invariant or deadlock is never an accepted
negative control. These checks prove finite model safety; they do not prove
that TypeScript is equivalent, that promises eventually settle, or that the
actual native UI is visible. Reducer execution and actual UI acceptance remain
the implementation owner's separate checks.

## Registry and exact repeat command

`tools/check_models.ps1` discovers `spec/**/*.tla` recursively by basename. No
change to that runner is needed. Select only `-Model LoadoutDiscovery`; colocated
`LoadoutDiscovery.cfg` and `LoadoutDiscovery.negative.json` provide the correct
and named-fault configurations. Add the reducer/selector state coverage to any
root-owned finite-state registry separately; this model does not edit it.

Run `Run-LoadoutDiscovery.ps1 -Evidence <new task/common-conditions/discovery-model directory>`.
It invokes the existing checker with the pinned TLC jar, Idle priority, one
processor/worker, 256 MiB heap and the approved F: build/temp root. It retains
configs, counterexample logs, model controls, source hashes and a repeatable
`DISCOVERY_MODEL_ACCEPTANCE.json`. It does not rerun unchanged large models.
## Reviewed implementation mapping

The actual reducer exports `initialLoadoutDiscoveryState`,
`loadoutDiscoveryReducer` and `loadoutDiscoveryVisible`. Its Requested branch
uses strict `event.requestId > state.requestId`; both completion branches first
require `state.kind === 'Loading'` and matching request id. Resolved uses
`data.bundles.length > 0` to distinguish Ready from Absent. Failed owns its
message. Constructing a fresh Loading object clears both prior payloads.
The visibility switch enumerates all five state kinds explicitly.

`main.tsx` increments `discoveryRequest.current`, dispatches Requested, checks
that ref again before either completion dispatch, and retires callbacks on
effect cleanup. Those outer guards are additional protection; reducer ownership
still stands independently. The optimizer navigation calls
`loadoutDiscoveryVisible`. The selected Failed route renders its Notice and a
button wired to `requestLoadoutDiscovery`.

The model's retry visibility abstracts recovery on the selected optimizer
route; it does not model page selection, visual layout, localization or every
non-failure panel message. Its installed-data content is a presence abstraction,
not a proof of bundle hashes, search results or backend startup performance.
Error payload ownership is modeled even when the message text is empty.
Source hashes in the acceptance receipt identify the reviewed reducer snapshot;
textual guard mapping is not a machine-checked TypeScript/TLA equivalence claim.