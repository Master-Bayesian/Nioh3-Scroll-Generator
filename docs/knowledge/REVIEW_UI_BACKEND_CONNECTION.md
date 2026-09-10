> Superseded where different by [LEGACY_UI_PARITY_20260909.md](LEGACY_UI_PARITY_20260909.md): 25-result pages, automatic saves, backup management and native early-playthrough search are now connected. Read its acceptance limits first.

# Connected review UI — September 9, 2026

This update supersedes earlier demo-only and wait-for-Figma status for this UI.
The original engineering workbench and Tk entry remain available.

## Launch and ownership

Run `deliverables/frontend-v2/search-ui-demo-v2/start-backend-ui.cmd`.
It builds the current checkout and opens Electron with `NIOH3_REVIEW_UI=1`.
`index.html` in the same output directory remains an isolated browser demo;
opening that file does not connect to Python or access saves/the game.
No packaged release was rebuilt or published for this iteration.

The React screen uses the existing offline SearchController, protected
OperationController and SaveSession. The sandboxed renderer never receives raw
candidate records, arbitrary filesystem access, CE scripts or process addresses.
The main-process CandidateRegistry retains cart records across search pages,
releases removed entries and clears abandoned references on renderer reload.
The cart itself is session-local; reload discards it.

## Connected behavior

- Three-playthrough R3/R4/R5 search, cancellation, next batch (maximum 20),
  sorted results, known-seed preview, effect/auxiliary filters and grace OR choices.
  Other playthrough choices are explicitly rejected by this adapter for now;
  they must not fall back to sample results in Electron.
- Real save selection and inventory, reviewed edits/deletes/backup restoration,
  explicit title-screen confirmation, operation receipts and uncertain-write
  recovery through the existing SaveSession.
- Selected cart subsets prepare and commit one atomic save installation batch.
  Displayed recommended level resolves through Python; -1 transfer count becomes
  uint32 maximum at the boundary. Search candidates remain immutable.
- Live cart addition routes through broker-only materialization and the existing
  sequential live batch adapter. Native stage-one installation and finalized
  R4 preview records remain distinct roles, even when a seed yields equal bytes.
  Interrupted live references block new preparation until status is inspected;
  an unexecuted recovered plan is cancelled before a fresh review.
- Editor temporary enemy/rule/terrain changes use existing runtime override
  operations. Capacity remains read-only because that override is not implemented.
- Closed selected conditions keep their footprint. Hover expands an overlay;
  groups sit side by side, wrap long labels and scroll internally without
  shrinking member controls. Scroll cards reserve all six effect and three rule
  rows, with 13px effects normally and 12px in compact desktop windows.

## Verification in this iteration

- 41 Python checks: review integration, cart batching, frontend contracts and live
  operation/application boundaries; no live game accessed.
- 27 Node checks: existing 26 desktop checks plus broker registry ownership,
  bounded lifetime, duplicate rejection and release. Strict TypeScript passed for
  both the desktop and the separately compiled review entry.
- 22 browser checks: hover/layout (8) plus drag grouping and category tint (14).
  Screenshots cover 1600x1000 and 1366x768. Compact windows permit scrolling the
  results pane so larger type does not clip record rows.
- 12 Electron integration checks use an isolated encrypted synthetic save:
  actual preload/worker inventory read, edit review without writes, explicit
  commit, 20-result real search, cart retention across page replacement, selected
  single-item batch commit, two-record inventory readback and renderer isolation.
- Real Electron known-seed preview and search smoke passed; the unconstrained
  20-result search took approximately 0.5 seconds on this machine. This is one
  bounded observation, not a general benchmark.

Evidence: `connected-verification.json`, `hover-verification.json`,
`group-verification.json`, and the accompanying PNGs in the delivery directory.
Reproduction: `verify-connected.mjs`, `verify-hover.mjs`, `verify-groups.mjs` and
`verify-desktop.mjs` under `apps/search-demo-v2` (browser tests use port 4178).

## Acceptance limits

No user save was modified and no game process was attached in these checks.
The newly connected live batch/editor override UI still needs in-game acceptance;
existing CE configuration and game build identity requirements remain in force.
English/Japanese full UI activation, unsupported playthrough integration and
packaged distribution remain separate work. No generator/RNG/finalizer was
rewritten for this UI connection.
