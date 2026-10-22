# Main damage frame and context contracts

RVAs/structure offsets are hexadecimal; counts are decimal. These are PC2.02
original-byte interpreter and conditional computation-graph results, not live or
hardware damage execution. Full formula, current actor binding, legality and real
optimizer readiness remain false. Direct Sol/Max work continues with Pro handoffs
owner-suspended.

The audit root is
`D:/Nioh3_v080_deliverables/deliverables/damage-pro-return-audit-20261015-r9`.
This lane owns `main-stack-frame-frontier`. Its independent ordered-container
lane is `main-ordered-container-frontier`; container results do not silently alter
the graphs below. Received R5/R8/R9 and sealed Stage2/3 files remain unchanged.

## Exact original call census

All60 inherited opaque stack-call sites match actual original E8 rel32 bytes.
They target21 functions, including26 calls to624518, six to2F7B64, four toB453C
and three to552890. A candidate frame pointer in a register is not proof that a
callee reads or writes through it. Conversely, a known return value does not
prove every secondary register, flag, write or alias contract.

`discovery-01/OPAQUE_CALL_CENSUS.json` binds the original native main instructions
and dependency inventory. Named exports are bounded by original unwind extents
or explicitly partial first-RET/JMP leaf windows. A partial leaf window must not
be treated as a complete CFG when another branch escapes it.

## Hot624518 frame preservation

Original624518 first reads the raw GS/TLS epoch and signed guard4BA66A0. For
`i32(guard) <= i32(epoch)` it returns STAT.Q+7C0, or address4BA66B0 if that pointer
is zero. It does not read R9 or dereference the returned pointer. RCX becomes24,
RDX becomes the TLS block; R8/R9/R10/R11/XMM0..5 remain unchanged. Returning the
fallback address proves no default-object data or lifetime. The cold191C964
initializer is still outside this supported domain.

The original main prologue establishes RSP=RBP-100. Its26 selected calls have no
intervening explicit RSP adjustment. The CALL and getter PUSH store at RBP-108
and RBP-110; all156 tracked frame slots start at or above RBP-D0. Thus the hot
getter cannot mutate those slots through stale R9 values. Returning-call ABI
assumptions from the inherited graph remain explicit; no whole-program ABI proof
is inferred.

`hot-getter-native-03/RECEIPT.json`, SHA256
`328855f830396fd522c5cdf918658c7d8e68ba7bb88f1cfe55cc30a157252bcc`,
retains114 cases:109 original CALL/getter/RET returns to the actual successor,
three real cold entries and two raw unknown frontiers. All26 sites, null/nonnull
returned pointers, unknown/stack R9, signed epoch extremes and complete frame
value/known-mask preservation are covered. Its1,848 executed instructions cover42
distinct original RVAs; all8,926 main instructions/39,134 bytes are separately
matched against original.text for frame/prologue provenance, not execution credit.

`hot-getter-graph-03` removes exactly4,056 identity alias nodes under this explicit
hot domain, reducing57,004 graph nodes to52,948 and60 opaque frame-call sites to34.
It reproduces all418 inherited scalar/gate ancestries before refining them and
recomputes the new ancestry,958 field/pair links and all references. Scalar-resolved
pair count stays378/418. This cleanup alone closes no additional factor values.
Its graph SHA256 is
`7d5dbaa3b1291b47d5264dcdb71c1b107b42ed4635878449afb04821caeed062`.

## Complete caller context copy and twelve-channel merge

Original7940EF..794153 obtains the entity through actor+3B0/query+8, copies52
bytes from entity+E2C to RBP+1B0, then tests entity+E60 byte0. Zero skips the
merge. A nonzero flag calls the complete straight-line1FF09F0:

```text
destination.byte0 |= source.byte0
for i in 0..11, in stored order:
    destination.f32[4+4*i] = F32(source.f32[4+4*i] * destination.f32[4+4*i])
```

Bytes1..3 are preserved padding. No element, damage-pool or UI names are assigned
to these channels without their downstream consumers. The conditional call,
copy widths/order and binary32 operand order are native contracts, not an
inferred additive affix bucket. Source/context graph production remains open.

Public Stage4 `damage_channel_context` accepts exactly `context_pair_hex` and
`context_pair_known_hex`, each104 bytes for the two adjacent structures. It
returns all52 destination bytes/masks, ordered stores and12 product traces.
Normalized source entities are disjoint from the stack destination. Zero flag
does not consume numeric fields: unknown operands and a copied NaN are preserved
in the accepted skip control. Nonzero branch requires consumed finite-RNE data;
missing data returns an unsupported partial state, never zero or a successful
no-op. This does not complete the larger main function or assign channel meaning.

Evidence:

- `channel-native-03/RECEIPT.json`, SHA256
  `268c6bc2f56cc7b20aff516d12f739fb12227353c58a91c64773c25460987de5`:
  115 whole original-block returns, three real unknown frontiers,6,478 steps and
  all57 original RVAs. Every channel has positive changed controls; flags, signed
  zeros, subnormals, random finite values, masks and untouched source graph match.
- Promoted native replay at
  `stage4-public-integration/channel-native-promoted-02/RECEIPT.json`, SHA256
  `01c2c686eb9327a197d45f70ccf56151198d053e035dfb5c6576525f62585eb3`,
  reproduces those118 cases using a declared57-instruction program checked
  against original.text, with no private discovery graph imports or helper answers.
- Actual Stage4 public118 full JSON/exit envelopes pass at
  `stage4-public-integration/damage-channels-02/PUBLIC_ACCEPTANCE.json`, SHA256
  `29a62c511cc33774c9f53c1b42c44c31b2854ec2040ca2e5867c45a5016b0c36`.
- Runtime `src/damage_channel_context.py` SHA256 is
  `5e0539e2658e2db7c5f44abd1a340825102f1efc67c64591e8cc9d4ac01bceec`.
  Current replay/evidence are under `evidence/damage-channel-context-v2` in Stage4.

## Reconnected graph and exact remaining scope

`channel-graph-02` replaces the156 alias barriers at the actual79414E call. The
inherited graph tracks11 of the12 written float fields; it receives11 exact raw
loads and11 MULSS nodes. The other tracked slots retain their original values.
This conditional graph has52,814 nodes and33 remaining opaque stack-call sites.
Scalar-resolved pair sites increase378 ->384 of418. The six newly resolved sites
are7941E6,794259,79426D,79431D,794387,794398. Every inherited arithmetic node,
branch, gate and unclosed secondary-register dependency is retained.

The new conditional graph SHA256 is
`c53bbf153d24cae8268aebe34ec74394fa7eeb8f04b419c9daded0f438bbd602`.
`SCALAR_FRONTIERS.json` records the34 remaining unresolved scalar sites, not34
independent game functions. Field/pair reach is not final channel eligibility.
The new graph's local domain file retains the inherited guard's original-root
binding/hash instead of a broken relative predecessor path. Original auxiliary
graphs/links are unchanged and must not be presented as newly recomputed files.

The bounded native/container/source proofs, unknown call results, cold TLS,
noncanonical queries, aliases, full HP/late writes and Ki/action scheduling still
need their own joins. These counts are graph diagnostics, not a percentage of the
entire optimizer project or a final damage certificate.

## Repeatable checks

Use PowerShell7, the project wrapper, PYTHONDONTWRITEBYTECODE=1 and fresh D:
outputs. `audit_hot_getter.py --output FRESH` replays the114 original-call controls.
`refine_hot_getter_graph.py --native-receipt ... --output FRESH` reproduces the
conditional quotient. Stage4
`evidence/damage-channel-context-v2/native_replay.py --output FRESH` replays the118
context cases with explicit verified NIOH3_R8_ROOT/NIOH3_R9_PACKET_ROOT.
`refine_channel_graph.py --native-receipt ... --public-receipt ... --output FRESH`
requires the native and actual CLI receipts before producing its graph/diagnostics.

## Corrected unknown-input instruction provenance

Independent review found that the first channel model reported a load-cluster
start for either missing float operand. A missing destination was therefore
incorrectly attributed to a source MOVSS. Numerical state and all ordered stores
matched, but this diagnostic was not an exact native demand site. The original
v1 module/audit and receipts are preserved; they are superseded for this diagnostic.

The current model distinguishes first_required_reader_rva from dependency.rva,
the first exact numeric demand. The native audit now checks the latter explicitly:
unknown branch flag794142; first source operand1FF09FD; last destination operand
1FF0AA2. Fresh channel-native-03 and promoted02 both pass115 returns/three raw
frontiers with those exact sites. Actual118 public envelopes pass again at
`stage4-public-integration/damage-channels-02/PUBLIC_ACCEPTANCE.json`, SHA256
`29a62c511cc33774c9f53c1b42c44c31b2854ec2040ca2e5867c45a5016b0c36`.
The promoted native02 receipt SHA256 is
`01c2c686eb9327a197d45f70ccf56151198d053e035dfb5c6576525f62585eb3`.
Current numerical/diagnostic module SHA256 is
`5e0539e2658e2db7c5f44abd1a340825102f1efc67c64591e8cc9d4ac01bceec`.
Use `evidence/damage-channel-context-v2` for current replay/public references.
The conditional channel-graph-02 binds these corrected receipts with unchanged
arithmetic, node counts and384/418 scalar resolution. Independent structural
recomputation also preserves all390 getter-init nodes,14 unclosed context
secondary-register/flags nodes and11 zero-flag no-call phi alternatives.
