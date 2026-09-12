# PC v2.01 read-only title-save observer

## Status and integration boundary

The observer is implemented and runs in the delivered Lua mock tests. The
subsequent live C0.files.01 run armed successfully after host-compatibility
corrections, recorded no events, and ended with cleanup unverified against the
exited process. This is not native save-path acceptance. Read
`docs/knowledge/TITLE_SAVE_APPROACH_RESET_20260912.md` before another experiment;
it supersedes the mandatory C0 sequence below. The observer never calls a game
function, changes target memory/save data, installs an exit hook or suspends the
process as a save-ownership mechanism. Hardware breakpoints do briefly stop
threads while observations are read; timing perturbation remains a research
limitation. Do not equate an unobserved event with its impossibility.

Supported executable only:

```text
file version: 2.0.1.0
file size:    77814240
disk SHA256:  4047ECB623F3AE033F7D6F3637CD1C5408C72A89A038463268A7C9AC5C394159
```

The current package's `.text/.rdata/.pdata` hashes are recorded in
`locators.json`. The runtime guard uses the attached process's executable disk
identity plus all 28 local byte signatures. Do not replace these hashes after an
update merely to make the observer arm; recover and revalidate the new version.

The unchanged `research/owned_breakpoint_lifecycle_ce.lua` must be present. The
patch intentionally does not replace this shared helper. A copy from the input
handoff is supplied separately for a clean research checkout lacking it.

## Preparation, before any mutation experiment

1. Integrate `SOURCE.patch` into an isolated branch of the supplied current
   worktree. Run the tests described in `TESTING.md`. Review `locators.json` and
   the exact observation instructions, not just the AOB match count.
2. Configure Cheat Engine to use the **Windows debugger**, not VEH. The observer
   requires `debug_getCurrentDebuggerInterface()==1`. It does not inject a VEH
   handler, auto-attach a process or silently switch debugger type.
3. There must be no pre-existing CE breakpoints. Each profile consumes the four
   hardware execute breakpoint slots. On conflict the script refuses to arm;
   it never removes another tool's breakpoints. Close unrelated injectors/tools
   and stop the existing observer normally before selecting another profile.
4. Use the supplied local file collector to identify the account key and slot.
   The account key is a pseudonym derived from the disk folder, not proof that
   the selected native account is already mapped to it.
5. Keep private backup/capture directories out of uploaded handoffs. Local
   fingerprint JSON includes filesystem paths; redact usernames/account folders
   before sharing while preserving `account_key`, slot, role, hash, size and
   nanosecond modification time. `private-files/` must never be uploaded.

## Minimal CE commands

After attaching to the supported `Nioh3.exe` and starting the Windows debugger,
load the script in CE Lua Engine. Loading alone does not arm anything.

```lua
local root = [[F:\Nioh3_ScrollEditor]]
assert(loadfile(root .. '/research/probe_title_save_ownership_ce.lua'))()
local probe, ok, state = TitleSaveOwnership.start {
  project_root = root,
  run_id = 'C0.files.01',
  profile = 'files',
  account_key = 'REPLACE_WITH_HEX_ACCOUNT_KEY_FROM_COLLECTOR',
  slot = 0,
  max_events = 1024,
  hash_payloads = false,
}
assert(ok, state and state.stop_reason or 'Observer did not arm')
```

The placeholder account key is deliberately invalid until replaced. To inspect
or stop without touching other scripts' breakpoints:

```lua
local state = TitleSaveOwnership.current.status()
print(TitleSaveOwnership.encode_json(state))
TitleSaveOwnership.current.stop()
TitleSaveOwnership.current.retry_cleanup() -- only if cleanup remains pending
TitleSaveOwnership.export([[F:\Nioh3_ScrollEditor\.codex_tmp\C0.files.01.jsonl]])
```

Use an existing local directory and a new `.jsonl` filename. Export refuses to
overwrite evidence and requires the observer to be stopped. Export is still
allowed with unresolved cleanup so evidence is not lost; the analyzer marks
such captures incomplete. There is no native acceptance implied by exporting.

`clear_captures()` is available only after stop and proven cleanup. It increments
the capture epoch; it does not make a failed run a successful run or re-arm the
observer. Prefer a fresh run ID/observer for each process and profile.

The attachment is bound to PID, process creation FILETIME, process handle and a
chained `MainForm.OnProcessOpened` epoch. Reattachment, including PID reuse,
stops recording and does not remove breakpoints from the replacement process.
A live MCP-hosted Lua chunk may run outside Cheat Engine's GUI thread, so the
production adapter installs/restores the `MainForm` handler and creates cleanup
timers through `synchronize()`. Executable hashing uses the .NET SHA-256 API
instead of the optional `Get-FileHash` cmdlet.
A cleanup failure remains explicit. Do not manually zero the owned-breakpoint
list to bypass it. If the old process has already exited and CE cannot inspect
its old breakpoint state, export that unresolved state; independently verify the
old process instance has gone before discarding its local observation session.

## Profiles (four execute breakpoints each)

| Profile | What it observes |
|---|---|
| `files` | Writer entry; WriteFile result/byte count; MoveFileExW result; common failure/success return |
| `file_handles` | Open, flush, close and common writer return; diagnostic follow-up to `files` |
| `ownership` | Request entry; common request return; task binding; completion consumption |
| `snapshots` | Queue capture; finished character snapshot; queue removal; serializer entry |
| `serialization` | Serializer entry; transformed staging copy-back; serializer result; coordinator result |
| `loading` | Read/decode entry and return; object-apply entry and return |
| `registry` | Snapshot entry; every registered callback; finished snapshot; load-side apply entry |
| `worker` | Task binding; worker thunk; write dispatch; coordinator result |

Do not arm profiles simultaneously or change a running script. A profile is a
bounded window into a flow, not a complete whole-process trace. In particular,
`files` does not capture every Windows handle API; `file_handles` is a separate
pass, and the registry has additional worker serialization beyond its direct
registered callbacks. Missing coverage is recorded as a limitation, not a zero
activity claim.

`hash_payloads=true` optionally computes an investigative MD5 of the known
character body only. It is off by default to reduce breakpoint dwell time. The
hash excludes the salt/checksum footer and is not a native generation token or
cryptographic commit proof. The per-observer total hashing budget is 64 MiB.
No plaintext contents are included in JSONL.

## Local file fingerprints

```powershell
py tools\capture_title_save_lifecycle.py --list
py tools\capture_title_save_lifecycle.py `
  --save-path '<selected SAVEDATA00\SAVEDATA.BIN>' `
  --run-id C0.files.01 --stage before_launch
# Repeat with unique stages: at_title, after_editor_result, after_exit,
# after_cold_restart, after_normal_save, as applicable.
```

The collector hashes main, game backup and system files and records nanosecond
mtime plus process creation identity. A changed file during hashing is a failed
capture, not a stable generation. Even three individually stable fingerprints
are not a simultaneous native-owned snapshot; the output explicitly states
`capture_is_atomic=false` and `native_save_ownership=not_acquired`.

The existing `--capture-private-files` option is local-only and optional. It is
not a substitute for the application's automatic verified pre-mutation backup.
The collector/observer never automatically restore files during an uncertain
native write.

## Minimum C0-C3 sequence after integration

Begin with **C0 only**. A Windows File I/O trace with stacks should begin before
launch, so writes before CE attachment are not silently omitted. The narrower
CE profiles establish objects/threads while the player performs the already
planned actions. Capture logs locally; sanitized event rows, not private save
bytes, belong in the next research package.

The repository provides `tools/title_save_fileio_trace.ps1` to start and stop
the built-in WPR FileIO profile into a run-specific directory. WPR requires an
Administrator PowerShell session for `Start`, `Stop`, and `Status`. A
non-elevated status query can incorrectly report that no recording is active.
If that trace cannot be collected, state the
gap and do not treat the CE observer's post-attachment window as startup-wide
coverage.

1. **C0.files.01:** cold launch, reach title, no application mutation, exit,
   cold restart. Capture local file stages and the writer profile through exit.
   Export before switching CE attachment. First determine whether this native
   writer executes for the target account/slot and which thread/caller is used.
2. **C0.ownership.01** and **C0.snapshots.01:** repeat the same non-mutating
   scenario in fresh processes. Resolve H/R/S pointers, busy rejection, queue
   removal and task association. When an actual object serializer is needed,
   use a separate `registry` pass; when load provenance is unclear, use
   `loading`. These diagnostic passes are not replacements for C1/C2.
3. Once the observer is integrated and its scope verified, perform **C1** in a
   disposable backed-up account/slot: cold title, append one known test scroll
   using the existing instrumented application, do not load a character, exit,
   restart. Record the candidate operation ID/serial, automatic backup receipt,
   application result and each file stage. This is deliberate defect research,
   not a claim the current path is safe.
4. **C2:** fresh process, load character, return to title, append, exit without
   loading again, restart. Compare against cold-title object ownership.
5. **C3:** fresh process, title append, load character, save normally, return to
   title, exit and restart. Record both the native load/apply and later save
   route. This is a positive control, not an acceptable user workaround.

Do not run all eight profiles for every matrix entry without need. Use the C0
results and independent file-I/O chronology to choose narrowed follow-up points.
Any accepted mutation with unknown outcome stops further mutation; preserve the
native/application receipts and local file versions rather than retrying or
restoring blindly. Actual account, slot and native request mapping must be
established before declaring any of these live gates satisfied.

## Analyze a capture

```powershell
py tools\analyze_title_save_observer.py .codex_tmp\C0.files.01.jsonl `
  --output .codex_tmp\C0.files.01.analysis.json
```

Exit zero means the bounded capture container has events and no recorded loss or
cleanup failure. It does **not** mean the native path was complete, the operation
committed, or publication is allowed. The analyzer always reports
`product_commit_proven=false`, `native_path_completion_proven=false` and
`release=BLOCK` until an actual product ownership protocol is separately proved.
