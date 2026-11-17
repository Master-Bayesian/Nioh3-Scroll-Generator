# Nioh3 CI quality policy

This policy is maintained in the versioned `skills/nioh3-ci-quality/` package. Its user-skill installation is a byte-identical deployment copy. Project documentation and local `AGENTS.md` files route here rather than maintaining parallel policies.

## 1. Scope and adoption

Reduce repeated work and critical-path time while preserving declared correctness requirements. Prefer meaningful E2E verification for complex behavior; retain existing required tests and add observable fault controls where they establish a real boundary. Documentation-only work needs instruction/metadata validation, not a new game session.

The tool and scheduling changes below are adoption targets, not a claim that they already exist in the workflows. Until a replacement is implemented and accepted, the checked-in gate commands and profiles remain the executable baseline. Never use this document alone as evidence that a narrower CI run is sufficient.

The baseline inspected at product 0.8.12 uses Node 24/npm, TypeScript 5.9.3, ESLint/typescript-eslint, Node tests through `tsx --test`, standalone Playwright scripts, and Rust 1.99.0. Read current manifests and workflows for actual versions; these observations are not permanent pins for every worktree.

## 2. Tools and quality boundaries

| Concern | Project decision |
| --- | --- |
| Frontend lint and types | Evaluate pinned Oxlint with `--type-aware --type-check` as one primary static pass. Keep the checked-in ESLint/tsc gates until equivalence is demonstrated. Retain a narrowly scoped ESLint supplement only for demonstrated gaps. |
| Bun check | Use as a TS 7 compatibility/reference checker or a measured alternative. Do not permanently duplicate equivalent full type analyses without a demonstrated benefit. |
| Frontend tests | Retain Node/tsx and Playwright. Vitest is not a prerequisite for input-aware selection or timing-based sharding; migrating runners needs an independent demonstrated benefit. |
| Frontend build | Retain esbuild and the current Node/npm build/package workflow during static-check migration. A checker change is not a package-manager/runtime migration. |
| Rust | Keep compiler checks, Clippy with the required targets/features and warnings treated as errors, plus `cargo fmt --check`. Do not replace frontend lint with Clippy. Avoid a redundant `cargo check` for an identical Clippy configuration unless it covers a distinct contract. |
| Native/Python inputs | Preserve their applicable existing syntax, build/provenance, ABI and runtime/parity gates. Clippy does not validate Python, CUDA, C++ or HLSL source. |
| Stateful protocols | Use named states, events, ownership, guards and terminal/recovery outcomes. Validate mapped TLA+ invariants/actions with the correct model and exact named fault controls, then run the affected E2E. |
| Product acceptance | Builds, fixtures, packaged-app execution, native observations and real game/save acceptance establish different facts. Record each scope and reuse applicable existing evidence before requesting new captures. |

Enable applicable correctness, suspicious, performance, React and type-aware rules broadly. Keep necessary exceptions narrow and explained. Do not blindly enable contradictory rule groups: Clippy explicitly discourages enabling the whole `restriction` group.

The current frontend baseline explicitly enforces these four semantic rules and has positive, negative and disabled-rule controls:

- Exhaustive finite-state switches, including a control where a default branch must not hide an omitted state.
- Unhandled/floating promises.
- Promises used as ordinary conditions or callbacks with incompatible handling.
- Awaiting a non-awaitable value.

Migrate and expand that coverage rather than treating these four rules as a comprehensive lint configuration. Audit existing test-directory and disable-directive exceptions. A migration must check the actual project files, not pass because parsing failed, a file was excluded, or a rule never ran.

The read-only Bun 1.4.4 probe on the 0.8.12 code checked 91 files. The only initial errors were two ordinary CSS side-effect imports; a temporary CSS module declaration made the check pass. No project file changed. This establishes bounded compiler compatibility, not full lint/build/E2E migration acceptance. The current typescript-eslint 8.47 peer range is below TS 6; do not blindly bump the whole dependency stack to 7.

When migrating a checker: preserve strictness, input coverage and required diagnostics; add precise asset declarations instead of disabling their checks; prove editor/linter compatibility; and compare the relevant faults before retiring a previous gate. Validate intended product-version edits and preserve unrelated dependency locks/checksums.

## 3. Measure before changing scheduling

Collect actual step and test/case durations from an identified commit, profile and runner. Distinguish execution from queue/setup, cold from warm caches, and wall-clock improvement from runner-minute cost. Do not sell a warm local checker measurement as end-to-end CI time.

The 0.8.12 baseline illustrates the priority: frontend foundation took 8m18s, with about 5s of type checking, 2m49s of text-fit checks and 1m54s of expanded UI checks; the Rust job took 21m19s. Timing-aware shards, independent gates, shared preparation and event deduplication therefore precede tool-name substitutions.

Treat these as separate datasets:

- **Duration map:** historical execution costs keyed by test/scenario identity, profile and relevant runner configuration.
- **Dependency/input map:** which code, data, generated files, binaries or configuration can affect a test.

Do not confuse a duration map with dependency tracing. Heavy tests that mount the real route tree legitimately remain selected for many changes. Use sharding to reduce that path rather than removing their coverage.

## 4. PR selection and full-run rules

| Event | Required behavior after the planner is accepted |
| --- | --- |
| Pull request | Run applicable mandatory static/formal gates and tests selected by validated impact analysis. Record a proven not-applicable domain explicitly, not as a passed test. Unknown or cross-domain impact falls back to complete validation. |
| Merge queue (`merge_group`) | Run the complete declared normal profile against the actual grouped source. Do not reuse PR pruning for the integration commit. |
| Main | Run the complete declared normal profile, including commits previously validated as PRs. |
| Formal release | Require successful exact-source CI and the complete release profile, including extended packaged acceptance, immutable-artifact promotion and public verification. |
| Manual/diagnostic run | Follow its declared profile; a requested full run overrides pruning. A partial diagnostic run cannot stand in for a required full gate. |

"Complete" refers to a maintained inventory of required tests, scenarios, locales/themes/viewports, target/feature combinations and model fault controls. It does not imply unsupported game versions or optional research probes were verified. Unavailable required native/hardware evidence remains pending; a fixture cannot substitute for it.

The current workflow path filters and lack of a merge-queue trigger are baseline facts, not implementation of this target. Do not silently grandfather exclusions into a newly advertised full-run policy. Build the inventory and required-check matrix before enabling pruning.

For PR analysis:

1. Resolve explicit base, head and tested-source SHAs and compute their appropriate merge-base diff. Fetch required history; bare `--changed`, only `HEAD~1`, or directory names do not establish PR impact.
2. Follow transitive dependencies in the direction from changed inputs to consuming tests. Include the old and new sides of deletions/renames; invalidate incompatible graph/cache records.
3. Declare non-imported inputs explicitly. Standalone browser harnesses load generated HTML and invoke builds/worker processes: an import graph alone cannot see those dependencies.
4. Treat lockfiles, compiler/linter/test/build configs, CI orchestration, shared setup, contracts/schema/client generators, generated clients/locales, resource/native tables, native ABI/build inputs and their relevant environment/profile changes as full-run triggers for their complete consuming domains. Ambiguous domain boundaries require a full repository profile.
5. Unknown paths, missing/stale graphs, unresolved dynamic inputs or missing merge-base history select the full suite. If the complete inventory or required runner cannot be obtained, block the gate rather than emitting an empty green plan.
6. A zero-test selection needs a recorded, validated reason covering every changed path. Static/formal requirements still apply. New tests must be discovered and assigned; missing timing data never makes a test disappear.

Represent selection with named `Full`, `Affected` and `Blocked` variants and explicit reasons/evidence, not independent skip flags. Keep lifecycle stages and aggregation outcomes explicit when the implementation becomes stateful.

## 5. Shards and parallel execution

- Use historical durations to balance estimated work, for example longest-first assignment to the least-loaded shard. Record the algorithm and version; file counts alone are not a timing plan.
- Give new/untimed tests a conservative nonzero cost. Preserve slow tests and timeouts in the execution inventory; an early failure or timeout is not evidence that the test is cheap.
- A single oversized UI script may be split by existing locale/theme/viewport/scenario boundaries. Preserve assertions and state isolation and show that every original required case is still executed exactly once.
- Prepare dependencies, generated contracts/locales and immutable build inputs before consumers. Generate/build shared artifacts once where their complete input/toolchain/target/profile identities permit reuse.
- Run independent static gates and test shards concurrently. Keep genuine declaration/build/native-worker prerequisites in the DAG. Avoid simultaneous generation into shared paths or Cargo invocations contending for one mutable target.
- Use isolated mutable state, ports, output directories and owned synthetic saves per job/test as applicable. Cache build artifacts, dependencies, graphs and timing data with complete identities; do not cache or fabricate a current pass verdict.
- Remove redundant development-branch push/PR full runs while preserving PR, main, merge-queue and explicitly requested validation. Do not cancel required final-source validation merely to improve a timing number.
- Rust may shard by manifest and heavy E2E boundary with appropriate features and target coverage. The current seven-manifest CI baseline omits a separate launcher static job; the target inventory must account for the six crates plus Tauri and the one-file launcher, preserving its native release acceptance.

## 6. Final gate and evidence

The final gate runs even when prerequisites fail or are cancelled. It verifies:

- Decision mode/reasons, base/head/tested SHA, inventory version, input and graph identities, toolchain and validation profile.
- The complete expected selected set, unique shard assignments and matching case/configuration identifiers.
- Every required report belongs to this source/profile, exists, is readable and reports a completed successful execution.
- Failed, missing, cancelled, stale-source or incomplete reports prevent success. A verified not-applicable decision is distinguishable from a test pass.

Use named aggregator outcomes such as `Planned`, `Running`, `Passed`, `Failed` and `Incomplete` when modeling the orchestration. New stateful selection/aggregation protocols require source-mapped formal safety checks and exact deliberate faults, just like product lifecycle protocols.

Retain a repeatable artifact containing the decision, changed-file inventory, input/graph identities, selected/skipped reasons, shard assignments, observed timings, diagnostics, formal controls, E2E receipts and repeat commands. Native/resource identities must remain verifiable across LF/CRLF checkouts. Cache timing never proves coverage.

Do not meet a speed target by disabling required rules/assertions, accepting parser errors, increasing timeouts without a justified contract change, losing locale/native scenarios, or treating missing reports as green.

## 7. Rollout and acceptance

1. **Inventory and baseline:** measure the current graph and duplicate work. Keep Node/npm/esbuild, Node/tsx and Playwright. Read actual branch versions and manifests; do not import unpublished feature work wholesale.
2. **Parallelism and duration shards:** preserve the complete existing inventory while changing scheduling. Demonstrate unique coverage, isolated state and reproducible timing/cost results.
3. **Affected-selection shadow mode:** continue the full reference run while computing/practicing selected runs. Exercise leaf/shared code, route-tree changes, deletions/renames, non-imported inputs, absent/stale graphs, absent history, new tests and missing/cancelled/stale shard reports. Fix observed misses before pruning can gate PRs.
4. **Static-tool migration:** pin versions, validate configuration/file/diagnostic equivalence and all critical fault controls, then compare measured alternatives. Prefer one proven primary frontend type-analysis path; remove temporary duplicate checks only after evidence supports it.
5. **Enable optimization:** keep a force-full path, validate merge-queue/main/release events, and retain the final aggregation/evidence gate. A two-minute ordinary frontend PR is a measurement target, not a promised duration for Rust, full integration or signed release.

For each step, report what changed, measured performance, checks actually run and remaining prerequisites. Keep product acceptance separate from CI planner/skill validation. A documentation-only adoption does not implement these stages.

## 8. Release and operational boundaries

Use the current checkout's prepared Python runner and build-root resolver. On this host the normal external root is `D:\Nioh3_v080_deliverables`; honor explicit `NIOH3_BUILD_ROOT`/`CARGO_TARGET_DIR`. Keep artifacts in task-specific `deliverables/` folders, preserve unrelated work and avoid global runtime/model/package-manager configuration changes.

Retain the repository's signed preparation and exact six-asset promotion path for a formal release. Require the complete declared release inventory and `extended_search=true`, verify actual report/source/outer-artifact identities, promote the verified bytes without rebuilding/re-signing, and verify public downloads/signature/feed. Investigate partial publication before any retry; do not overwrite immutable tags/assets or infer authorization for a new release from a previous one.

The skill grants no additional permission. Continue routine work within the user's existing authorization without repeated confirmation. Request additional live/external action only when its scope or authorization is actually missing, after preparing a concrete reviewable result. CI uses owned fixtures and must not dispatch real game/user-save mutations.

## 9. Primary references

- [Bun type checking and TS-version compatibility](https://bun.com/docs/runtime/check)
- [Oxlint type-aware linting and combined type checking](https://oxc.rs/docs/guide/usage/linter/type-aware.html)
- [Clippy lint groups and restriction guidance](https://doc.rust-lang.org/clippy/)
- [Vitest changed-test semantics](https://vitest.dev/config/changed) and [sequencer customization](https://vitest.dev/config/sequence), as references for selection/sharding semantics rather than a required migration

Consult current official documentation and the pinned local tool versions when implementing a migration; these links are not a claim that every future version is compatible.
