# v0.7.0 release-readiness review — September 9, 2026

Historical checkpoint: see [v0.7.0 pre-push completion](V070_PREPUSH_COMPLETION_20260909.md) for the current status. Pending lists below describe their original snapshot.

## Decision

The repaired tree is suitable for a selectively reviewed development-branch
push and release-candidate testing. It is **not yet signed off as the stable
v0.7.0 release**. No commit, push, tag or publication was performed.

This was a targeted end-to-end review of the current V2 implementation, not a
repeat of the frozen RNG/global reverse-engineering audit. The previous Astra
audit and B1-B6 freeze were checked as context. Real-game acceptance from the
preceding session is retained at its original scope; this review used source,
synthetic encrypted saves and isolated workers, not new player-save writes.

## Confirmed defects repaired

| Priority | Finding and consequence | Repair and evidence |
|---|---|---|
| P1 | Production cart UI still rejected every non-NG3 live candidate, despite the accepted NG1/NG2 backend. | Permit NG1-NG3; retain the NG4/NG5 research-install block. Full review UI is now type-checked. |
| P1 | Live backups existed only in an internal operation directory and could not be selected in backup management. | New live backups use the normal backups directory and account/slot/hash manifest. Existing restore validation is retained; regressions check manager discovery and missing/corrupt-backup dispatch rejection. Historical experiment backups are not silently moved. |
| P1 | A batch interrupted after a child dispatch remained uncertain forever even after that child was independently verified. | Status derives complete/partial from durable children under the execution lock. Explicit UI inspection recovers uncertain children before refreshing batch state. Regression proves no second insertion or batch replay. A genuinely unresolved child stays uncertain. |
| P1 | Changes to the production review UI did not trigger V2 CI, and the TypeScript project excluded that UI. Packaged legacy smoke targeted controls absent from the default production screen. | Include `apps/**` in CI paths and review TS/TSX in strict checking. Keep legacy workbench smoke explicit; add the actual connected UI workflow in source and package modes. Remove the personal Python path from the connected runner. |
| P1 | New early-R3 native completion behavior retained the old frozen algorithm identity, allowing caches to appear current across changed semantics. | Bump GenerationContext algorithm identity to `scroll-generation-v0.7-native-completion-1`; retain numerical implementations and R4 dual records. |
| P2 | An idle character can have non-idle scheduler bits for a fraction of a frame, causing avoidable preflight failures. | Bounded read-only wait, at most 0.5 seconds. Actual dispatch still rechecks the complete reviewed state and exact idle flags. No retry after a claimed/uncertain insertion. |
| P2 | The advertised source launch script still opened the engineering workbench, and the optional CE launcher did not select CE after native became the default. | Source launcher selects the connected review UI. CE research launcher explicitly sets `NIOH3_LIVE_ADD_EXECUTOR=ce`; the product default remains CE-free. |
| P1 for repository preparation | `.codex_tmp` was unignored, exposing local build/research state to broad staging. Native verification sources were unintentionally ignored. | Ignore local state and explicitly allow the reviewed native test drivers. No private files deleted or staged. A copied source-only inventory passed focused tests without captures/deliverables/build state. |
| P2 | The current handoff and README still said to wait for Figma and contained conflicting current/pending live-add claims. | Replace the entry handoff with a concise current checkpoint; preserve all previous text in a clearly historical document. |

## Verification performed

- Before repairs: full Python discovery **553 passed**; Node suite **31 passed**.
- After repairs: full Python discovery **557 passed**; strict TypeScript passed,
  including the production review screen.
- Independent copy of 496 tracked/unignored source candidates, about 19.9 MiB:
  **46 focused tests passed**, without local captured data or build state.
  This inventory is not an instruction to stage every candidate file.
- Connected production UI: **17 source checks passed** with a real encrypted
  synthetic container. Covered search, retained cart candidates across pages,
  selected addition, reviewed edit, multi-delete, backup restore/recycle,
  private API rejection and no renderer exceptions.
- Native accelerator source/DLL/ABI identity verified unchanged.
- Current package build and complete 152-file manifest verification passed.
  Four protected IPC/timeout/cleanup tests passed against the final binaries.
- The first packaged UI run completed search/edit/cart addition but timed out
  taking a screenshot. That run is not counted as a full pass. Screenshots are
  now explicitly opt-in; functional assertions remain mandatory. The final
  functional rerun passed all **17 packaged UI checks** on the final artifact.
- Source/packaged worker parity passed for R3/R4/R5 with CPU fallback allowed;
  this is not a GPU-only or all-hardware acceptance claim.

Local evidence: `deliverables/frontend-v2/v070-audit-*.log`,
`v070-reviewed-*.log`, `v070-clean-source-inventory.json`, and
`v070-reviewed-code-identity.json`.

Final local artifact:
`deliverables/frontend-v2/portable-v2-v070-reviewed-20260909/`.
Manifest SHA256: `fb38493922d0afc3a524f88c643d1438f569ef08b9ca4bb162b45238fe43814c`.
It remains `0.7.0-dev.0`, unsigned and unpublished.

## Remaining stable-release gates

1. **Production UI live-batch acceptance.** The nine-cell real-game matrix used
   application services and the native executor. Complete one bounded actual
   cart subset through the final packaged UI, then save/reload. Cover a safe
   pre-dispatch rejection and receipt inspection through the same UI. No need
   to repeat all nine generation cells or farm/challenge scrolls again.
2. **Native executor failure evidence.** Happy-path insertion, no-call dispatch,
   wrong-caller rejection and synthetic disconnect/replay tests exist. Still
   exercise active debugger conflicts, game exit during ownership, and injected
   context/cleanup failures against the synthetic executable. These are missing
   acceptance cases, not claims that observed player operations corrupted data.
3. **Whole-package update acceptance.** Signature/origin/hash unit checks and
   package integrity checks do not prove the PowerShell extract/replace/rollback
   sequence. Exercise local signed fixtures for successful replacement, bad
   archive, locked target and rollback, and test a beta feed before stable use.
   No real signed V2 rollout is claimed here.
4. **Release preparation.** Node still says `0.7.0-dev.0`, while the shared Python
   product version remains `0.6.10`. Choose and synchronize release metadata,
   update notes/feed/artifact naming, perform selective source/privacy review,
   then run hosted CI on the actual proposed commit. The source-only copy is
   useful evidence but not a substitute for a fresh hosted build of that commit.

These gates distinguish pushing work for review from publishing stable v0.7.0.
None requires another UI redesign or a wholesale Python/Rust migration.

## Improvements that are feasible but not release blockers by themselves

- **English/Japanese production UI:** catalogs/presentation foundations exist,
  but the active screen is hardcoded Chinese and both language choices are
  disabled. Do not advertise a trilingual V2 release yet. Complete string
  extraction and native-speaker UI review as one coherent follow-up.
- **Temporary challenge-cap editing:** the editor shows the current value as
  read-only. Native support is not connected. Label it accurately and keep this
  separate from the proven temporary enemies/terrain/rules operation.
- **Repeated effect occurrences:** current search groups use effect IDs/set
  semantics; duplicate same-name/different-value requirements need an
  occurrence-aware query model and native evidence. Free local editing already
  accepts arbitrary slots. Do not describe the search feature as complete.
- **Recovery UX:** mark verified cart items as added/unselected after partial
  batches, preserve actionable operation IDs, and map technical preflight errors
  to concise next steps. Retain full diagnostic detail in bounded logs.
- **Maintainability:** move the production code out of a directory named demo,
  format the dense TSX, and split large workspace components by workflow before
  adding equipment tools. Keep the legacy Tk and numerical core intact.
- **Performance/visual acceptance:** test real 1080p/1440p, Windows scaling,
  long groups and a foreground game. The screenshot timeout is not evidence of
  its cause or proof of the old white-screen/Discord issue being fixed.

## Explicit exclusions

Do not reopen the accepted R5-on-load cap/icon discrepancy: the user requested
no further workaround. Do not infer natural early-R4/R5 drops, multiplayer
propagation, all-hardware CUDA/D3D parity or a published update from local tests.
Do not stage private saves, account captures, game dumps, signing material,
build debris or unrelated experimental root scripts with the future V2 commit.
