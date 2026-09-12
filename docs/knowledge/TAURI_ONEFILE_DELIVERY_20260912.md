# Install-free one-file delivery

## Product decision

The v0.7.3 candidate must be one directly runnable download named
`Nioh3Studio-0.7.3-win-x64.exe`. It is not an installer and must not require
manual extraction, an installation wizard, Start-menu registration, or an
uninstall registration. The old NSIS candidate remains archived locally; it is
not advertised as the replacement for this requirement.

Publication is still prohibited until the owner reviews and approves a concrete
local package. The final local candidate and its scoped runtime acceptance are
recorded below; nothing has been pushed or published.

## Container and provenance

The outer executable is the exact manifest-owned launcher, the already verified
portable ZIP, and a 56-byte footer. The footer is the 16-byte ASCII magic
`NIOH3_ONEFILE_V1` (no NUL), the ZIP length as a little-endian unsigned 64-bit
integer, and the 32-byte SHA-256 digest of the ZIP.

`tools/build_tauri_onefile.py` verifies every ZIP member against the clean-source
portable manifest, rejects extra files, unsafe paths, duplicate names, missing
runtime components, and invalid launcher PE headers, and enforces the 60 MiB
download budget. It embeds `launcher/Nioh3Launcher.exe` without patching its PE
metadata. The launcher is built once alongside the app and included in the
normal portable manifest and Rust license inventory. The wrapper emits its own
SHA-256 sidecar. The payload SHA-256 is corruption detection; update authenticity
continues to come from the production Ed25519-signed update manifest.

## Runtime and update ownership

The launcher verifies and extracts the embedded package into a hash-addressed
cache under `LOCALAPPDATA/Nioh3Studio/onefile`. Reusing a verified current cache
is allowed. Inactive old caches are bounded and pruned; user settings and save
backups are separate. Cache extraction is runtime support, not installation.
The launcher waits for its child and uses cache leases so another instance
cannot remove an active runtime.

The normal signed ZIP update feed remains compatible with older Tauri copies.
One-file mode reconstructs a new outer EXE using the signed ZIP's own launcher,
replaces the original outer executable, and confirms both the outer hash and
inner manifest after restart. Cleanup removes only the exact recorded old EXE
and UUID download directories after startup acknowledgement. A failed startup
retains a rollback copy. Legacy directory mode keeps its existing update path.

## Required bounded acceptance

- Verify wrapper footer, payload digest, manifest members, stub identity, unsafe
  path rejection, source cleanliness, and output no-overwrite behavior.
- Launch the outer EXE directly with isolated app data and confirm real WebView2,
  Python worker handshake, and the product UI. Keep original package bytes intact.
- Confirm no installer/uninstaller registration or adjacent extracted runtime
  appears. Repeat launch and check verified cache reuse and bounded stale-cache
  pruning. Keep actual user saves and the game outside the test.
- Exercise replacement through the outer EXE, then require startup
  acknowledgement, exact previous-EXE cleanup and download-cache cleanup.
- Reuse the encrypted synthetic-save and real-clipboard checks; preserve the
  user's clipboard during verification.

## Report disposition

The user closed the unreproduced intermittent live-add and title-save reports
pending a fresh affected save and log. They are not release blockers and no
root cause is claimed fixed. Keep verified automatic backups, generation/race
guards, and no-replay receipts. Do not restart broad ETL/native-save research.
Website work, usage statistics, and possessed-enemy selection remain outside
this release. Record final candidate evidence separately after it is produced.

## Windows path regression found during acceptance

The first outer-update test exposed Windows PowerShell 5.1 rejecting `Join-Path`
when its parent uses Rust's canonical `\\?\` path format (`drive is null`).
Use `System.IO.Path.Combine`, `File.GetAttributes`, and `ProcessStartInfo` for
the outer-file workflow. Preserve the canonical prefix; stripping it can break
long paths. Use the helper directory as the child working directory because
the launcher establishes its own runtime location. The update acceptance must
pass the real canonical target and wait for both the actual app and launcher
PIDs; synthetic nonexistent PIDs alone missed this interoperability issue.

## Completed local candidate

Candidate `ca757396c42092a9f1ee19e960da67ca5f52e5cc` was built from a clean
isolated checkout. The outer EXE is 31,746,127 bytes, SHA-256
`7067203816c605b32683103234fa0fb7c6fda60999a5e1ec971b64b351cba72c`.
Delivery and raw evidence are under
`deliverables/releases/0.7.3-onefile-review-20260912/`.

The exact final EXE passed direct-launch/cache/registry acceptance, actual outer
replacement with both real process IDs and canonical paths, new startup and
rollback/download cleanup, packaged generation parity, and the encrypted
synthetic-save/real-clipboard UI workflow. There were no game writes. The
outer-update fixture deliberately keeps the product version constant while
changing archive bytes; signature validation remains separately tested.
Fifteen Rust host, ten launcher and seven Python wrapper regressions passed.
The earlier D44 candidate exposed the Windows path issue and is historical,
not the file to provide to the owner.
