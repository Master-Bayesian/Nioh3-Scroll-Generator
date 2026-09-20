# Frozen v2.02 resource-comparison evidence

`resource-comparison-v2.02-ng3.json` is the frozen PC v2.02 resource
comparison produced by the accepted `game-version-update-20260919` lane. It
records the per-file SHA-256 of the retained v2.02 generation resource against
the shipped v2.01 resource, plus the lane gates
(`static_generation_resources_equal`, `playthrough_context_equal`, and
`product_enablement_allowed`).

`tests/test_game_version_v202_resources.py` asserts against these exact bytes;
a test run never rewrites this file.

The file is a byte-exact copy of the report that previously lived only at
`deliverables/game-version-update-20260919/reports/resource-comparison-v2.02-ng3.json`;
`deliverables/` is ignored by Git, so a clean checkout could not run the
non-live tests. Source SHA-256:
`6116414CC1482BE4B1BE37DA2AF9CF297E791A56C0661FE719F0E8917B8399A0`
(12261 bytes).

This fixture is frozen offline evidence only. It is not a product runtime
input: registering the v2.02 profile enables no live path, and this comparison
does not establish live-game behavior, save-layout validity, or product
enablement.
