# Nioh 3 Studio 0.8.8

Download `Nioh3Studio-0.8.8-win-x64.exe` and run it directly, or use the signed
in-app updater. No manual extraction, Python, Node.js, Electron or Cheat Engine
setup is required. Microsoft Edge WebView2 remains a Windows prerequisite.

- Hell axes are natural again. The axe's three hell martial skills (碎地,
  大旋风, 噬魂) were missing, so every hell axe was marked unnatural; the
  ninja dual swords also gain their second skill (影斩). The skill list now
  matches the game's table (43 skills).
- Replacing a scroll effect sets its star the way the game does: every
  …之深奥 shows the star, and a plain effect put where a star effect was no
  longer keeps it.
- The project is now open source under the GNU General Public License v3.0.

## Supported scope and known limitations

The primary target is PC v2.02 (`2.0.2.0`). PC v2.01 (`2.0.1.0`) retains its
existing offline and registered scroll/count paths; seeded native equipment
addition is unsupported. PC v2.00.02 (`2.0.0.2`) retains its existing offline
scope. Unknown executable versions or missing resource/layout/ABI evidence are
refused; this release does not guarantee every DLC1 build or distribution
variant.

The dark theme's colors are still being refined. Menu recognition in one
reported environment remains unresolved. Changing terrain with a temporary
scroll edit can leave a mission without enemies (issue #29); when that happens
is not yet known.

Return to the title screen before editing a save file, then load it in game.
Keep an independent full-account backup. Automatic backups, single-writer locks,
rollback, native validation and protected-operation recovery remain enabled.
