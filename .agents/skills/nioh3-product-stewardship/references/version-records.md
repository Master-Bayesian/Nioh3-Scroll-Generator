# Version records

Keep one internal record at `docs/product/releases/v<version>.md` while a version is active.

Create it when work is assigned to the version. Keep it concise and update it as decisions change. It is an engineering record, not a duplicate of player-facing release notes.

Include:

- status and product goal;
- affected feature IDs;
- added, changed, fixed, deferred, or removed behavior;
- acceptance criteria;
- completed bounded evidence and missing live or visual acceptance;
- compatibility or migration notes when relevant;
- exact source revision and artifact hashes only after those artifacts exist.

Before calling a version ready for owner review, compare the record with the affected feature entries, player documentation, localization, tests, and packaged UI. Report disagreements; do not silently choose a preferred source.

Do not treat a version number, build, smoke test, or handoff package as publication approval.
