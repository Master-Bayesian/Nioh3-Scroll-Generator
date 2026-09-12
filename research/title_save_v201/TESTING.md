# Tests and reproducibility

## Executed in this delivery

The final targeted run executed **75 tests, all passed**, with no skips in this
environment. `results/TEST_LOG.txt` contains the actual output. This count is not
a claim about the project's full existing test suite or native acceptance.

Coverage includes:

- The actual observer Lua module and the unchanged owned-breakpoint helper,
  executed by a real Lua 5.4 runtime through ctypes, with mocked CE APIs.
- All eight four-site profiles, disk/signature preflight rejection, nil-success
  CE breakpoint returns, partial arming failure, owned-only cleanup, delayed
  cleanup, failed cleanup, attachment replacement and PID reuse guard behavior.
- Busy rejection versus unreadable/unknown state, correct frame/thread pairing,
  early failure and success, native writer register reuse after close, bounded
  memory/string/event reads, corrupt queue bounds, callback/log-sink errors and
  continuation, path/account pseudonymization and JSONL export.
- Exact raw `.text` signature comparisons, unique masked matches, and runtime
  Lua/JSON locator equality for all 28 observation points.
- Real filesystem tests for monotonically reserved stage directories, gaps,
  concurrent reservation, read-only three-file fingerprints and change detection.
- FILETIME process identity serialization and explicit identity-capture failure.
- Analyzer rejection of empty, truncated, mixed-process, malformed and
  cleanup-uncertain captures. Rename success never proves a product commit.

An independent static verifier also hashed all three supplied sections, checked
`.pdata` function intervals and decoded each observation boundary with objdump.
All **28 / 28** sites passed these mechanical checks; see
`results/STATIC_VERIFICATION.json`. The associated semantic claims come from
the documented control/data flow, not merely these tests.

The unmodified input collector's chronological defect was reproduced with
synthetic files: two successive stage captures produced `[1, 1]`, not `[1, 2]`.
See `results/BASELINE_REPRO.txt`. No private saves were used.

## Run the targeted tests

Run from the project root after applying the patch:

```powershell
$env:TITLE_SAVE_SECTIONS = '<handoff>\evidence\runtime-sections'
# Either make a Lua 5.4 shared library discoverable, or set one of:
$env:TITLE_SAVE_LUA_EXECUTABLE = '<path-to-Lua-5.4>\lua.exe'
# $env:TITLE_SAVE_LUA_LIBRARY = '<path-to-Lua-5.4>\lua54.dll'
python -m pip install -r requirements-dev.txt
python -m pytest -q tests\test_title_save_observer.py tests\test_title_save_capture_tools.py
```

When neither environment variable is set, the tests use the Lua 5.4 runtime
bundled by the pinned Lupa development dependency. This keeps the observer tests
executable in clean Windows CI while the live observer continues to run inside
Cheat Engine's own Lua environment.

On Linux, `liblua5.4.so.0` is automatically located. If the Lua runtime or raw
sections are missing, the corresponding tests skip and explain why. A skipped
suite must not be reported as full validation.

Raw sections are not redistributed in the result ZIP. Use the matching sections
from the supplied handoff. The static CLI can be rerun independently:

```powershell
py tools\verify_title_save_locators.py `
  --sections '<handoff>\evidence\runtime-sections' `
  --objdump '<path-to-objdump>\objdump.exe' `
  --output .codex_tmp\title-save-static-recheck.json
```

Without objdump it still checks hashes, exact bytes, uniqueness and `.pdata`,
but explicitly reports `independent_objdump_boundary=false`. Do not treat that
as an independently decoded instruction-boundary test.

## Patch application validation

`SOURCE.patch` is relative to **the supplied dirty project-source snapshot**,
not just commit 6264cbd355729e0b434ba5f540232a5d1362a79d. The unchanged lifecycle
helper was copied from the handoff's prior-research directory for standalone
execution; it is a dependency, not a new implementation attributed to this work.

The patch is checked and applied to a fresh copy of that baseline, then the
same 75 tests are run and the patched files compared byte-for-byte with the
working implementation. See `results/PATCH_APPLICATION_TEST.log` and
`results/PATCH_APPLICATION.json` in the finalized delivery.

Only `tools/capture_title_save_lifecycle.py` is modified among existing project
files. The observer, static locators/excerpts, analyzer, static verifier, two test
modules, Lua harness and this research documentation are new. No product
savegame, SaveApplication, native live-add, RNG or generation algorithm file is
modified.

## Explicitly not executed / not proved

- Cheat Engine with a live Windows game process.
- A live supported-executable attestation, hardware-breakpoint callback, cleanup
  after real game exit, or actual thread ownership.
- C0, C1, C2 or C3 against any personal/real game save.
- Native inventory insertion, native save/reload, or generation-specific ack.
- Windows File I/O/ETW trace collection and native stack unwinding.
- Complete repository, Tauri/Rust, packaging or publication regression.

The supplied project-source is a scoped snapshot, not a complete build checkout.
The isolated tests deliberately avoid relying on absent application imports.
The full Windows checkout must run its normal regression and live integration
gates after review. Synthetic observer success is supporting evidence, not a
substitute for any release gate.
