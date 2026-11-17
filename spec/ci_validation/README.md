# CI validation safety model

This finite model checks the scheduling rollout, not the correctness of the tests,
TypeScript resolution, native RNG or a live game. It has two abstract cases and
two shards, PR/integration events, optional inventory/graph/non-imported inputs,
and complete/missing/stale/failed/shadow report samples. It checks safety without
a liveness or eventual-job-success claim; terminal states allow stuttering.

## Source mapping

| Model | Implementation |
| --- | --- |
| `Plan`, `InventoryRequired` | `tools/ci/inventory.mjs` rejects missing manifests/inputs; `plan.mjs` never creates an empty success inventory. |
| `ReferenceIsFull` | `createPlan` always assigns the entire inventory to the reference; `gate.mjs` verifies `execution.mode`, exact selected IDs and shard coverage. |
| Unknown/non-imported/integration full decisions | `shadow.mjs::selectFrontend`: absent history/graph, structural or global inputs fall back to Full; main/merge-group are FullOnly. |
| `AssignmentIsUnique` | `core.mjs::balance` emits unique case assignments; `gate.mjs::sameSet` rejects missing/duplicate assignments. Duration estimates never decide coverage. |
| `Collect`, `Finalize` | `run.mjs` uses explicit Planned/Running/Passed/Failed transitions and writes source/plan-bound receipts; `gate.mjs` rejects incomplete, failed or stale reports. |
| Required shadow presence/equality | PR/manual shadow plans are executed independently; the final gate requires their exact selected set and a successful full reference for each case. |

Source/input identities require a clean committed checkout and Git-normalized
input blobs. Text newline conversion does not change source identity; deliberate
byte-sensitive edits remain dirty and fail before execution. Report/log hashes
continue to bind the actual evidence bytes. The CLI E2E exercises this distinction.

The real CLI E2E retains an owned Git fixture, import-affected Node executions,
positive aggregate controls and deliberately invalid artifacts with exact named
error codes. Actual product suites still run in the full workflow; controlled
fixture receipts are not product acceptance.

Each negative configuration changes one normally-false model constant. The
existing runner requires TLC exit 12 and the exact named invariant; unrelated
failures and parser errors do not count as controls. Run:

```powershell
./tools/check_models.ps1 -Model CIValidation -Evidence <fresh-evidence-directory>
```
