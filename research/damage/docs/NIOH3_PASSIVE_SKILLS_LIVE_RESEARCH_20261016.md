# Live configured passives and native text recovery

The owner entered the game, requested complete identification of configured
Common/Samurai/Ninja passives, then supplied three Manage Skills screenshots
and went to sleep. Work continued without requesting more actions or operating
game controls. Root alone owned the live CE session; delegated work was offline.

## Accepted result

Delivery root: F:/Nioh3_r16_build/deliverables/r16-live-passives-20261016/.
Read PLAYER_ENABLED_SKILLS.md and
skill-names/crosswalk-05/PLAYER_ENABLED_NAMED.json.

All25 checked positive-cost skills have exact native keys and Chinese names,
independent screenshot/native-sort joins and source130 cost provenance:

| Category | Checked count | Capacity |
| --- | ---: | ---: |
| Common | 8 | 31/42 |
| Samurai | 8 | 40/41 |
| Ninja | 9 | 40/41 |

The four additional Samurai source-enabled records are0864上段架式,
6121中段架式,E644下段架式 and6842琢技. All have zero cost and no three-record
skill-effect contribution. They remain separate action definitions rather
than being counted as four more configured passives.

Configured, source-eligible, actually emitted/effective, learned and currently
active conditional-status states remain separate. The source observer's
30-second idle window produced zero callbacks. It cannot prove an empty
descriptor override or a complete actual damage-source invocation.

## Data and native bindings

The process is PC2.0.2.0, executable SHA256
e22c4a635e4ec1e27a177b76e27d7f6a637f426c0ed3928b60f5693bc52ae130,
loaded.text SHA256
4cec8fb6ad867417a76df8201c1d4f54172443884910463c1953acad91ae6c29.
Raw captures pin PID/birth/module and player/owner relationships.

- Progress6020:839 complete nodes with stable rereads; initial/final semantic
  key/rank/source-active values match. This is endpoint stability, not history
  or consumer-time atomicity.
- Manager130:1101C0-stride rows, SHA256
  545e426ee6161feaac81d7374d43f5017f537e1605914bd16a8321c54443ed86.
  Native key is u16+18, category i8+AC, passive source kind u8+B9, cost f32+84.
- Manager598:1101 rows of94 bytes, SHA256
  1fcb81c31677a7d6d082c8c9a719e29770cf372639e6c9e3ba7fcd3d55dc971b.
  The live pointer vector independently matches every key/rank row in the flat
  payload. Key is u16+0,rank u32+4,sort identifier u32+8; +8 is not cost.
- Native UI comparator1E82AA4: source-active first, cost descending,A0
  descending,598+8 ascending,rank descending. No unknown final tiebreak is
  required for the25 checked positive-cost rows.
- Native text5A4B04 ->63D9C4 ->63DDA0: full u32 ID lookup through the tree at
  Q(module+45C8450)+1CD0. Locale is a guard, not a per-locale string offset.
  Current observed locale21 and23 loaded packs. Node key+20 and UTF16 pointer
  +28 are bounded/read/reread; no engine function was invoked.

441 requested text IDs were found.274 are nonempty/non-UNUSED. The197 requested
definitions contain166 known names,21 native empty names,8 masked UNUSED and
2 zero IDs. None is silently dropped. All25 configured labels are known.
Native strings preserve U+3000; explicit display conversion uses ASCII space.
Two human screenshot transcriptions were corrected separately: F52D叁 and
Ninja20DE冻霞. Original images/transcriptions are unchanged.

## Effects and important unresolved mechanisms

- A7AA is反攻格挡:2673/value300 -> D+9F0. Its exact native text says successful
  deflection grants a status and repeated successes extend duration. Typed
  getter2366098 calls624518 and loads D+9F0, but its actual guard consumer is
  not established.300 has no admitted time unit.
- 54B5 is格挡好手:A628 presence -> D+2FA. It is a different configured skill.
- 8E80 is灭气追击之法: native definition contributes120 to F4C5. Current source
  aggregate220 and D+35C approximately0.22 are consistent, but the additional
  100 has not been attributed to that skill or an authenticated clean base.
- 6674 is打剑反攻之法. Native text identifies a shuriken after successful
  precise dodge; actual projectile ownership and the historical16damage event
  still require the prepared attribution controls.

Both current selected/core D regions independently retain2FA=1,9F0=300.0,
2E8=0,849=1; outgoing factor is1.0. These are masked region observations,
not a cross-region atomic frame or a causal source-removal experiment.

The HP300 native template independently reproduces the selected Common UI
description. Generic percent templates/labels are captured, but100->10.0%
and the final generated Ninja description are not promoted without formatter
control flow. All known optional empty/DUMMY text stays in its own field.

## Reproduction, update and analytics

RUN_OFFLINE.ps1 uses bundled actual bytes to replay the text lookup, rebuild
197/25 names/costs, execute the compiler's real-corpus/negative CLI controls,
and run the existing migration CLI against bundled original sections. It needs
the prepared repository Python environment, not the game or CE.

Read [the reusable update runbook](../knowledge/COMBAT_RESEARCH_UPDATE_RUNBOOK.md).
The delivery's analytics-versioning/ contains stable semantic IDs,15 initial
native anchors plus6 current discoveries, unapproved candidate profiles,
native table/layout hashes, exact control vectors and update checklists.
Same-version relocation is a control, not acceptance of a future version.

ANALYTICS_CONTRACT.json defines unique perfect-guard episodes per eligible
combat minute, status creation/refresh/end/source identity, monotonic clocks,
configuration/form eligibility and explicit unknown coverage. Historical
4/20 labels are not upgraded into certified counts; no battle-rate or uptime
product is claimed in this collection.

The owned helper was detached and closed after all observers/timers/owned and
global breakpoint inventories were verified empty. GamePID35932 was left
running. First cleanup proof failure and its later independent reconciliation
remain separate artifacts. No game-function calls, register/state patches,
save writes, product compatibility changes or publication occurred.
