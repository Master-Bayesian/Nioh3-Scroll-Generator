# v0.8.0 regression fixes — 2026-09-25

Status: **source fixes with bounded evidence; development only**. This record
maps the v0.8.0 user reports and the 2026-09-22 static audit to the changes on
branch `claude/v081-regression-fixes`. It does not authorize a release, a tag,
or a live-game write, and it claims no live-game acceptance.

## User reports

| Report (v0.8.0) | Root cause | Fix |
| --- | --- | --- |
| Grace filter: `Grace filtering needs the rarity-specific Grace output map, which this development worker does not load yet` | `query.rs::check_grace_filter` refused every non-empty `grace_effect_ids`; the UI sends the single choice in that list too. The job layer also lacked the final `c.grace` acceptance. | Payload checks only in the parser; membership checked against the context-bound tables before compilation (R4 final Grace ids, the R5 measured map); `jobs::accepts` applies `search_jobs._run`'s final Grace filter; several Graces pivot on the union of their draw-1 preimages. |
| Terrain filter: `terrain option ids have no shipped reference implementation ...` | `QueryCompiler::compile` refused every `terrain_selection_ids`. | `crate::terrain` ports `terrain_choices` / `resolve_terrain_selections`; the catalog publishes and the search resolves the same ids; the row union feeds the native mask, `has_terrain_constraint` and the composed final acceptance on every route. |
| Temporary override: `...\data\pc_v2_02.json: file not found` | `runtime_app::identity()` passed the data root, not `data/game_versions`, to the profile loader. | One shared `profile_dir_for` rule for every runtime identity. |
| (after the path fix) `PC v2.02 runtime profile is not approved for product use` | The overrides resolved under the blanket native-write purpose, which PC v2.02 does not have. | New purpose `ProfilePurpose::TemporaryOverride`, approved for PC v2.02 only (`TEMPORARY_OVERRIDE_APPROVED_VERSIONS`); the challenge getter is version-selected. See the evidence below. |
| Editor "核对修改" does nothing | The catch used `(e as Error).message`; Tauri rejects with strings, so the status went blank. A successful check cleared the status and opened the confirmation off screen. | `errorText` normalizes every rejection; phase messages; the confirmation scrolls into view; an untouched recommended level skips the worker conversion. |

## Other audit items fixed

- **P0-01 resource binding.** The search compiler loaded the legacy v2.00.02
  tables while the materializer composed from the selected version. The
  factory now receives `Materializer::resource_version()`.
- **P1-01 grouped rolls.** Thresholds for any-of members are split out before
  the request invariants, as the reference does, instead of being refused.
- **P1-02 unconstrained R5.** Takes the full-family replay.
- **P1-03 level 0.** `fromSample` uses `??`.
- **P1-04 untouched header bytes.** `save.prepare_edit` keeps the stored header
  (including the `+0x08`/`+0x12`/`+0x31` mirrors) when every header field is
  unchanged, in Rust and Python.
- **P1-05 pending history.** Receipts are newest first and unresolved ones are
  never cut by the 128-entry window (Rust and Python).
- **P1-06 warnings.** `committed_with_warning` is shown as written-with-warning.
- **P2-01 backup time.** `save.backups` publishes the backup directory name,
  as the Python reference does, instead of an always-empty mtime.
- **Count editor.** It always used the PC v2.01 inventory addresses; it now
  selects the layout of the exact running version (PC v2.02 uses the natively
  accepted live-add inventory binding) and refuses any other build.
- **Live-add prepare (Pro review 2026-09-24).** Prepare records the actual disk
  checkpoint `D0` (`disk_persistence_baseline`) and no longer requires it to
  equal the live-before inventory; batch and legacy-parent rules follow the
  reviewed admission table. Execute/readback are unchanged.

## Live-add builder flag bit 25 (community receipts, 2026-09-25)

Four community preview receipts (two seeds typed by hand, two from search) were
all rejected with `Native builder output differs from reviewed record`. Byte
for byte the only compared difference was record `+0x1B`: the plan expected
the fixed `ASSEMBLY_FLAGS = 0x02800002`, the game built `0x00800002`
(bit 25 clear). Every effect byte matched and each preview settled as
`rejected_after_preview` with its container and index unchanged.

Cause, from the pinned runtime `.text` (static decode, both versions):

- The builder (v2.02 `0x227FD5B..0x227FD9E`, v2.01 `0x227C5CB..0x227C60E`) sets
  bit 25 only when the descriptor identity `J` (record `+0x02/+0x04/+0x14`)
  equals the ambient identity `A`.
- `A` is `0` unless: the session pointer (v2.02 `0x4BD0B08`, v2.01
  `0x4BCCAB8`) is set and its state byte `+0xD8` is 2 or 4; the online gate
  byte (`0x45BA280` / `0x45B6240`) is set; and the identity object
  (`0x4B5AD58` / `0x4B56D08`) has kind `(u16 +0x10 & 0xFF00) == 0x100` and
  class `+0x12` in {1, 2}. Then `A = u64 +0x00`.
- So an online player whose save identity is their own account gets
  `0x02800002`, and an offline player (or a foreign save identity) gets
  `0x00800002`. The fixed constant only ever matched the first case.

Fix: inspection reads `A` through exactly that chain after proving the four
helper bodies byte-identical to the reviewed image (and refuses an identity
the game has not initialized yet), stores it in the plan as
`builder_ambient_identity`, and prepare builds the expected record with
`assembly_record_in_context`. Preview, insertion and verification all compare
that one plan record, so no gate keeps the old expectation. Only bit 25
follows the rule; every other byte is still compared exactly. Python mirrors
the rule for parity.

**Live acceptance, online branch (2026-09-25, owner machine, test4 build):**
the running game read `A = <own account>` (session state 2, gate 1, kind
`0x101`, class 1) with all four helper bodies byte-identical in memory. One
live addition (seed 102271721, R4) previewed `0x02800002 == 0x02800002`,
inserted into slot 33 (serial 2505498) with stored flags `0x06800082`, and
after a normal in-game save the decrypted `SAVEDATA.BIN` held the same record
and passed `verify_persistence` against the verified live inventory (47/47).
A second online addition with the test6 build (seed 114514, slot 35, serial
2506389, stored flags `0x06800082`) also verified live; it had not yet been
saved in game when checked. **Still open:** the offline branch (`A = 0`) live.

The same machine first failed with `Use a canonical operation UUID`: four
research `v202-noop-<pid>.json` receipts in `live-add/native-executor/`
(written only by `examples/runtime_read_probe.rs`, never by a shipped build)
were read as operations. The receipt store now moves such files into
`foreign-receipts/` when it opens (kept, never read again) and names any that
appear later.

Also in this change: a game that exited or restarted no longer pins the
cached live-add executor to the dead process (`QueryFullProcessImageNameW ...
error 31`) when that executor owns no native state; "核对上次实时添加" with
nothing to check no longer locks "核对添加"; a rejected preview now says that
nothing was added.

## Closing and stale records never trap the player (owner direction)

The owner's rule: the app must not use its own unresolved state to refuse the
player. Before, one durable unsettled receipt anywhere in
`live-add/native-executor/` made the runtime host "busy" (`Live addition
requires an idle runtime host`), made `safe_to_shutdown` false, and so refused
both every later addition and closing the window, even when the receipt
belonged to a game process that had long exited. A dead worker also refused
closing.

- `ReceiptStore::unresolved_owner_of(pid, creation)`: an unsettled receipt owns
  only the exact process instance it dispatched into. The Windows transport
  reads the running instance's creation time and uses it for `ping` busy and
  dispatch admission. A receipt without an instance still owns (fail closed).
- `safe_to_shutdown` counts only what lives in this process (a retained
  allocation or debugger session); `LiveAddOwnership::unsafe_ownership` no
  longer counts durable unresolved operation ids. `LiveAddApplication::prepare`
  still refuses a new addition into the same process instance while one of its
  operations is unresolved, so duplicate protection is unchanged. This matches
  the Python `RuntimeApplication`, which never counted durable ids.
- Closing hides the window at once, gives busy workers up to 20 s
  (`CLOSE_GRACE`) to finish, then exits; an exited worker counts as closed.
- The add view shows "核对上次实时添加" only while its reminder is set; when
  that check fails (for example after the game restarted), the player can
  dismiss the reminder after checking the in-game inventory.

## Interface messages and feedback

- One `Notice` component for status and failure text: failures are styled as
  such, drop the `Error:` prefix, keep the raw technical text under a
  collapsed "技术详情", and offer "导出反馈文件".
- `review:feedback` writes one `feedback/nioh3-feedback-<unix>.txt`
  (diagnostics plus the last 1 MB of logs) and shows it in Explorer; Settings
  has the same entry ("反馈问题"). Unexplained failures now name their error
  code and point to this instead of "请复制日志".
- A failure no longer copies 128 KB of log into the player's clipboard.
- With several saves, the last chosen save is selected again, and the picker
  says how many saves it found instead of silently selecting nothing.
- The search status no longer mentions the backend.

## PC v2.02 temporary-override evidence

Static, from the retained section dumps (`Nioh3_v2.0.1.0.text.bin`, SHA-256
`F8799B5D…48023`; `Nioh3_v2.0.2.0.text.bin`, SHA-256 `4CEC8FB6…E6C29`), with
`tools/compare_text_sites.mjs`:

- `descriptor_complete` (v2.01 `0x20E195C` → v2.02 `0x20E50B8`): 0x500 bytes
  around the hook differ only in 31 rel32 call / RIP-relative displacement
  runs.
- Challenge-capacity getter (v2.01 `0x1028E30` → v2.02 `0x102AD60`): 0x400
  bytes of the body differ only in 28 displacement runs, and the 0x26-byte
  relocation-free prefix occurs exactly once in the v2.02 `.text`.

So register and stack use at both hooks is unchanged. Each session still
re-reads the exact hook bytes in the live process before writing. **Not yet
done:** one live PC v2.02 run of apply → reopen scroll → stop, with a save
backup, before this is published.

## Not changed

- The selected-save ↔ active-character binding flagged by the Pro review.
- `approval_status` / `product_enablement_allowed` in `pc_v2_02.json`; the
  blanket native-write gate (native generation/search) still refuses PC v2.02.
- The direct-save install lifecycle word (`0x06800082`, 2026-09-20) that makes
  a save-path addition unrevealed; it is already in v0.8.0.
