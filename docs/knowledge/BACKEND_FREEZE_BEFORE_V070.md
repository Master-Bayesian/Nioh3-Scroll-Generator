# Backend freeze before Frontend V2

Date: 2026-09-07
Release baseline: stable v0.6.10
Working branch: `codex/todo-321`

## Purpose and stop condition

This is the bounded safety and interface pass requested before the Frontend V2
and repository architecture redesign. It closes the B1-B6 audit findings
without expanding generation mechanics or starting a language/framework
rewrite. The stop condition is a tested, packageable working tree with explicit
remaining runtime-verification boundaries. It is not a release by itself.

## B1-B6 implementation status

| Area | Frozen contract |
| --- | --- |
| B1 save transactions | Account-scoped cross-process writer lock; a pre-commit v2 backup manifest pins Steam account, save slot, save schema/profile, semantic file role, size, and SHA-256; legacy manifests fail closed; the source main save must pass decryption and structure validation; multi-file restore uses unique staging, a durable journal, reverse-order rollback, and an explicit `committed_with_warning` result for post-commit report failures. |
| B2 native lifecycle | A timed-out remote call transfers its process/thread/allocation ownership to a background waiter, which frees remote memory only after confirmed thread completion; hook teardown only claims success after restoration or confirmed process exit; the window waits for background work and refuses to close while hook state is uncertain. |
| B3 operation policy | One policy service separates generated preview/install, unrestricted local editing, and temporary runtime override. Generated NG4/NG5 installs are blocked. NG1/NG2 rarity-4 custom-only generation remains available, and rarity-4 final preview retains its paired stage-one installation record. |
| B4 accelerator ABI | Seed accelerator ABI v2 exposes a build ID and execution policy. GPU paths default to strict mode, and an internal CUDA failure cannot silently enter the bulk CPU implementation. Explicit CPU fallback remains possible only through the caller policy. Source/DLL hashes and build metadata are verified in tests and release builds. |
| B5 service seam | `core_services.py` defines the UI-independent generation context, operation policy, candidate identity, typed errors, job states, preview, and install plan. Tk installation and the minimal JSON headless preflight use the same service. Cache identity now includes the complete generation-context digest. |
| B6 test and evidence contract | Top-level tests are discoverable by `unittest`; stable test IDs and hardware skip policy are exported as JSON; CI verifies the native build manifest; corrected documentation separates verified behavior, hypotheses, historical material, and disproved claims. |

## Identity and cache boundary

`GenerationContext` binds all of the following into one SHA-256 digest:

- product version;
- supported game profile;
- every packaged runtime data file and relative path;
- generation algorithm version;
- operation policy version;
- Seed accelerator ABI and native build ID.

Grace and primary-map caches use schema v2 and require that digest. Older
caches are not silently reused under a changed algorithm, resource set, or
native accelerator.

The Seed accelerator currently records:

- ABI: `2`
- source/build ID:
  `sha256:7826ab5226d82e8825eff32c81cdf6650129e46d6919cb04a58b7ea08065acf7`
- DLL SHA-256:
  `03b5ac233d96e9810643e2e9c48bbba40bfe50b06810d0e4c05567ad673cb118`

## Automated evidence

The final local run completed on Windows 11 with Python 3.12.14:

- `python -m unittest discover -v`: **449 tests passed in 67.171 seconds**;
- native build manifest verification passed for ABI, source hash, DLL hash, and
  exported build ID;
- the headless handshake returned the pinned resource, ABI, build, and context
  identities;
- the PyInstaller one-file package built successfully and remained alive for
  the five-second startup smoke;
- smoke package size: `18,444,094` bytes;
- smoke package SHA-256:
  `5150508A2DA91329E9A399FA3D967BF07D75E01E46A6726D3AE9437C8147E2D1`.

The test coverage includes:

- wrong-account and wrong-slot/hash restore rejection before writes;
- rollback after an injected second-target replace failure;
- post-commit report failure classified as success with warning;
- cross-process writer exclusion;
- remote-timeout allocation retirement and hook-stop uncertainty;
- window-close waiting behavior;
- generated-install policy across NG1-NG5 and rarity-4 record pairing;
- shared Tk/headless preflight policy and generation-context handshake;
- injected CUDA failure with zero hidden bulk-CPU calls;
- exact CUDA versus explicitly selected CPU cursor/result parity;
- native manifest source, binary, ABI, and build-ID verification.

## Evidence still requiring the game

The backend freeze does not claim these outcomes from source tests alone:

- a real save restore interrupted between physical file replacements;
- a real remote game call that remains executing past the timeout;
- exact-role enemy hook restoration after a non-zero live hit count;
- player-side generation, add, reveal, challenge, or propagation behavior not
  already listed as accepted in the versioned project status.

These are runtime acceptance tasks, not reasons to reopen the bounded B1-B6
implementation before the architecture redesign.

## Next phase

The next phase is an Astra-led repository architecture audit followed by a
combined backend/frontend redesign. Astra should begin read-only, treat these
contracts and regression tests as migration gates, and replace implementation
internals only when equivalent behavior is demonstrated. Frontend V2 may use a
different process or language boundary, but it must consume the same typed
policy outcomes, generation identity, job lifecycle, and save-transaction
result semantics.

Purple/empowered enemies, possessed Underworld variants, reroll prediction,
and other generation research are P3. They must not block product architecture
or Frontend V2. The reroll product TODO remains abandoned unless the user
explicitly reopens it.
