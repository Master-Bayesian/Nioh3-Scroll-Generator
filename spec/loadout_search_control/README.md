# Search cancellation ownership

| Action or invariant | Production correspondence |
| --- | --- |
| Start/Acknowledge/QueuedCancelRetained | OperationController.run retains an AbortSignal request while submission awaits the actual job ID, then cancels only its own epoch. |
| DeliverCancel | host.rs declares save.loadout_search cancellable; ProtectedJobs.cancel sets its existing atomic flag and CancelRequested state. |
| BeginChunk/EndChunk | CPU candidate boundaries or synchronized CUDA bursts. The observed ABI calls back synchronously before and after a device burst. |
| ObserveCancel/NoWorkRestartAfterObservedCancel | RunControl records a typed stop; run_plan checks that outcome before GPU-error fallback. |
| CleanupAndPublish/CancelConfirmedAfterCleanup | nk_search_observed releases per-search scratch/round allocations and returns before the action reports its terminal outcome. Persistent immutable context caches remain owned by Engine. |
| CancelledReadHasNoRank | run_limited_by_observed returns SearchInterrupted, not a partial Report. ProtectedJobs' publication lock makes accepted read-only cancellation win over a successful action not yet published. |
| ProtectedWriteKeepsOutcome | CompletionPolicy::ActionOutcome preserves actual write outcomes during EOF/shutdown; this cancellation policy applies only to read-only loadout results. |
| DeliverTerminal/CancelledUIHasNoRank | loadoutContextReducer retires a completed late response when the UI already requested cancellation. |

The model has one bounded operation, CPU/card choices and two abstract chunks.
It models cancellation observation, not instantaneous preemption of a running
CUDA kernel or driver allocation. No liveness, absolute latency, WDDM scheduling,
numerical damage, source admission, or gameplay proof is implied. Actual current
Engine/Search/CUDA and native WebView tests supply separate implementation data.
All six deliberate faults must fail the exact named invariant with TLC exit 12.
