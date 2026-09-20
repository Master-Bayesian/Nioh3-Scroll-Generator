# Frozen zh-CN catalog reference

`catalog_reference_zh.json` is the frozen M2.3c capture of the shipped Python
worker's zh-CN `search.catalog` replies for rarities 3, 4 and 5. It is an
independent oracle captured from the product implementation, and
`tests/migration/test_application_worker_parity.py` compares both workers
against it without regenerating it at runtime.

The file is a byte-exact copy of the capture that previously lived only at
`deliverables/m23c-application/catalog_reference_zh.json`; `deliverables/` is
ignored by Git, so a clean checkout could not run the gate. Source SHA-256:
`8E81756E6DC36E79C025D203CAF44BABE48EC254D3F2E40353F78ED3230484C5`.

The capture keeps the progression- and rarity-independent auxiliary half and
the recommended-level curve once, on rarity 3, and stores the rarity-specific
`ordinary_effects` and `grace_effects_ids` arrays for every rarity. Do not
regenerate it from the code under test.
