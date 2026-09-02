# Nioh 3 PC v2.01 knowledge base

This directory records migration evidence for file version `2.0.1.0`, Steam
build `24963987`, labeled PC v2.01 in player-facing text.

The version is approved for product use in v0.6.7. The approved runtime profile
remains exact-version and signature gated; later executable versions are not
implicitly compatible.

## Current evidence

- The live executable identity was captured with SHA-256
  `4047ECB623F3AE033F7D6F3637CD1C5408C72A89A038463268A7C9AC5C394159`.
- Context-anchor migration resolved all 22 profiled code sites and all 6 float
  constants. Repeated constants were disambiguated by RIP-relative xrefs.
- All deterministic R4 effect resources are byte-identical to PC v2.00.02:
  nine fixed-stride tables, 662 bonus-curve entries, six float constants, and
  the three mode-gate bytes.
- All captured auxiliary tables are byte-identical to PC v2.00.02: 487 enemy
  rows, 20 terrain rows, 301 special-rule rows, 29 conflict rows, and 7 special
  contexts.
- The active NG3 playthrough vector is identical. Other captured playthrough
  vectors contain save-dependent progression and are reported separately.
- The relocated LCG has the same 28-instruction normalized structure.
- The scroll assembly function added a feature-flag-9 rarity cap. Rarity 3 and
  4 matched full records for 10,000 Seeds each; rarity 5 matched all effect
  slots for 10,000 Seeds and differed only at the documented header cap.
- Repeated native auxiliary descriptors matched the old control vectors, and
  the current encrypted save parsed with the unchanged 400-slot layout.

## Product boundary

- Both PC v2.00.02 and PC v2.01 use separate exact runtime profiles.
- Static deterministic resources are shared only because their payloads were
  compared byte-for-byte.
- Raw rarity 5 remains available as an explicit custom request; v2.01's native
  feature cap is reported rather than mistaken for an effect-generation change.
- Player-side generation, installation, detail display, and challenge remain
  acceptance work after the v0.6.7 release.

See [evidence-register.md](evidence-register.md) and
[project-status.md](project-status.md).
