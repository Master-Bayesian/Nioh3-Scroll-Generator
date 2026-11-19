# Prepared-batch ownership and exact packet reuse

This bounded TLA+ model checks the protocol introduced by this patch. It is an
abstract safety model, not a proof that the Rust/C++ source refines the model and
not a proof of CUDA driver behavior or translated arithmetic. The model uses
one immutable scene/handle, up to two batches, three distinct packet keys,
and requests of one to three rows. Two packet keys deliberately hash to the same
bucket, and requests include duplicate rows. Device lanes may finish in any order.

Run the positive model and six deliberate defects with the official TLC jar:

```sh
python project-source/spec/PreparedBatchReuse/check.py \
  --jar /path/to/tla2tools.jar --output /tmp/nioh3-prepared-tla
```

The checker requires the positive model to finish without error. Each negative
control must report the specific expected invariant violation, rather than an
arbitrary syntax/tool error. `TLA_PREPARED_BATCH.json` records the jar/source hash,
commands, exit codes, state counts and per-control log names. Logs include the
counterexample trace for each fault.

## Transition-to-code mapping

| Model transition | Implementation responsibility |
| --- | --- |
| `Begin` | `gpu_prepared_reuse.rs`: `PreparedReuse::execute` looks up the full ordered raw packet and coalesces duplicates; all rows retain a lane mapping. |
| `Stage` | `prepared_workspace.h`: `Workspace::begin` and `reserve` reuse/grow staging only under the handle's exclusive lock after the previous synchronous call has returned. |
| `Submit` | `e2e_main.cu`: the five uploads, prepared kernel, and result download are submitted to the same retained low-priority stream while `Workspace::Submission` is alive. |
| `DeviceStep` | Arbitrary completion order of unique lanes. The model assigns an abstract correct answer, so it makes no claim about binary32 arithmetic or the game's formula. |
| `FailDevice` | Native failure returns an explicit error; `PreparedReuse::fail` clears cached successes and poisons the handle, including for later cache-only requests. |
| `Join` | `Submission::complete` synchronizes; its destructor attempts synchronization on exceptional paths before staging can be reused/freed. Lost-device cleanup is driver-dependent and is outside the model. |
| `CacheOnly` | Exact duplicate packets can expand from the handle-local cache without calling native CUDA. |
| `FinishCache` | Result cardinality/layout validation and `ContactCache::remember`; only complete finite successful component records can be retained. Rust tests additionally exercise bail/nonfinite results, which this model abstracts. |
| `Cancel` | Cancellation checks during candidate preparation and after native completion in `search.rs`; CPU checks in `ExactSweep::advance_controlled`. |
| `Publish` | `ExactSweep::advance_controlled` commits its pending checkpoint/rank only after every result is present and the final cancellation check succeeds. The model treats this check/commit boundary as one publication step. |
| `Abort` | A cancelled/failed pending batch returns without advancing the caller's checkpoint. |
| `Close` | `me_close` resets the prepared workspace before freeing the immutable device scene. The workspace joins before releasing buffers, events, stream and host staging. |

A successful immutable answer can remain internally cached when cancellation is
observed after native return. Such a cache entry is still exact. The cancelled
batch must not update checkpoint coverage or publish a new rank. The model keeps
these two responsibilities separate, matching the implementation.

## Invariants and fault controls

| Invariant | Deliberate defect | Expected effect |
| --- | --- | --- |
| `BufferLivesUntilJoin` | `DestroyInFlight` | Release the workspace while a submission still owns it. |
| `NoReuseBeforeJoin` | `ReuseInFlight` | Overwrite a staging generation before the submitted reader joins. |
| `CachedAnswersExact` | `CacheIncomplete` | Cache a lane before its component record exists. |
| `CompletedAnswersExact` | `HashOnly` | Treat a hash collision as packet equality and reuse the wrong answer. |
| `NoCancelledCommit` | `PublishCancelled` | Publish a batch whose cancellation was observed before the commit. |
| `NoFailedCommit` | `PublishFailed` | Commit a joined failure or partial result. |
| `PublishedOrderAndDuplicatesExact` | Covered by positive and failure paths | Reconstruct every requested row in original order, including duplicates. |

The positive bound is small enough to exhaust; it includes two-batch reuse and
colliding-cache-key scenarios. It does not model arbitrary cache capacity/FIFO
ordering, numeric arithmetic, hardware occupancy, multiple simultaneous CUDA
contexts, native pointers, or low-level weak memory. Those rely on independent
unit tests and retained-host acceptance. Deadlock checking is disabled because
closing/aborting is an intentional terminal state. No liveness claim is made.
