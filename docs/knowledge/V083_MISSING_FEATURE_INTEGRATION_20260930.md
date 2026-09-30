# v0.8.3 missing-feature integration

The owner required one candidate containing live equipment addition, scroll
replacement prediction and extra-painting prediction. Earlier test7 candidates
do not contain this completed scope and must not be presented as its delivery.
The owner requested personal execution without subagents.

## Product entry points

- Equipment & items -> Add new equipment -> Live editing: item, level, plus,
  rarity and 16-bit seed; native preview; review confirmation; one insertion.
  Existing save-file seeded search and modded addition remain available.
- Scroll editor -> read/select a save scroll -> Reroll & extra-painting
  prediction: ordinary-completion choices, decline, painting after the selected
  branch, and up to five simulated rounds. The panel displays raw effect values.

Prediction is certified only for PC 2.0.2.0, NG3 type 0xE604, rarity 4, ordinary
completion. Revelation eligibility and automatic slot choice remain unknown;
the panel states this limit. Simulated records project fields needed for the
next prediction and are never exposed as install payloads. No whole-record
native-write parity is claimed for these simulation records.

## Native equipment contract

`equipment_add.rs` uses the pinned PC202 executable and separate equipment
container ownership, 2500 x 0xF0 records. Native generation uses the historical
item-grant route with zero drop-source context and the current game's player
state. A nonallocating preview runs on the accepted dispatch thread in owned
scratch, through effect constructors, context initialization, generation and
builder. The confirmed insertion rebuilds that descriptor, compares all 240
scratch bytes against the preview with the allocated serial, then calls the
native insertion function exactly once.

The retained image's copy routine 0x552DD0 copies +0x00..+0x23 and
+0x28..+0xE3, preserving +0x24..+0x27 and +0xE4..+0xEF from the free slot.
The acquisition setter 0x54C380 writes the old u32 counter into +0x1C and
increments it once. Destination verification checks those preserved ranges,
all copied fields, the permitted new-item bit 0x80, the acquisition key/counter,
the generation serial/counter, exactly one changed container slot, and native
dispatch/cleanup evidence. It never claims an independent scroll serial-index
proof. Generation-chain code windows are captured twice and checked again at
the accepted dispatch stop.

The renderer persists a parent UUID before preparation. Prepared plan digests,
preview children, insertion claims and native receipts are durable. Status and
recovery never generate or insert again. A rebuilt-item mismatch has a distinct
rejected-before-insertion result only with acknowledged guard, unchanged
container and verified cleanup; incomplete proof stays uncertain. Unregistered
requests can be rejected without finding a running game.

## Evidence

All artifacts are under
`D:/Nioh3_v080_deliverables/deliverables/codex-v083-missing-features-20260930/`.

- `backend/completion-host-e2e.json`: framed production host and retained native
  records; accepted replacement and declined painting projections, thresholds,
  idempotence and rejection controls. Equipment status/recovery/cancel jobs and
  request-schema controls are also exercised without a valid game preparation.
- `backend/equipment-native-helper-e2e.json`: real Windows debugger and dispatch
  shim over an owned helper, with stand-in generation/builder/insertion.
- `backend/equipment-coordinator-e2e.json`: prepare, digest refusal, confirmed
  insertion, full readback, duplicate refusal and restart recovery over that helper.
- `backend/equipment-preview-mismatch-e2e.json`: changed stand-in output is
  refused before insertion, full container unchanged, and no replay.
- `ui/`: production components in Chromium; all three locales, branch
  progression, stale response disposal, dirty disable, job polling,
  confirmation, cancellation and lost-response recovery. Screenshots reviewed.
- `native-source/`: actual WebView2 shell; real read-only prediction/receipt
  routes, fixture inventory/equipment presentation, native maximize/restore;
  23 checks pass, including editor height, confirmation viewport bounds and
  visible native header. All three locale screenshots were inspected.
- `candidate/`: matching outer EXE and final packaged acceptance, when present.

Existing native scroll helper and 190 runtime regressions pass. Synthetic
save transaction and group-key replay host E2E pass; these are bounded save
evidence. The locale catalog contains 1166 messages. Chinese catalog item names
are preserved exactly where no localized game item name is bundled.

Focused production/worker and new-host-test Clippy checks pass. A broader
all-target Clippy invocation also encountered 13 pre-existing unwrap lint
violations in `equipment_seeds.rs` unit tests; those unrelated tests were not
rewritten. The new host test's unnecessary closure drop was removed.

## Remaining acceptance boundary

No new game-function call, inventory insertion or real-save write was performed
by this task. Historical September 26 CE insertion is separate evidence. The
new packaged live equipment path still needs owner-present game insertion and
shrine save/reload acceptance. No workflow requires closing the game; at most
return to title for the applicable save-file workflow. Public release is pending.
