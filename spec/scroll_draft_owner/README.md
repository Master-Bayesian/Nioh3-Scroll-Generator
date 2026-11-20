# Scroll draft ownership

The modeled source combines save identity, inventory snapshot and record.
The implementation also checks the source digest and record slot. All source
choices intentionally share the scroll seed and slot.

`ChangeSource` is a SaveSession selection or inventory replacement.
`RenderSource` is the Editor effect that invalidates the previous source
and its cached live overlay. `Prepare` maps to requireCurrentRecord in the
editor and the exact snapshot check inside SaveSession.prepare. A late
view can stay on screen until React renders the replacement, so the RPC
guard compares against the current store.

`OnlyReviewedSourceCanBeDispatched` applies even between the source change
and the render. The faulty same-seed fallback permits a stale dispatch.
