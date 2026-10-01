# Install selection and stale-plan follow-up

## Intake and failure cases before implementation

The new feedback manifest hash exactly matches retained clean source 7676ab7,
not test8-r1. Its protected worker is also the old a24e3c55 binary. The reported
no-write refusal/recovery loop therefore does not establish a test8-r1 failure.
Record that identity explicitly and reproduce the current refusal path.

The owner confirmed non-Steam users fail at startup game/version discovery.
Current discovery checks Steam roots only. Add an explicit native file-picker
choice; never guess a version or remove native-write identity checks.

Failure controls:
- Chosen Nioh3.exe outside Steam libraries is ignored or not persisted.
- A version is accepted from JSON/environment instead of the actual file.
- Missing, malformed, renamed or unreadable selection silently binds another
  game. Reject, keep the settings entry available, and allow explicit reset.
- Selection changes a running worker's GenerationContext or replays a job.
  Persist for the next launch; current roles keep their frozen session identity.
- An unsupported version or modified native executable gains write authority.
  Preserve worker/profile/executable/signature gates; selection is not approval.
- A stale-save commit refusal still fences the client or looks like a write.
  Test the real host error through the client; ambiguous results remain fenced.
- Users cannot distinguish older v0.8.3 test packages. Display verified source
  commit in the UI and export it with executable/package identity in feedback.
- Settings/error recovery is unreachable or untranslated. Inspect all locales
  and the matching outer EXE; use never-executed VERSIONINFO fixtures only.

No unit tests are added. All writable evidence stays under
`D:/Nioh3_v080_deliverables/deliverables/codex-v083-install-selection-20260930/`.
No game/save file is modified; a restart instruction concerns Studio only.

## Implementation and evidence

Settings and installation-related startup errors offer the native executable
picker. Only an absolute, real Nioh3.exe path is persisted; Windows VERSIONINFO
supplies the identity. The native bootstrap freezes the selection before any
worker lookup. Selecting or resetting a path affects the next Studio launch,
never an existing role's GenerationContext. Invalid explicit selections do not
fall back to Steam. The shared nioh3-data version registry rejects unknown
versions before workers start; native executable/profile/signature write gates
are unchanged.

Diagnostics retain the manifest source commit/dirty state and the actual inner
and outer executable paths. The sidebar displays that manifest's short commit
inside its existing fixed-height brand. Settings scroll within the available
viewport; the title bar and native picker remain reachable in all three locales.

The real framed protected-host E2E now prepares over an isolated encrypted
synthetic save, changes its equipment/field bytes as an intervening game save
would, and commits the old plan. It receives exactly
`OPERATION_FAILED: Save changed after preparation; no write attempted`, leaves
the changed save untouched and creates no operation ledger entry. The verifier
then restores its owned fixture, prepares a fresh plan and completes its existing
successful commit/readback assertions. Artifact: external
`backend/stale-plan-host.json`; the focused release-mode host E2E passed.

The native installation E2E uses a never-executed VERSIONINFO fixture outside
Steam. It verifies shared role context, ignores a forged JSON version, checks
reset/session immutability, missing/malformed selections, unsupported VERSIONINFO
and feedback/build identity. It also checks 1020x640 at 150% through CDP inside
WebView2; this is viewport emulation, not a system-DPI change. The native OS
file dialog itself is not automated: persisted-path bootstrap and inspect/reset
use the actual native host, while the picker entry/authority is inspected in
source and UI. No cracked executable or real game/save write is claimed.

Before repair, retained E2E receipts demonstrate the clipped build label and
out-of-viewport English settings. A loading synchronization defect in the
verifier was corrected to wait for the inspection result, not the button's
initial enabled state. Unsupported VERSIONINFO also exposed a real startup
error that discarded the worker's resource refusal into a pipe error; the host
now rejects it through the shared version registry before spawning.

Exact prepackage/native and matching outer-EXE receipts, source identity and
hashes belong to this task's external candidate delivery report. Publication
and any real-game/pirated-binary acceptance remain separate.

Prepackage closure: 27/27 native installation checks passed; all default and
constrained locale screenshots were inspected. Typecheck, the 1,179-message
locale audit, locked Tauri binary Clippy with warnings denied and diff checks
passed. Native sessions exited; their configuration and desktop logs were also
retained beside the receipts. Automated approval refused disposable-profile
deletion with only "blocked by policy", so the profiles remain. The repeatable
verifier recreates its never-executed fixtures under the project build root.

Matching outer-EXE closure: clean product d5fa638 produced test8-r2, SHA256
`354db3b0983b8683ba836e5c3aaccf50e1dd4d577f245c14f2814b466a400119`.
Installation/context checks passed 28/28; plan-footer/no-write-refusal checks
passed 70/70; retained feature-entry/prediction checks passed 24/24. Relevant
locale screenshots were inspected. Native maximize/restore retained the
existing one-CSS-pixel fractional-DPI tolerance. All three receipts identify
the same EXE/payload hash. The helper's packaged filename/Windows extended-path
comparison was corrected after build; no product bytes changed and the repair
is retained separately. Source/package identity and exact repeats are in the
external `candidate/DELIVERY.md` and `DELIVERY.json`.
