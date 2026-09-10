# Cart addition boundaries

## Implemented application primitives

`SaveApplication.prepare_install_many` validates every broker candidate using
the existing single-install policy, then creates one snapshot-bound plan.
Commit delegates to `SaveInstaller.install_many`: one backup/transaction with
independently allocated inventory keys and generation serials. A new optional
expected-source hash is checked inside the existing save lock before backup.

`LiveAddBatch` supports 1-200 distinct broker candidate IDs. It validates every
candidate before preparing the first item and checks capacity before dispatch.
Each insertion gets a freshly inspected plan and its own verified receipt.
An immutable batch plan and exclusive durable claim prevent batch replay.
Child references are written before native dispatch. Cancellation only stops
future items; completed additions are not rolled back. An unknown outcome stops
the sequence. Read status and reconcile the child using the existing operation
recovery API; never retry a claimed batch automatically.

Sequential live additions cannot simply compare each new live inventory to an
unchanged disk save. The optional `previous_operation_id` on prepare must name a
verified predecessor. Its source path/hash, process identity and scheduler owner
must match. Current inventory and native index must exactly match that child's
verified readback. Disk persistence is checked against the original batch
baseline. Unrelated pickup, game restart, save change, index drift or unverified
predecessor causes rejection. A later normal game save establishes persistence.

## UI and remaining integration

The standalone demo cart retains selected sample records across searches and
supports comparison, removal and selection of live/save addition mode. It does
not send writes. These new application primitives are not exposed through the
production protected-worker schema/broker yet. Broker integration must preserve
candidate ownership, per-item recommended level/transfer metadata, preview
approval, durable child IDs and partial/unknown outcomes. The demo's current
cart uses shared displayed-level settings; production cart entries need their
own immutable input metadata.

The editor separates persistable header/effect drafts from temporary enemy,
terrain, capacity and rule drafts. Temporary capacity/free overrides are UI
drafts only; no newly proven arbitrary-memory write path is claimed. Existing
runtime constraints remain authoritative until researched and implemented.

## Evidence

Synthetic tests in `tests/test_cart_batch.py` cover predecessor chaining, unexpected
inventory changes, invalid later candidates, cancellation, unknown execution,
duplicate dispatch protection, unsigned transfer records and source-hash guard.
No game process was attached and no real save was read or modified in this turn.
