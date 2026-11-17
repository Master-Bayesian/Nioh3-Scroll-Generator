# PC v2.02 nonzero descriptor-selector validation

Status: PC v2.02 NG3 normal-solo descriptor selector 1 is independently validated.
Production support requires this exact version and the pinned replay table
bytes. Other nonzero selectors, unverified versions and nonzero expedition remain
unknown. Owned E2E results remain distinct from native game evidence.

## Completed native acceptance

Two independently generated entries of seed `100000439` matched the seeded
context, five occurrences, all four source trials, entry/final RNG and persistent
task flags. Both selected wave-2 Red Kappa spawn `3902`; the owner visually
confirmed the possessed occurrence twice. A separate seed `100000029` matched
five non-possessed occurrences and all four failed trials; the owner confirmed
that no enemy was possessed. All three runs proved fresh constructor-to-manager
copies and explicitly verified removal of all owned breakpoints.

A separate bounded read-only comparison proved equality of all **1022** native
enemy lookup keys and eligibility/weight fields, all **20** declared placement
terrain slices, and source configuration `4543`. The frozen v2.01 capture remains
unchanged, with its original provenance. Semantic reuse by v2.02 is certified
separately in `data/enemy_states/pc_v2_02/selector_one_certification.json`.

`DescriptorSelectorSupport` grants the new production path only for PC v2.02,
NG3 normal solo, selector 1 and exact baseline table SHA-256
`1634c08164905d5a980d2ffcf9805c5c34055ad0480850669c77c10a385350c2`.
Unknown inputs still fail closed. Production never looks up a result by seed.
Generation identity revision 2 invalidates cached revision-1 unknown previews.

The owner's observation that one possessed enemy was also purple/One Difficulty
is retained separately. It does not certify a seed-only Curse oracle or the
nonzero expedition path.

## Distinct selectors

- `descriptor_selector`: the generation-context selector used to accept enemy
  candidate rows at `row+0x19`. This work targets value **1**.
- `occurrence_selector_class`: each generated descriptor's `+0x10` byte. The
  normal-solo profile captures source pass **0** only. Earlier expedition/class-1
  controls do not independently certify a nonzero descriptor selector.

The reported seed `141974638` now has a complete supported solo preview: five
occurrences, with only wave-2 Red Kappa marked possessed. Its base roster still
contains the same three species from the player's screenshot. Unverified
expedition previews retain the known base roster and explicit unknown states.

## Identity and source evidence

`selector_source_v202_locators.json` is derived from retained, loaded PC v2.02
code, bound to executable SHA-256
`E22C4A635E4EC1E27A177B76E27D7F6A637F426C0ED3928B60F5693BC52AE130`.
The retained loaded-text digest is
`4CEC8FB6AD867417A76DF8201C1D4F54172443884910463C1953ACAD91AE6C29`.
Protected on-disk `.text` bytes are different and cannot replace this loaded-code
evidence. Historical static identity does not establish a current live match.

The profile pins four hardware execute sites, additional generator/caller/layout
signatures, parent return sites and the executable's **file byte count**. The
legacy `identity.image_size` field means file size, not PE `SizeOfImage`.
The current descriptor selector is at parent `RBP+0x1650`; flags are at
`+0x1658/+0x1660/+0x1668`. The reused byte at `RBP-0xC8` is not that selector.
The 12-byte normal request stores playthrough at `+7`, not `+6`.

The runner preserves the original v2.01 phase admission and validator default.
Only the new named phase selects the v2.02 profile. It never scans for a close
match or silently retargets a CE session.

## Explicit capture states

`prepared -> armed -> origin_observed -> trials_observed -> materializing -> captured`

Zero source trials are allowed when native eligibility skips every descriptor.
Errors enter `rejected`; deadline/manual interruption enters `interrupted`.
Breakpoint ownership and cleanup are separate state: a captured run cannot pass
validation until cleanup is explicitly verified with an empty global inventory.
An unexpected preassigned flag is retained in the origin event before rejection.

The observer records live context, complete descriptor identity/order, native
eligibility/configuration, the generator-local RNG, source tickets, fresh
constructor copies and persistent manager-owned task flags. The thread-routed
global RNG is retained as a diagnostic; it is not substituted for the native
generator's consumed stream. No target memory, RNG or flag is written and no
game function is called by the observer.

## Repeatable offline E2E

Use the prepared project Python and a Lua 5.3 shared library matching the host:

```powershell
$env:CARGO_TARGET_DIR = '<shared-cargo-target>'
./tools/run_python_tests.ps1 -Python '<prepared-python>' -ScriptPath tools/verify_selector_source_v202_e2e.py -ScriptArgument @('--output-directory','<fresh-evidence-directory>','--lua-library','<lua53-library>')
./tools/check_models.ps1 -Evidence '<quality-evidence-directory>'
```

The E2E runs the actual Rust seed probe, the actual Lua observer in an owned
memory adapter and the actual validator CLI. It preserves predictions, raw
capture/cleanup fixtures, rejection outputs, source hashes and an artifact
manifest. Controls include wrong identities/signatures/threads, foreign
breakpoints, partial arming, failed timers, timeouts, preassigned flags, missing
links, unknown cleanup and a self-consistent causal trace that disagrees with
the independently seeded model. Owned fixtures are refused by default and
require `--allow-owned-fixture`; that mode never reports native parity.

The TLA+ capture model additionally checks the same admission/trace/seed/cleanup/
native-evidence obligations with exact named counterexamples when each guard is
disabled. These abstractions check the certification protocol, not native game
semantics.

## Native entry procedure

1. Run the seeded Rust probe before observing the game:
   `cargo run --release --locked --manifest-path crates/nioh3-worker/Cargo.toml --example selector_wraith_audit -- data 2.0.2.0 <prediction.json>`.
2. Prepare a different recognizable scroll through the existing protected save
   flow, with an encrypted-copy rehearsal, exact source hashes, whole-file backup
   and post-commit inventory/receipt readback. Do not ask the owner to find IDs.
   Candidate `100000439` predicts a wave-2 Red Kappa positive, three waves and an
   Onryoki boss. Its first effect is Melee Ki Damage, level 180, rarity 4, six
   challenges. This is a prediction, not a visual observation.
3. The owner loads the intended character and stops at scroll selection. Load
   the existing CE MCP plugin and use its local bridge. If Codex exposes no CE
   tools, the same installed bridge package is available through the runner's
   explicit `--bridge-python-root`; no Codex restart is needed for that route.
4. Run `run_possessed_enemy_observer.py --phase selector-source-v202 --pid <PID>
   --seed 100000439 --run-id <unique-run-id> --mode-label normal-solo
   --output <fresh-capture.json> --bridge-python-root <ce-package-python-src>`
   through the prepared Python. Do not enter the mission until `ARMED` has been
   printed after live signature/identity/breakpoint checks. The observer's
   collection window is 120 seconds.
5. The owner enters normally. Preserve the complete capture and the separate
   `.cleanup.json` receipt. Check which wave-2 Red Kappa actually has the game's
   possessed appearance; purple enemy-list text alone is not a complete oracle.
6. Run `validate_selector_source_v202.py <capture.json> --cleanup <cleanup.json>
   --prediction <prediction.json>`. It first checks causal assignment/copy
   provenance, then independently compares the seeded context, roster, entry
   RNG, trial order/tickets, native gates and final flags. Captured state is never
   used to seed the Rust model.
7. Retain independent entries, negative/boundary coverage, complete parameter
   comparisons and owner visual confirmation before changing a support policy.
   A pass for one entry does not authorize a different version or mission mode.

The raw captures, source identities, cleanup receipts, owner observations and
repeatable commands for this acceptance are retained under the task's
`possessed-selector-20261115` deliverables directory. The portable certification
fixture omits personal paths, account data and live memory addresses. The
production worker E2E compares the native-observed occurrences, runs a genuine
bounded possession-filter search, and verifies legacy-version, expedition and
changed-table guards. The UI E2E additionally preserves synthetic base-only,
unknown and empty fallback controls.
