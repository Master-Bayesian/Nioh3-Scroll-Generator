# Packaged optimizer verification tools

These tools separate build identity, engineering fixtures and player release
acceptance. The supplied `fbbac77` EXE is rejected. No replacement Windows EXE
was built or accepted by the Linux review.

## Build a coherent scene before wrapping

`tools/package_tauri.py` now requires a deployable scene contract. A menu
`bundle.json` is not a compatibility contract. Prepare a clean directory named
`kusarigama-strong` containing the 14 named runtime files listed by
`verify_loadout_release.REQUIRED`. They include both `main_cpu.dll` and
`main_device.dll`, frame, table, source, hit and event inputs. Private captured
images/context directories, sample saves, stale alternate DLLs and unrelated
sidecars are rejected; they must not be copied into a public EXE.

Build both DLLs from the same generated MAIN and export all inputs for that
address space/source generation. Then record the exact app/native commits:

For the retained single direct-actor contact, first run
`python tools/verify_loadout_release.py export-layout <engineering-scene> <new-directory>/kusarigama-strong`.
Seal the newly exported directory. Its base frame contains the canonical address
layout and empty character buffers. The player's context supplies the actual
frame at engine initialization. Keep the captured source program, its reference
inventory and populated STAT/D in the private generation. A host receipt cannot
authorize publishing that captured program inside the executable.

```powershell
python tools/verify_loadout_release.py seal C:/work/export/kusarigama-strong `
  --generated-main C:/work/native/main_gen.inc `
  --app-commit <full-app-commit> --native-commit <full-native-commit>
```

The command checks frame STAT/damage/MAIN spans against compiled MAIN domains,
all hit write addresses, event-plan MAIN and hit hash, both DLLs' actual x64 PE
exports, and binds every runtime file by SHA-256/length. The contract explicitly
says `host_e2e_required`; it does not claim semantic or numerical acceptance.
ABI values are queried later in a Windows subprocess. The supplied old scene
cannot be sealed against the optimized MAIN: its MAIN addresses differ.

Pass that directory to the existing `--loadout-runtime` packager. Staging
validates and replaces the entire runtime file set. One-file construction also
validates the embedded scene and reopens the produced EXE. After any signing
that changes EXE bytes, repeat the final artifact inspection and host E2E.

```powershell
python tools/verify_loadout_release.py check C:/release/Nioh3Studio-0.8.13-win-x64.exe `
  --output C:/evidence/loadout-static.json
```

A static pass only permits a candidate build. It is not player acceptance.

## Two explicit host modes

Use Windows x64, WebView2, Python with `tools/requirements.txt`, project npm
dependencies and a working CUDA device. Keep the game on the matching character,
style and attack, with stable equipment and status/buff inputs. The runner copies
only the approved save into isolated appdata; it never issues `save.commit`.
The source and isolated save must retain their original hashes.

A reference must be a previously independently checked `SEARCH_RESULT.json`
from the same save, required set and one-contact objective. It must contain
`top_case.components`, all ranked assignments and planned/answered counters.
Native MAIN winner validation remains a separate check: CPU/CUDA agreement is
not an independent source-producer oracle.

### Player mode: required for distribution

```json
{
  "mode": "player",
  "outer": "C:/release/Nioh3Studio-0.8.13-win-x64.exe",
  "scene_id": "kusarigama-strong",
  "save": "C:/approved-copy/7656119.../0000/SAVEDATA.BIN",
  "game_identity": "C:/host/game-identity.json",
  "reference": "C:/host/independent-reference/SEARCH_RESULT.json",
  "set_key": 45761,
  "set_pieces": 5,
  "python": "C:/Python312/python.exe",
  "search_timeout_ms": 180000
}
```

`game_identity` is the existing host identity JSON `{ "path": ".../Nioh3.exe",
"sha256": "..." }`, checked before launching. Replace sample paths and choose
the exact set/tier of the independent reference; values are not universal defaults.

```powershell
node apps/tauri/verify-packaged-loadout-search.mjs C:/host/player-config.json C:/evidence/player-run
```

Player mode forbids `profile_template` and `live_epoch_fixture`. Both CPU and
card searches begin with fresh isolated state. The product itself must provision
all resources needed to return a current `live` context and perform search. A
missing/unsupported context fails the leg and leaves `release_ready=false`.
The original 0.8.13 flow does not yet provide this fresh-player path; supplying a
private template cannot satisfy it.

### Engineering fixture mode: useful diagnosis, never release acceptance

Use the same config with `"mode": "engineering-fixture"` plus
`"live_epoch_fixture": "C:/host/current-live-epoch"`. This path must contain
`active.json` and the active SHA-named generation with `LIVE_BINDING.json`,
`EPOCH_INPUT.json`, and `bundle/context/`. Its current-process binding must still
match the running game. The runner does not rewrite PID, snapshot binding or
native expectations to force admission.

This mode first records the fresh-state missing-resource outcome, then copies
the explicitly supplied generation into isolated
`state/loadout-live-epochs/kusarigama-strong/`. Every executable and deployable
scene input in that generation must match the exact final EXE. The private
context hashes are recorded separately; private context bytes are not written
to receipts. It then exercises the real production bound-epoch receiver.

Successful engineering runs emit `passed=true`, `release_ready=false`,
`private_resources_copied=true`. The release verifier rejects these receipts.
The inherited historical profile-only template cannot exercise the dynamic
prepared-source path and is deliberately rejected by this runner.

## What the runner proves, and what it still needs

The runner launches the outer EXE, clears worker/data/contract/test-capture
redirects, and selects only `NIOH3_LOADOUT_GPU=0/1`. It observes first read and a
separate manual refresh job, performs the actual UI required-set search, and
checks published assignments, ordering, ties, exact expected rank count,
complete/answered/refused/no-path/source-uncomputed counters, all displayed
component availability, and the winner's total/physical/elemental/contact bits
against the independent reference.

An independent PowerShell observer captures actual loaded module paths from the
protected worker. The worker EXE must come from this payload's cache. A scorer
may come from that cache or a production immutable scene copy under the isolated
state directory, but every scene byte and the runtime contract must match the
payload. `context/` is a separately bound private input, not a public scene file.
A fresh Python subprocess queries the exact scorer ABI values. IPC execution
receipts prove native CUDA calls and submitted/completed contacts, or real CPU
executions plus certified repeats. A loaded-but-unused GPU DLL cannot pass.

Current runner scope is one flat deployed scene, one required set, fixed
attributes and one scored contact. It refuses unsupported runner scopes rather
than synthesizing success. Multi-contact/elemental objectives, source-family
counterexamples, cancellation, wear-preview recovery and actual original MAIN
remain in the consolidated host checklist/independent numerical checks.

Receipt files are `packaged-loadout-e2e.json`, per-leg `run.json` and
`modules.jsonl`, plus `release-gate.json` for successful player mode. Failure
records include the actual failed job and screen text; observers/children are
closed and save hashes are checked. These new Windows legs were syntax/lint
checked in the Linux review and remain unexecuted on Windows.

## Promote only the exact accepted bytes

A separate external host receipt avoids a circular artifact hash. It must bind
the final delivered EXE SHA-256 and source commit. Never replace it with a nearby
source-directory test or a handwritten success summary.

```powershell
python tools/verify_loadout_release.py check C:/release/Nioh3Studio-0.8.13-win-x64.exe `
  --require-host-e2e --host-receipt C:/evidence/player-run/packaged-loadout-e2e.json `
  --output C:/evidence/final-loadout-gate.json

python tools/verify_release_artifacts.py --directory C:/release `
  --version 0.8.13 --expected-sha <full-app-commit> `
  --loadout-host-receipt C:/evidence/player-run/packaged-loadout-e2e.json `
  --report C:/evidence/final-six-assets.json
```

The promotion helper accepts the same external receipt via
`-LoadoutHostReceipt <absolute-path>` in its read-only mode. It feeds that receipt
to both local and public artifact verification; the six-asset release remains
unchanged. Actual publishing remains an explicitly requested operation, outside
this review. `publish-release.yml` does not yet download an external host receipt;
its existing call now fails closed for optimizer candidates. The concrete local
helper path is usable; hosted receipt-artifact transport is a remaining CI hookup.

## Offline regression command

```sh
python tools/acceptance/reproduce_package_regression.py /path/to/extracted-handoff --output /path/to/package-regression.json
python -m unittest tools.tests.test_loadout_release tools.tests.test_packaging_optin -v
python -O -m unittest tools.tests.test_loadout_release -v
```

The first command executes the old archived production staging/wrapper and the
patched ones against the exact supplied payload plus isolated PE fixtures. It
shows the original wrapper accepting the broken package, the patched wrapper
rejecting it, old staging omitting CUDA, patched staging preserving CUDA and its
contract, and the old scene failing the optimized MAIN layout check. Synthetic
PE/receipt tests test the gate itself; they are not Windows numerical evidence.
