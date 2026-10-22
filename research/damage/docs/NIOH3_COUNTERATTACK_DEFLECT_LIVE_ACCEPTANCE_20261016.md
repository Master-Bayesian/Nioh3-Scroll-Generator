# R16 Counterattack Deflect controlled observations

Series root: F:/Nioh3_r16_build/deliverables/r16-counterattack-live-20261016/.
The successful new OS-only process epoch is PID30880, created
2026-10-19T13:56:37.4064407Z. Evidence timestamps, rather than the series label,
identify each collection epoch. PC2.0.2.0 executable and loaded.text hashes
match the retained native profile.

## Accepted result

Current Samurai owner-action controls associate configured A7AA (反攻格挡),
D9F0=300 and the sustained outgoing factor1.1/status0x0312. One ordinary guard
is neutral; one reported perfect guard gives about5.10..5.20seconds; two while
active give about10.10..10.20seconds. Disabling only A7AA changes D9F0 to0;
two owner-reported successful perfect guards then produce no sampled boost or
target status. Restoring A7AA returns300. Restoration was verified without
requesting a further guard; this is not an on-off-on positive replication.

All seven OS windows retain12064 qualified endpoint samples. One90-second
zero-transition refresh window has unknown overlap with the owner's later
confirmation and is excluded from guard/effect controls. The other controls
and their manual action labels are in CONTROL_MANIFEST.json and RESULTS.md.

The active magnitude is binary32 bits3F8CCCCD; the repeated guard does not
sample a1.21 stack. Current type0 vtable is module+3AA8120. Tree key0x0312
(decimal786), source words0xAB99/0xCC0C, object raw+74=0x01AE, source skillA7AA
and effect group0x2673 remain separate identities.

## Exact timer evidence

In refresh samples601/602, original and reread fields agree:

| f32 field | Before | After |
| --- | ---: | ---: |
| elapsed+10 | 102 | 104 |
| remaining+20 | 207.2999878 | 505.2999878 |
| clamp-bound+24 | 309.2999878 | 507.2999878 |

These sampled values exactly fit new_remaining=old_remaining+300-delta_elapsed,
with delta_elapsed=2, and new_bound=old_remaining+300. Elapsed does not reset.
Elapsed+remaining changes from309.2999878 to609.2999878. Bound+24 is neither
that cumulative total nor a demonstrated hard cap.

Independent whole-series fits are about60.0416 and60.0002 elapsed units per
wall second in these windows. The adjacent-sample median near77.8 is phase/
quantization sensitive, not a unit result. Initial309.2999878 rather than300,
its extra9.3, native timer units and caps beyond two successes remain open.
The arithmetic match with1.031 does not identify a modifier. FD4A's text is
limited to Onmyo item-effect duration and cannot attribute the residual.

## Verification and limits

Read analysis-review/REPORT.md and COMPLETED_CAPTURE_REVIEW.json for independent
raw-byte, clock and configuration checks. The13-case collector E2E covers
positive/rejection paths. Recorded replay reproduces all12071 pure outputs
(12064 samples and7 preflights),351895 exact reads and727932 clock stamps,
including raw regions, rereads, masks and timer fields; three transcript
mutation controls are rejected. These are file-backed algorithm checks, not
native event instrumentation.

OS sampling uses QUERY_LIMITED_INFORMATION|VM_READ, never writes or invokes
game code, and retains non-atomic reads and transition brackets. It does not
certify native perfect-guard/contact episodes, insertion/refresh calls,
continuous allocation identity, unseen transitions or sub-sample statuses.
The exact native trigger path and actual source-call descriptor remain open.

## Incident and final state

Before the successful OS study, an older PID35932 source-arm attempt timed out
before ARMED and the owner reported fatal0xC0000005. Exact cause/substage is
unknown. Old-target cleanup remains unverified after exit. The task-created
CE2452 was separately identified and closed; its later closure does not turn
the failed arm into a pass. This debugger route is suspended. Original failure
and recovery evidence remain in the package and experiment failure ledger.

All successful OS handles closed, the owner restored A7AA, final D9F0=300 and
factor1.0/status absent were observed, and the game was left running. No more
owner actions are requested. No product metric, new-version approval or
release follows from these controls.

The owner rejects repeating fragmented manual experiments for every buff.
Continue with native skill/source/class catalogs and shared lifecycle rules,
then use a few representative mechanism controls. Prepare one compact owner
action batch only for a specific unresolved edge. The prepared Pro task is a
local handback; it has not been externally submitted.

## Native follow-up (2026-10-19)

[The native mechanism record](NIOH3_NATIVE_BUFF_MECHANISMS_20261019.md) now closes
the1AE/312 creation and refresh paths, exact initial309.29998779296875-unit
arithmetic, growing refresh bound, per-key suppression and mixed damage-pool
behavior. It also corrects node+22..27 as padding on the proven insertion route;
historical source_metadata_u16 is not source attribution. Old sealed captures
remain unchanged. That follow-up does not promote the owner-reported actions
here into native-certified contact episodes or certify unconditional seconds.
