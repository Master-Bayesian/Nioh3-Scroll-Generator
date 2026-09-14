# Native possessed-enemy collection checklist

Use an already-running PC v2.01 game and a connected CE bridge. Do not combine
phases. Replace `<python>`, `<pid>`, `<session-id>`, and `<output-dir>` with the
verified local values.

## Gate A: confirm the native positive

Prepare independent output directories for runs A and B. Existing evidence
paths are never reused.

```powershell
<python> research/possessed_enemy_capture/run_possessed_enemy_observer.py --pid <pid> --session-id <session-id> --phase late-mask --run-id 156062997-sp-a-late-mask --output <output-dir>/run-a/late-mask.json
```

After the runner prints `ARMED`:

1. Enter natural-drop seed `156062997` through the normal single-player UI.
2. Let the first wave finish spawning; do not alter task memory or call the
   selector manually.
3. Confirm visually whether the first-wave Koroka is possessed and record only
   what was actually visible.
4. Wait for the runner to save both `late-mask.json` and
   `late-mask.cleanup.json`.
5. Verify `cleanup_metadata.verified` is `true`. If it is false or absent, stop
   the experiment and preserve the process and evidence for diagnosis.
6. Return through the normal task flow and repeat with run ID
   `156062997-sp-b-late-mask` and a new `run-b` directory.

Run A found the visually possessed Koroka was the only candidate with
`record+0x8F = 1`, while the selection mask and every `record+0xEA` were zero.
For run B, disable the external invincibility tool if practical and record that
condition. The positive-correlation gate passes only when `+0x8F = 1` again
maps uniquely to the visually possessed Koroka and cleanup is verified.
Candidate presence by itself does not pass, and `+0x8F` is not yet a general
semantic claim until a negative control is captured.

If the owner enters only after the observer has timed out, preserve the timeout
as zero-event evidence. Do not restart `late-mask` after the enemy is visible
and call it a normal repeat. If Run A came from the same still-running game
process, the visible state may be salvaged separately:

```powershell
<python> -m research.possessed_enemy_capture.capture_postspawn_snapshot --pid <pid> --port <port> --session-id <session-id> --source-capture <run-a-late-mask.json> --output <new-output-dir>/late-mask.postspawn.json
```

The command validates the same process lifetime, executable, module base,
bounded vector, and default unique Koroka record, then performs two consecutive reads.
For another target, also pass `--expected-candidate-count`,
`--expected-spawn-id`, `--expected-enemy-key`, and `--owner-observation`.
Its result is post-spawn state evidence only. It does not pass Gate A or recover
missed event order, and a fresh pre-entry `late-mask` run remains required.

## Gate B: negative and upstream comparison

After Gate A passes, collect the same `late-mask` phase for an explicitly
observed single-player negative control. Seed `86872488` is available as the
historical controlled negative, but its older online-positive observation must
remain a separate condition.

Use `--stop-on-first-final` for the negative run. This records and stops on a
complete final event even when `record+0x8F`, `record+0xEA`, and the mask are all
zero; without it, a valid all-zero negative would remain armed until timeout.

Collect upstream phases only if their data is needed to explain the first
confirmed positive/negative difference:

```powershell
<python> research/possessed_enemy_capture/run_possessed_enemy_observer.py --pid <pid> --session-id <session-id> --phase mission --run-id <run-id> --output <output-dir>/mission.json
<python> research/possessed_enemy_capture/run_possessed_enemy_observer.py --pid <pid> --session-id <session-id> --phase selector --run-id <run-id> --output <output-dir>/selector.json
<python> research/possessed_enemy_capture/run_possessed_enemy_observer.py --pid <pid> --session-id <session-id> --phase config-draw --run-id <run-id> --output <output-dir>/config-draw.json
<python> research/possessed_enemy_capture/run_possessed_enemy_observer.py --pid <pid> --session-id <session-id> --phase mt-selection --run-id <run-id> --output <output-dir>/mt-selection.json
<python> research/possessed_enemy_capture/run_possessed_enemy_observer.py --pid <pid> --session-id <session-id> --phase pool-removal --run-id <run-id> --output <output-dir>/pool-removal.json
```

For every phase, re-enter from the normal task flow, wait for verified cleanup,
and never reuse an output path. Preserve all timing, candidates, and control
differences even when they contradict the working hypothesis.

The first one-person-expedition run of seed `86872488` is now preserved as an
owner-observed positive. Its ten task records contain eight `record+0xE9 = 1`
records and two zero records; the two zeroes align with the ordinary occurrences
in the third-wave Ippon-Datara pair and fourth-wave Nuppeppo pair. Only the
second-wave Nuppeppo at spawn `0xF3F` has `record+0x8F = 1`, aligned with the
only visibly possessed occurrence.

The independent repeat is also complete. The only `record+0x8F = 1` record was
again wave-2 Nuppeppo spawn `0xF3F`, aligned with the only possessed occurrence.
The two `record+0xE9 = 0` records moved to wave-1 spawn `0xF3D` and wave-3
Ippon-Datara spawn `0xF42`, exactly matching the owner's two ordinary visual
partitions in that run. Wave 4 contained two non-possessed One Difficulty
Nuppeppo and one One Difficulty crab, all at `+0xE9 = 1`. Always summarize all
task records before filtering on `record+0xE9`; the filtered selector view omits
ordinary records.

The `assignment-origin` validation is complete in Run D. No mode-comparison
collector is currently approved. The upstream session object and request
construction must be recovered before another live entry is requested.

The command below is retained only as the validated Run D procedure; do not
repeat it without a new evidence question:

```powershell
<python> research/possessed_enemy_capture/run_possessed_enemy_observer.py --pid <pid> --session-id <session-id> --phase assignment-origin --seed 86872488 --mode-label one-person-expedition --run-id assignment-86872488-expedition-once --output <output-dir>/assignment-origin.json
```

Use the exact current PID and CE session ID. The observer reads the ordered
descriptors, actual LCG trials, descriptor-to-task copy, and persistent manager
link through four signature-gated execute breakpoints. Preserve any failure or
partial trace rather than automatically repeating it. Wait for explicit cleanup
verification before arming another phase.

Validate an assignment-origin capture with:

```powershell
<python> research/possessed_enemy_capture/validate_assignment_origin_capture.py <output-dir>/assignment-origin.json --cleanup <output-dir>/assignment-origin.cleanup.json
```

For the unresolved mode question, use the current Pro handoff named in
`docs/knowledge/CURRENT_HANDOFF.md`. Do not substitute a downstream branch
observer for the missing upstream causal analysis.

Do not infer that the task field named `spawn_id` is the physical world spawn
position. The normal-solo and expedition `0xF3F` records have different enemy
lookup keys. The owner's fixed-position observation is a separate visual fact
until a native object field joins them.

## Manifest discipline

Copy `run_manifest_template.json` into each run directory. Keep these evidence
classes separate:

- owner observations: what was visibly confirmed in the game;
- offline predictions: deterministic project-model output;
- native capture: breakpoint-backed runtime state;
- inference: interpretations still requiring comparison or Pro analysis;
- unknown: fields not yet observed.

Never modify placement, task masks, assigned indices, or `record+0xEA`. Never
call the selector manually and label the result as normal mission-entry behavior.

If the default port is occupied by a Codex-managed backend whose tools are not
callable in the current task, preserve that backend and use the skill's approved
isolated-port bootstrap. Reuse the same CE process between phases: only one CE
process can own the Windows debugger for the game even after its breakpoint
list becomes empty.
