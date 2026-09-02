# Project status — 2026-09-02

## Public baseline

The public baseline is stable v0.6.9, published on 2026-09-02 after the local
test suite, packaged startup checks, and the signed GitHub release workflow.

## Completed in the current working tree

| Area | Status | Evidence boundary |
| --- | --- | --- |
| Complete effect generation | Implemented for supported rarities 3, 4, and 5 | Exact offline generation and existing native parity suites. |
| Pro effect-path inversion | Implemented | Native C++ preimage accelerator plus exact full-record replay; R4 final results remain finalizer-verified. Product routing no longer mistakes the incomplete R4 stage-one inverse for a final-record inverse. |
| NVIDIA acceleration | Implemented and locally exercised | CUDA and DirectCompute paths exist; exact replay remains the acceptance gate. |
| Vendor-neutral GPU path | Implemented | Direct3D 11 compute supports AMD, NVIDIA, and Intel adapters. |
| Structural legality | Implemented and documented | Effect slots/conflicts/categories, enemy classes/roles, and special-rule ordering/budgets stop proved-impossible requests before search. |
| Free local effect editing | Implemented | All seven effect slots accept unrestricted IDs, values, prefix, metadata, and tail fields without Seed or legality checks. |
| Local header editing | Included in v0.6.5 | Mapped type/playthrough, mirrored level, mirrored recommended level, Seed, mirrored rarity, transfer count, and all seven unrestricted effect slots are written in one backup-gated transaction. Terrain, grouped enemies, and ordered special rules are previewed from Seed plus playthrough. |
| Player enemy tiers | Included in v0.6.5; expanded after v0.6.6 | The 142 native localized identities expand to 148 player-facing entries after exact same-name forms are separated. Low/middle/high generation-pool columns and the read-only guide retain must-contain semantics rather than pretending a chosen enemy is a complete composition. |
| Stable/Beta update selection | Included in v0.6.4 | Stable uses GitHub `releases/latest`; opt-in Beta compares the latest signed prerelease and stable release. |
| FB-014/015 | Included in v0.6.4 | Shinatsuhiko Grace naming and human-readable category errors have regression coverage. |
| FB-016 | Corrected in v0.6.6 | Live testing disproved the earlier `+0x1C` hypothesis and proved a `+0x28` generation-serial collision with an equipment record. New installs allocate against native equipment serials, and affected scrolls are repaired before insertion. |
| Combined terrain filtering | Expanded in v0.6.9 | Search exposes complete visible terrain results as an OR multi-select, including one aggregate option for every result containing Hell. Runtime editing retains exact `0x08` Crucible/Foulblooded and `0x2D` Crucible/Fire presets. |
| Exact same-name enemy forms | Implemented after v0.6.6; awaiting UI acceptance | Yamagata, Takeda, Hiruko, Kanai, and both Hattori identities are separate lookup-key constraints in search, runtime editing, and trilingual catalogs. The two Hattori identity labels remain provisional pending future player feedback. |

## Partially complete

| Area | Present capability | Remaining work |
| --- | --- | --- |
| AMD optimization | D3D11 compute backend and AMD adapter selection are implemented; an AMD integrated GPU passed local parity | Run correctness, throughput, cancellation, and memory-pressure tests on at least one AMD discrete GPU. Integrated-GPU evidence is not a discrete-GPU performance claim. |
| Unrestricted auxiliary editing | Included experimentally in v0.6.5; live hook installation/removal passed | The post-v0.6.6 working tree now writes both each enemy lookup key and its exact native role. Complete a new hit-backed detail/challenge acceptance pass for that change, terrain, and ordered rules. The UI explicitly states that these fields are not saved and will revert. |
| Search UX/performance | Results stream incrementally, event queues are bounded, structural preflight exists, and generic pivot construction is GPU-first. R4 one/two-group requests run the exact GPU finalizer; three or more groups retain the cheaper lossless `N-1` stage filter. Stable v0.6.8 fuses all auxiliary filters into bounded native CUDA calls, embeds precompiled DirectCompute shaders, and automatically continues internal pages until the requested result count. Stable v0.6.9 adds full right-pane and candidate-list scrolling. No-CUDA native CPU work requires explicit user consent. | Run manual packaged performance acceptance. Replace the Tk frontend in v0.7 to address resize, sash-drag, and scroll repaint latency; Electron is the current preferred candidate. |
| Special-rule localization | Legal native rows are filtered and most automatic-activation items are localized | Native item key `0x3011` remains unresolved and must stay visibly marked rather than guessed. |

## Frozen archival research

### Challenge-completion reroll prediction

This work was explicitly abandoned as a product TODO on 2026-09-01. The
paid/manual reroll candidate path has a static offline model, but it lacks the
required live native parity vector. The post-challenge four-slot replacement
mechanism is separate and still has unresolved candidate-object state and RNG
semantics. Preserve the frozen evidence and code, but do not resume research or
promise “N clears later this effect will appear.”

## Not complete

### Enemy empowered and possessed variants

Research for the release after PC v2.01 compatibility must determine the
native identity/state mechanism behind purple or empowered enemies and whether
possessed Underworld forms of ordinary enemies can be selected independently.
No inferred variant is eligible for the player catalog before live validation.

### AMD discrete-GPU acceptance

The backend is written, but discrete AMD hardware acceptance and performance
measurement remain outstanding.

### Unrestricted local auxiliary overrides

Enemy groups, terrain, and special rules are constructed into a runtime
descriptor and are not independent fields in the saved `0xE8` record. The
current Seed editor only selects another native combination. Deliberately
impossible layouts, including repeated bosses that violate native class rules,
require an application-owned override profile and a version-gated runtime hook.
The verified post-construction boundary is RVA `0x20DD558`, where `RBP` points
to the completed descriptor and `R12D` still contains the displayed Seed.

The three-Ichimokuren override has now been proved in both the detail UI and an
actual challenge. A settlement/save experiment then restored the native enemy
composition as soon as the hook was removed. Native serializer probes captured
the plaintext inputs for both character and system saves; neither contained a
direct unencoded auxiliary companion table in the tested dense or fixed-stride
layouts. Encoded or indirectly indexed persistence remains unexcluded, but the
current evidence supports an application-owned external override profile as the
practical persistence design rather than pretending the canonical record can
store illegal auxiliary fields. Product integration still requires a clean
process-start/process-exit safety test and fail-closed game-version gating.

Version 0.6.6 retains the bounded temporary variant of that design, adds exact
native combined-terrain presets, and keeps it explicitly experimental. It
matches one displayed Seed, verifies the exact v2.00.02 hook bytes,
reuses only native enemy-vector capacity, and can overwrite repeated enemy
groups, terrain, and three ordered special-rule keys. The application restores
the original instruction when the user stops the override or closes the app;
the retired 4 KiB trampoline remains allocated until game exit to avoid an
unload race with a thread that already entered it. Live installation and removal
passed against a running game process. A new application-owned hit still needs
detail/challenge observation before this feature can be promoted from
experimental to accepted.

Static descriptor analysis after v0.6.6 identified a concrete consistency bug:
an inner enemy entry stores its lookup key at `+0x04` and exact native role at
`+0x08`. The earlier hook changed only `+0x04`; the current working tree changes
both while preserving the dynamic `+0x0C` payload. This is a plausible cause of
"details changed but challenge enemy missing," but source tests are not live
acceptance. Separately, a zero application hit still means that the game reused
a cached descriptor and never crossed the hooked construction boundary.

## Next recommended order

1. Live-check the exact-role enemy overwrite after the application reports at
   least one hook hit, then finish detail/challenge validation for terrain and
   ordered special rules. Treat zero hits as a cache/reconstruction miss rather
   than a successful override.
2. Collect additional AMD-discrete and Intel hardware evidence when those
   systems are available; do not infer it from NVIDIA or integrated-AMD runs.
3. After this release, replace the Tk frontend in v0.7; Electron is the current
   preferred candidate pending an architecture decision.
4. Validate and tune the D3D11 accelerator on an AMD discrete GPU only when
   suitable hardware becomes available.
