# Loadout optimizer working plan

Start here when resuming optimizer work. The [product contract and implementation map](required-set-loadout.md) defines the score. This page records the current decision, the evidence we can reuse, and the order of the remaining work.

**Outcome.** A player selects one attack and a weapon type. The App ranks legal outfits from the current character's inventory by final damage under the displayed opponent, hit and buff assumptions. The winner includes the actual slot assignments. The beta permits a required set tier. A weapon type can include several owned weapons.

Use `$nioh3-loadout-scoreboard` for the mechanism loop. For shared guidance absent in this worktree,
use the primary checkout at `F:/Nioh3_ScrollEditor` on this host or the checkout named by `NIOH3_GUIDANCE_ROOT`.
Read `AGENTS.md` for shared constraints, `docs/research/REA_WORKFLOW.md` for native investigation,
`docs/research/EXPERIMENT_FAILURE_LEDGER.md` for substantive failures, and `docs/knowledge/CURRENT_HANDOFF.md`
when coordinating beyond the optimizer branch. Prefer local copies when present.

## Resume checkpoint — 2026-11-22, PC202

**Current local candidate.** `F:/Nioh3_r16_build/deliverables/loadout-martial-projectiles-20261122`
contains the new 0.8.13 NSIS candidate. The extracted production app exposes
42 attack objectives and 133 configurations, including 62 theoretical peak
choices. The installer fits the 60 MiB budget at 62,343,624 bytes. All 2,045
manifest members match the extracted bytes. `APP_BUILD_SOURCE.json` binds the
actual working tree inputs. All scene seals name the candidate's base commit.
The isolated App replays use the extracted assets and an acceptance worker on
retained inputs. Flying Kick completed the CUDA workflow and Crescent Moon
peak completed the CPU workflow, including manual refresh, ranked assignments
and the wear preview. The fresh-player check belongs to the final frozen
candidate.

**Candidate and remaining acceptance.** The local 0.8.13 NSIS candidate
in `loadout-beta-20261122` extracts to the expected files and launches with a fresh isolated profile.
The packaged picker exposes 58 peak conditions across 14 attack objectives.
The retained-character WebView2 run exercises first read, refresh, complete
CPU and device CUDA search, and wear preview with the packaged scoring assets.
The debug host supplies the retained ordinary-read bytes for that comparison.
The two backends return the independently checked winning score of 5,623.

Identical tables, source parameters, group lists and scorer libraries now
share package files keyed by their sealed digest. The portable ZIP is
77,724,837 bytes and exceeds the current one-file download budget. The
58,834,091-byte NSIS candidate fits that budget. Its retained scene contracts
still identify an earlier app source commit. Re-stage the intended scene set
against the final app source and corresponding generated MAIN before running
the fresh-player check. The final one-file verifier can now combine the
separately checked per-scene receipts for the same EXE. Local evidence is in
`F:/Nioh3_r16_build/deliverables/loadout-beta-20261122`.

**Standing attack coverage.** The REA-backed
[standing route resolver](../../tools/acceptance/resolve_pc202_basic_attacks.py)
resolves all 64 quick and strong starting routes from the installed action
resources, including the Ninja shield-spear strong preparatory transition.
The route receipt (`BASIC_ATTACK_ROUTES.json`) identifies 29 routes with one
positive damage row. The [staging tool](../../tools/acceptance/stage_pc202_standing_controls.py)
binds those declared contacts to the shared form layouts. Use the full
Samurai held-item layout for every class. The narrower katana layout lost
504 scores when one odachi and one fists weapon reached the held-item branch
across nine controls. After switching layouts, original MAIN, CPU and device
CUDA agree on all recovered scores. All 29 controls pass full CPU/CUDA
comparison across 24,696 outfit evaluations. Previously answered scores are
bit-identical. The Windows WebView2 control reads the retained katana role,
then completes the odachi high quick search and wear preview. See
`STANDING_SHARED_CPU_CUDA.json`, `STANDING_RESTORED_NATIVE.json` and
`standing-app/native-card-HycCox/NATIVE_E2E.json` in the local evidence
directory. The 0.8.13 installer above predates these additional controls.

The controls establish arithmetic for the declared first contact. For
overlapping rows, apply the common recipient and hit-history rules before
choosing that contact. For distinct windows, verify the selected benchmark's
first eligible contact. Ordinary heavy attacks with separate action events
and named skills still need their relevant event inputs and resource
endpoints bound before publication.

**File-derived martial skill scoring.** The [file-cast staging tool](../../tools/acceptance/stage_pc202_file_casts.py)
builds fixed-contact objectives from a resolved controller entry, the exact
action parameter and its ordered damage rows. The retained full cast can also
supply contact ownership and order. Open-ended sequences and additional events
need their separately established inputs. The shared actor constructor now
selects the attack channel per damage row. REA PC202 `35A220` reads row `+94`
and promotes channel 1 to 2 when the candidate weapon's channel is 2. Row
flags 38 and 39 govern temporary owner element coverage per contact. The
candidate's weapon element is carried into the ordinary actor packet.

Three staged objectives pass full candidate comparison on original MAIN,
CPU and device CUDA. Kick Flash covers the opening without its optional
continuation. Rising Frenzy and high-stance Crescent Moon each use three
frontal contacts. The original Crescent Moon capture binds the three
HP losses of 1690, 2873 and 2299 to actor rows 0, 1 and 2. Its later owned
projectile deals zero damage. The isolated Windows App reproduces all three
equipped values and completes read, refresh, search and wear preview. The
Rising Frenzy App control searches a weapon class different from the held one.
These cases use the retained backpack on a fixed opponent. Missing owner
damage factors use declared neutral 1.0 values; the ordinary comparison uses
an uncoated owner. See `ROLE_CONTROL_INPUTS.json` and `DIRECT_CAST_ACCEPTANCE.json`
under `F:/Nioh3_r16_build/deliverables/loadout-martial-offline-20261122`.

The controller reference from 25195 selects Kick Flash 25050. Optional
continuation 25160 can start before contact 0 completes, so its contact
schedule depends on the chosen continuation timing. The ordinary Flying
Kick entries 25166 and 25308 both schedule projectile 20015 with damage row
25244/0. The 25292 direct-row control matched numerically on the prepared
inputs and lacked a proven ordinary controller entry. Its scene is retired.
The full Flying Kick benchmark uses one unguarded shockwave contact. The
descriptor can dispatch secondary spawn 20026 once through the environment
query at `322468` / `6A3BB0`. Expiry and contact have distinct continuation
keys. The secondary has neither an action selector nor a global damage row.
Its type-3 module has no enabled spawn key. The remaining attached methods
handle terrain, pose and model masks. The ordinary route also invokes the
same global event 10145 at creation as the retained early Flying Circle.
See `FLYING_KICK_CAST.json`, `PROJECTILE_NATIVE_CONTROLS.json` and the REA
sessions in the current candidate directory. The same closure tool checks
both Flying Circle charge variants. The landing assumption remains explicit.

Four casts use the shared theoretical peak conditions: Flying Kick, the
Kick Flash opening, Rising Frenzy and high Crescent Moon. The fixed Samurai
actor plan derives Arts eligibility from the installed skill row and derives
follow-up eligibility from the selected and secondary controllers. The
retained Crescent Moon cast has an empty secondary controller on every
contact. PC202 `35A220` sets packet bit 25 only when FC is eligible and its
ordered Arts word comparison is at or below zero. The engine computes this
condition for each candidate before any temporary element overlay. Each
positive ordinary Crescent Moon contact is eligible to settle the one-hit
grant. The benchmark applies that grant on the first contact. Held-action
and existing standing producers keep their own typed rules. The tests bind
the same skill table and execute original perturbations of both predicates.
The merged Samurai CPU/CUDA kernel includes the peak and held-weapon input
domains. The five new rankings match original MAIN and both backends on the
retained inventory. `CRESCENT_EQUIPPED_ORIGINAL.json` records all three
original damage components for the equipped-outfit App comparison.

Finish the source-bound package and replay checks before preparing the
consolidated player session. The player check uses a stable save and one
readable actor in each relevant form. The scorer initialization and ordinary
ranking workflow do not require an attack input for each picker choice.

**Shared opponent.** The theoretical peak equipment comparison fixes Saturation,
standard Confusion and ordinary Weakness on the training Samurai. Ailment spell
slots, charges and held target-support items have no role in selecting this
starting state. The scorer still recomputes every outfit's eligible bonuses
against these statuses and keeps the selected attack's contact sequence.
Apply any established spell-use attribute requirement if point changes can
affect access. Katana mid strong, Water Tsubame Gaeshi and kusarigama strong
produce the same per-outfit components and rankings with either supply
fixture. The Windows App first read, manual refresh, CUDA search and outfit
preview pass on the stripped fixture. See the
[fixed opponent checks](../../deliverables/peak-elemental-sources-20261122/README.md).

**Prepared spells.** The [earlier producer and comparison receipts](../../deliverables/peak-spells-20261122/README.md)
establish the ordinary Carnage and Weakness magnitudes. Carnage uses each
candidate's jutsu power and competes with the ordinary damage group. It uses
the selected actor's prepared slot and the bound save's remaining uses.
Weakness is a separate opponent group. On the reference training
Samurai it contributes 1.20 to both damage lanes and combines with Saturated
and Confusion. The three-piece Grace of Tsukuyomi reward applies a distinct
1.10 grant on qualifying Onmyo Magic hits. Both grants enter the same incoming
damage group. Original aggregation selects 1.20 when both are active. This
resolves the half-magnitude equipment source for the fixed-opponent benchmark.
See the [set and original hit-path evidence](../../deliverables/peak-spells-20261122/weakness/README.md#grace-of-tsukuyomi).

Original MAIN and a local CUDA device agree across the complete katana strong,
kusarigama strong and Water Tsubame rankings with both spells ready. The
retained equipment is unchanged in these controls. The prepared keys and
charges are supplied in isolated fixtures because the retained actor had
neither spell ready. Windows WebView2 exercised the ordinary context read,
manual refresh, device search and wear preview using the same fixture.

The [element application replay](../../deliverables/peak-spells-20261122/ailments/README.md)
closes normal hit admission for Fire, Lightning and Wind on the retained
training Samurai. Each tested owned amulet yields 40.32625198 buildup per landed
kusarigama strong hit and reaches the native 100 threshold on the third hit
with no elapsed decay. The native settlement requests the corresponding
status, and the update selects the installed factory definition. Exact live
activation and timing remain distinct from the optimizer's already-active
assumption. Reuse this shared path across contacts and request player input
only for a runtime fact that changes the comparison.

**Peak ranking.** Use the [shared-provider evidence and ranking receipts](../../deliverables/peak-providers-20261121/README.md)
for the unchanged conditions.
The batch publisher adds the common rules to sealed ordinary attacks, fixed
multihit actions and both Water Sword objectives. Peak assumes the compatible
effects available to the outfit are active. It uses the native winner within
each group, consumes a one-hit grant on its first valid settlement, and keeps
the Arts cap, charged resource phase and each outfit's actual costs.

Power Pill competes with the same damage group from Deflect, Evade or Amrita
absorption. The four temporary weapon elements share one slot across the cast.
The selected water coating compares the held amulet and configured Summon Water
at each outfit's calculated jutsu power. Count and donor identity come from the
selected save and ordinary actor data. A direct contact receives an override
only when its original packet permits it. Independently generated elemental
contacts keep the power inputs established by their source path.
The earlier shared-provider comparison used the admitted elemental ailments
and one Confusion tier from their count. Its receipts preserve that condition.
The current comparison uses the fixed opponent state above. Water modifies the
physical lane, Confusion modifies both lanes, and the attacker's gear and skill
bonuses read the projected active keys. The selected attack element remains a
separate input.

The local 0.8.13 Windows candidate exposes 58 choices across 14 attack
objectives. The [final ranking receipts](../../deliverables/peak-providers-20261121/FINAL_RANKS.json)
cover all 53,440 CPU/device candidate scores. Original MAIN independently
matched every candidate in five representative cases and every winning contact
in the other choices. The packaged picker and isolated Samurai and Ninja
WebView workflows passed. The proof records the boundary between prepared
effects, elemental buildup and the declared peak starting state.

The baseline and device checks use this character's retained inventory and the
same required-set rule. Keep peak separate from the recorded state, specific
trigger sequences and future playstyle-weighted scores. Publish further skill
choices by binding their event and packet families once. Prioritize shared
status producers with a measurable rank effect. Use original helper controls
and held-out input comparisons for each new producer.

**Which outfit actually gets the bonus?** The [common source and ranking audit](../../deliverables/shared-source-family-20261121/README.md)
joins all 421 set rewards and the descriptor slots of all 1,101 skill rows to native
writers. The largest shared numeric family is already incorporated into production
scores. Original instructions and the production Rust writer agree on 17,379 inputs.
The desktop candidate now offers an immediate first contact after Amrita absorption
on eight attack choices. The scorer checks each outfit's effective sources. Both
CPU and device CUDA match original MAIN on 1,400 Samurai katana outfits and
1,000 Ninja diagnostic outfits. On the published katana strong attack one outfit
goes from 1,291 to 1,420 and passes another at 1,355. See the WebView receipt
for the regular first-read, refresh and ranked search on the retained character.

The 137 key-selected stores (71 rewards, 78 skill keys) also matched original
whole-bank output on 1,644 controls, including present zero-valued sources.
Next, group these switches by the event handlers that read them. One already feeds the
absorption condition. Prioritize the remaining handlers that change damage
or the number of attacks this character can sustain. Style Shift remains a
regression witness for pre/post-form grants and first-hit consumption.

**Buff eligibility after an outfit change.** Runtime type 36D8 belongs to
Samurai Arts Proficiency (琢技). The installed basic skill, the Arts-duration
passive's named source and native duration field, the full-gauge self-grant
and the distinct martial-key counter establish the connection. The counter
tracks eligible transitions, with duplicate keys ignored and the damage
contribution capped at five. The live idle read establishes absence at that
time. On November 9 the player confirmed the same target took 1,196 from a
normal katana strong attack and 1,552 from a full-gauge strike. Original
MAIN recovered 1,196 when the Arts input was cleared.

**Arts activation and the scoring endpoint.** For an eligible martial
action a newly granted Arts object initially waives Ki debits. Its charging
phase applies the native 70% factor. An action transition starts charging;
hit count alone cannot determine the phase. Ordinary Water Sword started
from a full gauge refreshes its timer during an uninterrupted hold, retaining
the waiver and the zero-key count. A depletion total is undefined for this
condition. The desktop candidate includes a finite opening (first three
actions, up to six frontal hits). For a previously active object the
current-state selection holds the starting effects fixed. Predicting its
natural evolution also requires the previous action and its pending state.

Use the [current receipts and reproduction](../../deliverables/arts-and-sources-20261121/README.md).
On this inventory and required set, the fresh opening gives 4,868 equipped
and 5,452 best. The existing two-key cases are controlled fixtures. In the
charging phase the equipment and winner sustain 62 and 86 contacts.
The four complete CPU/device rankings agree. Independent original MAIN covers
each equipped and winning contact in all four conditions and every neutral total.
Windows WebView2 accepted the ordinary first-read, manual refresh and real
CUDA paths for both fresh and pre-existing Arts openings. A legitimate
no-endpoint read now explains the result and allows a same-page change of
objective. The desktop acceptance uses retained game memory in an isolated
save directory; the existing installer predates these changes.

Use the [source, text and native controls](../../deliverables/effect-source-36d8-pc202/README.md)
for both this actor mechanism and ordinary statuses. For each new source family
start at its common grant caller and evaluate the candidate's effective equipment
and skill inputs, recipient, triggering event, and competing active sources.
The new ordinary-status index and candidate gate audit distinguish named
definitions, open equipment conditions and actual grants. Prioritize shared
producers across the inventory; the source ID or localized name alone cannot
establish activation.

The installed-scene builder generates Samurai dual swords / high / Water Sword
with full-Ki, frontal all-contact objectives. Each choice uses per-equipment
costs and the selected Arts phase. Untriggered uses ordinary Ki costs and
damage. Current reads the effect owner, type, counter, charge phase and
additive and holds the stated starting conditions fixed during the cast. A non-empty unrecognized
object list or a missing list header cannot establish absence. The role must be
at full health with the base skill enabled. Unsupported resource or source
branches remain visible as unanswered. This support covers ordinary Water Sword;
the elemental variant and other held arts require their own event bindings.

Use `tools/compile_water_sword_scene.py` with the public dual-swords layout, the
typed skill-word kernel, and the two static action receipts. The resting role
supplies cost controls and the enabled upgrade. Each candidate supplies maximum
Ki, high-stance reduction, agility and damage stats. Each phase selects its own
controller, skill key and recipient row. The input ABI preserves the unknown
halfword adjacent to the skill key. Chunk device work by actual contact count.
Indexing by template count overflowed the device's 4,096-contact limit on the
normal App route, although the first small-batch standalone audit had passed.
The details table aligns repeated occurrences of each template across outfits.

In `live-action-1795282524607/whole-cast-app`, `RANK_PARITY.json` ties both
production searches to the earlier independent original-instruction results:
all 1,120 assignments and all three damage channels match, including all 400
retained ranks and displaced weapon slots. The four first-place ordinals follow
the production tie-break: 87, 7, 647 and 247. `NATIVE_CONTACT_COUNTS.json`
retains the independently produced per-outfit schedule for packaged acceptance.
The original ordinary read does not contain the 36D8 lists. Its expected
missing-data response and a synthetic empty-list positive path are recorded
separately in `read-current-unknown` and `read-current-empty`. The later
real idle read below establishes membership for its own timestamp.

Product source `ded37243` passed the real desktop first-read, manual refresh and
search flow on retained Samurai memory for the untriggered objective. See
`whole-cast-app/webview/native-cpu-L2ncpd/NATIVE_E2E.json` and
`whole-cast-app/webview/native-card-FAxfdk/NATIVE_E2E.json`. Each used separate
read jobs and matched all 20 published ranks and item assignments. The controlled
positive object (two stacks, additive 0.5) yielded 49,606 physical on the equipped
outfit and 74,779 on the winner. Both backends matched all 1,120 candidates and
400 retained ranks for the control; original MAIN was checked individually for
the 44 equipped and 60 winning windows. This historical control supplied the
neutral action count to the active damage packet. The current paid and waived
routes above supersede these two active totals. It says nothing about the
historical role's actual object membership. See `ACTIVE_EFFECT_PARITY.json`.
The same active control went through the normal WebView current-effect selector,
first read, manual refresh and actual device search
(`webview-active-control/native-card-bjjZWl/NATIVE_E2E.json`).

The local 0.8.13 installer and single-file application are in `whole-cast-app`.
`INSTALLER_PAYLOAD.json` compares all 1,079 installed payload members after
NSIS extraction. `ONEFILE_PICKER.json` records a cold launch of the outer EXE
and both high-stance effect choices in the packaged WebView. That check used an
empty save directory and the public catalog. The broader standalone test that
asserts the absence of a running game was not run through completion. Its
process-status assertion is separate from the packaged picker test.

The real current-effect read is complete. In
`whole-cast-app/live-idle-1795295761619`, `ACTION_READ.json` and
`read/effect-query-inputs.json` record two consistent reads of the same
full-health Samurai (3198/3198), primary slot, high stance. Both native
lists reached their own sentinels with zero nodes on each read. Effect
0x36D8 was inactive. No attack, game write or save write occurred.
`RANK_PARITY.json` matches the native reference for all 1,120 outfits
and 400 retained ranks. `NATIVE_E2E.json` covers the current-effect
picker, independent first-read and refresh jobs, and all 20 displayed
ranks on real CUDA using these saved game bytes. Current / best were
28,707 / 43,707 physical, with 44 / 60 eligible contacts. These totals
assume all windows land and were not observed on an enemy during the read.

The PC202 type getter resolves object key 0x7011 through the native hash table
to definition ordinal 27, row ID 444 and type 0x36D8. The row ID is not the
lookup key. Native controls, curve 0xEACD and packet writes are retained under
`contact-window-review/rea-36d8/getter-verification`. Their presence in
the shipped scorer does not establish that an older character had this effect.

### Current-role session and reusable offline evidence

The 2026-11-21 session is complete. Its evidence directory is
`optimizer-fixed-and-source-20261121/live-action-1795282524607`. The fresh 0.8.13 WebView profile
read the current Ninja role and the manual refresh completed as a separate live job on the selected
kusarigama scene (`packaged-refresh/live-8tMsok/result.json`). Both ordinary reads have bit-identical
action fields, physical and elemental factors 1.0, and an inactive temporary element. The Samurai
read has primary dual swords, high stance (style 0) and 1237 maximum Ki. Its upper descriptor bit
is set, general reduction is enabled and the three owner cost factors are 1.0.

The second Water Sword window recorded one complete empty-field cast. Its 21 sustained actions
ended with 25337 taking Ki from 3.760765 to -57.139938 (rounded). The native positive-Ki check
then rejected further sustain. The closing action 25338 brought the balance to -209.391693.
All 22 adjusted cost fields matched independent PC202 cost replay bit for bit. Four sampled
decreases during the closing action together match its single 152.251755 net cost. They do not
measure the number of damage contacts. The first window contained no Water Sword action.
Use `water-sword-high-02/REPLAY.json`, `native-cost-review/NATIVE_COST_PARITY.json` and
`finish-net-rehearsal/REHEARSAL.json`. To reproduce the sampler check, run
`tools/acceptance/capture_water_sword_poll.py replay --capture <window> --output <new-json>`.
For other loadouts, derive the upgrade transitions and costs from their inputs before ranking;
reusing this role's 22 actions would build its equipment assumptions into every score.

The original section-3 transitions explain the observed acceleration. With Water Sword II equipped,
25331 can enter 25332. The edge from 25333 to 25334 clears the repeat counter. At 25335, the
first return to 25334 increments that counter; the next visit can advance to 25336. The final
pair 25336/25337 alternates while Ki permits. Independent replay using these transitions predicted
the entire cast before comparing the sample. See `native-cost-review/CONTINUATION_RULE.json`.
Across the ten distinct actions, 38 original direct rows were matched to the resource. Against
the benchmark recipient with component 0, each action including the finisher has two direct
windows. Under an all-windows-land objective this observed route has 44 eligible windows.
The empty-field run scored no actual contacts. The ordinary timeline and its extensions contain
no spawned projectile for this move (`contact-window-review/CONTACT_WINDOWS.json`).

For the same character's required-set dual-swords domain, the equipment pipeline reproduced
all 2,304 equipped materializer bytes, maximum Ki and agility. Native costs and transitions
were then evaluated on 1,120 candidate assignments. At full health, 1,064 have 1220 maximum
Ki and 56 have 1237. The nine attainable route lengths span 22 through 30 actions including
the finisher. This yields 44 through 60 eligible direct windows against the benchmark when
every window lands. The zero-component recipient selects local row indices 0 and 1 at every
phase, but the action parameters, row bytes and skill key can change. The original skill
producer selects 0x98CF for 25329–25331 and 25338, and 0x2E74 for 25332–25337. Keying
only by row index or keeping the opening skill for the entire cast would conflate them.
Use `native-cost-review/OUTFIT_COST_PARITY.json` and
`contact-window-review/OWNED_SCHEDULES.json`. The latter is reproducible with
`tools/acceptance/compile_water_sword_windows.py --session <session> --output <new-json>`.
The finite `HeldActionRoute` model checks the resource admission, first return, upgrade
and terminal charge for differing budgets with and without the upgrade skill. TLC explored
92 distinct states. Early upgrade, missing counter increment and dropped finisher each
violated the expected invariant (`held-action-model/model-controls.json`).

For damage, bind the actual high stance by writing 0 to packet+148, replace the action
controller and its row table at each phase, select the corresponding skill key at +136,
and retain the candidate's own weapon/actor inputs. The original MAIN calculates 28,707
physical and 0 elemental for the equipped role in a grounded, full-hit control with the
additional 36D8 object effect inactive. The 44 windows have the same outputs with their
individual pre- and post-debit Ki values. The packaged CPU and device kernels scored
56,716 windows across 1,120 assignments under the same declared starting effects.
CPU and CUDA agreed per window. Independently summing the original MAIN results in the
candidate's action order reproduced every final physical, elemental and total score,
rank and assignment, including four equal first-place outfits at 43,707 physical and
zero elemental. The comparator separately checked displaced weapon slots. The original
runner executed 22,533 calls including 133 direct controls and reused 34,183 further
scores by exact read evidence. This offline conditional ranking is the
reference for the subsequent desktop and live-effect checks above.
See `contact-window-review/WHOLE_CAST_PARITY.json`, `contact-window-review/KI_AND_104_TRACE.json`
and `independent-water-kernel-check/OWNED_ASSIGNMENTS.json`.

The new original-code positive control demonstrates when extra source bytes matter.
With packet+174 set to 1.25, changing source_owner+2DF4 from 0 to 0.5 changes the
opening window from 1892 to 2200. The inactive control stays at 935. These are
controlled inputs. None of the three fresh ordinary reads or either empty-field
window retained the two ordered object lists or the additive word.
`contact-window-review/36D8_INPUTS.md` records the native query and minimal fields.
The opt-in first-use observer now retains these separately in `effect-query-inputs.json`.
Its two reads preserve missing bytes and raw object candidates. Occupied lists alone
do not establish an eligible object. A complete idle empty/closed query is sufficient
to establish absence at the time of that read. No repeat of the sustained cast is needed.
The next ordinary reader is prebuilt. `next-ordinary-reader/SESSION_PREP.json` binds it
to its hash; `OPERATOR_REHEARSAL.json` in that directory distinguishes the genuine
incomplete old observation from an explicit empty-list fixture without blocking the role read.

For the fresh Ninja observation the packaged CPU and physical CUDA device ranked 1,600 eligible
outfits across four attack scenarios. Original MAIN agreed on 4,240 individual contacts.
Spiral Thrust scored at most 2482 (360 outfits), Wild Wolves II 4140 (520), Flying Circle on
early release 1491 (360) and at full charge 2790 (360). Each result uses the declared full-hit
training opponent and required set. They describe the fresh role in this model; the historical
Flying Circle mission modifier remains unavailable (`ninja-fixed-parity/CURRENT_ROLE_PARITY.json`).

The ordinary reader installs the PC202 skill lookup for direct martial attacks as well as for projectile contacts. Without it, Spiral Thrust reached 556F7B and all 360 eligible outfits failed. With the lookup installed, all four contacts of each outfit matched original MAIN on physical, elemental and total damage. CUDA reproduced the 360 complete scores and assignments. The maximum was 2482 in the explicitly neutral two-factor scenario. The previous four observed HP losses belong to a different equipped role. See `optimizer-fixed-and-source-20261121/SPIRAL_PARITY.json` under the evidence root.

For Wild Wolves II, original MAIN accounts for all four scheduled contacts under the full-hit assumption. The controlled set of 520 owned outfits includes 120 dealing elemental damage. All 2,080 contacts and the top 400 assignments matched (`optimizer-spawned-contact-20261121/trial-02/wolves-full-hit/PARITY.json`). The role's temporary element can change the result independently of the weapon's imbue. The retained role lacks its selector. On this controller the native builder sets packet bit 28 when owner+2E9C differs from -1, then uses the owner element and +2EA4 power. Zero is a valid element. Keep the neutral input and the role-selected input distinct.

The ordinary reader and scoring engine now bind that selector for each hit. With selector -1 the best outfit scores 4140. Setting element 0 and power 101 in an explicit control changed elemental damage on all 520 outfits and left every physical total unchanged; the best score became 8752. Both rankings matched original PC202 and device CUDA. See `wolves-full-hit/public-candidate/controls/CROSS_CASE.json` and the two sibling per-condition parity receipts.

The first-hit, damage-dealt-only scenario starts with a settled empty group 312, assumes a successful trigger and scores the hit while the effect is active. Use the candidate's materialized bank after equipment eligibility. The Ninja handler submits equipment source 1F6 before CB and 31. An occupied group can retain its earlier weaker member. The theory cannot establish an observed trigger, duration or uptime. On the retained role, 40 of 4,520 Ninja candidates qualified for 15% and 4,480 for 10%. Independent game instructions gave 844 → 929 and 532 → 612 for the contrasting TYPE6 weapons. Ordinary scalar writes outside the compiled MAIN domain are ineffective. The damage-dealt word must be included in both generated kernels and bound to the attacker's actual owner. Receipts are in `optimizer-action-buff-20261121/ninja-evade/CANDIDATE_STATUS_SOURCES.json` and `optimizer-fixed-and-source-20261121/*evade-PARITY.json`.

The earlier 1220-Ki role was missing two branch flags. Its conditional projection yielded 17, 18, 19 or 20 sustained actions before the finisher. The current session supplies the flags, cost factors and original action identities for a separate 1237-Ki role. The ordinary reader's `--action-facts record` option retains both observations and the raw addresses. Skipping the original reset can retain stale multipliers even when the status tree is empty.

Use `tools/acceptance/Read-OptimizerActionFacts.ps1` for the two form reads. The normal first read, manual refresh and comparison can reuse the same observations. Do not repeat past equip-refresh or record individual weapons for these facts.

The same status-element producer is active for four-hit Spiral Thrust. At neutral it preserves all 360 earlier outfit scores and assignments. In the element-0/power-101 control, all 360 elemental totals change, physical totals stay unchanged and the best becomes 5233. Both cases have 1,440 original-MAIN contacts. The five Samurai first-hit deflect comparisons (katana quick, strong, Sword Ki, odachi quick and strong) cover 5,544 candidate evaluations. Each matches original MAIN and hardware CUDA under the stated fresh-trigger assumption. Recordings of a pre-existing buff require their own membership and refresh history.

The portable layout stores byte-identical MAIN DLLs once under `worker/runtime/data/loadout-kernels/<sha256>/`. Each scene's original seal identifies its CPU and CUDA binary. The current role, its saves and native replay images are separate from this distributable content. A self-contained acceptance export restores the original filenames. The package and installed-worker checks validate the same bytes for local and shared kernels.

The Windows 0.8.13 candidate from `d9f11ac` contains 20 calculation scenarios. The single-file build passed three actual Windows launches including repair of an incomplete cache. The NSIS archive reproduced all 1,051 manifested files by size and SHA-256. A retained character read against the packaged resources ranked all 480 eligible outfits in the Ninja evade first-hit case on both CPU and device CUDA, scoring 1117 at the top and reproducing the 400 archived assignments. The standalone replay utility now hashes pooled libraries for checkpoint identity too. The registered user installation was left in place. See `optimizer-fixed-and-source-20261121/OFFLINE_ACCEPTANCE.json` and the current-role session above for subsequent live evidence.

The Water Sword observation includes both 25336/25337 and 25338. The retained Reaper sequence and a separate Windows process exercise the sampler's reconstruction, missed samples, partial finisher charges and intervening recovery.

Implementation proceeds on `codex/required-set-fastpath`. The numerical baseline is `6fa829c` and the previous plan checkpoint is `b5cb3c9`. The working tree adds a direct-cast compiler (`tools/compile_witnessed_cast.py`) and passes the native two-byte actor skill key through the event plan, first-use reader and encounter projection. Keep this pending implementation for the whole-move work below. The integration checkout carries its own app changes.

| Layer | Verified scope | Open product work |
| --- | --- | --- |
| Character and gear | One Samurai role projected into three weapon types. Six cross-type searches agreed on 6,384 candidate evaluations. Ninja source maps and stats matched three held-out outfits. | Reuse the shared form rules. Revisit a producer when a new candidate or state contradicts an existing witness. |
| Damage arithmetic | The representative single-contact domains answered 6,832 Samurai and 4,520 Ninja candidates across eight weapon types per form. The later Ninja host receipt checks every candidate against original MAIN, CPU and CUDA. | These fixtures exercise numerical paths. Each player-facing named move still needs the correct event sequence. |
| Player selections | The source catalog has 11,885 rows. The retained-scene scoreboard matches six observations on three action keys. Both Flying Circle charge branches, Spiral Thrust, four-hit Wild Wolves II and ordinary held Water Sword have complete current-role rankings on the declared opponent. Packaging binds installed evaluators to their catalog identities. | Extend the established event bindings to remaining named moves. Preserve the historical Flying Circle mission-table limitation for its original observations. |
| Buffs | The 406-row source table and REA results cover source identities and several native contribution rules. Class 0 has same-key strength competition. Deflect supplies a regression control. | Establish which sources each outfit can actually activate together and which contribution survives at each scored hit. The seven Deflect-only experiment bundles were withdrawn from the product path. |
| Delivery | The 0.8.13 local candidate was built and checked with Windows WebView, extracted NSIS files, packaged CPU/CUDA searches, fresh role inputs in both forms and a separate manual refresh through the Ninja UI. | Carry the current-role results into the independent acceptance handoff. |

The scoreboard's action key is the resource, action number, and parameter digest. A row counts as
`match` for its admitted observations only. `mechanism-witnessed` records a shared replayed packet
path, and `pending-replay` retains an observation awaiting an exact context. Catalog availability,
preparation for a retained context, and ordinary App acceptance each need their own evidence.
CPU and original MAIN compare arithmetic on prepared inputs. The game observation is the separate
external reference for the declared context. Full ranking and CUDA remain separate milestone checks.

[Clean-checkout scoreboard receipt and repeat commands](F:/Nioh3_ScrollEditor/deliverables/loadout-scoreboard-20261121/README.md).

## Next development work

Pick one shared mechanism per work unit. The lead owns the production edit, build, and verification.

Prioritize common producers by the number of damage-relevant equipment tiers
and skill sources they explain. The current PC202 search tables contain 78 set
keys with rewards, 411 distinct set/threshold pairs, and 421 reward assignments
to 266 effect groups. The [set reward inventory](../../deliverables/arts-and-sources-20261121/SET_REWARDS.csv)
uses the same table key and seven-row layout as `Tables::set_rewards`.
Compare these groups and the skill descriptors against both direct stat
writers and conditional event handlers. Keep table coverage, candidate
eligibility and production scoring as separate measurements.

Use Style Shift as a retained regression witness for event-granted effects.
Its two skill gates cover three grants. The controls exercise the following
boundaries when another production family uses them:

1. On each candidate outfit, evaluate the switch into the selected form using
   the pre-rebuild and post-rebuild inputs. Feed the resulting grants to the
   existing event executor. Exercise the candidate pair that differs only in
   Tourbillon eligibility.
2. Bind qualifying hit settlement and the next effective rebuild to the ordered
   damage events. Replay the retained Reaper pair (380, 423); carrying the
   first hit's bonus forward produces 449. Check the independent case in
   which another buff survives that rebuild.
3. Apply these common boundaries to the installed action families. Check a
   direct hit, a sustained attack and a spawned attack against original MAIN.
   Measure inventory-wide CPU/device parity and confirm that the desktop
   selector returns the same winning gear under the displayed conditions.
4. Use the same grant and consumption rules to build the theoretical-peak
   condition: per candidate, combine attainable effects and enforce their
   stacking, incompatibility and lifetime at the contact being scored.

1. **Equipment and buffs.** The [Style Shift qualification audit](../../deliverables/common-trigger-family-20261121/README.md)
   matches original grants using separate effective fields before and after
   the form change. The retained character has Flexibility enabled and
   Tourbillon disabled. A single candidate source change independently
   activates the Tourbillon branch. Use this pair as a control whenever implementing
   another ordinary grant with the same lifetime or consume boundaries. Trace
   the Katana mid strong candidates at ordinals 0 and 1350.
   Inventory item 115 changes to 160. Original MAIN agrees with 272 and 293 physical damage.
   Follow the change through `source_rebuild::Program::pipeline` and `Engine::prepare_frame`.
   For buff eligibility, pick a pair with a known source or competition difference.
   Compare it with independent native evidence and reuse the equipment, set, and guardian rules.
   Extend `loadout_context.rs` when the needed effect inputs can be produced for each outfit.
   Record which effects coexist and which assumptions apply.
2. **Whole moves.** Use the completed fixed-cast comparisons above for regressions. For an additional
   owner, resolve its native action and event data before requesting another player action.
   For projectiles, bind the owner's controller and the spawned damage row separately. Use both Flying
   Circle charge branches as the zero-direct-row controls, and Water Tsubame Gaeshi as the mixed control.
   For held moves, use the resource sequence executor after binding the branch and the outfit's debit.
   For Wild Wolves II, use the four-contact full-hit case. The player already tested it on a yokai.
   The fresh Ninja read confirms its current-role selector is inactive and its resulting ranking matches the neutral control.

### Verification tiers

- **Work unit.** Run the scoreboard check and replay the affected witnesses against original PC202 MAIN.
  Run the relevant lint. If a state protocol changes, run its TLA+ model and named fault control in that
  unit. Record the active source, inputs, unit witnesses, exact repeat command, and receipt.
- **Integration milestone.** Use the ordinary App picker, character read, and required-set search on the
  same inventory, target, and buff assumptions. Verify full-move components, candidate coverage, ranking,
  tie order, and slot assignments. Exercise cancellation, equipment preview, first read, and refresh in
  WebView. Run a fresh local package, device CUDA parity, and the background benchmark when the change
  affects those contracts. Keep the required checked-in CI and release profiles.

### Known limitation of the whole-move comparison

The retained native verifier (`optimizer-live-samurai-20261120/verify_observed_native.py`) executes each contact in order and compares the accumulated physical, elemental and total damage for each outfit. Errors within a move that cancel in all three totals could pass this comparison. No reordered contact was observed. The existing Spiral Thrust recording gives consecutive HP losses of 420, 420, 420 and 491 for one landed cast. Its damage is specific to that recording.

**Decision, 2026-11-21.** Keep whole-move damage and equipment ranking as the acceptance target. Retain the existing recording and comparator. Revisit individual contacts if an observed discrepancy or a demonstrated mechanic makes their sequence change the predicted total or winning outfit. This limitation requires no new capture or investigation at this checkpoint.

### Native calls behind the current move bindings

- **Held attacks.** Ordinary Reaper's entry and two alternating follow-up actions are known. Original PC202 `49B108` admits a positive-cost action while Ki is positive, including a final debit larger than the balance. The retained development App ranked the held sequence for its declared role, target and initial status. That reference outfit performed 23 contacts. The Samurai dual-swords file uses the same 128-byte transition record format. Water Sword's base entry, alternating strikes and finisher cost 200, 70/80 and 200 Ki. At 25331, records 5922/5923 lead to Water Sword II with condition key 0x2E74; 5924/5925 return to 25330. REA52 `2B2D88` and REA132 `28A88C` resolve this skill branch. The current-role session above verifies the remaining repeat-counter transitions and the natural finish. Damage ranking still requires binding the damage inputs for the selected row of each window.
- **Water Sword II eligibility for the observed character.** In the retained Samurai middle-stance snapshot, key 0x2E74 has rank 1 and enabled byte 1 in the configured skill node. Original skill definition 0x2E74 (table 00294, row 74) takes the enabled-node branch for this actor in REA132, so the extra skill condition succeeds. REA51 `20DFC0` tries transitions in order after filtering input and state. Under immediate selection, an earlier eligible II transition is tried before the ordinary loop. Queued actions, remaining Ki and the other transition conditions still affect the actual result.
- **Projectiles.** The timeline points into the owner's resource. For projectile keys, `93D6C0` subtracts 20000, `49D610` resolves the 336-byte spawn record, and REA37 `5C9BE0` distinguishes an explicit target action/row from a global primary row. The new [offline binding check](../../tools/acceptance/check_projectile_row_bindings.py) reads file-declared base and extension events from the original resources. Across 559 distinct owner actions, 858 base and 70 extension references select explicit rows that match the App's bank byte for byte. Another 1,466 references select a row from the installed global bank, while 199 specify no primary row. This inventory describes row selection; attached modules and conditional contacts have their own effects.
- **First-read integration.** Of the 559 owners with explicit action-row references, 331 have zero direct rows. Flying Circle spawns carrier 783025 at tick 24. Early release (25203) selects action 25244 row 0; full charge (25204) selects row 1. The normal reader installs the empty owner controller (first row index 293, count 0) and resolves the projectile row separately. Both charge scenes pass first read on the retained Ninja role. The observed HP losses were 1139 and 2191. Strict replay of both old observations reaches original MAIN's mission modifier lookup at 78C9A9; this process did not retain the table. The later role omits source-owner +2E48/+2E4C. With both explicitly set to neutral 1.0, all 360 candidates per charge, including 40 with elemental damage, matched original MAIN, CPU and CUDA on every component; CPU and CUDA returned the same rankings and slots. These comparisons use the prepared inputs; the two scenes remain staged for independent current-role acceptance.
- **Weapon inputs for elemental projectiles.** Record the damage row's element, weapon imbue and candidate attack separately. Water Tsubame's wave uses its own water row and the candidate's attack. Switching only weapon imbue from neutral to wind left the wave at 847; increasing attack input from 2589 to 2789 raised it to 1126 at either imbue. Two real owned katana with identical remaining gear and attributes gave 1057 and 898 water on the same target. The blade has its own weapon imbue inputs. Preserve this [repeatable original MAIN control](../../tools/acceptance/check_water_weapon_scaling.py) when changing projectile inputs, cache keys, or score bounds.
- **Common spawn resolver.** `action_rows::scheduled_projectiles` reads the owner's complete scheduled list by resource, action and parameter digest. It preserves slot, tick, event key, carrier, descriptor subclass, imbue branch and the primary row's origin. PC202 5C9BE0 gives an explicit action/row priority. In this corpus all 1,466 global requests select global row 372, whose physical and elemental motion are zero. Another 199 events declare no primary row. Keep those occurrences for attached modules. Neither category supplies evidence of a landing. REA40 65E908 copies spawn+C0/C4 to object+5F8/+5FC. Of 2,593 references, 85 permit lookup of the owner's imbue and 2,508 skip that lookup. Reversing Axe and Flying Kick permit it; Crescent Moon skips it although all three use carrier 783025. Projectile subclass and damage-row element remain separate inputs. The production selector's complete resource comparison is `cargo run --manifest-path crates/nioh3-loadout/Cargo.toml --example check_spawn_references -- <ALL_SPAWN_REFERENCES.json>`.
- **Wild Wolves II (Ninja hatchets, action 25299).** The PC202 resource schedules four damaging rows (1, 3, 5 and 7) at ticks 19, 27, 35 and 43. The player confirmed that all four hit a yokai in a follow-up run. The exact enemy species and per-contact damage in that run are unverified. For the full-hit objective, evaluate all four rows for each outfit against the stated reference opponent. The earlier warrior recording was retired after knockback caused a miss.
- **Contact count for sustained moves.** Track action repetitions, eligible contact windows and actual target contacts separately. Ordinary Reaper has a matched full Ki-limited run. For Water Sword action 25330 the four nonzero rows form two time windows. A [catalog check](../../tools/acceptance/check_contact_window_routes.py) found 71 groups with flag words 0x0102 and 0x0082 among 37 actions across six form/weapon combinations. At 117AB05 the effective recipient's component (+E8, member +80) selects which rows continue. Value 0 rejects bit 7; value 1 rejects bit 8. A null component or any other value continues through this particular check. The retained training warrior has value 0. When it remains the effective recipient, each Water Sword window admits the 0x0102 row and excludes the 0x0082 row. Rust matched the original branch on 740 row/recipient decisions. Recipient routing, same-target hit history and candidate Ki still matter when deriving a sustained move's eligible contacts under the declared hit assumptions.

Combat-style statistics, automatic inventory re-search, multi-move weighting and optimization over unowned generated equipment are deferred. Resume one of these when the product scope changes.

## Work unit workflow

- Open one chat per shared-mechanism work unit. Start with the checkpoint, current diff, and scoreboard.
  Name the target witnesses or the missing binding and the evidence needed to resolve it.
- Reuse the pinned REA/Ghidra workflow for focused native questions. Record the PC build, function or
  evidence ID, and the scope of each finding. Keep total-only and per-contact observations distinct.
- When the unit passes, update this checkpoint with the new status, remaining scope, and receipt, then end
  the chat. Record a concrete blocker or defer a branch if no unresolved fact affects the current unit.
- Use subagents only for independent read-only investigations, and keep sequential edits, builds, and tests in the lead chat.

### Pro handoff

Give Pro one GPU-independent question with a known acceptance test. Include the mismatching witness
rows, REA exports, captured values, and exact input identities. Ask for a small patch and predicted values.
Replay it locally, require the target witnesses to match, and pass `--check` before accepting the result.
Local owners perform device CUDA and live-game acceptance when needed.

### Live capture

Finish all available offline work first. Combine the remaining decisions into one capture window and
rehearse the collector and readback on retained files. Send one complete player checklist with equipment,
stance, target, lock-on, re-lock after leaving the equipment menu, ordered actions, and what to report.
Ask the player to reply `开窗`. After that reply, verify the process, arm the collector, and tell the player
when to start the listed actions. Wait for `完成`, then save and verify the readback.

## Local evidence index

The full receipts remain under the host's `F:/Nioh3_r16_build/deliverables/` directory. Paths below are relative to that directory.

| Evidence | Where to resume |
| --- | --- |
| Shared source predictions and search parity | `optimizer-live-samurai-20261120/SHARED_OPTIMIZER_ACCEPTANCE.json` |
| Single-slot equipment comparison | `optimizer-live-samurai-20261120/game-read-Katana-fb3c045cdae143c58b649374f64de7e4/katana-mid-strong-current/cpu/AUDIT_ROWS.jsonl`, ordinals 0 and 1350 |
| Catalog coverage and original single-contact domains | `optimizer-live-samurai-20261120/ALL_WEAPON_COVERAGE.json` and `ALL_TYPE_NUMERIC_MATRIX.json` in the same directory |
| Later Ninja observation and full numerical host acceptance | `optimizer-live-samurai-20261120/ninja-reference-game-read-cddd918b04054ddd8f2375d674c66479/all-ninja/HOST_ACCEPTANCE.json` |
| Source definitions, native competition and evidence limits | `optimizer-action-buff-20261121/rea-source-rules/README.md` and `RECEIPT.json`; REA batches 119–121 under `martial-event-catalog-20261111/` |
| Isolated buff fault controls | `optimizer-action-buff-20261121/buff-controls/RESULT.json` |
| Four-hit cast work in progress | `optimizer-action-buff-20261121/whole-cast/control-preparation/CAST_INPUTS.json` and `whole-cast/replay_witness.py` under the same task directory |
| Wild Wolves II full-hit count | `pro-r2-return/nioh3-damage-formula-r2-pro-return-pc202-20261018/evidence/move-catalog/hits.jsonl`, resource `0x600000158`, action 25299. The player independently confirmed four contacts on a yokai. |
| Spawn references, App row identities and observed Flying Circle controls | `optimizer-action-buff-20261121/projectile-row-bindings/BINDINGS.json`; `martial-event-live-20261111/phase03-flying-circle/OBSERVATION.json`; native selectors in `martial-event-catalog-20261111/rea37` and `rea38` |
| Weapon attack vs. imbue; same-target owned katana pair | `optimizer-spawned-contact-20261121/trial-02/WATER_WEAPON_SCALING.json` |
| All scheduled spawn selectors, ownership branches and per-weapon census | `optimizer-spawned-contact-20261121/trial-02/ALL_SPAWN_REFERENCES.json`; REA37 5C9BE0, REA40 65E908 and REA121 2B4A3C |
| Controlled per-candidate comparisons for two charge branches and cross-weapon contact window census | `optimizer-spawned-contact-20261121/trial-02/PARITY-early.json`, `PARITY-full.json`, `CONTACT_WINDOWS.json`; original game-entry replay in `historical.json` |
| Resource-dependent held attacks | `sustained-actions-20261111/native-app-held-final/report.json`; `event-graph-20261111/matched-resources/archive-01-entry-00091.bin`; `martial-event-catalog-20261111/rea52/event_2b2d88.c` and `rea132/event_28a88c.c` |
| Water Sword II skill check | `optimizer-live-samurai-20261120/dual-product/snapshot.json` and sibling `ordinary-read-memory.json`; `water-tsubame-live-20261111/CURRENT_STATIC_SKILLS.json`; `martial-event-catalog-20261111/rea51/event_20dfc0.c` |

Repeat the projectile binding check from this checkout in PowerShell 7. The output includes the action and damage-row identities and the two observed charge branches.

```powershell
$root = 'F:/Nioh3_r16_build'
$proofs = "$root/deliverables"
$inputs = @(
    "$proofs/event-graph-20261111"
    "$root/worktrees/research-damage/research/damage/evaluator/src"
    'crates/nioh3-loadout/src/pc202_damage_rows.bin'
    "$proofs/martial-event-catalog-20261111/GLOBAL_DAMAGE_BINDING.json"
    "$proofs/martial-event-live-20261111/phase03-flying-circle/OBSERVATION.json"
    "$proofs/optimizer-spawned-contact-20261121/trial-02/ALL_SPAWN_REFERENCES.json"
)
$inputs += @('--action-bank', 'crates/nioh3-loadout/src/pc202_projectile_rows.bin', '--catalog', 'data/loadout/catalog.json')
./tools/run_python_tests.ps1 -Python 'F:/Nioh3_ScrollEditor/.codex_tmp/v2-build-env/Scripts/python.exe' -MinimumFreeGiB 0 -ScriptPath tools/acceptance/check_projectile_row_bindings.py -ScriptArgument $inputs
cargo run --locked --offline --manifest-path crates/nioh3-loadout/Cargo.toml --example check_spawn_references -- $inputs[5]
```
