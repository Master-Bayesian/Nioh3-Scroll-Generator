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

## Work-unit policy

Use one chat per shared mechanism. Reconcile the active diff, checkpoint, and scoreboard.
Close the unit after its target witnesses pass and the checkpoint records the source, receipt, and next action.
Use subagents only for independent read-only investigations, and keep sequential edits, builds, and tests in the lead chat.

Run affected native replay, the scoreboard gate, and relevant lint per unit. If the unit changes a
state protocol, run the applicable TLA+ model and named fault in that unit. Run the plan's WebView,
installer, device CUDA, and background-performance checks at applicable integration milestones.
Follow the repository's required CI and release gates.

## Shared guidance on this host

When a linked worktree lacks shared guidance, look in `F:/Nioh3_ScrollEditor` or the primary
checkout specified by `NIOH3_GUIDANCE_ROOT`. Use `AGENTS.md` for shared constraints,
`docs/research/REA_WORKFLOW.md` for native work, `docs/research/EXPERIMENT_FAILURE_LEDGER.md`
for substantive failures, and `docs/knowledge/CURRENT_HANDOFF.md` for coordination.

## Pro and live game boundaries

Give Pro a bounded GPU-independent question. Include mismatches, REA evidence, captured values,
and exact inputs. Request a focused patch and predicted values. Admit the patch after local
native replay, the target `--require-match` checks, and the baseline `--check` pass.

Finish the available offline work and rehearse the collector. Send one complete player checklist
with equipment, stance, target, re-lock after equipment menus, ordered actions, and observations.
Wait for `开窗` before verifying the process and arming. Confirm when armed and tell the player
to execute the listed actions. Wait for `完成` and verify the readback.

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
