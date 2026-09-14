# PC v2.01 native possessed-enemy capture

These tools collect raw evidence for the game's native possessed-enemy
assignment path. They are read-only observers: they do not write game memory,
call game functions, alter selector state, or launch the game.

This research is no longer limited to a visible Crucible terrain effect. Enemy
candidate generation and possessed-state assignment are separate mechanisms.
An enemy appearing in the generated candidate list is not proof that the game
marked that occurrence as possessed.

## Current control status

- The project owner reports that natural-drop seed `156062997` contains a
  possessed Koroka in the first wave during single-player play, alongside one
  other enemy. This is the current owner-observed positive control.
- The offline auxiliary model predicts Koroka key `0x0008BC34` and Jailor Demon
  key `0x000E6F42` in group 1 for input `(156062997, 3)`. This is a structural
  prediction, not native proof of possession or of the companion's observed
  identity.
- Seed `86872488` is a controlled normal-solo negative and now also has a
  captured one-person-expedition positive. It must not be treated as a
  seed-only positive; session mode and the expanded online task records matter.
- No product filter or inverse-solver claim is authorized from these state
  correlations. Static writer/source recovery and one matching native causal
  validation are complete. The upstream mode/session mechanism remains open.

Run A on 2026-09-12 confirmed that the Koroka candidate alone had
`record+0x8F = 1`; all five other candidates had zero. The owner visibly
confirmed that Koroka was possessed. The older selection mask and
`record+0xEA` were zero for every candidate, so they are not a complete oracle
for this native positive. `+0x8F` is now the leading candidate state field, not
yet a fully established semantic contract.

A later entry missed its pre-entry capture window but was salvaged while the
owner still had the possessed Koroka visible. Two consecutive read-only
post-spawn reads retained the same owner, vector, record identities, and the
same unique Koroka `record+0x8F = 1` result. Some live raw-record bytes changed
between reads, so the salvage is useful state evidence but is not an independent
breakpoint repeat and does not close Gate A.

Formal Run B then captured a fresh mission entry with newly allocated candidate
records. The owner again confirmed that Koroka was visibly possessed, and the
native result repeated exactly: Koroka alone had `record+0x8F = 1`, all other
task candidates had zero, every `record+0xEA` value was zero, and the selection
mask was zero. Run A and Run B therefore pass the repeated-positive state gate
under the shared invincibility-trainer condition. A negative control is next.

The first attempted negative run was misconfigured: the operator confirmed that
the `156062997` scroll had been entered again instead of `86872488`. Its newly
allocated candidate fingerprint matched the two positive runs and the Koroka was
again visibly possessed with `record+0x8F = 1`. Preserve it as a third positive
repeat under a mislabeled raw run ID; it contains no negative-control evidence.
Future controls require an exact seed readback before arming.

The corrected normal-solo `86872488` control matched its six-enemy offline
fingerprint, including Nuppeppo at spawn IDs `0xF3D` and `0xF40`. The owner
confirmed that no enemy was visibly possessed. All six task candidates had
`record+0x8F = 0`, `record+0xEA = 0`, and a zero selection mask. This supplies
the first owner-observed/native-state negative and creates a positive-to-negative
`+0x8F` split. Because the seeds and enemy sets differ, the next stronger control
is the same `86872488` scroll in a one-person online expedition, where the owner
historically expects possession.

That expedition control is now complete. The native task vector contained ten
records, including four Nuppeppo. The owner reviewed the recording and confirmed
one possessed Nuppeppo in wave 2, one ordinary Nuppeppo in wave 4, and one
ordinary Ippon-Datara in wave 3. The only `record+0x8F = 1` record was wave-2
Nuppeppo spawn `0xF3F`; the same wave also contained a non-possessed Nuppeppo.
This same-run, same-wave, same-species split makes `+0x8F` the leading native
possession-state marker across the captured conditions, but does not prove its
writer or causal role.

The expedition also produced a separate scoped One Difficulty correlation:
the only task records with `record+0xE9 = 0` were Ippon-Datara spawn `0xF42`
and Nuppeppo spawn `0xF45`, matching the two same-species pairs where the owner
observed one ordinary occurrence. Preserve this as a candidate field semantic
pending a repeat. Candidate-only summaries omit those records, so analysis must
retain the complete mission-record view as well as the filtered selector view.

The independent expedition repeat retained the same ten task identities at
newly allocated addresses. `record+0x8F` again marked only wave-2 Nuppeppo spawn
`0xF3F`, aligned with the only possessed occurrence. The two `record+0xE9`
zeroes moved to wave-1 record `0xF3D` and wave-3 Ippon-Datara `0xF42`; the owner
observed exactly one ordinary enemy in each of those waves and all wave-4
enemies as One Difficulty. The moving native partition therefore follows the
moving visual One Difficulty assignment across the two runs.

## Safety and identity gates

- The runner requires an already-running `Nioh3.exe` and an explicit PID.
- If more than one CE bridge session is connected, the runner requires an exact
  `--session-id`; it never chooses one silently.
- The runner refuses to retarget a CE session already attached to another PID.
- Before arming, it verifies the attached PID, process name, executable path,
  approved PC v2.01 executable SHA-256, and phase instruction signatures.
- Each phase owns at most four hardware execute breakpoints through
  `research/owned_breakpoint_lifecycle_ce.lua`.
- A phase refuses to arm while any unknown debugger breakpoint exists.
- Existing capture and cleanup files are never overwritten.
- Normal completion, timeout, errors, and interruption all attempt cleanup and
  write separate cleanup evidence. A run is incomplete unless cleanup is
  explicitly verified.
- Raw captures retain every record. Derived analysis must inspect all task
  records before applying the selector/candidate filter.

## Evidence-gated collection order

Run only one phase for each fresh mission entry. Never arm two phases together.

1. Preserve completed runs A and B of seed `156062997`: on two fresh entries,
   the owner-visible Koroka maps to the only candidate with
   `record+0x8F = 1`; cleanup is verified for both runs.
2. Preserve the completed normal-solo negative and same-seed expedition
   positive, including all ten expedition task records and the owner's separate
   visual labels.
3. Preserve the completed independent expedition repeat. Its `+0x8F` partition
   repeated, while its changed `+0xE9` partition matched the changed visual One
   Difficulty assignment exactly.
4. Preserve the integrated Pro result: descriptor `+0x0F` is the static source
   copied into task `+0x8F`; later `+0xE9` selection is separate. The exact
   seed-to-entry state and live producer trace remain open.
5. Preserve Run D as the completed seed `86872488` one-person-expedition
   `assignment-origin` validation. It closes the positive typed copy chain.
6. Use the current mode-upstream Pro handoff before designing another live
   phase. The causal session object and request construction must be identified
   before a breakpoint plan is approved.

`run_possessed_enemy_observer.py` arms one phase, polls the result, saves the
capture JSON, and records cleanup separately. It leaves the existing CE target
attachment intact for the next phase. `dump_possessed_enemy_runtime.py` reads
bounded runtime structures and writes raw files plus a manifest; it is not part
of the initial positive-control gate. `capture_postspawn_snapshot.py` is a
strict salvage tool for a missed window: it accepts a prior same-process
late-mask capture as the owner-address source, rejects possible PID reuse and
identity drift, uses no breakpoint, reads twice, and requires stable owner and
record identities. Its output must remain classified as post-spawn evidence.

Follow `COLLECTION_CHECKLIST.md` for an approved operator sequence. Do not enter
the scroll until a reviewed probe exists and the runner prints `ARMED`.
