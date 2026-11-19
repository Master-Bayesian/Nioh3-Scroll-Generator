# Standalone exact search checkpoints

Open corresponds to acquiring the sole writer lock and checking the
request, scene, ordered inventory, backend and scoring binaries.
Calculate and Commit correspond to collecting a batch of exact outcomes
and publishing its cursor and ranking in one completed checkpoint.
Close models normal release or a process crash. Pending calculations are
discarded and the existing atomic prefix remains available.

CoverageMatchesOutcomes maps to checkpoint validation.
CompleteRequiresEveryAnswer maps to ExactCheckpoint::complete. The finite
model includes outside-limit cases in its answered tally, as they have a
definite outcome. ResumeUsesOriginalDomain maps to both digest validations.
Each negative configuration bypasses exactly one of these conditions.
