# v0.7.0 hosted build fixes

## Native source identity on Windows checkouts

The first hosted preparation of commit
`375bbc27dbcc040e093c0edd75293d828142888f` reproduced the freeze baseline's
native source identity failure. A local copy of the working files had passed,
but a fresh Git checkout with `core.autocrlf=true` converted the CUDA source to
CRLF. Its hash became
`c4f3ea4ac18bb1f236ef0649a854e94cfd610cae9c36003c21928e1dc0e3e61c`
instead of the DLL's recorded LF source hash
`7826ab5226d82e8825eff32c81cdf6650129e46d6919cb04a58b7ea08065acf7`.

The repair pins native source files to LF in `.gitattributes`. It preserves the
strict source/DLL/ABI check and does not alter numerical code, the build manifest
or the accelerator DLL. Validation must use a real Git checkout with Windows
line-ending conversion enabled, not just a copy of the developer's files.

Hosted release preparation runs against the pushed commit, verifies the actual
packaged workers and UI, and signs its update manifest using the existing
repository secret. The version tag is pushed only after this preparation passes.
The tag workflow rebuilds, verifies, signs and publishes the release assets.
