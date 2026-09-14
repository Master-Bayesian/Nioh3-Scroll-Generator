# Publish mode

Publish mode promotes already verified hosted bytes. It does not rebuild them. Follow sections 4-6 of the canonical [release runbook](../../../../docs/knowledge/RELEASE_RUNBOOK.md) exactly.

## Read-only promotion gate

Before requesting publication authorization:

1. Refresh remote release, branch, tag, and workflow state without mutating it. Confirm the selected workflow run belongs to the full candidate SHA and was not a failed or retried known-bad candidate.
2. Download the hosted artifact into a new directory. Verify sidecars, production Ed25519 signature, version/channel/platform, official tag URL and filename, ZIP CRC and safe paths, every manifest member's size and SHA-256, clean source SHA, outer footer/payload/stub identity, and the test inventory.
3. Run the required extracted-package, direct outer-EXE, updater, rollback/cleanup, WebView2, and applicable product acceptance against those exact bytes. Do not rebuild or rename files during promotion.
4. Refresh `origin/main`, require it to be an ancestor of the candidate, and prove the intended version tag does not already exist. Never move or replace a release tag.
5. Prepare an authorization summary containing the version, full SHA, target `main` update, annotated tag, successful workflow run URL/ID, every asset filename/size/SHA-256, exact planned mutations, and remaining acceptance limitations.

Stop and obtain explicit owner authorization immediately before the push/tag/release mutation. A hosted preparation dispatch is also a remote mutation and needs its own current authorization before it is run. If exact bytes or targets change after authorization, repeat the read-only gate and request renewed authorization.

## Authorized publication and verification

After authorization, use the runbook's atomic main/tag operation and create the release from the already verified downloads. Upload only the specified outer EXE, ZIP, matching SHA-256 sidecars, `tauri-update.json`, and test inventory. Do not rebuild, move an existing tag, replace the feed with a test signature, or announce broader acceptance than the evidence supports.

After publication, query the release again, download public assets into a fresh directory, and repeat exact hash, signature, member, source-SHA, and latest-stable checks. Record the release URL, tag commit, workflow run, asset sizes/hashes, and any bounded acceptance gaps in the canonical publication and handoff records without changing the tag.

If any mutation or public verification step fails, stop. Preserve the evidence and enter diagnose mode; do not retry a known-bad SHA or publish replacement bytes under the same asserted identity.
