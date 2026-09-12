# v0.7.2 title-save static recovery integration

> This report records the integration checkpoint. For the subsequent live C0
> outcome and revised next step, read the
> [2026-09-12 approach reset](TITLE_SAVE_APPROACH_RESET_20260912.md).
> The original next-live-step section below is superseded.

## Decision

Publication remains **blocked**. The PC v2.01 native save pipeline and a bounded
read-only Cheat Engine observer are now integrated, but no live C0-C3 title
lifecycle capture has established the authoritative title inventory, caller
thread or acknowledgement that excludes a later stale-generation write.

## Reviewed input

The reviewed archive was `Nioh3_TitleSave_v201_20260911.zip`, 234,875 bytes,
SHA-256
`B78D1AA3697D9C9E0647B7C201F45116780EA7D37BC89793C98D1A8EB8C6E9CC`.
All 64 ZIP members passed CRC validation, no unsafe path was present, and all 63
manifest entries matched their declared size and SHA-256. The patch baseline
matched the current dirty worktree at commit
`6264cbd355729e0b434ba5f540232a5d1362a79d`, including the untracked lifecycle
collector and the unchanged owned-breakpoint helper. The pre-application state
is preserved under `.codex_tmp/pre-title-save-v201-pro-patch-20260911/`.

## Integrated native result

The current PC v2.01 save path is split into registered game objects, queued
snapshots, staging buffers, asynchronous file tasks and controller-side result
consumption. The recovered chain is:

```text
request wrapper 0x5B62D8
  -> snapshot/coalesce 0x5B760C
  -> request gate/build 0x5B6BE0 / 0x5B6F10 / 0x5B6F64
  -> stage and dequeue 0x5B7518 / 0x5B77CC
  -> submit/bind 0x5B7050 / 0x5B7110

worker 0x5B65F0 -> 0x5B6610 -> 0x5B6BCC -> 0x5B6C98
  -> serialize 0x5B7E7C
  -> temporary write and rename 0x5B7AE4

controller 0x9204F0 -> 0x1B2B6C -> completion consumption 0x1B2D60
```

The queue can be empty while a worker retains old staged bytes. The observed
mutex covers snapshot capture/copy rather than the whole asynchronous save.
The serializer staging buffer is transformed in place and is not an
authoritative plaintext inventory. The load/apply chain
`0x2979F74 -> 0x1349610 -> 0x2188A7C` rebuilds multiple managers and is not a
safe standalone cache-refresh API.

The registration walker at `0x9DB730`, with its callback at `0x9DB752`, is the
next justified observation point for identifying the object that owns scroll
inventory. These meanings remain scoped to the supplied PC v2.01 executable
identity.

## Integrated research tooling

- `research/probe_title_save_ownership_ce.lua` validates the disk executable
  identity and 28 local signatures before arming one four-site hardware
  breakpoint profile. It performs no target-memory or save-data writes.
- `research/title_save_v201/locators.json` and `locators.lua` retain the exact
  current-version RVAs, masks, function ranges and provenance.
- Eight bounded profiles separate file, handle, request ownership, snapshot,
  serialization, loading, registry and worker evidence.
- `tools/capture_title_save_lifecycle.py` now reserves monotonically increasing
  stage directories atomically and records process creation FILETIME in the
  same representation as the observer.
- `tools/analyze_title_save_observer.py` rejects incomplete, mixed-process or
  cleanup-uncertain captures without converting trace completeness into a
  product commit claim.

## Integration corrections

The Pro patch supplied research files under a tree ignored by the repository.
`.gitignore` now explicitly admits the reviewed observer and
`research/title_save_v201/**` so a later commit cannot silently omit them.

The supplied tests used pytest while every existing hosted workflow installed
only product dependencies and invoked unittest. The development requirements
now pin pytest and Lupa, the observer tests use Lupa's Lua 5.4 runtime when an
explicit Lua executable/library is absent, and the Tests, Frontend V2 and
release workflows run the 75 research tests explicitly. These dependencies are
test-only and are not imported by the product entry points.

The static verifier originally accepted GNU objdump arguments only. It now
supports the LLVM objdump/objcopy pair included with Visual Studio on Windows,
while retaining the GNU path.

The first live C0 arming attempt exposed two host-compatibility defects that the
mock adapter had not modeled. Cheat Engine's MCP Lua chunks run off the GUI
thread, so `MainForm.OnProcessOpened` and timer creation now execute through
`synchronize()`. Cheat Engine's child PowerShell also lacked the auto-loaded
`Get-FileHash` cmdlet; executable attestation now computes SHA-256 through .NET
cryptography. Both failures happened before any breakpoint was armed. The mock
adapter now exercises the synchronization boundary, and the test suite checks
that the cmdlet dependency cannot return.

## Local verification

| Check | Result |
| --- | --- |
| Pro result files before integration adjustments | 46/46 exact SHA-256 matches |
| Observer/capture pytest with Cheat Engine 7.7 Lua 5.3 DLL | 75 passed |
| Observer/capture pytest with pinned Lupa Lua 5.4 | 75 passed |
| Current v2.01 locator verification with LLVM | 28/28 signatures, unique matches, unwind ranges and decoded instruction boundaries passed |
| Full Python unittest discovery | 614 passed in 83.255 seconds |
| Real collector sequence after the earlier PREP stage | created stage 002; three files remained unchanged |

No game process was started and no game memory or save file was modified during
this integration.

## Next live step

The next run is the non-mutating `C0.files.01` control. Begin Windows File I/O
tracing before launching the game, reach the title screen, attach Cheat Engine,
run only the `files` profile, exit normally and cold-start once more. Stop and
export the observer before changing attachment. Analyze that capture before
choosing the `ownership`, `snapshots` or `registry` follow-up profile.

The current non-elevated WPR preflight was rejected with Windows error
`0x80070005`; no recording remained active. Use
`tools/title_save_fileio_trace.ps1` from an Administrator PowerShell for the C0
trace. If elevated tracing is unavailable, record that limitation explicitly
and use the CE `files` profile from the title screen through process exit; do
not imply that pre-attachment startup writes were observed.

C1-C3 must not start until C0 proves that the observer sees the relevant title
writer and cleanup works in the real CE/game pair. C1 and C2 deliberately
exercise the known risky append path only after the normal automatic backup is
verified. Any accepted operation with an unknown result stops further mutation
and is never replayed.
