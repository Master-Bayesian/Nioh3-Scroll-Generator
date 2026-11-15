# Nioh 3 Studio 0.8.10-beta.1

Private compatibility candidate. Run Nioh3Studio-0.8.10-beta.1-win-x64.exe directly; no extraction or Python/CUDA toolkit installation is required. Microsoft Edge WebView2 remains a Windows prerequisite.

- Scroll searches using enemies, special rules and terrain can use DirectCompute when CUDA is unavailable. Natural seed enumeration, rarity-4 primary searches and the corresponding batch predicates use the same fallback.
- Bulk CPU search still requires the explicit Settings opt-in. Search criteria, generated records, result order, cancellation and continuation retain their existing meaning.
- Unavailable acceleration no longer gives every user the same driver-update instruction; the message points to technical details and exported feedback.

The local acceptance includes real NVIDIA and AMD integrated DirectCompute adapters. Discrete Radeon user acceptance is still pending; this candidate is not the stable release.

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