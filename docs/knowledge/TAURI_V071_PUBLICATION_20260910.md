# Tauri v0.7.1 publication — 2026-09-10

Published stable release: https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/tag/v0.7.1

- Immutable product source/tag commit: 5014a48d9983ba0620d862afa84e0111ce52a3ca.
- Clean hosted build and backend checks: run 34435249593.
- Exact-candidate acceptance: evidence/tauri-v071-acceptance.json.
- Successful signing-only workflow: run 34436638118. No executable rebuild.
- ZIP: Nioh3Studio-0.7.1-win-x64.zip, 29,669,003 bytes (28.3 MiB).
- ZIP SHA-256: b1e99389b8836d4c203fad1f079c05fa242fd3d09d23d1a507ce6abd569f3997.
- Installed manifest files: 39,801,428 bytes; executable: 12,204,032 bytes.
- 757 archive members verified against the embedded clean-source manifest.
- Official Ed25519 signature and ZIP hash verified after artifact download and
  again after downloading the uploaded GitHub release assets.
- GitHub latest is v0.7.1. Withdrawn Electron v0.7.0 remains a draft; its original
  tag and preservation branch are unchanged.

The exact hosted package passed local WebView2 UI acceptance (empty defaults,
Japanese known-seed preview, favorites, isolated inventory, private-method refusal)
and real application replacement/restart/old-version/cache cleanup. Hosted UI
startup was healthy but its CDP port was unavailable; do not describe that hosted
UI automation as passing. Backend/core/frontend/Rust checks and packaged R3/R4/R5
parity passed on the hosted builder. This shell migration made no new game writes.

Local delivery is under deliverables/frontend-v2/published-v0.7.1/. Run
portable/Nioh3Studio.exe or distribute the complete ZIP, not the EXE alone.
WebView2 is a shared system prerequisite. Migration from legacy/Electron needs
one manual ZIP installation; subsequent Tauri updates use tauri-update.json.
See RELEASE_RUNBOOK.md for the path-alias, workflow-format and WebView2 runner
lessons and the guarded candidate-reuse route.
