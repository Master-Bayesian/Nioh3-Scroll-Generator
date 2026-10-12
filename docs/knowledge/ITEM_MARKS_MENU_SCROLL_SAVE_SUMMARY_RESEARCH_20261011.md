# Item marks, selected scroll and save summary (2026-10-11, PC v2.0.2.0)

Live capture with the owner at the keyboard, read-only on the game: process memory
was only read (`ReadProcessMemory`), and every shrine save was snapshotted and
decrypted for comparison. No byte was written to the game during the capture.

## Protection (lock) and 喜好项目 (favourite)

Targets: a newly added R4 scroll (save scroll slot 37, inventory key 65537) and
a 薄绿 (equipment slot 1554, key 65536). Each step was read in memory before the
save, then compared with the decrypted save after a shrine rest.

| Step | Scroll `+0x18` | 薄绿 `+0x18` | Other record bytes |
|---|---|---|---|
| baseline | `0x06800080` | `0x00000080` | — |
| protect (memory at once, then saved) | `0x06800084` | `0x00000084` | unchanged |
| unprotect | `0x06800080` | `0x00000080` | unchanged |
| add to 喜好项目 | `0x06820084` | `0x00020084` | unchanged |
| remove from 喜好项目 | `0x06800084` | `0x00000084` | unchanged |

- Protection is flag-word bit `0x04`; 喜好项目 is bit `0x20000` (byte `+0x1A`
  bit `0x02`). Scrolls and equipment share both.
- Favouriting also sets protection; removing the favourite keeps protection;
  the owner confirmed protection can be removed while the favourite stays.
- Memory and save carried the same record bytes at every step. The other save
  differences were the header summary, checksums and per-save counters.
- Item (道具) records were not captured; their marks remain unverified.

Code: `nioh3-save` `layout::{RECORD_LOCK_FLAG, RECORD_FAVORITE_FLAG,
record_marks, set_record_marks}`; `EquipmentFields.locked/favorite`;
`EquipmentPatch.locked/favorite`; scroll edits accept `marks`.

## Live arrays and the inventory cursor

- Equipment container `[[Nioh3.exe+4751530]]+0x10` = player + `0x370E0`
  (already documented); the owned-scroll array is at player + `0x25BB30`
  (equipment + `0x224A50`), 400 records of `0xE8`, byte-identical to the save's
  scroll slots (slots 1, 36, 37 compared).
- With the cursor on the newest scroll, the inventory detail widget pointer
  (`menu + 0x6850 + 0x1B0`) pointed at scroll slot 37 on both the 全部道具 tab and
  the scroll tab; moving the cursor to the next scroll gave slot 36 (the
  second-newest by inventory key). `read_menu_selection` now returns
  `MenuSelection::Scroll`.

## Save summary (load menu)

The decrypted save opens with a summary block: `+0x10` tag `0x1AB5A87D`,
`+0x20` u32 save time (Unix), `+0x28` u32 character level, `+0x30` u64 play time
in microseconds. The load menu showed level 364 / 194:09 / 2026-10-11 19:35:21 and
level 450 / 752:25 / 2026-09-29 11:25:39, matching both saves to the second.
A fresh character save read level 1, 169 s, and generator difficulty 1; the two
third-playthrough (顿悟) saves read difficulty 3, so the difficulty field is the
playthrough. `nioh3-save` `character::save_summary`; `save.discover` /
`save.register` add `summary`.

## Not yet captured

- Whether the game's own protect function does anything beyond the bit (UI
  refresh happened on menu change; the capture saw only the bit).
- Item (道具) marks.
- The shared inventory key passed `0xFFFF` (scroll key 65537); the owner's live
  equipment add appeared stuck in the UI while the item was added.
