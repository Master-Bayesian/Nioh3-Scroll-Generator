# Nioh 3 Studio 0.8.10

Download Nioh3Studio-0.8.10-win-x64.exe and run it directly, or use the signed in-app updater. No extraction, Python, Node.js or CUDA toolkit installation is required. Microsoft Edge WebView2 remains a Windows prerequisite.

- Enemy, special-rule and terrain searches can use DirectCompute when CUDA is unavailable. Natural seed enumeration, rarity-4 primary searches and their batch predicates have the same fallback. Bulk CPU search still requires the explicit Settings choice.
- GPU failures retain their error code and show a useful explanation with technical details and feedback export, rather than telling every user to update a driver.
- Known scroll IDs show the generation and display/addition settings used by View. Search-only filters and result count are distinguished. A changed ID or relevant setting marks the preview stale and prevents addition until refreshed.
- The uncertain-write recovery panel in Add current scroll uses the full available width. Recovery checks and acknowledgement behavior are unchanged.
- Beta/rc launchers use valid numeric Windows version fields while keeping the complete version text.

## Supported scope and known limitations

The primary target is PC v2.02 (`2.0.2.0`). PC v2.01 (`2.0.1.0`) retains its
existing offline and registered scroll/count paths; seeded native equipment
addition is unsupported. PC v2.00.02 (`2.0.0.2`) retains its existing offline
scope. Unknown executable versions or missing resource/layout/ABI evidence are
refused; this release does not guarantee every DLC1 build or distribution
variant.

Item marks are known only for scrolls and equipment; stars on consumable items
stay in this program. Menu recognition in one reported environment remains
unresolved. Changing terrain with a temporary scroll edit can leave a mission
without enemies (issue #29); when that happens is not yet known.

Return to the title screen before editing a save file, then load it in game.
Keep an independent full-account backup. Automatic backups, single-writer locks,
rollback, native validation and protected-operation recovery remain enabled.