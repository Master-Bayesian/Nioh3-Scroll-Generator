# PC v2.01 possessed-enemy Pro review integration

## Accepted result

The GPT-6 Pro response is preserved under
`deliverables/Nioh3_Assignment_Origin_v201_20260913/` and in the matching ZIP
whose SHA-256 is
`9611A80DBA9A414493AD25DD112344C49D5AF26A4D9D6C9091DEC0CE28ED7601`.

For PC v2.01, the recovered source chain is:

1. `0x10285C4` conditionally writes byte one to generated enemy descriptor
   `+0x0F`.
2. `0x1BF2D51..0x1BF2D5E` copies the full descriptor into temporary task
   `+0x80..+0x93`, so source `+0x0F` becomes task `+0x8F`.
3. `0x4BBC2B..0x4BBC3F` copies the embedded descriptor into the persistent task
   inserted into the observed manager.
4. The later path around `0xE3ADF0` consumes the existing task `+0x8F` and can
   independently assign `+0xE9`; it does not create `+0x8F`.

The local function at `0x10283C0` visits eligible descriptors in order,
advances the generator's existing LCG once per actual trial, compares the
native float32-derived ticket with an inclusive threshold, and returns on the
first success. Class 1 is a conditional fallback after class 0 failure. This is
a prepared-input algorithm, not a complete seed oracle.

## Runtime validation

Run D, seed `86872488` in a one-person expedition, captured two origin entries,
seven trials, ten descriptor-to-task copies, and ten persistent task links.
Every parent-frame `+0xC0` LCG transition and ticket validates. Only descriptor
spawn label `0xF3F`, class 1, wave index 1, position 1 received the source flag,
and the same byte appeared as `+0x8F` in its persistent task. Cleanup was
verified with no owned or global breakpoints remaining.

This confirms the proposed source, trial stream, copy path, and persistent task
identity for that PC v2.01 transaction. It does not bind the descriptor's spawn
label to a physical world spawn point.

## Open mode question

Owner and community observations indicate that one-person expedition may
activate enemies from a pre-authored configuration that normal solo suppresses.
The upstream session object, request construction, and exact activation path
have not been recovered. Downstream configured-count branches do not establish
that causal mechanism and are not an approved live-probe contract.

The current next step is the self-contained Pro package
`deliverables/Nioh3_PC_v2.01_Possessed_Enemy_Mode_Upstream_Pro_Handoff_20260913_v1/`.
It asks Pro to work backward from the validated generator and session request,
identify the correct upstream mode discriminator, and propose at most one
targeted read-only capture if static evidence is insufficient.

Product forward or inverse generation remains blocked until the upstream mode
inputs and complete seed-to-entry state are recovered and independently
validated. No release decision follows from this research.
