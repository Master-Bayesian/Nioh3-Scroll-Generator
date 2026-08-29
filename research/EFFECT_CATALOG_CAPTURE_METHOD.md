# Effect catalog capture method

## Goal

Build a versioned, context-sensitive, multilingual catalog from the game's own
effect and localization resolvers. Static cheat-table labels and spreadsheets
are hints only and never become verified names without matching runtime
evidence.

## Identity

An entry is identified by game version, record type, playthrough, rarity,
generation stage, slot role, slot index, and all six 32-bit fields of the
0x18-byte effect slot. A raw effect ID alone is never an identity key. Stage-one
generator outputs and final saved records must never share a namespace.

## Address evidence

Every captured function or table must record:

- module-relative RVA and an AOB signature;
- the caller and callee chain used to locate it;
- the read/write breakpoint or cross-reference that led to it;
- one complete input record and the observed output;
- a second-launch signature validation result.

Absolute addresses from one process launch are diagnostic data only and must
not be stored as product constants.

## Multilingual capture

Resolve the stable game text ID first. Enumerate every locale exposed by the
loaded localization manager, switch or query the locale-specific text table,
and store the native result independently for each locale. Missing strings are
recorded as missing; they are not filled with another language and are not
machine-translated.

The required provenance values are `native_resolver`, `manual_capture`, and
`unverified_hint`. Only `native_resolver` and independently retained
`manual_capture` evidence may be presented as verified in the product.

## Coverage gate

For every supported record type and rarity, enumerate all reachable RNG output
buckets for primary, ordinary-secondary, and special/Grace roles separately.
Every raw result must have at least one representative seed and full record.
Unknown names remain hexadecimal until the native resolver is captured.

## Reuse for other item families

Future weapon or armor work must create a new item-family context and repeat
the same pointer discovery, slot-layout, resolver, multilingual, and
second-launch signature gates. Scroll field offsets must never be assumed to
apply to another item family.
