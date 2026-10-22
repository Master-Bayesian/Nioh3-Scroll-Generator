# Complete curve resource reconstruction

This record closes parameter acquisition gaps on the PC 2.0.2.0 research profile.
It does not close source qualification, complete actor reconstruction, final damage
or equipment legality. Both sealed native checkpoints remain unchanged. The new
resource evaluator is in the unsealed Stage3 working model.

The executable SHA-256 is
`e22c4a635e4ec1e27a177b76e27d7f6a637f426c0ed3928b60f5693bc52ae130`.
All paths below are under
`D:/Nioh3_v080_deliverables/deliverables/damage-pro-return-audit-20261015-r9/`.

## Manager 720: resource order is not native storage order

The complete historical flat capture contains 337 records of 12 bytes. Its selected
attack-curve JSON included only two curve keys and omitted A865. That omission was
a property of the excerpt, not a native missing-key result.

The new read-only capture preserves the actual 48-byte context, all flat records
and all 337 pointers. Every pointer addresses a complete flat record, and the
index is a bijection. The native consumer orders records by the u16 key at offset
8 and the signed coordinate at offset 0. The physical flat records are not in this
order.

S1 payload **00194** contains exactly the same 337 complete records after its
eight-byte header. Every 12-byte record is unique; every key/coordinate pair is
unique. A recorded row permutation reconstructs every captured flat byte. Sorting
the serialized records by the native comparator also reproduces the complete
native indexed row sequence. This proves the data and the resulting order for this
snapshot; it does not identify the game's loader implementation.

Authoritative artifacts:

- `actor-curve-runtime-capture/CAPTURE_RECEIPT.json`
- `curve720-resource-reconstruction/replay-01/RECEIPT.json`
- `reconstruct_curve720_from_resources.py`

The six A865 nodes are 2->5, 3->7, 4->10, 5->15, 6->25 and 7->40. Their interpolation
flag is zero. Passive and synchronization consumers use the captured native
permutation, rather than silently sorting an incomplete excerpt.

### A distinct consumed720 reader: FBFD40

The later ordinary-hit objective path actually calls FBFD40 with key0x7982 and
binary32 coordinate3082.0. Independent REA instruction review distinguishes its
view: it reads context+0/+8, the contiguous flat range, then calls D1C1E0 for the
unsigned-key equal range. It does not use the passive reader's+18/+20 pointer
vector. Four rows move in that vector, so the two sequences are not generally
interchangeable. The FBFD40 numeric window also compares binary32 coordinates
and interpolates without consulting byte+A; D24860's integer/step evaluator
cannot substitute for it.

For0x7982, flat[201,217) corresponds exactly to resource rows207..222 and happens
to equal its pointer-index group. The brackets around3082 are raw rows at2300
and9300. Root executed the original111-byte post-lookup window on a CPU in an
isolated child, without a Python arithmetic model or expected answers. All54
finite knot/adjacent-ULP/ordinary results exactly match the separate public model
CLI; three nonfinite requests refuse. Coordinate3082 returns bits58399c40.
Original window SHA256:
`6917ac38c89118aeead43d86ace97777bf71289242caaddac12490b9ca5c94cf`.

Evidence is under fixed-scenario-e2e-20261015/{parameter720-review,
parameter720-cpu-01,parameter720-comparison-01}. The independent reviewer checked
the original REA reader/lookup bytes and all337 resource/flat-row witnesses.
The numerical proof is conditional on the explicit versioned raw parameters and
finite RNE/DAZoff/FTZoff machine domain. The later actual hit did not retain its
manager/context/flat-range pointer chain; current-hit parameter epoch binding,
loader implementation, complete score and equipment legality remain open.

## Manager 718: the old capture included four bytes from the next record

The original table manifest binds manager +718 to the captured bonus-curve object.
Its flat range has 55,608 bytes and its pointer index has 662 entries. This gives
a physical stride of **84 bytes**, not the 88 bytes read by the old collector.
S1 payload **01119** has an eight-byte header followed by exactly 662 records of
84 bytes, for a total of 55,616 bytes.

The consumed record layout is:

| Offset | Field |
| --- | --- |
| 0x00 | Raw row ID; not the curve key |
| 0x04 | Signed 32-bit coordinate |
| 0x08 | Eighteen binary32 samples, ending at 0x4F |
| 0x50 | Unsigned 16-bit curve key |
| 0x52 | Unsigned 16-bit sample count |

The actual 2FA678 selector reads the key and coordinate; 2FA7E0 reads the count and
samples. They do not consume bytes 84 through 87. All 662 consumed prefixes match
the serialized records exactly, including every byte and row multiplicity. The
recorded native pointer permutation is complete and follows the native comparator.

The extra four bytes in each of the first 661 historical reads equal the next
physical record's row ID. The final extra four bytes lie outside the table.
Differences in physical adjacency explain the earlier **532 matching / 130
unmatched** full 88-byte sequences. No curve coefficient change is needed.
The original captures and earlier partial reports remain immutable.

This conclusion is tested by mapping only 84 bytes for each isolated native sample
consumer: **3,972 cases pass without the four spill bytes**. The full selector then
passes **11,916 cases** on the reconstructed physical records and exact pointer
order, across all **131 native curve keys**. These counts describe parameter
consumers, not 131 effect groups or 11,916 complete damage formulas.

The original capture manifest has SHA-256
`dd18be2adae4fab68a6866b2382dfbf01d93ede1adae94fab95165b0e514d357`.
It records PID 21384 and capture time 2026-10-13T02:25:49.044101+00:00. Its module
base agrees with the constructor lane's independent 267,486-anchor PE relocation
proof. Original capture profile enablement remains a separate product boundary.

Authoritative artifacts:

- `curve718-resource-frontier/reconstruction-01/RECEIPT.json`
- `curve718-resource-frontier/strict84-controls-01/RECEIPT.json`
- `curve718-resource-frontier/reconstruct_curve718.py`
- `curve718-resource-frontier/audit_consumed_rows.py`

## Public coefficient evaluator

Stage3 exposes `grade_curve_value(key_u16, x_i32, grade_u32)`. It uses the complete
resource-backed 718 table and the verified native row order. It preserves unsigned
grade clamping, signed coordinates and binary32 rounding after every operation.
It rejects unsupported numerical domains instead of substituting zero. A missing
key in this complete captured table follows the actual native zero-return branch.

All 11,916 retained native selector results match the public model. Fifteen fresh
public CLI processes check numerical results, a missing key and invalid widths.
The coefficient CSV lists all 18 displayed grades for every record, retaining the
effective sample ordinal when a row's smaller sample count caps higher grades.

- Module: `working/nioh3_r9_stage3/src/grade_curves.py`
- Data: `working/nioh3_r9_stage3/evidence/grade-curves/`
- Acceptance: `curve718-resource-frontier/public-catalog-01/RECEIPT.json`
- Table: `curve718-resource-frontier/public-catalog-01/ALL_DISPLAYED_GRADE_COEFFICIENTS.csv`

The local result is a raw curve value. Source admission, effective aggregate grade,
wrapper scale, destination pool and later transforms determine how it affects
damage. The evaluator reports those remaining boundaries explicitly.

## Reproduction after an update

1. Establish a new executable and archive baseline. Decode the resource slices
   using the acquisition runbook; ordinals are snapshot coordinates, not stable
   semantic names.
2. Compare complete records and their multiplicities before interpreting a
   whole-file mismatch. Check native object extents and the exact consumer's
   maximum read, rather than treating the collector's read size as the stride.
3. Recover actual key/index ordering. Keep row IDs, curve keys, source row indices
   and native pointer ordinals distinct. Do not assume physical resource order is
   native lookup order.
4. Revalidate selector, interpolation, sample-count and rounding instructions.
   Geometry or matching names alone does not certify a new version.
5. Replay exact reconstructed bytes and native consumers from fresh output paths.
   Preserve raw captures, failed assumptions, source hashes, row witnesses and
   resource metadata. Do not overwrite the accepted old baseline.

These runs use offline instruction interpretation and read-only parameter
captures. They are not live hardware execution of combat or an optimizer-ready
certificate.
