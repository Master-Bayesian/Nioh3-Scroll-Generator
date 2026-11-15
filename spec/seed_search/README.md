# Seed backend and committed-window contract

- `Init` / `Select`: `SeedBackendPreference::from_environment`, `SeedBackendPreference::select`, and `Accelerator::selected_backend`. Auto prefers usable CUDA, then the verified DirectCompute seed helper. An explicit backend preference cannot silently select another GPU backend. Only the operation-scoped `ExecutionPolicy::AllowBulkCpu` permits bulk CPU.
- `Dispatch` / `Decide`: one bounded native window in `SearchBackend::collect_page_filtered`. DirectCompute limits its window to one million trials and uses 65,536-item GPU dispatches; auxiliary execution compacts natural seeds first.
- `Publish`: verify every returned seed/cursor, apply the existing certified acceptance, recount an exact prefix when a page fills, then commit the page cursor. Output overflow and GPU failures return errors before a cursor is committed.
- `Cancel` / `Resume`: retain the last decided prefix and resume from its exclusive cursor; cancellation does not commit an unfinished GPU window.
- `Failure`: a device, shader, ABI, artifact identity or capacity failure cannot publish partial output. The automatic explicit-CPU fallback may re-decide the same uncommitted window; strict GPU never authorizes bulk CPU.

`CpuRequiresOptIn`, `CursorIsCommitted`, `PublishedPrefix`, `PublishedOnce`, and `GpuHasCapability` name the implementation boundaries. The normal configuration must pass. The CPU-fault configuration deliberately removes CPU authorization and must violate `CpuRequiresOptIn`; the cursor-fault configuration advances an undecided checkpoint and must violate `CursorIsCommitted`. These finite protocol checks supplement the actual CPU/CUDA/NVIDIA/AMD worker and packaged E2E artifacts; they do not certify shader arithmetic or game legality.
