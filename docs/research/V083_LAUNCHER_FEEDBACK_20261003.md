# Launcher access refusal and confirmed Sudama name — 2026-10-03

## Evidence and scope

The owner supplied these three complete launcher log lines:

```text
unix_seconds=1790860311 launcher=0.8.3 拒绝访问。 (os error 5)
unix_seconds=1790860337 launcher=0.8.3 拒绝访问。 (os error 5)
unix_seconds=1791010326 launcher=0.8.3 拒绝访问。 (os error 5)
```

They have no operation, affected path or native source context. They do not
establish the tester's root cause. This is the portable launcher startup error;
the earlier runtime/game process-enumeration refusal is a different boundary.
The previous child-spawn failure had an `Unable to launch` prefix, so a preceding
file/cache stage is plausible for the quoted bare message, not confirmed.

Library evidence: chat/error screenshot `libfile_39bb569bed8081919618b1d7151602c1`
(file_00000000b78c81f5bb41360ac9a2af44), and clear game inventory screenshot
`libfile_a7cd2756d050819188b0669f86efbcf2`
(file_00000000e6c881f6a1ad0fc18bea1f85, 1920x1080). Root inspected the pixels;
this local task retained the official Library read metadata/OCR and reference.
Library materialization supplied a signed transfer; Windows does not support
the official helper's required `os.setxattr`, already established in this task.
No raw-download or metadata-bypass route is used, and no local pixel inspection
of these original attachments is claimed.

## Confirmed bounded launcher defects and repair

Owned Windows files reproduce a readonly `.cache.lock` returning bare OS error
5. A separately pinned valid cached `Nioh3Studio.exe` returns sharing error 32;
the old verifier converted the read failure to `valid=false`, started recursive
cache removal and deleted its marker before removal failed. Both red results
are retained in `launcher-before.log` in the external review packet.

The launcher now keeps typed IO failures with operation, affected path, native
ErrorKind and original OS message/source. Critical payload read, cache/lease,
extraction/commit, file verification and child spawn/wait operations are tagged.
Permissions/locking/read errors other than a known missing file propagate before
cache repair. Actual hash mismatch or missing-file repair retains the prior
verified-marker, exact membership, lease and process checks. Profile files,
backup/restore, game adapters and native shutdown are unchanged.

IO recovery explains the failed step and offers the matching full portable ZIP
in a new empty writable folder, running its inner `Nioh3Studio.exe`, while keeping
existing app data. It does not prescribe elevation, disabling protection or
repeated download. Verification failures and an active runtime retain separate
guidance. Logs include necessary runtime paths, redact the LOCALAPPDATA account
prefix and do not collect arguments, keys, file contents or unrelated processes.

Cache identity is the SHA256 of the complete embedded ZIP, not the filename or
version alone. Different payloads use different runtime directories; launchers
serialize preparation with the global cache lock and hold a shared lease through
child completion. The pre-existing concurrent/reuse/active-cache regressions
remain required. These facts do not prove whether the tester has an old process,
ACL issue, file lock or other blocker. Unknown or extra-file trees are retained;
no broad cache/profile deletion, permission rewrite or automatic write replay
is added.

## Owner-confirmed item name

The owner explicitly confirmed `0x3336` as `魑魅的魂核` after the tool screenshot
showed that exact raw ID. The clear game image shows a named Sudama core with a
selected detail panel at Lv150. The earlier tool record is Lv146; the screenshot
alone is not claimed to identify the same instance or its raw ID. The mapping
uses the owner's confirmation plus the image, not inference from neighbours.

The former PC v2.01 workbook name/text ID was unresolved; an empty source did
not establish a placeholder, invalid item or lack of a real name. The shared
catalog key 13110 is now named and keeps its existing soul-core grouping. Existing
catalog key 48491 remains `魑魅魂核`; neither key nor enemy is merged with another
yokai. The auxiliary enemy key 0X00007B82 has matching native text ID 0x0065BAF6
and names `魑魅` / `Sudama` / `魑魅` in the three captured locales. UI translations
use that identity and the project's existing soul-core terminology:
`魑魅的魂核` / `Sudama Soul Core` / `魑魅の魂核`. The full English/Japanese item-pool
strings are UI translations, not a newly captured native item-name claim.

Regression covers the raw ID/group and locale identity. Production-browser
fixtures also retain a separate unlisted soul core 0xFFFE, so naming the confirmed
entry does not remove unknown-name selection/edit-field coverage. These fixtures
do not establish the tester record's legality, actual edit success or remaining
menu-reading issue. An open, selected item in the new game image means the former
NoSelection repair cannot be declared to resolve that environment.

## Minimal next evidence and recovery

Use the new local diagnostic candidate only after root review; publication is
paused. Send one newest complete `launcher.log` entry from one failed start,
including operation/path/kind/native message, and the exact candidate filename
and SHA256. No game/save dump, credentials or repeated old screenshots are needed.
If the matching complete ZIP is available, extract into a new empty writable
directory and run its inner entry to distinguish cache failure from application
startup; retain the existing LOCALAPPDATA tree and logs. Do not rename/remove a
runtime or change ACLs before its ownership and active-user state are established.

The external packet owns final source/build identity, targeted checks and any
package acceptance. This research note does not itself claim the tester is fixed.
