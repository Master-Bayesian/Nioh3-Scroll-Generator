# v0.7.3 local review delivery

The local candidate is complete and awaiting the owner's review. It has not
been pushed or published. Public latest remains v0.7.1 and v0.7.2 remains draft.

- Candidate source: `4b4214d7dc10bfa1df0ceb5c484c1eca8ae1d568`.
- Delivery: `deliverables/releases/0.7.3-review-20260912/`.
- Read `REVIEW.md` there for the change list, Pro closure, acceptance boundaries
  and suggested initial review. `verification.json` and `SHA256SUMS.txt` record
  the concrete artifact identity.
- Setup: 28,328,408 bytes; SHA-256
  `9043ddb9123673a9cc0c26b6606610f4629e5c0cb6ce0c65ab1061af2524b17f`.
- Portable ZIP: 30,699,748 bytes; SHA-256
  `94a95d80b8bf6052da7b6e72b8c351795c2aceec974e9e0c5ea8c7517ee315de`.
- Complete source ZIP and a binary-safe diff from `6264cbd` accompany the app.

## Final acceptance

Python 624/624, Node source 51/51, Node packaged 51/51, Rust 11/11, title
observer pytest 75 passed/one skip, and the isolated native fault matrix passed.
The clean checkout collects exactly the same 624 Python test IDs. Packaged
R3/R4/R5 searches match the source. All 735 portable files match the manifest
and archive. The manifest records the candidate commit with `dirty: false`.

Actual packaged WebView2 passed search/rules/favorites, synthetic inventory,
title-confirmed edit/delete/restore, original-byte backups, restore checkpoints,
journals, and automatic replacement of the Windows clipboard on worker error.
A running isolated package clone still exported useful diagnostics after one
manifest hash was changed. The original package stayed intact and clipboard
contents were restored after testing. The installer, installed UI and uninstaller
passed. Actual updater restart/handshake, previous-version deletion and staged
cache deletion passed against isolated package copies.

Live seed `10033001` was inserted once at a shrine, preserving 41 earlier
records. The user then normally saved/reloaded; read-only verification matched
all 42 records and the native index. Raw evidence is kept locally in
`deliverables/releases/0.7.3-validation/` and is not part of the source ZIP.

## Build notes and next action

Only one complete local candidate was built. The filesystem did not support
junctions, so dependencies were installed into the clean detached worktree and
Rust compiled there normally. No dependency-cache link was required. The Tauri
bundler rewrote Cargo.toml line endings without changing its normalized Git
object; the isolated file was restored to the committed form after comparing
the hashes. The checkout remains clean. Its expected already-patched NSIS
marker warning is covered by actual installer and update acceptance, not
resolved by modifying unrelated dependency versions or rebuilding blindly.

Do not resume packaging merely because the current branch later contains this
documentation-only follow-up. The candidate product bytes still correspond to
`4b4214d`. Do not push any branch, tag, feed or artifact before the owner reviews
the completed changes and authorizes publication. A product change following
review requires a new candidate identity. Public signing/hosted publication
checks remain pending authorization.

The original intermittent live-add cause and title-save corruption are not
claimed fixed. Native title ownership/C1-C3, actual-NG2 and possessed-enemy
research remain deferred under the owner's decisions. See the
[Pro closure](V073_PRO_REVIEW_CLOSURE_20260912.md) and
[current title policy](TITLE_SAVE_APPROACH_RESET_20260912.md).
