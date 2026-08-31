# Project status — 2026-08-31

## Public baseline

This status ships with the v0.6.4 source tag. The GitHub Release is published
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
| Stable/Beta update selection | Included in v0.6.4 | Stable uses GitHub `releases/latest`; opt-in Beta compares the latest signed prerelease and stable release. |
| FB-014/015/016 | Included in v0.6.4 | Shinatsuhiko Grace naming, human-readable category errors, and save-wide item-instance-key collision repair have regression coverage. |

## Partially complete

| Area | Present capability | Remaining work |
| --- | --- | --- |
| Local scroll editor | Unrestricted effect-slot editing, record selection, backups, restore, and delete | Add direct terrain, enemy, special-rule, level, recommended-level, rarity/type, and other auxiliary/header field editors. These controls are local-only and must not enforce Seed legality. |
| AMD optimization | D3D11 compute backend and AMD adapter selection are implemented; an AMD integrated GPU passed local parity | Run correctness, throughput, cancellation, and memory-pressure tests on at least one AMD discrete GPU. Integrated-GPU evidence is not a discrete-GPU performance claim. |
| Search UX/performance | Results stream incrementally, event queues are bounded, structural preflight exists, and native/GPU preimages reduce the domain | Re-test low-end-machine responsiveness and the remaining Tk resize/scroll repaint lag with the packaged build. |
| Special-rule localization | Legal native rows are filtered and most automatic-activation items are localized | Native item key `0x3011` remains unresolved and must stay visibly marked rather than guessed. |

## Not complete

### Challenge-completion reroll prediction

The paid/manual reroll candidate path has a static offline model, but it lacks
the required live native parity vector. The post-challenge four-slot replacement
mechanism is separate and still has unresolved candidate-object state and RNG
semantics. The current product must not promise “N clears later this effect will
appear.” Resume from the revised reroll freeze only.

### Full local auxiliary editing

The editor does not yet expose direct mutation of terrain, enemies, or special
rules. This is the largest non-reroll user feature still missing from the prior
TODO list.

### AMD discrete-GPU acceptance

The backend is written, but discrete AMD hardware acceptance and performance
measurement remain outstanding.

## Next recommended order

1. Complete unrestricted local auxiliary/header editing and add exact byte-diff
   regression tests for every exposed field.
2. Validate the D3D11 accelerator on an AMD discrete GPU and tune batch sizes
   only from measured throughput and memory behavior.
3. Resume challenge-completion reroll research with a clean candidate-object
   capture before confirmation, then require a second independent native vector.
4. Re-run packaged UI responsiveness checks and release the accumulated working
   tree only after user acceptance.
