---
name: nioh3-agent-orchestration
description: Route and review Nioh 3 Studio multi-agent work on the Astra-root and DeepSeek-worker split. Use when delegating to or reviewing worker agents in this repository; not for single-agent edits.
---

# Nioh 3 agent orchestration

Use this for repository work split across an Astra root and DeepSeek worker
agents. `AGENTS.md` routes here for the detail; read it and the current
`docs/knowledge/CURRENT_HANDOFF.md` state.

## Roles and routing

- The root runs on Astra and owns outcome, dependencies, ticket design,
  prioritization, risk and uncertainty decisions, and acceptance.
- Workers run on DeepSeek V4.1 Flash and execute coding, tests, documentation,
  packaging, and bounded evidence collection. Routing is operational, not a
  claim that Flash cannot reason.
- The default and only worker route is the official
  `router_deepseek_deepseek_v4_1_flash` role with model
  `deepseek/deepseek-v4.1-flash`, set by the owner. Do not select another
  DeepSeek provider or a quota fallback.
- Preserve the actual parent model and the router's model-scoped resolution; do
  not make Flash a global root default or change provider or global config from
  this repository.
- Workers may run a different model, so keep tickets self-contained instead of
  relying on inference.

## Shape each ticket

The root defines the completion criteria before dispatch. Give every ticket one
observable outcome plus the input and evidence paths, the files it owns, its
dependencies, meaningful acceptance, and a concrete escalation condition for the
root. Keep each ticket bounded and self-contained.

When a ticket turns out ambiguous or blocked, the worker asks the root one
concrete question and waits. It does not explore outside the named roots or
retry in a loop while waiting for the answer.

## Parallelize and serialize

- Run truly independent tickets in parallel only with mutually exclusive file
  ownership.
- Serialize integration that depends on worker output, and keep a single owner
  for live CE or game work.
- Do not mechanically split one task across agents when the reread and handoff
  cost exceeds the work.

## Keep context bounded

- Name the allowed search roots in the ticket, and keep discovery inside the
  named subsystem.
- Match filenames or paths first, then search content, and widen to one
  justified directory at a time; never default to recursive scans of the user
  home, whole drives, or all sessions.
- Bound the input actually scanned (file sizes, log or session window), not
  only the output that is displayed.
- Pass focused inputs instead of the whole session or archive.
- Reuse an agent for closely related follow-up; start a fresh bounded handoff
  when the topic or context bloats.
- Long runtime or repeated compaction signals task design, not a hard time
  cutoff; duration estimates are advisory.

## Review and evidence

- Review concise evidence reports; do not repeat every code or test
  investigation. For a critical risk, ask a targeted review question.
- Judge missing evidence by product need and risk rather than a blanket demand
  for two sources per field or proof of every null.
- Separate meaningful behavior tests from mere JSON, checker, or text mirroring,
  and reuse existing executable checks.
- Route broad unknown-cause reverse engineering through the existing Pro handoff
  workflow; the root decides that handoff instead of extending tickets
  indefinitely.

## Report and wait

- Report succinctly: conclusion, files, evidence, checks, remaining decisions,
  raw logs on disk, no fixed word floor.
- Message `/root` on completion or a genuine blocker, then finish; skip routine
  progress chatter.
- The root waits for events through the existing waiting mechanism. A fallback
  health check (for example around 29 minutes) covers failed notifications; it
  is not a worker time limit. Do not add duplicate recurring automation, exceed
  real tool bounds, ignore user interrupts, or poll minute by minute.

## Failures, skills, and checks

- A substantive research, build, packaging, or release failure (not an ordinary
  expected test failure) gets a bounded
  `docs/research/EXPERIMENT_FAILURE_LEDGER.md` entry, delegated to a DeepSeek
  agent. Include the objective, observed symptom, root cause or `unknown`,
  evidence paths, disposition, reproduction status, and follow-up state. The
  ledger is non-canonical, normally unloaded, and never blocks the main task.
- Skills record successful workflows. Keep a one-off failure in the ledger
  instead of promoting it into a skill or active conclusion, and keep symptoms
  separate from rules.
- Prefer deterministic scripts or checks for mechanical rules. Keep skills
  focused on decisions and workflow boundaries; do not duplicate authoritative
  documentation.
- When a task exposes a stable, reusable workflow improvement, update the
  matching skill, runbook, or executable check.
- Validate any changed skill with the system skill validator. Delegate simple
  documentation, archival, and audit work to DeepSeek when practical.

## Cost

Judge cost by the total qualified outcome: root plus worker tokens or cost when
measurable, including cache, retries, rework, and owner waits. Do not judge by
model list price or self-reported walltime, and do not make numerical
model-performance promises.

## Boundaries

Preserve every shipping safety, environment, and authorization requirement, and
never widen product, game, or publication scope.
