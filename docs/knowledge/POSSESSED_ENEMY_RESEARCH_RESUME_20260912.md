# PC v2.01 native possessed-enemy research resume

Status: **active bounded evidence collection**

Created: 2026-09-12

## Decision

Resume possessed-enemy research with natural-drop seed `156062997` as a new
single-player positive control. The missing-positive-control condition in
`CRUCIBLE_POSSESSED_RESEARCH_FREEZE_20260911.md` is satisfied at the owner
observation level, so a bounded native confirmation experiment is justified.

This does not overturn the freeze's technical conclusion. Seed `86872488`
remains invalid as a seed-only positive, candidate presence remains distinct
from possession, and no inverse solver or product filter is established.

The research question is now native possessed-enemy assignment rather than a
terrain-specific "Crucible" feature. The current scroll has no known visible
Crucible terrain marker in the offline model, yet the owner reports a possessed
enemy. The terrain relationship is therefore an open question, not an input
assumption.

## Evidence state before capture

### Confirmed project facts

- The approved runtime target is PC v2.01. The prepared late-mask phase has four
  signature-gated hardware execute sites and no game-memory write API.
- Offline auxiliary generation for `(156062997, 3)` produces Koroka key
  `0x0008BC34` and Jailor Demon key `0x000E6F42` in group 1.
- The runner now exposes the late-mask phase, requires unambiguous CE session
  selection, validates exact executable identity, refuses evidence overwrite,
  and records cleanup on normal and exceptional exits.

### Owner observations

- Seed `156062997` came from an original game drop.
- In single-player, a possessed Koroka appears in the first wave together with
  one other enemy.
- The scroll can be entered repeatedly for controlled capture.

### Inferences

- The offline Koroka candidate is the likely native record that should carry
  the final possessed flag. This must be checked against the captured enemy key
  and record state.
- The second first-wave enemy may be the offline-predicted Jailor Demon. The
  owner has not yet identified it for the research record.

### Unknowns

- Whether the final possessed flag repeats across fresh single-player entries.
- Which selection-mask bit, assigned index, configuration, RNG state, or other
  late input causes the positive assignment.
- Whether terrain, session, mission-instance, player, or progression state is
  part of the assignment contract.

## Staged experiment

### Gate A: native positive confirmation

1. Validate the live PID, process name, executable path and SHA-256, CE session,
   module identity, phase signatures, and empty pre-existing breakpoint set.
2. Arm only the four-site `late-mask` phase before normal mission entry.
3. Capture seed `156062997` single-player run A through the first wave.
4. Record the owner's visual observation separately from runtime evidence.
5. Compare the complete final records without assuming which field is the
   possession oracle. Run A established `record+0x8F` as the leading field and
   disproved `record+0xEA` as a complete oracle for this case.
6. Verify cleanup from the file-backed cleanup result.
7. Repeat as independent run B. Use a restart run C only if A and B disagree.

Stop if the native positive is absent, the positive record is not Koroka, the
target identity changes, any signature fails, or cleanup is unverified.

### Gate B: negative comparison

After Gate A passes, capture the same late-mask phase for an explicitly observed
single-player negative control. The preserved `86872488` single-player runs can
support comparison, but the new run should use the current collector and target
identity contract when practical.

Compare complete records, assigned indices, selection-mask bytes, final flags,
event order, and timing state. Do not explain a difference merely because it is
correlated with the positive.

### Gate C: upstream isolation

Collect `mission`, `selector`, `config-draw`, `mt-selection`, or `pool-removal`
only after the late positive/negative split is demonstrated. Each phase gets a
fresh normal mission entry and verified cleanup. Do not arm phases together.

## Analysis and delivery boundary

Codex owns bounded capture, reproducible tooling, evidence grading, control
comparison, and package verification. Open-ended causal reconstruction, PRNG
derivation, selector design, and inverse-search design belong in a new
self-contained Pro handoff under `deliverables/`.

The older
`Nioh3_PC_v2.01_Crucible_Possessed_Enemy_Materials_20260910_v2` package is
historical evidence only. Its seed `86872488` positive premise is superseded by
the 2026-09-11 freeze. New evidence must go into a new package that states this
contradiction explicitly and includes a ZIP plus an external SHA-256 digest.

No game-memory write, game-function call, product filter, or release claim is
authorized by this resume decision.

## Run A result and revised gate

Read-only run `156062997-sp-a-late-mask-20260912` captured the correct Koroka
candidate (`0x8BC34`, spawn `0xF40`, assigned index `0`) while the owner visibly
confirmed it was possessed in wave 1. The external invincibility tool was
enabled to hold the wave and is recorded as a potential confound.

The older 12-byte selection mask and every captured `record+0xEA` value were
zero. However, full-record comparison found that Koroka alone had
`record+0x8F = 1`; the other five candidates all had zero. This establishes a
native field correlated with the visual positive in run A and disproves the old
mask/`+0xEA` pair as a complete oracle for this case. It does not yet prove the
general meaning or generation rule for `+0x8F`.

The first cleanup snapshot was taken before CE's deferred removal confirmation
and was correctly marked unverified. A separate owner-scoped recheck confirmed
the probe inactive, cleanup pending false, no owned breakpoints, and an empty
global breakpoint list. The runner now performs bounded cleanup polling so
future normal and interrupted runs do not mistake the first deferred snapshot
for final cleanup.

Gate A is revised: repeat the same seed once through normal single-player entry,
preferably with invincibility disabled, and require `+0x8F = 1` to map uniquely
to the visibly possessed Koroka. Then capture a visible negative control before
promoting `+0x8F` from a correlation to a candidate native state contract.

## Post-spawn salvage after the missed Run B window

The next entry began after the armed Run B window had already timed out, so it
cannot be represented as a successful breakpoint capture. While the owner kept
the possessed Koroka visible, a new no-breakpoint salvage collector used the
Run A owner address only after confirming the same PID lifetime, executable,
module base, vector bounds, task record identities, and unique Koroka identity.
It then read the complete vector twice, 250 ms apart.

Both reads had a stable owner, vector, and record identity set. The six task
candidates again had exactly one `record+0x8F = 1`: Koroka `0x8BC34`, spawn
`0xF40`, assigned index `0`. All six had `record+0xEA = 0`, and the 12-byte mask
remained zero. Raw records were not byte-stable because several live-state bytes
changed on the active Koroka and one other task candidate; the candidate identity
and the three tracked state fields remained stable.

This is a second owner-observed/native-state correlation in the same process,
not an independent pre-entry control-flow repeat. Gate A therefore remains open.
The next useful experiment is still a freshly armed `late-mask` run before entry,
preferably with the external invincibility tool disabled.

## Formal Run B result

Formal read-only Run B `156062997-sp-b5-late-mask-20260912` was armed before a
fresh normal single-player entry. It captured a newly allocated set of six task
candidate records and stopped at the final late-mask event. The owner separately
confirmed that the first-wave Koroka was visibly possessed.

The native state repeated Run A exactly at the compared semantic fields: Koroka
`0x8BC34`, spawn `0xF40`, assigned index `0`, was the only task candidate with
`record+0x8F = 1`; every task candidate had `record+0xEA = 0`; and the 12-byte
selection mask was zero. The Run A and Run B record addresses differ, confirming
that the comparison is not merely a second read of the old record allocation.
The first cleanup snapshot was pending, and the runner released the bridge and
reconnected to the same CE session until it verified the probe inactive, no
owned breakpoints, and an empty global breakpoint list.

Gate A now passes for repeated positive native state under the shared condition
that the owner used external invincibility to hold wave 1. This establishes
`record+0x8F` as a repeated marker for this positive condition, not its general
semantic or cause. Gate B, an owner-observed single-player negative captured by
the same collector, is now the next required experiment. Do not collect upstream
phases until the positive/negative state split is known.

## Misconfigured first negative attempt

The first run armed as the intended `86872488` negative did not enter that
scroll. After capture, the owner confirmed that the previous `156062997` scroll
had been entered again. The native fingerprint independently agrees: all six
candidate semantics and assigned indices match the positive runs, newly
allocated record addresses prove a fresh entry, Koroka alone has
`record+0x8F = 1`, and the owner again saw its possessed effect.

This run is a third positive repeat, not a negative. Its raw run ID and original
directory remain unchanged for evidence integrity, with an explicit mismatch
manifest recording intended versus actual identity. The owner-scoped cleanup
recheck verified no remaining owned or global breakpoints. Before the next
control, the operator must read back the exact displayed seed `86872488`; an
abstract "negative ready" response is insufficient.

## Corrected normal-solo negative result

After the owner read back exact seed `86872488`, the corrected negative was
captured from a fresh normal single-player entry. Its six-enemy native
fingerprint matches the existing offline generation, including Nuppeppo at
spawn IDs `0xF3D` and `0xF40`, and is distinct from the `156062997` positive
fingerprint. The owner progressed through the run and confirmed that no enemy
had the possessed visual effect.

All six task candidates had `record+0x8F = 0` and `record+0xEA = 0`; the 12-byte
selection mask was also zero. Against formal positive Run B, the deterministic
comparison therefore shows a one-to-zero `record+0x8F` split aligned with the
separate visual labels. Cleanup initially remained pending after the runner's
rapid reconnect limit, then one owner-scoped recheck verified no owned or global
breakpoints. The runner now allows a longer bounded GUI-thread interval and five
reconnect cycles; that change is statically tested but has not yet completed a
new live capture.

This cross-seed negative strengthens `record+0x8F` as a state-marker candidate,
but seed and enemy identity remain confounded. The highest-value next control is
the same `86872488` scroll in a one-person online expedition. Count it as an
online positive only if the owner actually observes a possessed occurrence. A
same-seed candidate moving from `0` to `1` with the visual state would rule out
the simplest seed-fixed and enemy-fixed explanations before upstream tracing.

## Same-seed one-person expedition result

The owner read back seed `86872488` and entered a one-person expedition while
the late-mask collector was armed. Online mode produced ten task records rather
than the six normal-solo records; this enemy-count expansion is expected game
behavior and is not itself a research finding. The owner later reviewed the run
recording and confirmed four Nuppeppo occurrences: both second-wave occurrences
were One Difficulty, exactly one of them was visibly possessed, and exactly one
of the two fourth-wave occurrences was ordinary. In the third wave, exactly one
of the paired Ippon-Datara occurrences was ordinary.

The native final event contains exactly one `record+0x8F = 1`: Nuppeppo spawn
`0xF3F` in wave 2. The other second-wave Nuppeppo, both fourth-wave Nuppeppo,
and every other task record have zero. This supplies a same-run, same-wave,
same-species positive/negative partition in addition to the earlier cross-seed
controls. It substantially strengthens `record+0x8F` as a native possession
state marker across the observed Koroka single-player and Nuppeppo expedition
conditions. It still does not establish whether the field is the cause, a
generated result, or a copied downstream marker.

The run also exposes a separate One Difficulty correlation. Eight task records
have `record+0xE9 = 1`; the only two zero records are Ippon-Datara spawn `0xF42`
in wave 3 and Nuppeppo spawn `0xF45` in wave 4, exactly the two same-species
pairs in which the owner observed one ordinary occurrence. The wave/species/count
partition supports inferred record mapping without requiring a left/right
visual identity. Record `+0xE9` is therefore a candidate One Difficulty state or
eligibility field for this run, not yet a general semantic contract.

Every `record+0xEA` value and the 12-byte selection mask remained zero. Cleanup
was initially pending, then the runner's longer bounded GUI-thread retry path
verified the probe inactive with no owned or global breakpoints after four
reconnect cycles. The raw capture is preserved with all ten task records; the
original candidate-only summary is retained with its limitation, and a v2
summary adds the complete mission-record view.

Authoritative local evidence is indexed by the
[expedition run manifest](../../audit/possessed_enemy_capture/86872488/20260912-one-person-expedition-a/run_manifest.json);
the [complete-record v2 summary](../../audit/possessed_enemy_capture/86872488/20260912-one-person-expedition-a/late-mask.summary.v2.json)
is derived from the immutable raw capture and does not replace it.

The planned independent repeat is completed below. It retained all task records
and showed why a field's native partition must be compared with the new run's
visual partition rather than assumed to remain fixed across entries.

## Independent expedition repeat and field separation

Run B used the same seed and one-person expedition mode in a fresh game-process
lifetime. Its ten task identities match Run A, but every record address is newly
allocated. The owner reported one One Difficulty and one ordinary enemy in wave
1; two One Difficulty Nuppeppo in wave 2, with one possessed; two Ippon-Datara
in wave 3, with one One Difficulty and one ordinary; and two non-possessed One
Difficulty Nuppeppo plus one One Difficulty crab in wave 4.

The only `record+0x8F = 1` record was again Nuppeppo spawn `0xF3F` in wave 2.
Both wave-2 Nuppeppo had `record+0xE9 = 1`, so the observed One Difficulty state
and possession state remain separated within the same wave and species.

Run B again had eight `record+0xE9 = 1` records and two zero records, but the
zero partition changed. Run A zeroes were wave-3 Ippon-Datara `0xF42` and
wave-4 Nuppeppo `0xF45`; Run B zeroes were wave-1 record `0xF3D` and the same
wave-3 Ippon-Datara `0xF42`. This change exactly matches the owner's visual
report: Run B's wave-4 Nuppeppo were both One Difficulty, while wave 1 gained
one ordinary occurrence. The moving partition therefore strengthens `+0xE9`
as a generated One Difficulty state marker in these two expedition runs rather
than weakening it as a field. It is not yet a general all-mode semantic.

The raw capture stopped at its first complete final event with 17 events and no
capture error. Cleanup required three bounded reconnect cycles, then verified
the probe inactive with no owned or global breakpoints. The
[Run B manifest](../../audit/possessed_enemy_capture/86872488/20260913-one-person-expedition-b/run_manifest.json)
and [A/B full-record comparison](../../audit/possessed_enemy_capture/86872488/20260913-one-person-expedition-b/expedition-a-vs-b.late-mask-comparison.json)
separate native state from the owner's visual labels.

The owner-approved route now stops repeated live collection. The next step is
bounded static object/writer provenance followed by a self-contained GPT-6 Pro
handoff containing the five controls and matching PC v2.01 module sections.
The Pro task is to recover the earliest causal `record+0x8F` source, separate it
from the `record+0xE9` One Difficulty path, and derive any PRNG inputs. At most
one targeted live validation should follow, and only when the static result
names an exact RVA, object identity, and falsifiable probe. The existing v3
`selector` temporal bracket is a fallback, not the next default experiment.

No replay or screenshot survives. The owner's wave-by-wave textual reports are
the visual observations for the control matrix; manifests must keep them
separate from raw native bytes and inferred duplicate-enemy record mappings.

## Pro origin recovery and mode-hiding clarification

The returned PC v2.01 analysis recovered the earlier origin that exact
`task+0x8F` displacement scans missed. `0x10285C4` writes generated enemy
descriptor `+0x0F`; the task constructor copies the descriptor to `+0x80`, so
the byte becomes task `+0x8F`. The later `0xE3ADF0` selector consumes this state
and separately assigns `+0xE9`. The four-site read-only `assignment-origin`
observer and its offline validator are now integrated. See
[the Pro integration record](POSSESSED_ENEMY_PRO_REVIEW_INTEGRATION_20260913.md).

An owner-relayed community observation also refines the mode interpretation.
Expedition likely activates enemies from a pre-authored complete configuration,
while normal solo suppresses some fixed world spawn positions. The possessed
second-wave Nuppeppo in `86872488` uses a position where no enemy appears in
normal solo; One Difficulty assignments can move between entries, while the
possession association remained stable in the observed expedition runs.

This does not turn the captured task field named `spawn_id` into a physical
world-position key. Normal solo task `0xF3F` has lookup `0x4388`; expedition
task `0xF3F` has Nuppeppo lookup `0xDCB98`. Preserve the physical-position
observation separately until a native descriptor or actor field binds it.

Run D completed the requested `assignment-origin` validation and confirmed the
source write, LCG trials, descriptor copy, and persistent task link. The
mode-dependent upstream session object and request construction are still
unknown. No downstream branch observer is approved as a substitute for that
causal recovery. Continue from the current mode-upstream Pro handoff named in
`CURRENT_HANDOFF.md`; request another live entry only if its analysis identifies
one precise upstream capture that can falsify the proposed mechanism.
