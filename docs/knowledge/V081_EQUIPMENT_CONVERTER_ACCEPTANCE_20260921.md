# v0.8.1 equipment converter handoff acceptance (2026-09-21)

The final local evidence packet is staged at
`D:/Nioh3_v080_deliverables/deliverables/v081-equipment-converter-20260921/handoff/`.
The assembled archive is
`D:/Nioh3_v080_deliverables/deliverables/v081-equipment-converter-20260921/Nioh3_v202_Equipment_Converter_20260921.zip`;
`FINAL_REPORT.md` beside it records the immutable byte size, SHA-256, and one
structural-validator result.

Local static evidence confirms the reviewed 24-byte slot copy, per-path
normalizer arguments, unsigned whole-slot ordering, and the 11-byte cold
branch (`mov dword [rbx],2; jmp 0x551CCC`) used for the key-2 supplement. The
converter body is 561 bytes (`0x5515FC..0x55182D`) and the helper capture totals
531 raw bytes. These facts do not prove normal-source provenance, the allowed
input domain, a stable final index, exact scalar semantics, same-record
identity through uninspected calls, legality, persistence, or live acceptance.

Scope is offline static review only. The `pc_v2_02` profile remains a candidate;
product enablement and equipment hard-illegality rules remain disabled. Full
executables/sections, tables, saves, native processes, CE/debugger state,
network, builds, and product writes are intentionally absent.
