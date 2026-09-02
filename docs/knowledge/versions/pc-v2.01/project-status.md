# PC v2.01 project status — 2026-09-02

## Status

`released`; integrated in stable v0.6.8 on 2026-09-02.

## Passed

- Installed executable identity and live PE-section capture.
- Automatic relocation of every profiled site, with explicit evidence-backed
  overrides for duplicated float constants.
- Runtime validation of the parameter-manager, playthrough-selector, and mode
  pointers through a complete read-only table capture.
- Byte-for-byte equality of deterministic effect and auxiliary resources.
- Equality of the active NG3 playthrough vector.
- Static compile and focused migration/wrapper tests.
- Native rarity-3 and rarity-4 full-record parity across 10,000 deterministic
  natural Seeds each.
- Native rarity-5 effect-slot parity across 10,000 deterministic natural
  Seeds. PC v2.01 adds a feature-flag-9 header cap; no other mismatch occurred.
- Repeated native auxiliary descriptor parity, including exact equality with
  the shared v2.00.02 control Seeds.
- Read-only decryption and parsing of the current 9,437,616-byte encrypted
  save with the unchanged 400-slot layout.
- Product version registry, native runtime profile, auxiliary-hook profile,
  rarity-5 preservation mode, and complete 401-test release regression.
- The GitHub release workflow passed its 401-test suite, built the one-file
  executable, signed `latest.json`, and published both assets. The downloaded
  release manifest, size, SHA-256, Ed25519 signature, and GUI startup smoke all
  passed locally after publication.

## Stable v0.6.8 search and UI follow-up

Stable v0.6.8 fuses CUDA auxiliary filtering, embeds
precompiled DirectCompute shaders, automatically continues bounded solver pages
until the requested candidate count, prompts before any no-CUDA native CPU
fallback, and adds a vertical scrollbar to the local editor. The user accepted
the local search fix before publication. GitHub Actions passed all 409 tests,
built the one-file executable, signed the update manifest, and published the
stable release. The downloaded manifest signature, asset size and SHA-256, and
packaged startup smoke passed locally.

## Known version difference

PC v2.01 calls the new feature-availability helper with flag 9 while assembling
the record header. When that flag is unavailable, requested rarity 5 is capped
to 4 at offsets `0x30` and `0x31`. The seven generated effect slots remain
identical to the existing exact model. Product mode preserves an explicit raw
rarity-5 request, while research parity continues to expose the unmodified
native result.

## Remaining acceptance

- First player-side v2.01 generation, installation, detail display, and live
  challenge remain post-release acceptance evidence, not a pretense of an
  already observed result.

## Architecture decision

The urgent PC v2.01 compatibility work does not wait for a language rewrite.
The version profile is JSON and intentionally language-neutral so a later Rust
core can consume the same RVAs, signatures, table identities, and gates.

Rust should replace Python orchestration and CPU exact replay incrementally in
v0.7. Bulk Seed search should use CUDA or D3D11 compute when available and must
never silently fall back to a whole-space CPU scan. A user may explicitly
confirm the exact native CPU fallback after a clear performance warning.
Rewriting the same brute-force CPU algorithm in Rust alone is not an adequate
performance design.
