# Tauri 2 migration and Electron withdrawal

## Public release state

At the user's request, v0.7.0 was changed from a public release to a draft.
GitHub's latest stable release was explicitly restored to v0.6.10. Anonymous
GitHub API checks confirmed `latest = v0.6.10`, no v0.7.0 in the public release
list, and the original `latest.json` update asset still present. No tag or source
history was deleted or rewritten. Already downloaded files are not remotely
removed; already staged updates cannot be recalled by changing the release feed.

The preserved v0.7.0 source is commit
`06270e154e424d1f3f64078b75d7ace172bf8d27`. The published archive is retained at
`deliverables/frontend-v2/published-v0.7.0/`, SHA-256
`d269a82b9086707d09a2bc6b382c6d1890911193b6f8e7f8372cbedfb11032ca`.
Its complete portable directory and EXE remain available locally.

## Preserved patch work, not a published release

The working v0.7.1 changes restore startup update checking and add successful
startup cleanup for both old and new update receipts, remove failed download
caches and verified ZIPs, strip Japanese ruby/font markers, and replace favorite
glyphs with SVG stars. The focused nine-test run and TypeScript checks passed.
The first visual regression run used an incorrect Grace expectation: seed
76634363 actually contains Magatsuhi, not Sarutahiko. That expectation was fixed;
visual acceptance is still pending at this checkpoint. No v0.7.1 release was made.

## Migration constraints

- Keep the existing React layout, catalog, favorites/cart behavior and locales.
- Replace the Electron host with Tauri 2 and the shared Windows WebView2 runtime.
  Do not bundle Chromium or a Node runtime under a different name.
- Retain verified Python/C++/CUDA numerical semantics and native memory adapters.
- Keep offline search killable. Save commits and native calls retain protected
  ownership, uncertain-result recovery and no automatic write replay.
- Port the broker's private candidate transfers and schema/identity checks.
  Renderer input must never become arbitrary filesystem, shell or memory access.
- Restore update check/download/install/rollback/cleanup parity before release.
- Preserve Electron as a separately runnable fallback branch, not the public
  recommended download. Do not publish migration builds without package and
  transition validation.
- Record compressed size, installed size and shared-runtime prerequisites as
  release acceptance data. The withdrawn ZIP was 184,569,902 bytes; its EXE alone
  was 246,482,944 bytes. Size acceptance was missing from the earlier release gate.

See [the release runbook](RELEASE_RUNBOOK.md) and
[the hosted failure record](V070_HOSTED_BUILD_FIXES_20260909.md).

## Tauri implementation checkpoint

The Rust host now brokers the existing framed Python workers, private candidate
transfers, search/cancellation, protected operations, preferences, favorites,
diagnostics, dialogs and native window controls. The release package embeds the
shared React frontend and uses system WebView2. Default search conditions are
empty. Exact-crate third-party notices are included with source provenance.

The local package passed real WebView2 startup, known-seed Japanese preview,
favorites, synthetic inventory and private-operation rejection. Rust tests cover
real worker search, JavaScript/Rust signature parity, installer rollback and
user-file-preserving cleanup. The final release workflow additionally verifies
real application restart/cleanup and packaged CPU/GPU parity before signing.

No new in-game write acceptance is claimed for the Rust host. Backend algorithms
and native dispatch code are unchanged. Public release publication must use the
same verified hosted artifact, not the dirty local development package.
