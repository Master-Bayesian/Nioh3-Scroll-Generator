# Project status — 2026-09-01

## Public baseline

This status ships with the v0.6.6 source tag. The GitHub Release is published
only after its tag completes the signed Windows release workflow.

## Completed in the current working tree

| Area | Status | Evidence boundary |
| --- | --- | --- |
| Complete effect generation | Implemented for supported rarities 3, 4, and 5 | Exact offline generation and existing native parity suites. |
| Pro effect-path inversion | Implemented | Native C++ preimage accelerator plus exact full-record replay; R4 final results remain finalizer-verified. |
| NVIDIA acceleration | Implemented and locally exercised | CUDA and DirectCompute paths exist; exact replay remains the acceptance gate. |
| Vendor-neutral GPU path | Implemented | Direct3D 11 compute supports AMD, NVIDIA, and Intel adapters. |
| Structural legality | Implemented and documented | Effect slots/conflicts/categories, enemy classes/roles, and special-rule ordering/budgets stop proved-impossible requests before search. |
| Free local effect editing | Implemented | All seven effect slots accept unrestricted IDs, values, prefix, metadata, and tail fields without Seed or legality checks. |
| Local header editing | Included in v0.6.5 | Mapped type/playthrough, mirrored level, mirrored recommended level, Seed, mirrored rarity, transfer count, and all seven unrestricted effect slots are written in one backup-gated transaction. Terrain, grouped enemies, and ordered special rules are previewed from Seed plus playthrough. |
| Player enemy tiers | Included in v0.6.5 | All 142 native display identities are separated into low/middle/high generation-pool columns; the read-only guide lists the ten real group structures and keeps must-contain semantics distinct from a complete composition. |
| Stable/Beta update selection | Included in v0.6.4 | Stable uses GitHub `releases/latest`; opt-in Beta compares the latest signed prerelease and stable release. |
| FB-014/015 | Included in v0.6.4 | Shinatsuhiko Grace naming and human-readable category errors have regression coverage. |
| FB-016 | Corrected in v0.6.6 | Live testing disproved the earlier `+0x1C` hypothesis and proved a `+0x28` generation-serial collision with an equipment record. New installs allocate against native equipment serials, and affected scrolls are repaired before insertion. |
| Combined terrain filtering | Included in v0.6.6 | Search accepts multiple required visible terrain effects, preflights the native-row intersection, and preserves both display effects in previews. Runtime editing exposes exact `0x08` Crucible/Foulblooded and `0x2D` Crucible/Fire presets. |

## Partially complete

| Area | Present capability | Remaining work |
| --- | --- | --- |
| AMD optimization | D3D11 compute backend and AMD adapter selection are implemented; an AMD integrated GPU passed local parity | Run correctness, throughput, cancellation, and memory-pressure tests on at least one AMD discrete GPU. Integrated-GPU evidence is not a discrete-GPU performance claim. |
| Unrestricted auxiliary editing | Included experimentally in v0.6.5; live hook installation/removal passed | Complete application-owned detail/challenge acceptance for terrain and ordered rules. The UI explicitly states that these fields are not saved and will revert. |
| Search UX/performance | Results stream incrementally, event queues are bounded, structural preflight exists, complete unrestricted-primary R4 requests use the Pro inverse, and generic fixed-draw construction prefers DirectCompute | Replace the Tk frontend in v0.7 to address resize, sash-drag, and scroll repaint latency instead of extending the current workaround stack. |
| Special-rule localization | Legal native rows are filtered and most automatic-activation items are localized | Native item key `0x3011` remains unresolved and must stay visibly marked rather than guessed. |

## Not complete

### Challenge-completion reroll prediction

The paid/manual reroll candidate path has a static offline model, but it lacks
the required live native parity vector. The post-challenge four-slot replacement
mechanism is separate and still has unresolved candidate-object state and RNG
semantics. The current product must not promise “N clears later this effect will
appear.” Resume from the revised reroll freeze only.

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

## Next recommended order

1. Finish live detail/challenge validation of the application-owned temporary
   auxiliary override, including terrain and ordered special rules.
2. Validate the D3D11 accelerator on an AMD discrete GPU and tune batch sizes
   only from measured throughput and memory behavior.
3. Resume challenge-completion reroll research with a clean candidate-object
   capture before confirmation, then require a second independent native vector.
4. Re-run packaged UI responsiveness checks and release the accumulated working
   tree only after user acceptance.
