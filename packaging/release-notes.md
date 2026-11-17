# Nioh 3 Studio 0.8.12

Download Nioh3Studio-0.8.12-win-x64.exe and run it directly, or use the signed in-app updater. No extraction, Python, Node.js or CUDA toolkit installation is required. Microsoft Edge WebView2 remains a Windows prerequisite.

- Support hell-weapon effect seed search in the existing save-file equipment-add flow. Select an eligible weapon, request applicable hell/ordinary/star/grace/set effects, review a result, and add the selected combination.
- Choose an eligible hell martial skill independently of the effect seed. The reviewed effects, skill and generation context stay bound through the queue, preview and save confirmation. Changed search inputs invalidate old results.
- Fix equipment addition failing on long-running saves whose equipment inventory key exceeds 65535. Equipment uses its existing native u32 key contract; source checks, backups and overflow refusal remain enabled.
- Enable native-validated Crucible Wraith prediction for the certified PC v2.02 solo selector-one route. Refresh old cached previews to use the corrected prediction. Other modes and unvalidated selectors retain their existing confirmation limits.

Hell effect results apply to the selected supported generation context and the character's progress. The effect seed does not determine the martial skill or every enemy/region drop modifier. The separate live-add workflow retains its existing scope.

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
