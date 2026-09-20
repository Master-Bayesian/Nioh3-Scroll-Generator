# Frozen v2.02 identity-mapping evidence

`identity-mapping.json` is the frozen `go-v202-candidate-profile` mapping that
records how each of the 60 accepted PC v2.01 live-add code-identity ranges was
relocated in the PC v2.02 image: 50 mapped (17 from the accepted v2.02
inventory, 33 from a unique instruction-shape match) and 10 left unmapped with
their reason and candidate list. It is an independent oracle:
`tests/test_live_add_identity.py` asserts against these exact bytes, and a test
run never rewrites this file.

The file is a byte-exact copy of the mapping that previously lived only at
`deliverables/game-version-update-20260919/go-v202-candidate-profile/identity-mapping.json`;
`deliverables/` is ignored by Git, so a clean checkout could not run the
non-live tests. Source SHA-256:
`1F0DB8EB22D8300A561B5FA921D26343431AD9BAF9C2676074C5C696EDE20588`.

The exporter that writes that mapping was promoted from the same ignored folder
to `tools/export_v202_identity_resource.py` (original SHA-256
`3854D631A691EBFC6480E31D751FF5EAE1F3878B05F7F84161092B87A2848484`, tracked
SHA-256
`39C21AA66A634D876CB7E12F049BAE8F9B29571333C327C7243B15B969D33B71`). The
repository-root resolution was adapted to the `tools/` location. The ignored
gap-evidence output stays at its original lane path, with a parent-directory
creation guard added there; the derivation logic is unchanged.

The mapping was derived for the exact installed executable
`E22C4A635E4EC1E27A177B76E27D7F6A637F426C0ED3928B60F5693BC52AE130` from the
pinned v2.01 resource, the accepted v2.02 inventory, and the captured v2.01 and
v2.02 `.text` images. Regenerating it needs those external inputs and is not
part of the test run. The mapping is offline evidence only: it does not
establish live-game behavior, and the skip-guarded `.text.bin` leg of the test
remains the on-host check wherever the v2.02 lane image exists.
