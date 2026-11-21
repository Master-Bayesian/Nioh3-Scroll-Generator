# Live equipment seed review

This finite model covers the frontend additions to the live equipment workflow. It treats the seed result and prepared preview as opaque values. It does not represent the engine, file IO, localization, or a new native write protocol.

| Model action or invariant | Implementation |
| --- | --- |
| `ChangeContext`, `StartSearch`, `FinishSearch`, `ResultsAreCurrent` | `LiveEquipmentAdd` constructs a new context for changed fields or save snapshots. `LiveEquipmentSeedSearch.find` checks the captured context and request epoch after each awaited call. |
| `SelectSeed`, `EditManualSeed`, `SelectedSeedIsCurrent`, `SearchSeedRequiresSelection` | A result click records its seed and context. The seed field uses that value only while the context matches. Manual editing clears the association. |
| `Prepare`, `Preview`, `RestoreMalformedRequirements` | `LiveEquipmentAdd.prepare` persists the operation UUID and wanted groups before calling the existing prepare action. `storedOperation` restores a valid UUID and classifies its wanted data independently. A UUID-only legacy record uses the existing manual review path. |
| `Confirm`, `Dispatch`, `ConfirmedPreviewIsReviewable` | The prepared UI and execute handler require valid persisted conditions and a native preview that matches every wanted group. The existing confirmed plan digest still protects dispatch. |
| `Cancel`, `InvalidRecoveryCanCancel` | Status, recovery and cancellation still use the recorded UUID when requirement metadata is malformed. |

The existing [durable protocol model](../live_add/LiveAddOperation.tla) continues to cover at-most-once native dispatch and settlement. The browser fixture exercises the production UI and mocked bridge for both the positive cases and the rejection paths.
