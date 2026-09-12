# Research handoff workflow

## Purpose

Research and reverse-engineering work in this repository is split into two roles:

- Codex gathers bounded evidence, builds reproducible capture tools, records the current source and binary context, and produces a verified handoff package.
- A Pro model performs the open-ended reverse engineering, mathematical derivation, causal analysis, or solver design from that package.

This split prevents long investigations from depending on chat history or an active game process and gives the Pro model the complete evidence needed to reach a reviewable result.

## What counts as research

Use this workflow when a task requires one or more of the following:

- reverse engineering native game code, data structures, save formats, or network behavior;
- deriving RNG, seed inversion, generation, probability, or selection algorithms;
- identifying a root cause whose mechanism is not already represented by a tested contract;
- collecting live game, Cheat Engine, debugger, disassembly, or binary-table evidence;
- validating behavior across game versions, languages, playthroughs, rarities, or hardware paths;
- deciding whether an observed behavior is deterministic, persistent, portable, or safe to expose as a product feature.

Routine implementation, regression repair with a known cause, UI work, packaging, and release operations are not research unless they encounter an unresolved mechanism.

## Required package layout

Create a dedicated directory under `deliverables/` and a ZIP of that directory. Use a descriptive, versioned name such as:

```text
deliverables/<topic>-pro-handoff-<platform-version>-<YYYYMMDD>/
deliverables/<topic>-pro-handoff-<platform-version>-<YYYYMMDD>.zip
```

The package must contain:

```text
README.md
TASK_FOR_PRO.md
ENVIRONMENT.json
SHA256SUMS.txt
evidence/
project-source/
```

`README.md` provides the reading order, scope, authoritative files, and known stale or abandoned leads.

`TASK_FOR_PRO.md` states the exact question, confirmed inputs, required outputs, constraints, and acceptance criteria. It must be usable as the Pro model's initial prompt without relying on the original conversation.

`ENVIRONMENT.json` records the repository commit and dirty-state flag, game/application version, binary or resource identities, locale, platform, and ground-truth vectors relevant to the task.

`SHA256SUMS.txt` covers every package file except itself and uses package-relative paths.

`evidence/` contains the smallest complete set of raw captures, disassembly, screenshots or extracted text, runtime observations, helper scripts, and machine-readable proof needed to reproduce the current conclusion. A complete scratch corpus may be included when pruning it would hide relevant negative results, but the README must identify authoritative files and abandoned leads.

`project-source/` contains the source files, tests, schemas, manifests, and compact data resources required to implement or verify the result. Do not copy unrelated build output or the full repository.

## Evidence rules

Label each material statement as one of:

- confirmed by native control flow or byte parity;
- observed in a controlled runtime experiment;
- inferred from incomplete evidence;
- unknown and requiring further work.

For every controlled vector, record:

- exact input and expected output;
- game and executable identity;
- playthrough, rarity, record type, locale, and generation stage where relevant;
- runtime addresses and stable RVAs separately;
- positive and negative controls;
- capture time and tool version;
- whether the probe read or wrote game memory or save data.

Do not use a current process address as a stable signature. Do not claim live acceptance from static analysis, synthetic tests, or package startup alone.

## Live capture procedure

1. Finish and inspect the observer before asking the user to perform the triggering action.
2. State exactly what the user should do and when the capture window is active.
3. Prefer read-only observation. Record every target-memory write when mutation is required.
4. Use the minimum number of controlled runs and reuse same-scroll positive/negative controls when they isolate the variable more strongly than separate samples.
5. Remove owned breakpoints, hooks, timers, and runtime overrides after capture. Verify cleanup explicitly.
6. Save raw output before interpreting it. Keep intermediate timing states separate from final-state observations.

## Privacy and redistribution

Do not include private saves, account identifiers, signing material, access tokens, full game executables, or redistributable copyrighted assets. Sanitize paths and binary records only when the removed data is irrelevant to the research question. If a field is necessary, document why and minimize it.

## Package verification

Before delivery:

1. Confirm every required file exists and is non-empty.
2. Recompute hashes for the authoritative evidence files.
3. Generate `SHA256SUMS.txt` after the final copy.
4. Create the ZIP and enumerate it to confirm it opens and contains the expected root directory.
5. Compute and report the ZIP SHA-256 and byte size.
6. Open `TASK_FOR_PRO.md` once from the final package and verify that it contains no chat-only references.
7. Add the package path, status, and unresolved product boundary to `CURRENT_HANDOFF.md`.

## Integration after Pro analysis

Treat the Pro response as a proposed result. Review its derivation, reproduce its tests, and compare it with the packaged ground-truth vectors. Add a new live vector when the feature affects game or save behavior. Only then move the result into domain contracts, application services, workers, UI controls, or release notes.
