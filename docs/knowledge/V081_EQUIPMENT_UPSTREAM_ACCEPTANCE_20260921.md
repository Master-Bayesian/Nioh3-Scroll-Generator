# PC v2.02 equipment upstream bridge acceptance — 2026-09-21

The bounded Pro-response intake and A-only bridge capture are packaged at
`D:\Nioh3_v080_deliverables\deliverables\v081-equipment-bridge-20260921\`.
The immutable ZIP is `Nioh3_v202_Equipment_Bridge_20260921.zip`, 268,525 bytes,
SHA-256
`1cedfe11aa113a2f0604c29869c4583e99aebb1bb8e1bf1a61ed130225c688b7`.
Structural validation returned `ok=true`, 60 verified file members, and 59
checksum entries; the retained log is in the package's `validation/` sibling.

The accepted A export validates the three direct calls in one caller range
`0x2188610..0x21889B9`:

- `0x21887F7 -> 0x5513C8`
- `0x218880D -> 0x557F34`
- `0x2188841 -> 0x54D324`

Five bodies (8,026 raw bytes) are retained, including the init continuation
`0x5513F7 -> 0x551404` and terminal boundary `0x55145A -> 0x55145B`.
The export check passes all three target matches. It retains 203 non-exported
direct-call edges as an explicit closure boundary; no B/C or general caller
scan was added.

This is historical offline static evidence tied to the PC v2.02 candidate
profile and retained P0/P1 identities. The full executable, transformed
section binaries, parameter/table bytes, saves, process/CE state, private P3
records, and vendor payloads are omitted. Historical section hashes and the
current-file identity observation are separate and are not substituted for
each other. The package does not prove normal/script/editor provenance, first
value production, same-record aliasing into `0x54D324`, legality, persistence,
or product safety. Product enablement remains forbidden; any next research
step must identify one exact upstream code/table dependency.
