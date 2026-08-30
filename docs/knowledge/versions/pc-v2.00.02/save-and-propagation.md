# Save record and propagation protocol

Evidence grade for the compact exchange tuple: `native-control-flow` / B+.
The executable specification and fixed vectors are retained, but the repository
does not contain a complete raw network-packet corpus or the entire send-chain
disassembly.

## Canonical exchange message

The recovered online message class is `Online::EmakiItemExchange`, message type
`0x084E90D2`. Its core payload is:

```c
struct EmakiItem {
    uint32_t random_seed;
    uint32_t exchange_count;
    uint8_t  is_incomplete;
    uint8_t  padding[3];
    uint32_t value;
};

uint64_t account_id;
```

The seven `0x18`-byte effect slots are not sent. The receiver rebuilds the
canonical scroll from the compact tuple. This is why direct local effect edits
normally revert on propagation.

`value` is packed as:

```text
bits  0..1   item category
bits  2..11  effective level
bits 12..23  recommended internal level
bits 24..27  flag nibble
bits 28..31  rarity
```

The sender increments `exchange_count` before transmission. The account ID is
lineage/origin metadata and is not part of the recovered effect-selection RNG.

Additional canonicalization behavior:

- rarity is read from `+0x31`, normalized to at least 3; `+0x30` is the local
  mirror that canonical records keep synchronized;
- effective level uses `min(record +0x06, 180)`, unless record flags at `+0x18`
  contain `0x00200000`, which serializes level zero;
- a zero flag nibble is replaced with 1;
- `is_incomplete` is true when any effect slot's byte `+0x0E` has its sign bit
  set.

The category field is only two bits. Record types for latent playthroughs 4 and
5 map to category indices 4 and 5 in the recovered type table, which this wire
field cannot represent directly. Their exchange semantics are therefore
uncertified and must not be claimed to propagate.

## Canonical record fields

The current executable specification is `emaki_exchange.py`.

| Offset | Field |
| ---: | --- |
| `+0x00` | record type / playthrough context |
| `+0x02,+0x04,+0x14` | split origin account ID |
| `+0x06,+0x08` | scroll item level mirrors |
| `+0x0F` | packed flag source |
| `+0x10,+0x12` | recommended internal level mirrors |
| `+0x20` | random Seed / displayed scroll ID |
| `+0x30,+0x31` | rarity mirrors |
| `+0x34` | first effect slot; seven slots, stride `0x18` |
| `+0xDC` | stored transfer count |

The record is `0xE8` bytes. The decrypted user save uses the `RNNUSR` format;
the current inventory group starts at `0x176CCE` and contains 400 fixed physical
slots. These offsets are version-scoped and must be recaptured after an update.

Record types are `0x1E82`, `0x516D`, `0xE604`, `0xDD82`, and `0xD523` for
playthrough contexts 1 through 5 respectively. The last two are latent,
unreleased contexts in v2.00.02.

Each `0x18` effect slot contains prefix at `+0x00`, effect ID at `+0x04`,
numeric value at `+0x08`, metadata at `+0x0C`, and two tail fields at `+0x10`
and `+0x14`.

## Product safety boundary

- Canonical generation and propagation: choose record type, rarity, and Seed;
  rebuild the complete native record; then insert into a zero slot.
- Local editing: direct effect/value/metadata changes are useful for personal
  play but are explicitly non-propagating.
- Every write transaction backs up the save, repairs checksum, verifies an
  encrypt/decrypt roundtrip, and refuses the write if the source hash changed.
- Two-account receipt remains the final propagation acceptance test. Offline
  parity and local display are not substitutes.

## Known gaps

- A complete confidence-annotated schema for every unknown `0xE8` byte has not
  been recovered.
- Category semantics for latent playthrough 4/5 record types are not certified.
- Raw saves and account-specific corpora are sensitive evidence and must remain
  outside public documentation and releases.
