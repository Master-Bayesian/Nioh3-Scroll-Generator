# Known-ID settings and preview ownership

The finite keys abstract a seed plus playthrough, rarity, scroll level, displayed
recommended level and transfer override. Search filters and candidate count are
excluded: they do not determine this known-ID action.

- `BeginView` maps to the known-ID click handler's immutable `requested` query.
- `CompleteView` maps to `previewSeed` and `setSubmitted(requested)`: a late
  response describes its captured request, never a newly edited form.
- `ChangeSettings` maps to the ordinary form/input controls.
- `knownIdPreviewState` names not-preview, empty, pending, current and stale
  presentation states. Restoring the same settings makes the existing preview
  current again; changing search-only filters cannot make it stale.
- `UsePreview` maps to `knownIdUseBlocked`, both result action buttons, and the
  guarded `addCurrent` handler. A busy or stale known-ID preview cannot start an
  addition with a different effective target.
- `LeavePreview` keeps the existing search and collection workflows separate.

The normal configuration checks request/display binding and current-only use.
The two declared faults must violate their exact named invariants. The actual
browser/native UI gate records real worker preview requests, delayed responses,
form changes, action guards, three locales, and controlled recovery-panel
geometry. Recovery is a scripted UI fixture; no game or user save is written.
This model does not prove generator arithmetic or native equipment insertion.
