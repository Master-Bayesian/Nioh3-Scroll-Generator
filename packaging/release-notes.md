# Nioh 3 Studio 0.7.1 — Tauri 2

The desktop host now uses Tauri 2 and the shared Microsoft Edge WebView2 runtime.
The verified Python/C++/CUDA generation and native game adapters are retained.

## Installation

Download the Nioh3Studio ZIP, extract the whole folder, then run Nioh3Studio.exe.
Keep worker, packages and licenses beside the executable. No Python, Node,
Electron or Cheat Engine installation is required. Windows x64 and Microsoft
Edge WebView2 Runtime are required.

This first migration from v0.6.x or the withdrawn Electron v0.7.0 needs one manual
download. Their updater formats cannot install Tauri safely. Favorites and language
preferences from Electron are imported when available. Subsequent Tauri updates
use the signed Tauri feed, with in-place replacement, rollback on launch failure,
and cleanup after the new interface and backend have started successfully.
Unexpected user files in an old program folder are preserved.

## Changes

- Restore automatic startup update checks and explicit download/install controls.
- Start with no selected effects, Grace, enemies, rules, terrain or attempt filters.
- Fix Japanese names containing game ruby/font markers.
- Use sharp vector favorites stars, including high-DPI displays.
- Keep search, editor, favorites, selected-item cart, backup management and three UI languages.
- Preserve automatic backups, protected write ownership and recovery receipts.

The shared WebView2 runtime is not bundled. Tauri's host and backend migration was
checked with synthetic saves and packaged workers; prior in-game native-adapter
acceptance remains applicable to the unchanged backend. No new game writes were
performed for this shell migration. Requested R5 may still be normalized by the
game to R4, as previously observed.
