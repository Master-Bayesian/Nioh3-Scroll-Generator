# 独脚踏鞴工作室 / Ippon-Datara Studio

> Find, add, edit, and protect Nioh 3 scrolls from one Windows desktop app.

[![Latest release](https://img.shields.io/github/v/release/Master-Bayesian/Nioh3-Scroll-Generator?display_name=tag&sort=semver)](https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/latest)
![Platform](https://img.shields.io/badge/platform-Windows%20x64-2563eb)
![Desktop](https://img.shields.io/badge/desktop-Tauri%202-24c8db)

Ippon-Datara Studio is a desktop toolkit for the PC version of Nioh 3. It helps
players search for game-legal scrolls, add selected results, edit scrolls they
already own, and recover from changes with verified automatic save backups.

The current stable release is **v0.7.1**, rebuilt on **Tauri 2**. The release
package includes everything the app needs except the shared Microsoft Edge
WebView2 Runtime that is normally already present on Windows.

[Download the latest release](https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/latest)
· [Read the v0.7.1 release notes](packaging/release-notes.md)

## What you can do

### Search for game-legal scrolls

Build a search around the result you want instead of scanning seed ranges by
hand. You can filter by:

- primary and secondary effects, including roll thresholds;
- Grace;
- enemies and legal enemy-group combinations;
- special rules, terrain, levels, and attempt settings;
- scroll type, playthrough, and rarity.

Search results are generated from the recovered game rules and replayed before
they are shown. You can preview a known scroll ID, compare results, save
favorites, collect candidates in the cart, and choose exactly which scrolls to
add.

### Add selected scrolls

Supported results can be added directly while you are in game or written to a
save while the game is at the title screen. The app verifies the current game
and save context, creates a backup, and records the operation before it writes.

The v0.7.1 release does **not** require Cheat Engine, Python, Node.js, or
Electron. Compatibility and safety checks remain inside the app; if the game or
save state is not supported, the operation stops instead of guessing.

### Edit scrolls you already own

The scroll editor discovers local saves and lets you review changes before
applying them. It can edit persistent basic fields and effect slots, update the
remaining attempt count, and delete selected scrolls.

Local effect editing is intentionally unrestricted. It does not automatically
make an arbitrary combination game-legal or suitable for sharing. Other players
regenerate effects from the scroll seed, so use the legal search and addition
workflow when you want a canonical result.

Mission details such as enemies, terrain, special rules, and the attempt limit
are temporary runtime changes. The app labels them as temporary and provides a
clear stop/restore action.

### Back up and restore saves

Every supported write creates and verifies a backup first. The Backups page can
show backups for each account and save slot, open the relevant folders, restore
a selected backup, or move an app-owned backup to the Recycle Bin. Restoring a
backup checkpoints the current save again before replacement.

Automatic backups are a safety layer, not a reason to skip your own long-term
save archive.

## Quick start

1. Download `Nioh3Studio-0.7.1-win-x64.zip` from the
   [latest release](https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/latest).
2. Extract the **entire ZIP** to a writable folder.
3. Run `Nioh3Studio.exe` and keep all adjacent folders and files together.
4. Choose English, Simplified Chinese, or Japanese from the language control.
5. Follow the status message and in-app prompts before any game or save
   operation.

Requirements:

- Windows x64
- Microsoft Edge WebView2 Runtime
- A supported PC build of Nioh 3 for game-connected features

Moving only the executable will not work because the packaged workers,
resources, licenses, and integrity manifest must remain beside it.

### Upgrading from an older version

Users moving from v0.6.x or the withdrawn Electron v0.7.0 need to download and
extract the Tauri package once. Later Tauri releases can use the signed in-app
updater, including replacement rollback and cleanup if the new version does not
start correctly.

## A simple workflow

1. **Search:** choose effects or mission conditions, run the search, and inspect
   complete previews.
2. **Collect:** favorite useful results or place them in the cart for comparison
   and batch selection.
3. **Add:** choose in-game addition or title-screen save addition, then review
   the operation.
4. **Edit:** use Scroll editor for local changes to scrolls you already own.
5. **Recover:** use Backups if you need to inspect or restore an earlier save.

## Important boundaries

- Search and local generation do not by themselves prove that every scroll will
  propagate to a second account through normal online play.
- Local effect edits persist on your machine, but recipients regenerate effects
  from the canonical seed fields.
- Temporary mission-detail overrides are not stored as permanent scroll data.
- Game-connected features are version-gated. Do not bypass an unsupported-build
  warning.
- Let active search and save operations finish or cancel through the app before
  closing it.

## More workshop tools

Ippon-Datara Studio is intended to grow beyond the current scroll workflows.
Additional tools are in active development; the disabled **Coming soon** entry
in v0.7.1 is intentional. New tools will be documented here only after their
user flow and safety boundaries have been verified.

## License and attribution

The project does not currently declare a project-wide open-source license.
Public access to the source does not by itself grant redistribution rights.
Third-party software terms and attributions are listed in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Help and diagnostics

For a problem report, open **Settings → Copy log** and include the copied details
with a clear description of what you were doing. Runtime logs are size-limited
and rotated automatically.

- [Open a GitHub issue](https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/issues)
- QQ group: `1106302479`
- Authors: MasterBayesian and Saber_Li

For implementation notes, validation evidence, and versioned reverse-engineering
records, see the [engineering knowledge base](docs/knowledge/INDEX.md).
