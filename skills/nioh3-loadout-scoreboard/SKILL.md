---
name: nioh3-loadout-scoreboard
description: Verify a Nioh 3 loadout optimizer mechanism against retained PC202 game observations and native replay.
---

# Nioh 3 loadout scoreboard

Use the optimizer checkout with `docs/product/loadout-plan.md`. Read its resume checkpoint, run its
scoreboard command, and pick one mechanism and specific witnesses for the work unit.
Use `tools/run_python_tests.ps1` for Python.

Treat catalog bindings, observed damage, current CPU replay, and original MAIN as separate evidence.
Whole-cast HP loss, per-contact HP loss, and split damage establish different facts. Keep the recorded
measurement shape. Use `--check` for baseline regressions and `--require-match` for the unit's target witnesses.

Follow the working plan for milestone checks, bounded Pro handoffs, and the one-window capture procedure.
If shared project guidance is missing from a linked worktree, use the plan's host fallback.

## Run

Use PowerShell 7 in the optimizer checkout. Use the prepared Python environment from the
primary checkout or the project runner's configured environment. On this host:

```powershell
$env:NIOH3_BUILD_ROOT = 'F:/Nioh3_r16_build'
$env:CARGO_TARGET_DIR = "$env:NIOH3_BUILD_ROOT/build-cache/quality-target"
$python = 'F:/Nioh3_ScrollEditor/.codex_tmp/v2-build-env/Scripts/python.exe'
./tools/run_python_tests.ps1 -Python $python -MinimumFreeGiB 0 -ScriptPath tools/loadout_scoreboard.py -ScriptArgument @('--check')
```

Add `--output` with a fresh path under the work unit's `deliverables/` directory to save
the per-action receipt. Add `--require-match` for each target witness ID. The default data
roots are the local evidence archive and the retained PC202 scene bundles. Use
`--evidence-root`, `--bundle-root`, and `--replay-root` when these move.

If the active branch predates the scoreboard files, use the shared guidance for REA and work-unit
routing. Run the scoreboard in the optimizer checkout that contains the witness tool and fixtures.
