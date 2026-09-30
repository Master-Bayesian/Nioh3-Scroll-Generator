# v0.8.3 unavailable-character repair

The owner reported an immediate player-vtable error on opening Equipment &
items in test7-r2, without having opened the game. The feedback was exported
as `nioh3-feedback-1790735972.txt`.
The owner subsequently confirmed that closing the game can leave its process
stuck in the background, consistent with the process observation below.

## Diagnosis

A residual `Nioh3.exe` process (PID 16364, birth
`2026-09-29T20:37:07.9494458Z`) had no main window. Its executable identity
matched the supported PC file version 2.0.2.0. Read-only observation found:

- Player global at module + `0x4751850` was non-null.
- Player vtable was module + `0x402D9E8`, rather than the loaded-character
  vtable at module + `0x402DA20`.
- The independent inventory global at module + `0x4751530` was null.

The inventory chain established that no character was available. The old
ordering checked the residual player vtable before this unloaded state and
therefore reported a misleading layout mismatch. These observations do not
justify changing the expected vtable or accepting another player layout.

## Bounded repair

Resolve the independent inventory global/root before checking a non-null
player's vtable. A null global/root uses the existing exact error
`character layout: no character is loaded`. A non-null chain with a foreign
vtable, or a valid vtable with the wrong container relation, still rejects.
All write identity, reread and process-birth guards remain in place.

The equipment page treats only a missing process or that exact unloaded
character error as normal information. It clears a previous live snapshot so
stale currency/equipment controls cannot remain editable. Save-file mode stays
available. Other errors retain their diagnostic presentation. Both messages
are localized in Chinese, English and Japanese.

## Evidence and limits

Artifacts are retained under
`D:/Nioh3_v080_deliverables/deliverables/codex-v083-character-layout-20260930/`.
The failure scenarios and browser regression were written before repair.
The meaningful pre-repair run passed 52/61 checks with nine expected failures;
the repaired run passed 61/61. TypeScript and the 1,117-message locale audit
also passed. Root inspected all six missing/unloaded screenshots in the three
locales.
The earlier `unavailable-red` run is retained as a selector/harness failure,
not product evidence.

Backend regression and process-observation receipts live in `backend/`.
The prewritten protected-wire regression reproduced the unloaded-state error
while both foreign-vtable and wrong-container controls passed. After the
ordering repair, all three pass (0.20 s), with JSONL request/response receipts.
The replacement single-EXE source identity, hashes, native page check and
delivery status are recorded separately in `candidate/DELIVERY.md` so the
package is built from a frozen clean source. Browser mocks alone do not
establish native or packaged acceptance. This task performs no game writes,
save writes, injection or game termination, and does not establish acceptance
of seeded insertion into an owner's real save.
