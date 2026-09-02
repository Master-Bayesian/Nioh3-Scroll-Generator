# Illegal auxiliary override proof

## Purpose

The unrestricted local editor must support deliberately impossible auxiliary
layouts, such as three copies of Ichimokuren. Replacing the canonical Seed is
not sufficient because the native generator rejects those layouts by design.

The save record does not serialize enemy groups, terrain, or special rules as
independent fields. PC v2.00.02 constructs them in a transient descriptor. The
verified post-construction boundary is:

```text
RVA 0x20DD558
RBP  = completed AuxiliaryDescriptor*
R12D = displayed Seed
```

The descriptor contains:

```text
+0x00/+0x08/+0x10  outer enemy-group vector begin/end/capacity
+0x18              three ordered uint16 special-rule keys
+0x1F              terrain enum
outer stride       0x28
inner stride       0x14
inner +0x04        enemy lookup key
```

## First proof

`probe_illegal_auxiliary_override_ce.lua` is a fail-closed Cheat Engine proof
for Nioh3.exe v2.00.02. It validates the hook bytes, matches one exact Seed,
requires at least three already allocated non-empty groups, then replaces the
first enemy lookup key in each group with Ichimokuren (`0x0006DE91`). It shrinks
each selected group to one entry and the outer vector to three groups.

It deliberately preserves every still-unresolved encounter and mode field. It
does not allocate memory, patch the save, or claim persistence.

## Acceptance gate

Do not integrate this hook into the application until one target scroll proves:

1. the detail page displays three Ichimokuren groups;
2. entering the challenge spawns three Ichimokuren groups in the expected order;
3. returning to the list and reopening the detail reapplies the override;
4. disabling the breakpoint restores the native Seed-derived result;
5. the game exits normally without an invalid free or stale pointer.

## Live result: three Ichimokuren

The first PC v2.00.02 live proof used displayed Seed `203900415` and replaced
the first three generated groups with enemy lookup key `0x0006DE91`
(Ichimokuren). The native descriptor originally contained four groups. The
override reduced it to three groups and reduced every selected inner vector to
one entry.

The descriptor-complete hook applied five times with zero validation failures.
The scroll detail page displayed one Ichimokuren because the UI de-duplicates
identical enemy names. The actual challenge spawned three consecutive
Ichimokuren groups, proving that the challenge consumes the three distinct
descriptor groups even though the detail page renders one unique name.

This proves the runtime representation can express an enemy combination that
the Seed-driven native generator cannot produce. It does not prove or disprove
persistence elsewhere in the save container. The canonical `0xE8` record has
no independent auxiliary fields, but a companion table, encoded cache, or
instance property remains an open hypothesis until controlled save/restart
experiments test it. The breakpoint was removed after the challenge; native
restoration, cross-process persistence, and clean process exit remain separate
acceptance checks.

### Settlement persistence result

The override was then re-enabled for the same Seed, applied 30 times with zero
validation failures, and the challenge was completed and settled normally.
The game finished saving. After removing the breakpoint, reopening the scroll
in the same game process immediately restored the native Seed-derived enemy
composition. Therefore the modified final descriptor is not automatically
serialized by challenge settlement or the ordinary save path.

This result still does not exclude a separate companion property that must be
edited directly. `probe_auxiliary_property_origin_ce.lua` compares property
`0x1B2` after its copy with the descriptor immediately before and after the
canonical Seed constructor to test that remaining hypothesis.

### Property `0x1B2` origin result

The read-only three-stage probe completed without failures for Seed
`203900415`:

| Stage | Valid enemy vector | Rule keys | Terrain |
|---|---:|---|---:|
| property `0x1B2` copy returned | no | `0x001B, 0, 0` | `0x00` |
| immediately before Seed constructor | no | `0x001B, 0, 0` | `0x00` |
| immediately after Seed constructor | yes, 4 groups / 5 entries | `0x6171, 0, 0` | `0x74` |

The property-copy and pre-constructor snapshots were identical, including an
invalid outer-vector state. The post-constructor snapshot contained the exact
known native auxiliary vector. Therefore property `0x1B2` is not a stored copy
of the selected scroll's enemy/rule/terrain descriptor on this path. This
rejects that specific companion-property hypothesis; it does not by itself
exclude every possible encoded or separately indexed save structure.

### Direct companion-layout correlation scan

`scan_auxiliary_companion_layouts.py` decrypted the post-settlement save and
independently regenerated auxiliary output for eight supported occupied scroll
slots. It searched the complete 9,437,616-byte save for direct representations
of each slot's terrain enum, first special-rule key, and first enemy lookup key.

No field matched as:

- a dense contiguous array in occupied-scroll order;
- a fixed-stride table indexed by physical scroll slot; or
- a fixed-stride table indexed by occupied-scroll ordinal,

for any stride from the field width through `0x1000` bytes. This rejects direct
unencoded companion arrays in those layouts for this character save. It does
not reject hashed keys, compressed/encrypted substructures, indirection through
another index, or a variable-length serialization.

### Save serializer and system-save plaintext result

A controlled save captured the native temporary-file write chain for PC
v2.00.02. The game opened both character and system `SAVEDATA.BIN.tmp` files,
wrote `0x9001B0` bytes for the character save and `0x39978` bytes for the system
save, and returned from every `WriteFile` call through game RVA `0x61E5EF`.
Static unwind and call-reference recovery identified these functions:

| Function | RVA range | Observed role |
|---|---|---|
| save-state dispatcher | `0x61D044..0x61D0F7` | selects the save operation |
| save wrapper | `0x61D600..0x61D613` | enters the buffer/write path |
| buffer/write coordinator | `0x61D6CC..0x61D942` | prepares and dispatches the file write |
| temporary-file writer | `0x61E514..0x61E71C` | writes the complete `.tmp` file |
| serialization transform | `0x61E8AC..0x61EF48` | builds the header and transforms the payload |

At serialization-transform entry, `[RCX+0xE0]` was the plaintext payload
pointer, `[RCX+0xE8]` was its byte count, and `RDX` was the final output buffer.
The transform built a `0x158`-byte header. RVA `0xB9491E`, initially encountered
downstream, belongs to an optimized 256-byte SIMD copy routine rather than an
encryption primitive.

The captured character plaintext payload was `0x900058` bytes. At payload
offset `0x1796F6` it contained the exact known `0xE8` record for displayed Seed
`203900415`; adding the `0x158` file header gives the established decrypted-file
record offset `0x17984E`. This dynamically confirms that the native serializer
input uses the same canonical record layout consumed by the offline editor.
The illegal three-Ichimokuren runtime descriptor never appeared in that record.

| Captured artifact | Size | SHA-256 |
|---|---:|---|
| character plaintext payload | `0x900058` | `9DED93C4DA728578D3E52FE39EF3B60B541D8F7565FA76EF3B36D7963BFF137B` |
| system plaintext payload | `0x39820` | `F812EE089A061A6599092BD335C120F4ACA64DD0130053B0579AD0999C770904` |
| later decrypted character save | `0x9001B0` | `F7A81FFE4310F1A05C6CEDE407C18101FA94FEBEA90E0F473545D9552FA804A2` |

The character plaintext payload and the later decrypted character save differ
in 83 bytes because additional saves and state changes occurred between the two
captures. They are separate temporal samples and must not be described as
whole-file identity evidence.

The captured system plaintext payload was `0x39820` bytes. The correlation scan
was repeated against it using the same eight independently regenerated scroll
samples. It found no dense array and no physical-slot- or occupied-ordinal-
indexed fixed-stride table through stride `0x1000` for terrain, first rule, or
first enemy. Seed `203900415`, rule key `0x6171`, and all five target native
enemy lookup keys were absent from the system payload. The target terrain byte
`0x74` occurred four times, but the multi-scroll correlation was empty, so those
single-byte occurrences do not establish terrain semantics.

Together, the character and system plaintext captures reject a direct
unencoded auxiliary companion array in every tested layout. They do not prove
that no hashed, compressed, variable-length, or indirectly indexed structure
exists. They also do not prove that every scroll is regenerated eagerly during
game startup. The demonstrated behavior is narrower: the detail/challenge path
constructs the auxiliary descriptor on demand from Seed, and ordinary
settlement/save serialization did not preserve the illegal descriptor mutation.

Unless a future probe discovers one of the still-open encoded, variable-length,
or indirectly indexed representations, the practical application design is to
store override profiles outside the game save, keyed by save account plus
scroll instance identity. It only needs to install one version-gated hook for
each game process. The game constructs the descriptor on demand when a scroll
detail/challenge path needs it; the hook should remain idle for every non-target
record and apply the saved profile only when a matching target descriptor is
constructed. This provides restart persistence without claiming that the game
eagerly regenerates every scroll at startup. Propagation and any hidden
companion persistence remain unverified and must not be inferred from the
canonical record layout alone.
