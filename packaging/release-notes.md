# Nioh 3 Studio 0.8.11

Download Nioh3Studio-0.8.11-win-x64.exe and run it directly, or use the signed in-app updater. No extraction, Python, Node.js or CUDA toolkit installation is required. Microsoft Edge WebView2 remains a Windows prerequisite.

- Fix an empty enemy section when a scroll has known enemies but its detailed enemy-state preview is unavailable. For example, NG3/R4 scroll 141974638 now displays Enenra, Kappa and Red Kappa.
- Show unconfirmed Crucible Wraith status separately from the known enemy list. A missing preview is distinguished from a confirmed empty roster.
- Preserve ordinary per-occurrence previews and the existing generation, filtering and save protections. An old cached preview should be refreshed by viewing its ID again or running a fresh search.

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
