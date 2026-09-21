# Publish

Use the [runbook](../../../../docs/knowledge/RELEASE_RUNBOOK.md) publication
helper with an explicit successful build run ID, full source SHA and version.
Run its default read-only plan first; after owner authorization use `-Publish`.
The manual publication workflow exposes the same boundary. Neither path builds
the app or signs replacement bytes.

The helper verifies the exact repository/workflow/run, six assets, clean source
manifest, production signature, archive members and outer EXE identity before
promotion. It then checks branch/tag state, promotes the immutable commit,
uploads the unchanged files and verifies public stable downloads and the update
feed. Read its persisted plan/result rather than recreating these checks in
ad-hoc shell commands.

An existing matching public release is a verification-only operation. A
conflicting tag, different asset or unexplained partial publication needs a
decision; preserve it instead of replacing or force-pushing it. A failed public
check means publication is incomplete even if the release page exists.

After success, update the publication record and current handoff in a separate
documentation commit. Keep the product tag unchanged. Give the user the outer
EXE and public release links, and stop the release-specific fallback wakeup.
