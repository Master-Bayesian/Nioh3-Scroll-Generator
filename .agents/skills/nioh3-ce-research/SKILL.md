---
name: nioh3-ce-research
description: Prepare and perform bounded Nioh 3 Cheat Engine or runtime evidence collection, validate native observations, and package unresolved mechanisms for Pro analysis. Use for live probes, native tables, equipment applicability, and game-version research; not routine UI or release work.
---

# Nioh 3 CE research

Read `AGENTS.md`, `docs/knowledge/CURRENT_HANDOFF.md`, and
`docs/knowledge/RESEARCH_HANDOFF_WORKFLOW.md`. Use `docs/knowledge/INDEX.md`
to locate the authoritative subsystem and version evidence. Apply the current
owner decision to select the active question and scope each run explicitly.

## Prepare the bounded experiment

- Write the question, current evidence, missing controlled vector, run limits,
  and expected output before collecting. Select the smallest relevant row in
  [the probe map](references/probe-map.md), then inspect that probe and imports.
- Prefer observation and file-backed captures. Add owned hardware breakpoints
  only when they answer the question. Obtain explicit authorization before any
  game-function call, register/state change, code patch, equipment injection,
  or save modification, with the target and write boundary recorded.
- Verify the approved executable/profile, PID and process-birth identity, module
  base, and every probe-site signature before arming. Resolve addresses from the
  current module and revalidate attachment in each callback. Route failed
  identity or signature checks to bounded static evidence.

## Execute and close

Read [runtime lifecycle and evidence](references/runtime.md) before live work.
When the CE MCP bridge is unavailable, run its approved self-start helper; it
verifies the pinned installation and launches CE only when the approved plugin
is absent, without attaching a game. Inspect the observer, dependencies, stop
path, and mocked tests before requesting the trigger.

Start every phase with fresh initialization. Use the owned-breakpoint and timer
helpers with explicit hit, time, and output limits. Complete cleanup
verification before starting another phase. Use positive and negative controls
that isolate the question and label intermediate versus final state.

Save raw output and a run manifest before interpretation. Grade the run using
its controls, identity checks, event completeness, and cleanup proof; retain
partial or exploratory status when a checkpoint is incomplete. Verify owned
breakpoint/hook/timer removal and authorized byte restoration, including the
process-exit state. A late trigger remains a zero-event timeout; bounded
post-spawn salvage is available only with a validated same-process owner hint
and is classified as post-spawn state evidence.

## Evidence and handoff

Scope every claim by executable, record type, playthrough, rarity, caller path,
generation stage, and online/offline state. Report table membership, natural
generation, save persistence, and network propagation as separate claims. Use
the evidence grades in `INDEX.md`: native byte parity, native control flow,
native table, observed, inferred, or unknown.

Codex gathers bounded evidence and reproducible tools. Route unresolved causal
analysis, reverse engineering, RNG mathematics, and inverse-solver design to a
self-contained Pro package using `RESEARCH_HANDOFF_WORKFLOW.md`: a dedicated
`deliverables/` directory and ZIP containing `README.md`, `TASK_FOR_PRO.md`,
`ENVIRONMENT.json`, `SHA256SUMS.txt`, `evidence/`, and `project-source/`.
Include controls, current source dependencies, and explicit unknowns. Verify
the package and record its status, keeping the final ZIP hash outside the
archive. Reproduce applicable Pro conclusions with independent evidence before
product integration; label package startup, mocked tests, and static
signatures as bounded checks.
